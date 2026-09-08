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

/// Whether `name` appears in `body` marked as code rather than used as a word.
///
/// Three markings, all of them things an author does deliberately: a call's parentheses
/// (`normalize()`), a code span (`` `normalize` ``), and a selector (`c.fetch`). This is what
/// separates a reference to a symbol from a sentence that happens to contain the same letters —
/// "the variant's own `normalize()` validates content" against "cache misses fetch from SAP".
///
/// The author's own notation decides it, which is the point: nothing here guesses at intent.
pub fn referenced_as_code(body: &str, name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let mut from = 0;
    while let Some(rel) = body[from..].find(name) {
        let start = from + rel;
        let end = start + name.len();
        let before = body[..start].chars().next_back();
        let after = body[end..].chars().next();
        let bounded = before.is_none_or(|c| !is_word(c)) && after.is_none_or(|c| !is_word(c));
        if bounded
            && (after == Some('(')
                || after == Some('`')
                || before == Some('.')
                || before == Some('`'))
        {
            return true;
        }
        from = start + body[start..].chars().next().map_or(1, char::len_utf8);
    }
    false
}

/// Whether a name is an ordinary English word rather than an identifier a reader would recognise
/// as one.
///
/// The list is deliberately short and holds only words nobody would coin as a symbol *in
/// isolation*. A compound reads as an identifier however common its parts — `buildSearchInfos` and
/// `knownSortFields` are unambiguous in plain prose — so only the bare word is in question here.
///
/// Being on this list is not an exemption: [`referenced_as_code`] still fires on `lock()` and
/// `` `page` ``. The list decides one thing only, which is whether a bare occurrence in prose is
/// evidence of anything.
pub fn is_common_word(name: &str) -> bool {
    COMMON_WORDS
        .binary_search(&name.to_lowercase().as_str())
        .is_ok()
}

