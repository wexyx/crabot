use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Default)]
pub(super) struct Editor {
    buffer: Vec<char>,
    cursor: usize,
    history: Vec<String>,
    index: Option<usize>,
    draft: String,
    selected: usize,
}
impl Editor {
    pub(super) fn text(&self) -> String {
        self.buffer.iter().collect()
    }
    /// The `@fragment` being typed at the cursor, if the caret sits in one.
    ///
    /// The fragment stops at the first character an Agent id cannot contain, so
    /// `@alice,` reads as a finished name plus a comma, and a mention already
    /// followed by a space is no longer being typed.
    pub(super) fn mention_prefix(&self) -> Option<String> {
        let text: String = self.buffer[..self.cursor].iter().collect();
        let at = text.rfind('@')?;
        // Only a fragment that starts at a word boundary is a mention in progress.
        if text[..at]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric())
        {
            return None;
        }
        let rest = &text[at + 1..];
        let fragment: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '/'))
            .collect();
        // A space after the name means the mention is finished, not in progress.
        (!rest[fragment.len()..].starts_with(char::is_whitespace)).then_some(fragment)
    }
    /// Replace the in-progress mention fragment with a completed `@name`.
    pub(super) fn complete_mention(&mut self, name: &str) {
        // `mention_prefix` already proved a mention is in progress. Re-derive its span
        // here so the caret can sit past punctuation the mention does not own.
        let at = self.buffer[..self.cursor]
            .iter()
            .rposition(|c| *c == '@')
            .expect("complete_mention called outside a mention");
        let fragment = self.buffer[at + 1..self.cursor]
            .iter()
            .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '/'))
            .count();
        self.buffer.drain(at + 1..at + 1 + fragment);
        self.buffer.splice(at + 1..at + 1, name.chars());
        self.cursor = at + 1 + name.chars().count();
        // A name at the very end of the line needs a trailing space; one followed by
        // punctuation or a space already has its separator.
        if self.cursor >= self.buffer.len() {
            self.buffer.insert(self.cursor, ' ');
            self.cursor += 1;
        }
        self.index = None;
    }
    pub(super) fn before_cursor(&self) -> String {
        self.buffer[..self.cursor].iter().collect()
    }
    pub(super) fn clear(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
        self.index = None;
        self.selected = 0;
    }
    pub(super) fn insert(&mut self, text: &str) {
        self.selected = 0;
        for c in text.chars().filter(|c| !c.is_control() || *c == '\n') {
            if self.buffer.len() >= 16384 {
                break;
            }
            self.buffer.insert(self.cursor, c);
            self.cursor += 1;
        }
    }
    pub(super) fn submit(&mut self, remember: bool) -> String {
        let value = self.text();
        if remember && !value.trim().is_empty() && self.history.last() != Some(&value) {
            self.history.push(value.clone());
            if self.history.len() > 200 {
                self.history.remove(0);
            }
        }
        self.clear();
        value
    }
    pub(super) fn suggestions(&self) -> Vec<&'static str> {
        let value = self.text();
        if !value.starts_with('/') || value.contains(char::is_whitespace) {
            return vec![];
        }
        super::command_catalog::suggestions(&value)
    }
    pub(super) fn menu(&self) -> Vec<String> {
        let items = self.suggestions();
        let selected = self.selected.min(items.len().saturating_sub(1));
        let start = selected
            .saturating_sub(2)
            .min(items.len().saturating_sub(5));
        items
            .iter()
            .enumerate()
            .skip(start)
            .take(5)
            .map(|(i, name)| {
                format!(
                    "{} {:<16} {}",
                    if i == selected { "›" } else { " " },
                    name,
                    super::command_catalog::description(name)
                )
            })
            .chain((!items.is_empty()).then(|| {
                format!(
                    "↑/↓ 选择 · Tab 补全 · Enter 确认 · {}/{}",
                    selected + 1,
                    items.len()
                )
            }))
            .collect()
    }
    /// Enter on a partial command completes it first; exact commands execute.
    pub(super) fn complete_partial(&mut self) -> bool {
        let items = self.suggestions();
        let Some(candidate) = items.get(self.selected.min(items.len().saturating_sub(1))) else {
            return false;
        };
        if *candidate == self.text() {
            return false;
        }
        self.clear();
        self.insert(candidate);
        self.insert(" ");
        true
    }
    pub(super) fn key(&mut self, key: KeyEvent, secret: bool) {
        let candidates = if secret { vec![] } else { self.suggestions() };
        if !candidates.is_empty() {
            match key.code {
                KeyCode::Up | KeyCode::PageUp => {
                    self.selected = (self.selected + candidates.len() - 1) % candidates.len();
                    return;
                }
                KeyCode::Down | KeyCode::PageDown => {
                    self.selected = (self.selected + 1) % candidates.len();
                    return;
                }
                KeyCode::Tab => {
                    let candidate = candidates[self.selected.min(candidates.len() - 1)];
                    self.clear();
                    self.insert(candidate);
                    self.insert(" ");
                    return;
                }
                _ => self.selected = 0,
            }
        }
        match key.code {
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => self.cursor = 0,
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cursor = self.buffer.len()
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.buffer.drain(..self.cursor);
                self.cursor = 0;
            }
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.insert(&c.to_string())
            }
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(self.buffer.len()),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.buffer.len(),
            KeyCode::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                self.buffer.remove(self.cursor);
            }
            KeyCode::Delete if self.cursor < self.buffer.len() => {
                self.buffer.remove(self.cursor);
            }
            KeyCode::Enter
                if key
                    .modifiers
                    .intersects(KeyModifiers::ALT | KeyModifiers::SHIFT) =>
            {
                self.insert("\n")
            }
            KeyCode::Up | KeyCode::PageUp if !secret && !self.history.is_empty() => {
                if self.index.is_none() {
                    self.draft = self.text();
                }
                let i = self.index.unwrap_or(self.history.len()).saturating_sub(1);
                self.index = Some(i);
                self.buffer = self.history[i].chars().collect();
                self.cursor = self.buffer.len();
            }
            KeyCode::Down | KeyCode::PageDown if !secret => {
                if let Some(i) = self.index {
                    let next = i + 1;
                    if next < self.history.len() {
                        self.index = Some(next);
                        self.buffer = self.history[next].chars().collect();
                    } else {
                        self.index = None;
                        self.buffer = self.draft.chars().collect();
                    }
                    self.cursor = self.buffer.len();
                }
            }
            _ => {}
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_menu_selection_completion_and_history_are_separate() {
        let mut editor = Editor::default();
        editor.insert("previous message");
        editor.submit(true);
        editor.insert("/");
        assert!(editor.menu()[0].starts_with("› /help"));
        editor.key(KeyCode::Down.into(), false);
        assert!(editor.menu()[1].starts_with("› /chat"));
        assert!(editor.complete_partial());
        assert_eq!(editor.text(), "/chat ");
        assert!(!editor.complete_partial());
        editor.clear();
        editor.insert("/chat");
        assert!(!editor.complete_partial());
        editor.clear();
        editor.key(KeyCode::Up.into(), false);
        assert_eq!(editor.text(), "previous message");
        editor.clear();
        editor.insert("/chat\tproject");
        assert!(editor.suggestions().is_empty());
        editor.clear();
        editor.insert("/not-a-command");
        assert!(!editor.complete_partial());
    }
    #[test]
    fn an_in_progress_mention_is_only_read_at_a_word_boundary() {
        let mut e = Editor::default();
        assert_eq!(e.mention_prefix(), None);
        e.insert("@al");
        assert_eq!(e.mention_prefix().as_deref(), Some("al"));
        e.insert("ice and mail@example.com");
        // The caret is past a space, so no mention is being typed.
        assert_eq!(e.mention_prefix(), None);
        e.clear();
        e.insert("mail@example.com");
        assert_eq!(e.mention_prefix(), None);
        e.clear();
        e.insert("@");
        assert_eq!(e.mention_prefix().as_deref(), Some(""));
    }
    #[test]
    fn completing_a_mention_replaces_only_the_fragment() {
        let mut e = Editor::default();
        e.insert("@bo");
        e.complete_mention("bob");
        assert_eq!(e.text(), "@bob ");
        // The caret sits after the completed name, ready for the rest of the request.
        assert_eq!(e.mention_prefix(), None);
        e.insert("look at ");
        assert_eq!(e.text(), "@bob look at ");
        e.clear();
        // A name completed mid-line keeps the text that follows it.
        e.insert("@bo look at ");
        for _ in 0.." look at ".len() {
            e.key(KeyCode::Left.into(), false);
        }
        assert_eq!(e.mention_prefix().as_deref(), Some("bo"));
        e.complete_mention("bob");
        assert_eq!(e.text(), "@bob look at ");
        e.clear();
        e.insert("@alice,");
        e.complete_mention("alice");
        assert_eq!(e.text(), "@alice,");
    }
    #[test]
    fn unicode_editing_history_and_completion() {
        let mut e = Editor::default();
        e.insert("你好");
        e.key(KeyCode::Left.into(), false);
        e.insert("，");
        assert_eq!(e.text(), "你，好");
        e.submit(true);
        e.key(KeyCode::Up.into(), false);
        assert_eq!(e.text(), "你，好");
        e.clear();
        e.insert("/admin-c");
        e.key(KeyCode::Tab.into(), false);
        assert_eq!(e.text(), "/admin-config ");
        e.clear();
        e.insert("secret");
        e.submit(false);
        e.key(KeyCode::Up.into(), true);
        assert_eq!(e.text(), "");
    }
}
