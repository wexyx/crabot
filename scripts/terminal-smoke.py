"""Exercise the real TTY editor, colors, secret masking and terminal restoration."""
import fcntl
import json
import os
import pathlib
import pty
import select
import shutil
import signal
import struct
import subprocess
import tempfile
import termios
import time
from terminal_screen import snapshot

root = pathlib.Path(__file__).resolve().parent.parent
version = subprocess.check_output([str(root / "target/debug/agent-node"), "--version"]).strip().removeprefix(b"Crabot ")
directory = tempfile.mkdtemp(prefix="crabot-terminal-")
pid, master = pty.fork()
if pid == 0:
    os.chdir(directory)
    os.execve(str(root / "target/debug/agent-node"), ["agent-node", "--cli"], {
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"), "TERM": "xterm-256color",
        "BIND_ADDR": "127.0.0.1:0", "ADMIN_AGENT_PROVIDER": "mock", "CRABOT_DATA_DIR": directory,
    })

columns = int(os.environ.get("CRABOT_TEST_COLUMNS", "110"))
fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 28, columns, 0, 0))
output = bytearray()
position = 0

def wait_for(text):
    global position
    expected = text.encode()
    deadline = time.monotonic() + 6
    while time.monotonic() < deadline:
        if expected in output[position:]:
            position = len(output)
            return
        if select.select([master], [], [], 0.1)[0]:
            output.extend(os.read(master, 65536))
    raise AssertionError("Terminal did not show " + text)

def send(text):
    os.write(master, text.encode())

try:
    wait_for("请求批准")
    visible = snapshot(output, 28, columns)
    assert "█" in visible[0], "startup lost its first wordmark row: " + repr(visible[:3])
    assert not any("┌" in row or "└" in row for row in visible), "wordmark should not have a frame"
    assert visible[-1].startswith("│"), "session info must stay on the last row"
    assert b"\x1b[2J\x1b[1;1H" in output, "startup must clear the screen and move to the top"
    assert output.index(b"\x1b[2J") < output.index(version), "clear before the banner"
    assert b"\x1b[27;1H" in output, "reserve two rows for input and footer without scrolling the banner"
    assert b"\x1b[28;1H" not in output, "do not start the input region on the last row"
    assert b"\x1b[?1049h" not in output, "preserve native terminal scrollback"
    assert "直接输入任务".encode() not in output
    assert "┌ text".encode() not in output
    assert version in output and b"http://127.0.0.1:" in output
    assert b"\x1b[?1000h" not in output, "native selection must be enabled by default"
    assert b"\x1b[" in output, "missing terminal styling"
    send("/")
    wait_for("↑/↓ 选择")
    send("\x1b[B")
    wait_for("› /chat")
    send("\t")
    wait_for("/chat ")
    send("\r")
    wait_for("选择聊天")
    send("\r")
    wait_for("group")
    assert b"\x1b[48;5;6m" in output or b"\x1b[46m" in output, "selected command must be highlighted"
    send("/admin-c\t")
    wait_for("/admin-config ")
    send("\r")
    wait_for("Crabot · 内置 Agent")
    # The current mock is not a user-selectable runner; Enter picks Crabot.
    send("\r")
    wait_for("› OpenAI")
    for _ in range(6):
        send("\x1b[B")
        time.sleep(0.06)
    wait_for("› Ollama")
    send("\r")
    wait_for("模型名称")
    for value, prompt in [("fixture", "接口地址"), ("-", "协议"), ("", "系统提示词"), ("-", "API Key")]:
        send(value + "\r")
        wait_for(prompt)
    send("tty-secret-fixture")
    wait_for("******************")
    assert b"tty-secret-fixture" not in output, "secret appeared in terminal"
    send("\r")
    wait_for("环境变量 JSON")
    send("-\r")
    wait_for("配置已保存并生效")
    settings = json.loads((pathlib.Path(directory) / "default-agent.json").read_text())
    assert settings["MODEL_API_KEY"] == "tty-secret-fixture"
    send("\x1b[A")
    wait_for("/admin-config")
    send("\x15/he\t\r")
    wait_for("/help network")
    time.sleep(0.1)
    send("\x1b[200~你好\n第二行\x1b[201~")
    wait_for("第二行")
    send("\x1b")
    time.sleep(0.15)
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
    assert b"\x1b[?1049h" not in output, "inline REPL must preserve native scrollback"
    print("TTY passed: colors, completion, history, multiline paste, masked configuration, clean exit")
finally:
    if pid is not None:
        try:
            os.kill(pid, signal.SIGKILL)
            os.waitpid(pid, 0)
        except ProcessLookupError:
            pass
    os.close(master)
    shutil.rmtree(directory)