/// Common English words, sorted for binary search.
///
/// Data rather than a dependency, on the same terms as this module's refusal of a regex engine.
/// Short by intention: every entry is a word this rule will stop reporting in prose, so the cost of
/// a wrong entry is a silent rule and the cost of a missing one is noise that says so out loud.
const COMMON_WORDS: &[&str] = &[
    "about",
    "above",
    "action",
    "active",
    "add",
    "address",
    "after",
    "again",
    "against",
    "all",
    "allow",
    "alone",
    "along",
    "also",
    "always",
    "and",
    "another",
    "answer",
    "any",
    "append",
    "apply",
    "area",
    "around",
    "ask",
    "attach",
    "available",
    "away",
    "back",
    "bad",
    "base",
    "before",
    "begin",
    "behind",
    "below",
    "best",
    "better",
    "between",
    "big",
    "bind",
    "body",
    "book",
    "both",
    "bottom",
    "bound",
    "box",
    "break",
    "bring",
    "build",
    "but",
    "buy",
    "call",
    "can",
    "cancel",
    "care",
    "carry",
    "case",
    "catch",
    "cause",
    "cell",
    "center",
    "chain",
    "chance",
    "change",
    "channel",
    "check",
    "child",
    "choice",
    "choose",
    "claim",
    "class",
    "clean",
    "clear",
    "close",
    "code",
    "collect",
    "colour",
    "column",
    "come",
    "common",
    "company",
    "compare",
    "complete",
    "condition",
    "connect",
    "consider",
    "contain",
    "content",
    "context",
    "continue",
    "control",
    "copy",
    "cost",
    "count",
    "cover",
    "create",
    "current",
    "cut",
    "data",
    "date",
    "day",
    "dead",
    "deal",
    "decide",
    "deep",
    "default",
    "delete",
    "deliver",
    "depend",
    "describe",
    "design",
    "detail",
    "detect",
    "develop",
    "device",
    "different",
    "direct",
    "disable",
    "discard",
    "dispatch",
    "display",
    "distance",
    "document",
    "does",
    "done",
    "door",
    "down",
    "draw",
    "drive",
    "drop",
    "during",
    "each",
    "early",
    "easy",
    "edge",
    "edit",
    "effect",
    "either",
    "element",
    "else",
    "empty",
    "enable",
    "end",
    "enough",
    "enter",
    "entry",
    "equal",
    "error",
    "even",
    "event",
    "ever",
    "every",
    "exact",
    "example",
    "except",
    "exist",
    "expect",
    "explain",
    "extend",
    "face",
    "fact",
    "fail",
    "fall",
    "false",
    "family",
    "fast",
    "fetch",
    "field",
    "file",
    "filename",
    "fill",
    "filter",
    "final",
    "find",
    "finding",
    "finish",
    "first",
    "fit",
    "fix",
    "flag",
    "flight",
    "float",
    "flow",
    "follow",
    "food",
    "for",
    "force",
    "form",
    "format",
    "forward",
    "free",
    "friend",
    "from",
    "front",
    "full",
    "function",
    "future",
    "game",
    "general",
    "get",
    "give",
    "global",
    "good",
    "great",
    "green",
    "group",
    "grow",
    "guard",
    "handle",
    "happen",
    "hard",
    "has",
    "have",
    "head",
    "hear",
    "heart",
    "help",
    "here",
    "hide",
    "high",
    "history",
    "hold",
    "home",
    "hook",
    "hope",
    "host",
    "hour",
    "house",
    "how",
    "idea",
    "ignore",
    "image",
    "import",
    "include",
    "index",
    "info",
    "inner",
    "input",
    "insert",
    "inside",
    "instead",
    "into",
    "issue",
    "item",
    "job",
    "join",
    "just",
    "keep",
    "key",
    "kind",
    "know",
    "label",
    "land",
    "language",
    "large",
    "last",
    "late",
    "later",
    "law",
    "lead",
    "learn",
    "leave",
    "left",
    "length",
    "less",
    "let",
    "letter",
    "level",
    "life",
    "light",
    "like",
    "limit",
    "line",
    "link",
    "list",
    "little",
    "live",
    "load",
    "local",
    "lock",
    "log",
    "long",
    "look",
    "loop",
    "loss",
    "low",
    "made",
    "main",
    "major",
    "make",
    "man",
    "many",
    "map",
    "mark",
    "master",
    "match",
    "matter",
    "may",
    "mean",
    "measure",
    "member",
    "memory",
    "merge",
    "message",
    "method",
    "middle",
    "might",
    "mind",
    "minute",
    "miss",
    "mode",
    "model",
    "money",
    "month",
    "more",
    "most",
    "mount",
    "move",
    "much",
    "must",
    "name",
    "near",
    "need",
    "network",
    "never",
    "new",
    "next",
    "nice",
    "night",
    "node",
    "none",
    "normal",
    "normalize",
    "not",
    "note",
    "nothing",
    "notice",
    "now",
    "number",
    "object",
    "off",
    "offer",
    "office",
    "often",
    "old",
    "once",
    "one",
    "only",
    "open",
    "option",
    "order",
    "other",
    "out",
    "output",
    "over",
    "owner",
    "pack",
    "package",
    "page",
    "pair",
    "parent",
    "part",
    "party",
    "pass",
    "past",
    "path",
    "pattern",
    "pause",
    "pay",
    "people",
    "period",
    "person",
    "phase",
    "pick",
    "picture",
    "piece",
    "place",
    "plan",
    "play",
    "please",
    "point",
    "policy",
    "poll",
    "pool",
    "poor",
    "pop",
    "port",
    "position",
    "possible",
    "post",
    "power",
    "prepare",
    "present",
    "press",
    "price",
    "prices",
    "print",
    "private",
    "problem",
    "process",
    "produce",
    "program",
    "project",
    "proof",
    "proper",
    "provide",
    "public",
    "pull",
    "purpose",
    "push",
    "put",
    "query",
    "queue",
    "quick",
    "quiet",
    "quite",
    "raise",
    "range",
    "rate",
    "reach",
    "read",
    "reader",
    "ready",
    "real",
    "reason",
    "record",
    "recorder",
    "reduce",
    "refresh",
    "region",
    "register",
    "reject",
    "relate",
    "release",
    "remain",
    "remove",
    "render",
    "repeat",
    "replace",
    "report",
    "request",
    "require",
    "reset",
    "resolve",
    "resolved",
    "resource",
    "response",
    "rest",
    "result",
    "retry",
    "return",
    "reuse",
    "review",
    "rewrite",
    "rich",
    "right",
    "ring",
    "rise",
    "risk",
    "road",
    "role",
    "room",
    "root",
    "round",
    "route",
    "row",
    "rule",
    "rules",
    "run",
    "safe",
    "same",
    "save",
    "say",
    "scale",
    "scan",
    "school",
    "score",
    "screen",
    "search",
    "season",
    "seat",
    "second",
    "section",
    "see",
    "seem",
    "select",
    "send",
    "sense",
    "series",
    "serve",
    "server",
    "service",
    "session",
    "set",
    "settle",
    "shape",
    "share",
    "shared",
    "shift",
    "ship",
    "short",
    "should",
    "show",
    "side",
    "sign",
    "signal",
    "simple",
    "since",
    "single",
    "site",
    "size",
    "skip",
    "sleep",
    "slice",
    "slow",
    "small",
    "social",
    "some",
    "soon",
    "sort",
    "sound",
    "source",
    "space",
    "speak",
    "special",
    "speed",
    "spend",
    "split",
    "stack",
    "staff",
    "stage",
    "stand",
    "star",
    "start",
    "state",
    "status",
    "stay",
    "step",
    "still",
    "stop",
    "storage",
    "store",
    "story",
    "stream",
    "street",
    "strong",
    "study",
    "style",
    "such",
    "sum",
    "summary",
    "support",
    "sure",
    "swap",
    "switch",
    "system",
    "table",
    "take",
    "talk",
    "target",
    "task",
    "team",
    "tell",
    "test",
    "text",
    "than",
    "that",
    "the",
    "their",
    "them",
    "then",
    "there",
    "these",
    "they",
    "thing",
    "think",
    "this",
    "those",
    "though",
    "thread",
    "three",
    "through",
    "throw",
    "thus",
    "time",
    "title",
    "today",
    "together",
    "token",
    "too",
    "top",
    "total",
    "touch",
    "toward",
    "town",
    "track",
    "trade",
    "train",
    "transfer",
    "tree",
    "trip",
    "true",
    "trust",
    "try",
    "turn",
    "two",
    "type",
    "under",
    "understand",
    "unit",
    "unless",
    "until",
    "update",
    "upon",
    "usage",
    "use",
    "user",
    "using",
    "usual",
    "valid",
    "value",
    "version",
    "very",
    "view",
    "visit",
    "voice",
    "wait",
    "walk",
    "wall",
    "want",
    "warn",
    "watch",
    "water",
    "way",
    "week",
    "weight",
    "well",
    "what",
    "when",
    "where",
    "which",
    "while",
    "white",
    "who",
    "whole",
    "why",
    "wide",
    "will",
    "win",
    "window",
    "wire",
    "with",
    "within",
    "without",
    "word",
    "work",
    "worker",
    "world",
    "worth",
    "would",
    "wrap",
    "write",
    "writer",
    "wrong",
    "year",
    "yes",
    "yet",
    "young",
    "your",
    "zone",
];

