"""Real PTY: submit echo before a delayed model, human permission dialog, mouse mode."""
import fcntl
import http.server
import json
import os
import pathlib
import pty
import select
import shutil
import signal
import struct
import tempfile
import termios
import threading
import time

class Model(http.server.BaseHTTPRequestHandler):
    calls = 0
    def log_message(self, *args):
        pass
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["content-length"])))
        Model.calls += 1
        time.sleep(0.7)
        index = sum(m["role"] == "tool" for m in body["messages"])
        plan = [("agent_start", {"client_id": "fixture-worker", "provider": "mock", "role": "tester"}),
                ("agent_stop", {"client_id": "fixture-worker"})]
        if index < len(plan):
            name, args = plan[index]
            delta = {"tool_calls": [{"index": 0, "id": "call-" + str(index), "type": "function",
                                    "function": {"name": name, "arguments": json.dumps(args)}}]}
            reason = "tool_calls"
        else:
            delta, reason = {"content": "Waiting for human permission."}, "stop"
        self.send_response(200)
        self.send_header("content-type", "text/event-stream")
        self.end_headers()
        self.wfile.write(("data: " + json.dumps({"choices": [{"delta": delta, "finish_reason": reason}]}) + "\n\ndata: [DONE]\n\n").encode())

root = pathlib.Path(__file__).resolve().parent.parent
directory = tempfile.mkdtemp(prefix="crabot-tty-permission-")
server = http.server.HTTPServer(("127.0.0.1", 0), Model)
pid, master = pty.fork()
if pid == 0:
    os.chdir(directory)
    os.execve(str(root / "target/debug/agent-node"), ["agent-node", "--cli"], {
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"), "TERM": "xterm-256color",
        "BIND_ADDR": "127.0.0.1:0", "ADMIN_AGENT_PROVIDER": "crabot", "CRABOT_DATA_DIR": directory, "AGENT_WORKDIR": directory,
        "MODEL_PROVIDER": "compatible", "MODEL_API": "chat", "MODEL_NAME": "fixture",
        "MODEL_API_KEY": "fixture", "MODEL_BASE_URL": "http://127.0.0.1:" + str(server.server_port),
    })
threading.Thread(target=server.serve_forever, daemon=True).start()
fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 120, 0, 0))
output, position = bytearray(), 0
def wait_for(text, timeout=8):
    global position
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if text.encode() in output[position:]:
            position = len(output)
            return
        if select.select([master], [], [], 0.02)[0]:
            output.extend(os.read(master, 65536))
    raise AssertionError("Missing terminal output: " + text)
def send(text):
    os.write(master, text.encode())
try:
    wait_for("请求批准")
    assert b"\x1b[?1000h" not in output
    began = time.monotonic()
    send("permission-test\r")
    wait_for("你：permission-test", 0.5)
    elapsed = time.monotonic() - began
    wait_for("需要你的确认（")
    assert b"fixture-worker" in output
    send("\x1b[200~确认\x1b[201~\r")
    wait_for("已允许一次")
    state = {"collections": {}}
    for line in (pathlib.Path(directory) / "state.jsonl").read_text().splitlines():
        for change in json.loads(line)["changes"]:
            rows = state["collections"].setdefault(change["collection"], {})
            if change["deleted"]:
                rows.pop(change["key"], None)
            else:
                rows[change["key"]] = change["value"]
    assert next(iter(state["collections"]["management_approvals"].values()))["status"] == "completed"
    # Human approval must not trigger an extra model request.
    time.sleep(1)
    assert Model.calls == 3, Model.calls
    send("\x04")
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        try:
            if select.select([master], [], [], 0.1)[0]:
                output.extend(os.read(master, 65536))
        except OSError:
            break
    _, status = os.waitpid(pid, 0)
    pid = None
    assert os.waitstatus_to_exitcode(status) == 0
    assert b"\x1b[?1000l" in output
    print(f"TTY permission passed: Enter echo {elapsed*1000:.0f}ms, explicit confirmation applied, no extra model call")
finally:
    if pid is not None:
        try:
            os.kill(pid, signal.SIGKILL)
            os.waitpid(pid, 0)
        except ProcessLookupError:
            pass
    os.close(master)
    server.shutdown()
    server.server_close()
    shutil.rmtree(directory)
