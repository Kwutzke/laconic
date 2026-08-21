//! Deny-list matching.
//!
//! AC4 is decided here, not in each rule: matching is case-insensitive and respects word
//! boundaries, so `previously` does not match inside `previouslyKnownAs` and a term never fires on
//! a longer word that merely contains it.
//!
//! Hand-rolled rather than regex-backed. Every term in the set is a literal phrase, so the whole
//! requirement is "find this phrase at word boundaries" — and a regex dependency buys nothing but a
//! second syntax in which a term can be wrong.

/// A word character for boundary purposes. A phrase may contain spaces and punctuation; what must
/// not touch it on either side is a letter, digit or underscore.
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whitespace runs collapsed to one space, so a phrase split across the lines of one block still
/// matches. A block is the unit a rule receives; where its author happened to break the line is not
/// information, and `// changed\n// to use a map` is the same narration as one line.
pub fn normalise(body: &str) -> String {
    body.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The first deny-list term that appears in `body` at word boundaries, if any.
pub fn first_match<'t>(body: &str, terms: &[&'t str]) -> Option<&'t str> {
    let haystack = normalise(body).to_lowercase();
    terms
        .iter()
        .copied()
        .find(|term| contains_at_word_boundary(&haystack, &term.to_lowercase()))
}

/// Whether `needle` — already lowercase — occurs in `haystack` bounded by non-word characters.
pub fn contains_at_word_boundary(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let mut from = 0;
    while let Some(rel) = haystack[from..].find(needle) {
        let start = from + rel;
        let end = start + needle.len();
        let before_ok = haystack[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !is_word(c));
        let after_ok = haystack[end..].chars().next().is_none_or(|c| !is_word(c));
        if before_ok && after_ok {
            return true;
        }
        // Advance by one character rather than one byte: a needle can begin mid-multibyte
        // otherwise, and `str::find` would panic on a non-boundary index.
        from = start + haystack[start..].chars().next().map_or(1, char::len_utf8);
    }
    false
}

/// Words in a comment body, lowercased and stripped of surrounding punctuation.
pub fn words(body: &str) -> Vec<String> {
    body.split(|c: char| !is_word(c))
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AC4, both halves.
    #[test]
    fn a_term_never_fires_inside_a_longer_word() {
        assert!(contains_at_word_boundary(
            "previously we did x",
            "previously"
        ));
        assert!(!contains_at_word_boundary(
            "previouslyknownas",
            "previously"
        ));
        assert!(!contains_at_word_boundary("we fixed the parser", "fix"));
        assert!(contains_at_word_boundary("we fix the parser", "fix"));
    }

    #[test]
    fn matching_is_case_insensitive() {
        assert_eq!(
            first_match("Changed To use a map", &["changed to"]),
            Some("changed to")
        );
        assert_eq!(
            first_match("CHANGED TO use a map", &["changed to"]),
            Some("changed to")
        );
    }

    #[test]
    fn punctuation_bounds_a_term() {
        assert!(contains_at_word_boundary("(previously)", "previously"));
        assert!(contains_at_word_boundary("previously.", "previously"));
        assert!(contains_at_word_boundary("previously", "previously"));
    }

    /// A phrase spanning a line break still matches. A block is the unit a rule receives, so
    /// `// changed` followed by `// to use a map` is the same narration as one line — and a rule
    /// that missed it would be trivially evadable by pressing Enter.
    #[test]
    fn a_phrase_matches_across_the_lines_of_one_block() {
        assert_eq!(
            first_match(" changed\n to use a map", &["changed to"]),
            Some("changed to")
        );
        assert_eq!(
            first_match("we changed to a map\nand it is faster", &["changed to"]),
            Some("changed to")
        );
    }

    #[test]
    fn multibyte_text_does_not_panic() {
        assert!(!contains_at_word_boundary("— naïve — résumé", "fix"));
        assert!(contains_at_word_boundary("— probably naïve", "probably"));
    }

    #[test]
    fn words_splits_on_non_word_characters() {
        assert_eq!(
            words("  Handles the logic, etc. "),
            ["handles", "the", "logic", "etc"]
        );
    }
}