/// Words in a comment body, lowercased and stripped of surrounding punctuation.
pub fn words(body: &str) -> Vec<String> {
    cased_words(body)
        .into_iter()
        .map(|w| w.to_lowercase())
        .collect()
}

/// The same split, with case preserved.
///
/// `implInInterface` needs this and the other rules do not: it compares a comment's words against
/// declared identifiers, where case is the difference between an exported symbol and its unexported
/// twin. Every other rule matches prose against prose, where case carries nothing.
pub fn cased_words(body: &str) -> Vec<String> {
    body.split(|c: char| !is_word(c))
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `is_common_word` binary-searches the list, so an out-of-order entry silently stops matching
    /// — the rule would go noisy again for exactly one word and nothing would say so.
    #[test]
    fn the_word_list_is_sorted_and_unique() {
        let mut sorted = COMMON_WORDS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.as_slice(),
            COMMON_WORDS,
            "the list is binary-searched, so it must be sorted and free of duplicates"
        );
    }

    /// The three markings, and the sentence each is there to tell apart.
    #[test]
    fn code_form_separates_a_reference_from_a_word() {
        assert!(referenced_as_code(
            "The variant's own normalize() validates content.",
            "normalize"
        ));
        assert!(referenced_as_code(
            "Delegates to `buildInfos` here.",
            "buildInfos"
        ));
        assert!(referenced_as_code(
            "Cache misses call c.fetch first.",
            "fetch"
        ));

        assert!(!referenced_as_code(
            "Cache misses fetch from SAP before returning.",
            "fetch"
        ));
        assert!(!referenced_as_code(
            "Each lock acquisition gets its own connection.",
            "lock"
        ));
        // A longer word merely containing the name is not a reference to it.
        assert!(!referenced_as_code("The locker() helper.", "lock"));
    }

    #[test]
    fn a_compound_is_not_a_common_word_however_common_its_parts() {
        assert!(is_common_word("lock"));
        assert!(is_common_word("Work"), "the test is case-insensitive");
        assert!(!is_common_word("buildSearchInfos"));
        assert!(!is_common_word("knownSortFields"));
        assert!(!is_common_word("snapshoter"));
    }

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
