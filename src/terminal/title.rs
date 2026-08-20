// Claude Code rotates ◐◓◑◒ in the title while working, and uses the star set
// between turns; both are one glyph followed by a space.
const CLAUDE_ACTIVITY_GLYPHS: &str = "·✢✳✶✻✽◐◓◑◒";

/// The activity glyph an agent put in front of its title, when the title is
/// exactly one recognized glyph followed by a space or nothing: braille
/// spinner cells, or the set Claude Code cycles between turns and while
/// working.
pub(crate) fn leading_activity_glyph(title: &str) -> Option<char> {
    let title = crate::platform::terminal_title_for_presentation(title).trim();
    let first = title.chars().next()?;
    let after_first = &title[first.len_utf8()..];
    let recognized =
        matches!(first, '\u{2800}'..='\u{28ff}') || CLAUDE_ACTIVITY_GLYPHS.contains(first);
    (recognized
        && (after_first.is_empty() || after_first.chars().next().is_some_and(char::is_whitespace)))
    .then_some(first)
}

pub(crate) fn stripped_terminal_title(title: &str) -> Option<String> {
    let title = crate::platform::terminal_title_for_presentation(title).trim();
    if title.is_empty() {
        return None;
    }

    let stripped = match leading_activity_glyph(title) {
        Some(first) => title[first.len_utf8()..].trim(),
        None => title,
    };

    (!stripped.is_empty()).then(|| stripped.to_string())
}

#[cfg(test)]
mod tests {
    use super::{leading_activity_glyph, stripped_terminal_title};

    #[test]
    fn leading_activity_glyph_requires_a_lone_recognized_first_char() {
        assert_eq!(leading_activity_glyph("◐ task"), Some('◐'));
        assert_eq!(leading_activity_glyph("⠋ task"), Some('⠋'));
        assert_eq!(leading_activity_glyph("  ✳  "), Some('✳'));
        assert_eq!(leading_activity_glyph("◐task"), None);
        assert_eq!(leading_activity_glyph("task"), None);
        assert_eq!(leading_activity_glyph(""), None);
    }

    #[test]
    fn strips_one_recognized_leading_activity_glyph() {
        for title in [
            "⠋ task",
            "✳ task",
            "  ⠙   task  ",
            "✢ task",
            "✻ task",
            "◐ task",
            "◓ task",
            "◑ task",
            "◒ task",
        ] {
            assert_eq!(stripped_terminal_title(title).as_deref(), Some("task"));
        }
        assert_eq!(
            stripped_terminal_title("⠋ ⠙ task").as_deref(),
            Some("⠙ task")
        );
    }

    #[test]
    fn preserves_unrecognized_or_unbounded_symbols() {
        for (title, expected) in [
            ("★task", "★task"),
            ("★ production", "★ production"),
            ("✨ task", "✨ task"),
            ("☼ status", "☼ status"),
            ("@ task", "@ task"),
            ("task ⠋ detail", "task ⠋ detail"),
            ("[prod] task", "[prod] task"),
        ] {
            assert_eq!(stripped_terminal_title(title).as_deref(), Some(expected));
        }
    }

    #[test]
    fn preserves_unicode_text_and_elides_empty_results() {
        assert_eq!(
            stripped_terminal_title(" ⠋ 修复🙂标题 ").as_deref(),
            Some("修复🙂标题")
        );
        assert_eq!(stripped_terminal_title("  "), None);
        assert_eq!(stripped_terminal_title("⠋   "), None);
    }

    #[cfg(windows)]
    #[test]
    fn strips_one_windows_elevation_decoration_before_activity_glyph() {
        assert_eq!(
            stripped_terminal_title("Administrator:   ⠋ task").as_deref(),
            Some("task")
        );
        assert_eq!(
            stripped_terminal_title("Administrator: Administrator: task").as_deref(),
            Some("Administrator: task")
        );
        assert_eq!(stripped_terminal_title("Administrator: "), None);
    }
}
