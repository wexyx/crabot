/// Inward-leaning perspective: C leans right, T leans left, over a shaded base.
pub(super) fn lines(columns: usize) -> &'static [&'static str] {
    if columns < 60 {
        return &[
            "  ▄▀▀ █▀▄ ▄▀█ █▄▄ █▀█ ▀█▀",
            " █▄▄  █▀▄ █▀█ █▄█ █▄█   █",
            "  ░░   ░░  ░░  ░░  ░░   ░",
        ];
    }
    &[
        "      ██████╗██████╗  █████╗ ██████╗  ██████╗ ████████╗",
        "    ██╔════╝██╔══██╗ ██╔══██╗██╔══██╗ ██╔═══██╗╚══██╔══╝",
        "   ██║      ██████╔╝ ███████║██████╔╝ ██║   ██║    ██║",
        "  ██║      ██╔══██╗ ██╔══██║  ██╔══██╗ ██║   ██║    ██║",
        " ╚██████╗  ██║  ██║ ██║  ██║  ██████╔╝ ╚██████╔╝     ██║",
        " ╚═════╝  ╚═╝  ╚═╝  ╚═╝  ╚═╝  ╚═════╝    ╚═════╝      ╚═╝",
        "  ░░░░░░   ░░  ░░    ░░  ░░    ░░░░░░     ░░░░░░       ░░",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;
    #[test]
    fn wordmark_fits_normal_and_compact_frames() {
        for columns in [30, 40, 56, 60, 76] {
            assert!(lines(columns).iter().all(|line| line.width() <= columns));
        }
    }

    #[test]
    fn outer_letters_lean_toward_the_center() {
        let logo = lines(76);
        // C moves left toward its base; T moves right. Count cells, not UTF-8 bytes.
        let left = |row: &str| row.chars().position(|c| c == '█').unwrap();
        let right =
            |row: &str| row.chars().count() - row.chars().rev().position(|c| c == '█').unwrap() - 1;
        assert!(left(logo[1]) > left(logo[2]));
        assert!(left(logo[2]) > left(logo[3]));
        assert!(right(logo[1]) < right(logo[2]));
        assert!(right(logo[2]) < right(logo[3]));
    }
}
