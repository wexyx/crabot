---
name: image-ocr
description: Extract text from an existing image with Tesseract, checking language packs and reporting uncertain recognition.
---

# Image text recognition

Confirm the image path, requested language and intended output. Use existing image tools when the current tool catalog supports them; otherwise inspect `tesseract --version` and `tesseract --list-langs` through `shell`. Do not claim the image was visually inspected based only on OCR.

If Tesseract or the language pack is missing, explain the required installation and ask before making changes. Use https://tesseract-ocr.github.io/tessdoc/Installation.html for installation details; reuse the system's package manager and avoid broad upgrades.

Run Tesseract on the authorized input path with text to stdout or a file under the workspace. Quote paths as individual shell arguments; never interpolate untrusted image text into a command. Start with the requested language and default layout recognition; change page segmentation only when the layout requires it. Do not install all language packs by default.

Preserve original files. Label uncertain characters, numbers and tables; OCR text is not ground truth and cannot establish image details beyond recognized text. If the source is outside the workdir, request directory permission instead of copying it via an alternate tool to bypass isolation.
