/// Upright block lettering. The segments define the CRAB / OT color boundary.
const FULL: &[(&str, &str)] = &[
    (" ██████╗██████╗  █████╗ ██████╗", " ██████╗ ████████╗"),
    ("██╔════╝██╔══██╗██╔══██╗██╔══██╗", "██╔═══██╗╚══██╔══╝"),
    ("██║     ██████╔╝███████║██████╔╝", "██║   ██║   ██║   "),
    ("██║     ██╔══██╗██╔══██║██╔══██╗", "██║   ██║   ██║   "),
    ("╚██████╗██║  ██║██║  ██║██████╔╝", "╚██████╔╝   ██║   "),
    (" ╚═════╝╚═╝  ╚═╝╚═╝  ╚═╝╚═════╝", " ╚═════╝    ╚═╝   "),
];
const CRAB: &[&str] = &[
    "  ▄▄       ▄▄ ",
    " █ ▀▄     ▄▀ █",
    " ▀█▄ ▀● ●▀ ▄█▀",
    "   ▀███████▀  ",
    "  ▄▀███████▀▄ ",
    "  ▀ ▄▀   ▀▄ ▀ ",
];
const COMPACT: &[(&str, &str)] = &[
    ("▄▀▀ █▀▄ ▄▀█ █▄▄ ", "█▀█ ▀█▀"),
    ("▀▄▄ █▀▄ █▀█ █▄█ ", "█▄█  █ "),
];

fn rows(columns: usize) -> Vec<(String, String)> {
    if columns < 24 {
        return vec![("(V) CRAB".into(), "OT".into())];
    }
    if columns < 54 {
        return COMPACT
            .iter()
            .map(|(left, right)| {
                let crab = if columns >= 32 { "(V) " } else { "" };
                (format!("{crab}{left}"), (*right).into())
            })
            .collect();
    }
    FULL.iter()
        .enumerate()
        .map(|(index, (left, right))| {
            let crab = if columns >= 72 { CRAB[index] } else { "" };
            let gap = if crab.is_empty() { "" } else { "  " };
            (format!("{crab}{gap}{left:<33}"), (*right).into())
        })
        .collect()
}

pub(super) fn lines(columns: usize) -> Vec<String> {
    rows(columns)
        .into_iter()
        .map(|(left, right)| left + &right)
        .collect()
}

pub(super) fn color_parts(text: &str) -> Option<(&str, &str)> {
    for columns in [80, 60, 40, 24, 12] {
        for (left, right) in rows(columns) {
            if text == format!("{left}{right}") {
                return Some(text.split_at(left.len()));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;
    #[test]
    fn wordmark_fits_normal_and_compact_frames() {
        for columns in 12..120 {
            for line in lines(columns) {
                assert!(line.width() <= columns, "{columns}: {line}");
                assert!(color_parts(&line).is_some());
            }
        }
    }

    #[test]
    fn upright_letters_and_crab_keep_their_columns() {
        let logo = rows(80);
        assert!(logo[2].0.contains('●'));
        let column = |row: &str| row.chars().position(|c| c == '╔' || c == '║').unwrap();
        assert_eq!(column(&logo[1].0), column(&logo[2].0));
        assert_eq!(column(&logo[2].0), column(&logo[3].0));
        assert!(
            logo.iter()
                .all(|(left, right)| left.width() == 49 && right.width() == 18)
        );
        assert!(color_parts("v0.1.0 · ~/.crabot · http://localhost:8787").is_none());
    }
}
