use super::{Settings, Wizard};
use crossterm::{
    cursor::{MoveToColumn, MoveUp},
    event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers},
    queue,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, Clear, ClearType},
};
use futures_util::StreamExt;
use std::io::{Write, stdout};
use unicode_width::UnicodeWidthChar;

struct RawMode;
impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
    }
}

/// Own one input stream throughout initialization. Mixing terminal events and
/// stdin line readers can leave a reader consuming keys after the wizard exits.
pub(super) async fn edit(settings: Settings) -> Result<Settings, String> {
    let mut wizard = Wizard::new(settings);
    wizard.refresh_catalog().await;
    terminal::enable_raw_mode().map_err(|e| e.to_string())?;
    let _guard = RawMode;
    let mut events = EventStream::new();
    let mut input = String::new();
    let mut error = String::new();
    let mut drawn = 0;
    let mut out = stdout();
    loop {
        queue!(out, MoveToColumn(0)).map_err(|e| e.to_string())?;
        if drawn > 0 {
            queue!(out, MoveUp(drawn)).map_err(|e| e.to_string())?;
        }
        queue!(out, Clear(ClearType::FromCursorDown)).map_err(|e| e.to_string())?;
        let width = terminal::size()
            .map(|(w, _)| usize::from(w.saturating_sub(1)))
            .unwrap_or(79);
        let field = wizard.field();
        let shown = if field.secret {
            "*".repeat(input.chars().count())
        } else {
            input.clone()
        };
        let menu = wizard.choice_menu();
        let guidance = if menu.is_empty() && error.is_empty() {
            wizard.catalog_text()
        } else {
            String::new()
        };
        let text = format!(
            "{}\n{}{}{}{}❯ {}",
            wizard.prompt(),
            menu,
            if menu.is_empty() { "" } else { "\n" },
            guidance,
            if error.is_empty() {
                String::new()
            } else {
                format!("{error}\n")
            },
            shown
        );
        drawn = 0;
        for line in text.lines() {
            let mut used = 0;
            let visible: String = line
                .chars()
                .filter(|c| !c.is_control())
                .take_while(|c| {
                    used += c.width().unwrap_or(0);
                    used <= width
                })
                .collect();
            let colored = std::env::var_os("NO_COLOR").is_none();
            if colored && line.starts_with("›") {
                queue!(out, SetForegroundColor(Color::Cyan)).map_err(|e| e.to_string())?;
            }
            queue!(out, Print(visible)).map_err(|e| e.to_string())?;
            if colored {
                queue!(out, ResetColor).map_err(|e| e.to_string())?;
            }
            queue!(out, Print("\r\n")).map_err(|e| e.to_string())?;
            drawn += 1;
        }
        out.flush().map_err(|e| e.to_string())?;
        let event = events
            .next()
            .await
            .ok_or("终端输入已结束")?
            .map_err(|e| e.to_string())?;
        match event {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                match key.code {
                    KeyCode::Esc => return Err("配置已取消；未保存更改".into()),
                    KeyCode::Char('c' | 'd') if ctrl => return Err("配置已取消；未保存更改".into()),
                    KeyCode::Up | KeyCode::PageUp if input.is_empty() => {
                        wizard.move_choice(false);
                    }
                    KeyCode::Down | KeyCode::PageDown | KeyCode::Tab if input.is_empty() => {
                        wizard.move_choice(true);
                    }
                    KeyCode::Enter => {
                        if input.trim() == "/cancel" {
                            return Err("配置已取消；未保存更改".into());
                        }
                        match wizard.submit(std::mem::take(&mut input)).await {
                            Ok(Some(settings)) => return Ok(settings),
                            Ok(None) => error.clear(),
                            Err(message) => error = format!("配置无效：{message}"),
                        }
                    }
                    KeyCode::Backspace => {
                        input.pop();
                    }
                    KeyCode::Char('u') if ctrl => input.clear(),
                    KeyCode::Char(c)
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                            && input.len() < 16384 =>
                    {
                        input.push(c)
                    }
                    _ => {}
                }
            }
            Event::Paste(text) => {
                for c in text.chars().filter(|c| !c.is_control()) {
                    if input.len() >= 16384 {
                        break;
                    }
                    input.push(c);
                }
            }
            _ => {}
        }
    }
}
