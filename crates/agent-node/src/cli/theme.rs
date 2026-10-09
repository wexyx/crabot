use crossterm::{
    queue,
    style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor},
};
use std::io::Write;

pub(super) struct Theme {
    enabled: bool,
}
impl Theme {
    pub(super) fn write_banner(&self, out: &mut impl Write, text: &str) -> std::io::Result<()> {
        let Some((crab, ot)) = super::wordmark::color_parts(text).filter(|_| self.enabled) else {
            return self.write(out, text);
        };
        queue!(
            out,
            SetForegroundColor(Color::Rgb {
                r: 74,
                g: 144,
                b: 226
            }),
            Print(crab),
            SetForegroundColor(Color::Rgb {
                r: 166,
                g: 184,
                b: 204
            }),
            Print(ot),
            ResetColor
        )
    }
    pub(super) fn new() -> Self {
        Self {
            enabled: std::env::var_os("NO_COLOR").is_none(),
        }
    }
    fn color(text: &str) -> Option<Color> {
        let text = text.trim_start();
        if text.starts_with("│ 有更新 ") || text.starts_with("│ 更新失败") {
            Some(Color::Red)
        } else if text.starts_with("│ 更新已安装") {
            Some(Color::Green)
        } else if text.starts_with("▌") {
            Some(Color::Cyan)
        } else if text.starts_with("执行中 ·") {
            Some(Color::DarkGrey)
        } else if text.starts_with("│") || text.starts_with("┌") || text.starts_with("└") {
            Some(Color::DarkGrey)
        } else if text.starts_with("错误")
            || text.starts_with("[结束]")
            || text.starts_with("error:")
            || text.contains("persistence failed")
        {
            Some(Color::Red)
        } else if text.starts_with("[完成]") || text.starts_with("完成") || text.starts_with("就绪")
        {
            Some(Color::Green)
        } else if text.starts_with("你：") || text.starts_with("❯") || text.starts_with("CRABOT")
        {
            Some(Color::Cyan)
        } else if text.starts_with("[调用工具")
            || text.starts_with("调用工具")
            || text.starts_with("等待")
            || text.starts_with("正在")
            || text.starts_with("需要确认")
            || text.starts_with("目录访问")
        {
            Some(Color::Yellow)
        } else if text.starts_with('/') {
            Some(Color::Magenta)
        } else if text.starts_with("─") || text.starts_with("Enter ") {
            Some(Color::DarkGrey)
        } else {
            None
        }
    }
    pub(super) fn write_update(&self, out: &mut impl Write, text: &str) -> std::io::Result<()> {
        let color = if text.starts_with("有更新") || text.starts_with("更新失败") {
            Color::Red
        } else if text.starts_with("更新已安装") {
            Color::Green
        } else {
            Color::DarkGrey
        };
        if self.enabled {
            queue!(out, SetForegroundColor(color), Print(text), ResetColor)
        } else {
            queue!(out, Print(text))
        }
    }
    pub(super) fn write(&self, out: &mut impl Write, text: &str) -> std::io::Result<()> {
        if self.enabled
            && text.starts_with("› ")
            && !text.starts_with("› 拒绝")
            && !text.starts_with("› 允许")
            && !text.starts_with("› 本对话")
        {
            return queue!(
                out,
                SetBackgroundColor(Color::DarkCyan),
                SetForegroundColor(Color::White),
                Print(text),
                ResetColor
            );
        }
        let label = text.strip_prefix("› ").or_else(|| text.strip_prefix("  "));
        if self.enabled
            && label.is_some_and(|s| {
                s.starts_with("拒绝") || s.starts_with("允许一次") || s.starts_with("本对话允许")
            })
        {
            let color = if label.unwrap().starts_with("拒绝") {
                Color::Red
            } else {
                Color::Green
            };
            if text.starts_with("› ") {
                return queue!(
                    out,
                    SetBackgroundColor(color),
                    SetForegroundColor(Color::Black),
                    Print(text),
                    ResetColor
                );
            }
            return queue!(
                out,
                SetForegroundColor(Color::DarkGrey),
                Print(text),
                ResetColor
            );
        }
        if let Some(color) = Self::color(text).filter(|_| self.enabled) {
            queue!(out, SetForegroundColor(color), Print(text), ResetColor)
        } else {
            queue!(out, Print(text))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn banner_uses_two_colors_and_respects_no_color() {
        for line in super::super::wordmark::lines(80) {
            let mut colored = Vec::new();
            Theme { enabled: true }
                .write_banner(&mut colored, &line)
                .unwrap();
            let text = String::from_utf8(colored).unwrap();
            let (crab, ot) = super::super::wordmark::color_parts(&line).unwrap();
            assert_eq!(
                text,
                format!(
                    "{}{crab}{}{ot}{}",
                    SetForegroundColor(Color::Rgb {
                        r: 74,
                        g: 144,
                        b: 226
                    }),
                    SetForegroundColor(Color::Rgb {
                        r: 166,
                        g: 184,
                        b: 204
                    }),
                    ResetColor
                )
            );
            let mut plain = Vec::new();
            Theme { enabled: false }
                .write_banner(&mut plain, &line)
                .unwrap();
            assert_eq!(String::from_utf8(plain).unwrap(), line);
        }
    }
    #[test]
    fn semantic_colors_and_no_color_mode() {
        assert_eq!(Theme::color("│ 默认 Agent"), Some(Color::DarkGrey));
        assert_eq!(Theme::color("普通回复正文"), None);
        assert_eq!(Theme::color("│ 有更新 v1.2.3 · /update"), Some(Color::Red));
        assert_eq!(
            Theme::color("执行中 · 12.3s · Esc 打断 · python"),
            Some(Color::DarkGrey)
        );
        for text in [
            "│ 默认 Agent",
            "你：hi",
            "[调用工具：group_list]",
            "[完成]",
            "错误：失败",
            "› 拒绝      允许一次",
            "  拒绝    › 允许一次",
        ] {
            let mut colored = Vec::new();
            Theme { enabled: true }.write(&mut colored, text).unwrap();
            assert!(colored.contains(&27));
            let mut plain = Vec::new();
            Theme { enabled: false }.write(&mut plain, text).unwrap();
            assert_eq!(String::from_utf8(plain).unwrap(), text);
        }
    }
}
