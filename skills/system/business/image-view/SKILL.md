---
name: image-view
description: Show, display, preview or share a local image or screenshot in chat. Use when the user asks to see an image, view a screenshot, or get an image preview link. This publishes images; it does not perform OCR or inspect their pixels.
---

# Show an image

This Skill is already loaded when present in ENABLED CAPABILITIES. Otherwise find it using `find(query="image preview")`, or load it with `find(target="skill",id="image-view")`. Use OCR or a vision-capable Agent instead when the user wants the image's contents analyzed.

Use `shell` to execute:

```sh
python3 '<supplied Skill directory>/show.py' "$CRABOT_TMP_DIR/path/to/image.png"
```

Use this Skill's actual directory supplied by the host, or `skills[0].directory` from find if loaded on demand, not the placeholder above. The host injects a short-lived local service address and credential into the approved shell process. Do not print, persist or forward these credentials.

Only images under the current workspace's `$CRABOT_TMP_DIR` are accepted. URLs, arbitrary host paths and symlinks escaping tmp are rejected. Do not fetch arbitrary URLs or copy private files just to bypass this boundary. The service does not fetch any network resource.

The script returns JSON containing `reference` and `preview_url`. Include `reference` verbatim in the final answer so Web displays the image. Publishing an image does not mean you inspected its pixels; do not invent image contents. If execution or publication fails, report the error, not a fabricated link.
