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
