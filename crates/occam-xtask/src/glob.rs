//! Path and identifier globs: `*` stops at `/`, `**` does not, and `**/` also matches nothing at all.

/// Whether `pattern` matches the whole of `text`.
pub fn matches(pattern: &str, text: &str) -> bool {
    if let Some(rest) = pattern.strip_prefix("**/") {
        return matches(rest, text) || text.split_once('/').is_some_and(|(_, tail)| matches(pattern, tail));
    }
    if let Some(rest) = pattern.strip_prefix("**") {
        return tails(text).any(|tail| matches(rest, tail));
    }
    if let Some(rest) = pattern.strip_prefix('*') {
        let shortest = text.len().saturating_sub(text.find('/').unwrap_or(text.len()));
        return tails(text).filter(|tail| tail.len() >= shortest).any(|tail| matches(rest, tail));
    }
    let mut chars = text.chars();
    match chars.next() {
        Some(c) => pattern.strip_prefix(c).is_some_and(|rest| matches(rest, chars.as_str())),
        None => pattern.is_empty(),
    }
}

/// Every suffix of `text`, the whole of it and the empty one included.
fn tails(text: &str) -> impl Iterator<Item = &str> {
    text.char_indices().map(|(i, _)| i).chain([text.len()]).filter_map(|i| text.get(i..))
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn literals_match_only_themselves() {
        assert!(matches("a/b.rs", "a/b.rs"));
        assert!(matches("", ""));
        assert!(!matches("a", "ab"));
        assert!(!matches("ab", "a"));
        assert!(!matches("a", ""));
        assert!(!matches("", "a"));
    }

    #[test]
    fn a_star_stops_at_a_slash() {
        assert!(matches("gp_*", "gp_Pnt"));
        assert!(matches("*.rs", "a.rs"));
        assert!(matches("a/*/c", "a/b/c"));
        assert!(matches("a/*", "a/"));
        assert!(!matches("*.rs", "a/b.rs"));
        assert!(!matches("gp_*", "gq_Pnt"));
    }

    #[test]
    fn a_double_star_crosses_slashes() {
        assert!(matches("**", "a/b/c"));
        assert!(matches("**", ""));
        assert!(matches("a/**", "a/b/c"));
        assert!(matches("**.rs", "a/b.rs"));
        assert!(!matches("a/**", "b/c"));
        assert!(!matches("**.rs", "a/b.txt"));
    }

    #[test]
    fn a_double_star_slash_may_match_nothing() {
        assert!(matches("**/b.rs", "b.rs"));
        assert!(matches("**/b.rs", "a/b.rs"));
        assert!(matches("**/b.rs", "x/y/b.rs"));
        assert!(!matches("**/b.rs", "xb.rs"));
        assert!(!matches("**/b.rs", "a/c.rs"));
    }
}
