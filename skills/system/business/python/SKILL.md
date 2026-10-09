---
name: python
description: Run Python scripts for data processing, calculations and file analysis using shell.
---

# Python

Use the registered `shell` tool for every command; Python has no separate execution tool or permission switch.

1. Inspect `python3 --version` and existing project dependencies. Reuse an existing interpreter or virtual environment.
2. For a short calculation, use `python3 -c 'print(2 + 3)'`. For longer scripts, create a file under `$CRABOT_TMP_DIR` with a quoted heredoc, then execute it with `python3`. Quote file paths and pass user values as arguments, never interpolate untrusted text into code.
3. Use the Agent's working directory for relative inputs. Write generated files under `$CRABOT_TMP_DIR` unless the user specifies a destination.
4. Follow shell approval decisions. Obtain authorization before installing dependencies, modifying files or performing external side effects. Do not bypass a denied command with another interpreter.
5. Verify the exit status and output. A script returning a path is not evidence that an image was visually inspected. Use the `image-view` Skill to return an image preview to the user.

Commands have the current OS user's filesystem and network access. Never claim directory isolation. For large output, inspect selected lines or write results to a file instead of flooding the model context.
