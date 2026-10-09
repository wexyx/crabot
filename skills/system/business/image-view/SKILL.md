---
name: image-view
description: Show a generated local image to the user through a Web preview link.
---

# Show an image

Load this Skill, then use `shell` to execute:

```sh
python3 '<skills[0].directory returned by find>/show.py' "$CRABOT_TMP_DIR/path/to/image.png"
```

Use the actual `skills[0].directory` returned by `find(target=skill, id=image-view)`, not the placeholder above. The host injects a short-lived local service address and credential into the approved shell process. Do not print, persist or forward these credentials.

Only images under the current workspace's `$CRABOT_TMP_DIR` are accepted. URLs, arbitrary host paths and symlinks escaping tmp are rejected. Do not fetch arbitrary URLs or copy private files just to bypass this boundary. The service does not fetch any network resource.

The script returns JSON containing `reference` and `preview_url`. Include `reference` verbatim in the final answer so Web displays the image. Publishing an image does not mean you inspected its pixels; do not invent image contents. If execution or publication fails, report the error, not a fabricated link.
