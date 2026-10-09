---
name: json-report
description: Summarize a JSON array of numbers using an approved Python script.
---

# JSON Report

Use when the user asks for a numeric summary. Load this Skill with `find(target=skill, id=json-report)`.
Use the registered `shell` tool to run `python3 '<skills[0].directory>/scripts/report.py' '[1,2,3]'`,
replacing the directory with the actual returned path and the array with the user's input.
Shell execution requires the normal host approval. Report the returned count, sum and mean. If execution
is denied or fails, describe that accurately instead of inventing a result.
