//! Deny-list matching. AC4 is decided here rather than in each rule.
//!
//! Hand-rolled rather than regex-backed: every term is a literal phrase, so a regex dependency buys
//! nothing but a second syntax in which a term can be wrong.

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

/// The body with inline code spans replaced by a space — a quotation is not a use, and prose about
/// a rule contains that rule's terms. Replaced rather than removed, to keep the word boundary.
///
/// An unterminated backtick strips nothing, and deciding parity *before* stripping is what provides
/// that: pairing while emitting is already past the text it removed by the time it learns the block
/// is unbalanced.
pub fn strip_code_spans(body: &str) -> String {
    if body.matches('`').count() % 2 == 1 {
        return body.to_string();
    }
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(open) = rest.find('`') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else { break };
        out.push_str(&rest[..open]);
        out.push(' ');
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

/// The first deny-list term that appears in `body` at word boundaries, if any.
///
/// Code spans come out first, and only here and in `task` — wherever the subject is a listed term,
/// a backtick around it marks a mention rather than a use. `words` feeds `restate`, where a
/// backticked identifier is a real mention of the symbol and must still count; `fileref` matches a
/// path shape, which backticks are the ordinary way to write.
pub fn first_match<'t>(body: &str, terms: &[&'t str]) -> Option<&'t str> {
    let haystack = normalise(&strip_code_spans(body)).to_lowercase();
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

    /// Every case here is a real finding laconic reported against its own source.
    #[test]
    fn a_term_inside_a_code_span_is_a_quotation() {
        // From the column-arithmetic reporting code: a narration example quoted inside a span.
        assert_eq!(
            first_match(
                "`x := \"日本\" // changed to use a map` reports four columns right",
                &["changed to"]
            ),
            None
        );
        // From the pack-concern suite: the span crosses a line break, which the block joins.
        assert_eq!(
            first_match(
                "leaving it out would make `<!-- changed to use a\nmap -->` invisible",
                &["changed to"]
            ),
            None
        );
        // The same term outside a span still fires, with a span elsewhere in the block.
        assert_eq!(
            first_match(
                "`TODO` is all caps; we changed to a map here",
                &["changed to"]
            ),
            Some("changed to")
        );
    }

    /// One stray backtick must not silence the rest of the block — the failure would be worse than
    /// the quotation it is guarding against, and trivially evadable on purpose.
    ///
    /// Both sides of the stray mark, because only one of them is the position greedy pairing gets
    /// right: a term *before* the unpaired tick survives whatever the loop does, since the loop
    /// breaks there and flushes the remainder. A term after it is the case that was silently lost.
    #[test]
    fn an_unterminated_code_span_strips_nothing() {
        assert_eq!(
            first_match("we changed to a map ` and never closed it", &["changed to"]),
            Some("changed to")
        );
        assert_eq!(
            first_match(
                "a stray ` mark; we changed to a map for `speed`",
                &["changed to"]
            ),
            Some("changed to")
        );
        // Rust's double-backtick escape is the idiom that produces an odd count in real prose.
        assert_eq!(
            first_match(
                "write `` ` `` to mean a tick; we changed to a map",
                &["changed to"]
            ),
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
