"""First-run option selection and cancellation, using isolated pseudo terminals."""
import fcntl
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
import time

binary = pathlib.Path(__file__).resolve().parent.parent / "target/debug/agent-node"


def exercise(cancel=False):
    directory = tempfile.mkdtemp(prefix="crabot-first-run-")
    pid, master = pty.fork()
    if pid == 0:
        os.chdir(directory)
        os.execve(str(binary), ["agent-node", "--cli", "--server-port", "0"], {
            "PATH": os.environ.get("PATH", "/usr/bin:/bin"), "TERM": "xterm-256color",
            "CRABOT_DATA_DIR": directory, "AGENT_WORKDIR": directory,
        })
    fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 32, 110, 0, 0))
    output = bytearray()
    position = 0

    def wait(text):
        nonlocal position
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            if text.encode() in output[position:]:
                position = len(output)
                return
            if select.select([master], [], [], .1)[0]:
                output.extend(os.read(master, 65536))
        raise AssertionError("Missing " + text + ": " + output.decode(errors="replace"))

    def send(text):
        os.write(master, text.encode())

    try:
        wait("Crabot · 内置 Agent")
        if cancel:
            send("\x1b")
            wait("配置已取消")
        else:
            send("\r")
            wait("› OpenAI")
            for _ in range(6):
                send("\x1b[B")
                time.sleep(.06)
            wait("› Ollama")
            send("\r")
            wait("模型名称")
            send("fixture-model\r")
            wait("接口地址")
            send("\r")
            wait("使用厂商默认协议")
            send("\r")
            wait("系统提示词")
            send("\r")
            wait("API Key")
            send("\r")
            wait("环境变量 JSON")
            send("\r")
            wait("默认 Agent 配置已保存")
            wait("请求批准")
            settings = json.loads((pathlib.Path(directory) / "default-agent.json").read_text())
            assert settings["ADMIN_AGENT_PROVIDER"] == "crabot"
            assert settings["MODEL_PROVIDER"] == "ollama"
            assert settings["MODEL_NAME"] == "fixture-model"
            assert not settings.get("MODEL_API")
            send("\x04")
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            if select.select([master], [], [], .05)[0]:
                try:
                    output.extend(os.read(master, 65536))
                except OSError:
                    pass
            exited, status = os.waitpid(pid, os.WNOHANG)
            if exited:
                pid = None
                assert os.waitstatus_to_exitcode(status) == (2 if cancel else 0)
                break
            time.sleep(.05)
        assert pid is None, "process did not exit"
        if cancel:
            assert not (pathlib.Path(directory) / "default-agent.json").exists()
        assert termios.tcgetattr(master)[3] & termios.ICANON, "terminal raw mode not restored"
    except BaseException:
        print(output.decode(errors="replace"), flush=True)
        raise
    finally:
        if pid is not None:
            os.kill(pid, signal.SIGKILL)
            for _ in range(50):
                if os.waitpid(pid, os.WNOHANG)[0]:
                    break
                time.sleep(.05)
        os.close(master)
        shutil.rmtree(directory)


exercise()
exercise(cancel=True)
print("First-run TTY passed: runner/provider/protocol pickers, saved values, cancellation, terminal restoration")
