/// Normalize a tag to Stack Overflow-compatible format.
///
/// Allowed characters: a-z, 0-9, +, #, ., -
/// Preserves dots (e.g., `next.js`), plus signs (e.g., `c++`),
/// and hash (e.g., `c#`).
pub fn normalize_tag(tag: &str) -> String {
    let lowered = tag.trim().to_lowercase();

    let replaced: String = lowered
        .chars()
        .map(|c| match c {
            ' ' | '_' => '-',
            _ => c,
        })
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '#' | '.' | '-'))
        .collect();

    // Collapse consecutive hyphens
    let mut collapsed = String::with_capacity(replaced.len());
    let mut prev_hyphen = false;
    for c in replaced.chars() {
        if c == '-' {
            if !prev_hyphen {
                collapsed.push(c);
            }
            prev_hyphen = true;
        } else {
            collapsed.push(c);
            prev_hyphen = false;
        }
    }

    // Strip leading/trailing hyphens
    collapsed.trim_matches('-').to_string()
}

/// Normalize a slice of tags: normalize each, deduplicate, filter empty.
///
/// Preserves the order of first occurrence.
pub fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::with_capacity(tags.len());

    for tag in tags {
        let normalized = normalize_tag(tag);
        if !normalized.is_empty() && seen.insert(normalized.clone()) {
            result.push(normalized);
        }
    }

    result
}
