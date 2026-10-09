use super::{editor::Editor, theme::Theme, tool_timeline::ToolTimeline};
use crossterm::{
    cursor::{Hide, MoveTo, MoveToColumn, MoveUp, Show},
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste},
    execute, queue,
    terminal::{self, Clear, ClearType},
};
use std::{
    cell::RefCell,
    io::{Write, stdout},
};
use unicode_width::UnicodeWidthChar;
#[derive(Default)]
struct Frame {
    seen: String,
    pending: String,
    cursor_row: u16,
    drawn: bool,
    last: String,
}
/// Inline REPL: committed output belongs to native terminal scrollback.
/// Only the current input/permission region is redrawn; mouse events are never captured.
pub(super) struct Screen {
    theme: Theme,
    frame: RefCell<Frame>,
}
impl Screen {
    pub(super) fn banner(&self, server: Option<&str>) -> Result<(), String> {
        let mut out = stdout();
        // Startup art is literal terminal output, not Markdown or a chat message.
        for line in super::banner::text(
            server,
            terminal::size().map(|v| usize::from(v.0)).unwrap_or(80),
        )
        .lines()
        {
            self.theme
                .write_banner(&mut out, line)
                .map_err(|e| e.to_string())?;
            write!(out, "\r\n").map_err(|e| e.to_string())?;
        }
        out.flush().map_err(|e| e.to_string())
    }
    pub(super) fn enter() -> Result<Self, String> {
        terminal::enable_raw_mode().map_err(|e| e.to_string())?;
        // Clear the visible screen once at interactive startup, retaining native scrollback.
        // Do not use the alternate screen: mouse selection and copying stay terminal-native.
        if let Err(error) = execute!(
            stdout(),
            Clear(ClearType::All),
            MoveTo(0, 0),
            EnableBracketedPaste,
            DisableMouseCapture
        ) {
            let _ = terminal::disable_raw_mode();
            return Err(error.to_string());
        }
        Ok(Self {
            theme: Theme::new(),
            frame: RefCell::new(Frame::default()),
        })
    }
    pub(super) fn draw(
        &self,
        title: &str,
        transcript: &str,
        status: &str,
        session_info: &str,
        update_notice: &str,
        editor: &Editor,
        secret: bool,
        _scroll: usize,
        permission: Option<&str>,
        _tools: &ToolTimeline,
    ) -> Result<(), String> {
        let (cols, rows) = terminal::size().map_err(|e| e.to_string())?;
        if cols < 12 || rows < 8 {
            return Ok(());
        }
        let width = usize::from(cols - 2);
        let mut frame = self.frame.borrow_mut();
        let fresh = if transcript.starts_with(&frame.seen) {
            transcript[frame.seen.len()..].to_owned()
        } else {
            format!("\n{transcript}")
        };
        frame.seen = transcript.into();
        frame.pending.push_str(&fresh);
        let mut committed = String::new();
        if let Some(end) = frame.pending.rfind('\n') {
            committed = frame.pending[..=end].to_owned();
            frame.pending.drain(..=end);
        }
        let text = if secret {
            "*".repeat(editor.text().chars().count())
        } else {
            editor.text()
        };
        let before = if secret {
            "*".repeat(editor.before_cursor().chars().count())
        } else {
            editor.before_cursor()
        };
        let input = wrap(&format!("❯ {text}"), width);
        let cursor = wrap(&format!("❯ {before}"), width);
        let input_capacity = 1;
        let input_start = cursor.len().saturating_sub(input_capacity);
        let mut lines = vec![];
        let pending = wrap(&super::markdown::render(&frame.pending), width);
        if !frame.pending.is_empty() {
            lines.extend(
                pending
                    .into_iter()
                    .rev()
                    .take(usize::from(rows / 3).max(1))
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev(),
            );
        }
        if let Some(permission) = permission {
            lines.clear();
            lines.extend(
                wrap(permission, width)
                    .into_iter()
                    .take(usize::from(rows).saturating_sub(6)),
            );
        }
        if !secret && permission.is_none() && !editor.suggestions().is_empty() {
            lines.extend(editor.menu());
        } else if !status.is_empty() {
            lines.push(wrap(&status, width).into_iter().next().unwrap_or_default());
        }
        lines = lines
            .into_iter()
            .map(|line| wrap(&line, width).into_iter().next().unwrap_or_default())
            .collect();
        let (footer_left, footer_right) = super::footer::layout(session_info, update_notice, width);
        let footer = vec![format!("{footer_left}{footer_right}")];
        lines.truncate(usize::from(rows).saturating_sub(input_capacity + footer.len()));
        let input_row = lines.len();
        lines.extend(input.iter().skip(input_start).take(input_capacity).cloned());
        lines.extend(footer);
        let cursor_row =
            (input_row + cursor.len().saturating_sub(1).saturating_sub(input_start)) as u16;
        let cursor_col = cursor
            .last()
            .map(|s| s.chars().map(|c| c.width().unwrap_or(0)).sum::<usize>())
            .unwrap_or(0);
        let fingerprint = format!("{cols}:{rows}:{title}:{lines:?}:{cursor_row}:{cursor_col}");
        if committed.is_empty() && frame.last == fingerprint {
            return Ok(());
        }
        let mut out = stdout().lock();
        queue!(out, Hide, MoveToColumn(0)).map_err(|e| e.to_string())?;
        if !frame.drawn {
            // Reserve the whole input + footer region. Starting on the last row would
            // scroll the banner up when the footer's newline is written.
            queue!(out, MoveTo(0, rows.saturating_sub(lines.len() as u16)))
                .map_err(|e| e.to_string())?;
        }
        if frame.drawn && frame.cursor_row > 0 {
            queue!(out, MoveUp(frame.cursor_row)).map_err(|e| e.to_string())?;
        }
        queue!(out, Clear(ClearType::FromCursorDown)).map_err(|e| e.to_string())?;
        let mut committed_rows = 0usize;
        if !committed.is_empty() {
            let rendered = super::markdown::render(&committed);
            for line in wrap(&rendered, width) {
                committed_rows += 1;
                self.theme
                    .write(&mut out, &line)
                    .map_err(|e| e.to_string())?;
                write!(out, "\r\n").map_err(|e| e.to_string())?;
            }
        }
        if frame.drawn {
            for _ in
                0..(usize::from(frame.cursor_row) + 1).saturating_sub(committed_rows + lines.len())
            {
                write!(out, "\r\n").map_err(|e| e.to_string())?;
            }
        }
        for (index, line) in lines.iter().enumerate() {
            if index + 1 == lines.len() {
                self.theme
                    .write(&mut out, &footer_left)
                    .map_err(|e| e.to_string())?;
                self.theme
                    .write_update(&mut out, &footer_right)
                    .map_err(|e| e.to_string())?;
            } else {
                self.theme
                    .write(&mut out, line)
                    .map_err(|e| e.to_string())?;
            }
            if index + 1 < lines.len() {
                write!(out, "\r\n").map_err(|e| e.to_string())?;
            }
        }
        let up = (lines.len().saturating_sub(1) as u16).saturating_sub(cursor_row);
        if up > 0 {
            queue!(out, MoveUp(up)).map_err(|e| e.to_string())?;
        }
        queue!(out, MoveToColumn(cursor_col.min(width) as u16), Show).map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())?;
        frame.cursor_row = cursor_row;
        frame.drawn = true;
        frame.last = fingerprint;
        Ok(())
    }
}
impl Drop for Screen {
    fn drop(&mut self) {
        let _ = execute!(stdout(), Show, DisableMouseCapture, DisableBracketedPaste);
        let _ = terminal::disable_raw_mode();
        println!();
    }
}
pub(super) fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    let mut used = 0;
    for c in text.chars() {
        if c == '\n' {
            lines.push(String::new());
            used = 0;
            continue;
        }
        if c.is_control() {
            continue;
        }
        let w = c.width().unwrap_or(0);
        if used + w > width.max(1) {
            lines.push(String::new());
            used = 0
        }
        lines.last_mut().unwrap().push(c);
        used += w;
    }
    lines
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wrapping_is_unicode_aware_and_filters_terminal_controls() {
        assert_eq!(wrap("你好a", 4), vec!["你好", "a"]);
        assert!(!wrap("a\x1b[2Jb", 30).join("").contains('\x1b'));
    }
    #[test]
    fn metadata_is_last_and_notice_is_shown_once() {
        let (left, right) = super::super::footer::layout("模型 · 上下文", "", 120);
        assert_eq!(left, "│ 模型 · 上下文");
        assert!(right.is_empty());
        assert!(!left.contains("已恢复"));
    }
}
