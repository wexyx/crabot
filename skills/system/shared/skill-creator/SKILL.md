---
name: skill-creator
description: Create or update reusable Crabot Skills with instructions, scripts and references, and register them in the instance Skill library.
---

# Skill Creator

Use this skill when asked to create or improve a reusable skill, not for ordinary one-off task execution.

1. Identify the intended trigger, output and permissions from the user's request. Inspect repository conventions and any existing skill before editing. Ask only for choices that materially change the result.
2. Use `skill(action=list)` to check existing names and library IDs. For updates, use `skill(action=read,id=...)` and preserve unrelated files and the current enabled state.
3. Write a concise `SKILL.md` with YAML `name` and `description`, then the task-specific workflow. Explain when to use it. Do not duplicate generic model knowledge or imply new execution permission.
4. Include scripts only for repeatable or error-prone work. Put larger conditional instructions in `references/`, link them from `SKILL.md`, and load them only when relevant. Validate scripts through the normal shell approval flow when available; otherwise report that execution was not tested.
5. Register using `skill(action=save,expected_version=0,definition=...)` for a new package. A definition has `id`, `description`, `enabled:false`, `allow_python:false` and `files` mapping safe relative paths to UTF-8 text. For updates include the library `id` and `expected_version` from read; on conflicts reread instead of overwriting.
6. Verify with `skill(action=read,id=<returned library id>)`. Tell the user where to enable the new skill and select projects/Agents in Web. New packages start disabled; authoring never grants execution or widens bindings. New enabled snapshots are used on subsequent tasks.

Example definition (replace all sample content):

```json
{"id":"review-code","description":"Review changes against this repository's conventions.","enabled":false,"allow_python":false,"files":{"SKILL.md":"---\nname: review-code\ndescription: Review repository changes.\n---\n\nInspect the diff and project instructions; report actionable findings with file locations.\n"}}
```

Package limits: 1–64 ASCII letters/digits/hyphens/underscores in the ID; description at most 2 KiB; at most 32 files, each up to 64 KiB and 256 KiB total. Paths cannot be absolute, hidden, traversing or symlinks. Built-in packages are read-only: create a differently named user skill when customization is required.

The host saves user packages under the current instance's `skills/user/` directory. Do not write into installed release directories or assume that creating a folder alone registers a skill. Never store API keys, passwords or private task data in reusable packages.
