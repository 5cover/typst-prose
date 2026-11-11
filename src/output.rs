/// Joins extracted units into the stdout format shared by both modes: one unit per
/// blank-line-separated block, newline-terminated.
pub fn render(units: &[String]) -> String {
    units
        .iter()
        .filter(|u| !u.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join("\n\n")
        + "\n"
}

/// Collapses runs of ASCII whitespace (space, tab, carriage return, newline) to one space and
/// trims both ends. Other characters are kept, including no-break and thin spaces, so text
/// such as `A~B` keeps its U+00A0.
pub fn collapse_ascii_spaces(text: &str) -> String {
    text.split([' ', '\t', '\r', '\n'])
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::collapse_ascii_spaces;

    #[test]
    fn keeps_non_breaking_and_thin_spaces() {
        assert_eq!(collapse_ascii_spaces("A\u{a0}B"), "A\u{a0}B");
        assert_eq!(collapse_ascii_spaces("A\u{2009}B"), "A\u{2009}B");
    }

    #[test]
    fn collapses_ascii_runs_and_trims() {
        assert_eq!(collapse_ascii_spaces("  one \t two\r\n three  "), "one two three");
        assert_eq!(collapse_ascii_spaces("\u{a0} x"), "\u{a0} x");
    }
}
