use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub(super) fn text(server: Option<&str>, columns: usize) -> String {
    let width = columns.saturating_sub(2).clamp(12, 100);
    let data = agent_runtime::paths::data_dir();
    let home = agent_runtime::paths::user_home();
    let data = data
        .strip_prefix(&home)
        .map(|p| {
            if p.as_os_str().is_empty() {
                "~".into()
            } else {
                format!("~/{}", p.display())
            }
        })
        .unwrap_or_else(|_| data.display().to_string());
    let mut lines = super::wordmark::lines(width);
    lines.push(String::new());
    let mut metadata = String::new();
    for value in [
        crate::app::version::DISPLAY,
        data.as_str(),
        server.unwrap_or("Server 未启动"),
    ] {
        if !metadata.is_empty() && metadata.width() + 3 + value.width() > width {
            lines.push(std::mem::take(&mut metadata));
        }
        if !metadata.is_empty() {
            metadata.push_str(" · ");
        }
        metadata.push_str(value);
    }
    lines.push(metadata);
    let mut output = String::new();
    for line in lines.drain(..) {
        let mut part = String::new();
        let mut used = 0;
        for c in line.chars() {
            let size = c.width().unwrap_or(0);
            if used + size > width {
                output.push_str(&format!("{part}\n"));
                part.clear();
                used = 0;
            }
            if !c.is_control() {
                part.push(c);
                used += size;
            }
        }
        output.push_str(&format!("{part}\n"));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unframed_banner_keeps_metadata_and_adapts_to_terminal_width() {
        let banner = text(Some("http://127.0.0.1:8787"), 80);
        assert!(!banner.contains("```") && !banner.contains('┌') && !banner.contains('│'));
        assert!(banner.contains("http://127.0.0.1:8787"));
        assert!(banner.contains(crate::app::version::DISPLAY));
        for columns in [40, 80, 100] {
            assert!(
                text(None, columns)
                    .lines()
                    .all(|line| line.width() < columns)
            );
        }
        assert!(!banner.contains("Local data:"));
    }
}
