---
name: package-manager
description: Install or troubleshoot requested command-line dependencies on macOS or Linux, including Homebrew bootstrap when missing.
---

# Package installation

Use `shell` for commands. Inspect OS, architecture, requested executable and available package manager before installing anything. Reuse existing installations; do not install Homebrew on Linux when the existing system package manager suffices.

Before changing the machine, explain the package, trusted source, installation directory and whether administrator privileges or outside-workspace writes are required. Request authorization for installation; permission to investigate is not permission to install. Runtime command and directory approvals still apply. Do not disable the sandbox or change execution permissions yourself.

For macOS Homebrew, check `command -v brew`, `/opt/homebrew/bin/brew` and `/usr/local/bin/brew`. If missing, consult https://docs.brew.sh/Installation for current prerequisites and official installation instructions. Download any bootstrap script to the workspace for inspection first; do not pipe a downloaded script directly into a shell. Never collect passwords. If sudo, an interactive license dialog or filesystem isolation blocks installation, show the exact host-terminal step and pause rather than retrying or bypassing it.

For Linux, identify the distribution and available manager (`apt-get`, `dnf`, `apk`, etc.). Propose the specific package names and command; do not perform a broad upgrade. Consult distribution documentation when package names differ. For Python dependencies, prefer a workspace virtual environment; never use `--break-system-packages` or modify system Python.

Verify the installed executable and version with a harmless command, then resume the originally requested task. Report any manual action still needed. Do not modify shell profiles or uninstall conflicting software without separate user authorization.
