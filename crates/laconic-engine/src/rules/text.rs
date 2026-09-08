//! The seven rules whose whole input is the stripped comment body.
//!
//! Every instruction names the change to make, never the defect. The consumer is an LLM acting on
//! one diagnostic with no surrounding context: it will read `remove this comment` and remove the
//! comment, and read `narration detected` and write a comment explaining that narration was
//! detected.

use crate::domain::Attachment;
use crate::rule::{BlockContext, BlockRule, RuleHit};
use crate::rules::matching::{contains_at_word_boundary, first_match, normalise, strip_code_spans};

/// Change-log and process language: what the edit was, rather than what the code is.
const NARRATION: &[&str] = &[
    "changed to",
    "updated to",
    "previously",
    "as requested",
    "now we",
    "note that we",
    "moved to",
    "refactored to",
    "this now",
    "per your",
];

/// *adapted from* can be a required licence notice, which is why this rule ships autofix off and
/// why the header carve-out runs before it.
const ATTRIBUTION: &[&str] = &[
    "written by",
    "authored by",
    "adapted from",
    "courtesy of",
    "based on code from",
];

const HEDGING: &[&str] = &[
    "should work",
    "for now",
    "in most cases",
    "if needed",
    "probably",
    "might need",
];

const VAGUE: &[&str] = &[
    "handles the logic",
    "does the necessary",
    "various things",
    "as appropriate",
];

const TASK_MARKERS: &[&str] = &["TODO", "FIXME", "XXX", "HACK"];

/// Characters a banner is built from.
const RULE_CHARS: &[char] = &['-', '=', '*', '_', '#', '~', '+'];

pub struct Narration;

impl BlockRule for Narration {
    fn id(&self) -> &'static str {
        "narration"
    }
    fn check(&self, ctx: &BlockContext) -> Option<RuleHit> {
        let term = first_match(&ctx.block.body(), NARRATION)?;
        Some(RuleHit::new(
            ctx.block.span.clone(),
            format!(
                "remove this comment: it narrates the edit (\"{term}\") rather than describing the code"
            ),
        ))
    }
}

pub struct Banner;

impl BlockRule for Banner {
    fn id(&self) -> &'static str {
        "banner"
    }

    /// Matching is over the body from concern 7, never the raw line, which begins with the marker.
    ///
    /// A trailing comment is never a label: it annotates the code on its line. This rule ships
    /// autofix **on**, so that distinction is what stands between `// ACTA, ELEKTRA` beside a list
    /// of ids and `laconic fix` deleting the only record of which id is which.
    fn check(&self, ctx: &BlockContext) -> Option<RuleHit> {
        let body = ctx.block.body();
        let labels_allowed = ctx.block.attachment != Attachment::AttachedTrailing;
        let reason = banner_reason(&body, labels_allowed)?;
        Some(RuleHit::new(
            ctx.block.span.clone(),
            format!("remove this comment: it is {reason}, which carries no information"),
        ))
    }
}

/// Whether `banner` would report this block — the precedence test `detached` applies. Exposed
/// rather than duplicated, since the two predicates disagreeing is the double-reporting it prevents.
pub fn is_banner(body: &str, attachment: Attachment) -> bool {
    banner_reason(body, attachment != Attachment::AttachedTrailing).is_some()
}

fn banner_reason(body: &str, labels_allowed: bool) -> Option<&'static str> {
    let lines: Vec<&str> = body
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return None;
    }

    if lines
        .iter()
        .all(|l| l.chars().count() >= 3 && l.chars().all(|c| RULE_CHARS.contains(&c)))
    {
        return Some("a rule of repeated punctuation");
    }
    if lines.iter().all(|l| is_step_label(l)) {
        return Some("a step number");
    }
    if labels_allowed
        && lines.len() == 1
        && !claimed_by_task(lines[0])
        && (is_section_label(lines[0]) || is_fenced_label(lines[0]))
    {
        return Some("a section label");
    }
    None
}

/// Whether `task` has a claim on this line, which stands `banner` down. Gates the label branch
/// rather than one predicate, so no label form added later can opt out of it. A marker at a word
/// boundary, not the whole line: a decorated marker carries content and `banner` autofixes.
fn claimed_by_task(line: &str) -> bool {
    let lower = line.to_lowercase();
    TASK_MARKERS
        .iter()
        .any(|m| contains_at_word_boundary(&lower, &m.to_lowercase()))
}

/// A label fenced on **both** sides — `--- helpers ---`, `=== Setup ===`. Case is not the signal:
/// reading all-caps as one missed 58 of 85 real banners in a corpus. Both sides required, so a
/// heading running on into content is left alone.
fn is_fenced_label(line: &str) -> bool {
    let line = line.trim();
    let fence =
        |s: &mut dyn Iterator<Item = char>| s.take_while(|c| RULE_CHARS.contains(c)).count();
    if fence(&mut line.chars()) < 3 || fence(&mut line.chars().rev()) < 3 {
        return false;
    }
    let inner = line.trim_matches(|c: char| RULE_CHARS.contains(&c) || c.is_whitespace());
    inner.chars().any(char::is_alphabetic)
}

/// `Step 3:` and `STEP 3 -`. A step number restates position, which the reader can already see.
fn is_step_label(line: &str) -> bool {
    let lower = line.to_lowercase();
    let Some(rest) = lower.strip_prefix("step") else {
        return false;
    };
    let rest = rest.trim_start();
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return false;
    }
    let after = rest[digits.len()..].trim_start();
    after.is_empty() || after.starts_with([':', '-', '.', ')'])
}

/// A standalone all-caps label — `HELPERS`, `--- SETUP ---`. A shape test only; whether a task
/// marker takes precedence is [`claimed_by_task`]'s question, asked once for every label form.
fn is_section_label(line: &str) -> bool {
    let stripped = line.trim_matches(|c: char| RULE_CHARS.contains(&c) || c.is_whitespace());
    let letters: Vec<char> = stripped.chars().filter(|c| c.is_alphabetic()).collect();
    letters.len() >= 2 && !letters.iter().any(|c| c.is_lowercase())
}

pub struct Attribution;

impl BlockRule for Attribution {
    fn id(&self) -> &'static str {
        "attribution"
    }
    fn check(&self, ctx: &BlockContext) -> Option<RuleHit> {
        let term = first_match(&ctx.block.body(), ATTRIBUTION)?;
        Some(RuleHit::new(
            ctx.block.span.clone(),
            format!(
                "remove this comment: authorship (\"{term}\") belongs in version control. If it is a required licence notice, keep it and add `laconic:ignore attribution — <why>`"
            ),
        ))
    }
}

pub struct Hedging;

impl BlockRule for Hedging {
    fn id(&self) -> &'static str {
        "hedging"
    }
    fn check(&self, ctx: &BlockContext) -> Option<RuleHit> {
        let term = first_match(&ctx.block.body(), HEDGING)?;
        Some(RuleHit::new(
            ctx.block.span.clone(),
            format!(
                "rewrite this comment: replace \"{term}\" with the condition it hides — what holds, and what is untested"
            ),
        ))
    }
}

pub struct Vague;

impl BlockRule for Vague {
    fn id(&self) -> &'static str {
        "vague"
    }
    fn check(&self, ctx: &BlockContext) -> Option<RuleHit> {
        let body = ctx.block.body();
        if let Some(term) = first_match(&body, VAGUE) {
            return Some(RuleHit::new(
                ctx.block.span.clone(),
                format!("rewrite this comment: \"{term}\" names no behaviour — say what it does"),
            ));
        }
        ends_with_etc(&body).then(|| {
            RuleHit::new(
                ctx.block.span.clone(),
                "rewrite this comment: replace the trailing \"etc.\" with the cases it stands for",
            )
        })
    }
}

/// `etc.` as a sentence ender, not `etc.` mid-sentence where it may be doing real work.
fn ends_with_etc(body: &str) -> bool {
    let normalised = normalise(body).to_lowercase();
    let trimmed = normalised.trim_end_matches(['.', ' ', ')', ']', '"', '\'']);
    trimmed.ends_with("etc")
        && trimmed
            .strip_suffix("etc")
            .is_some_and(|before| before.is_empty() || before.ends_with([' ', ',']))
}

pub struct Task;

impl BlockRule for Task {
    fn id(&self) -> &'static str {
        "task"
    }
    fn check(&self, ctx: &BlockContext) -> Option<RuleHit> {
        let body = normalise(&strip_code_spans(&ctx.block.body()));
        let marker = TASK_MARKERS
            .iter()
            .find(|m| task_without_reference(&body, m))?;
        Some(RuleHit::new(
            ctx.block.span.clone(),
            format!(
                "add an issue reference to this {marker}, as {marker}(#123) or {marker}(KAT-45), or delete it"
            ),
        ))
    }
}

/// `TODO(KAT-123)` and `TODO(#456)` pass; a bare `TODO` does not. The reference is what makes the
/// marker findable again, and a marker nobody can find is a comment that will never be resolved.
fn task_without_reference(body: &str, marker: &str) -> bool {
    let mut from = 0;
    while let Some(rel) = body[from..].find(marker) {
        let start = from + rel;
        let end = start + marker.len();
        let bounded_before = body[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric() && c != '_');
        if bounded_before && !has_reference(&body[end..]) {
            return true;
        }
        from = end;
    }
    false
}

/// The closing parenthesis is required, for the same reason [`strip_code_spans`] decides parity
/// before it strips: `split(')')` yields the whole remainder when there is no `)`, so `TODO(` with
/// nothing closing it reads as a marker that *has* a reference and `task` goes silent on it.
fn has_reference(after: &str) -> bool {
    let Some(rest) = after.strip_prefix('(') else {
        return false;
    };
    let Some(close) = rest.find(')') else {
        return false;
    };
    !rest[..close].trim().is_empty()
}

pub struct FileRef;

impl BlockRule for FileRef {
    fn id(&self) -> &'static str {
        "fileref"
    }

    /// A source-file path per the pack's extension list — concern 1. The instruction names the
    /// alternative, because the consumer acts on the diagnostic alone.
    ///
    /// No code-span carve-out here, unlike the deny-list rules. Backticks around a deny-list term
    /// mark a *mention* of the term; backticks around a path are how a path is ordinarily written,
    /// so stripping them makes the rule evadable by the formatting most authors already use.
    fn check(&self, ctx: &BlockContext) -> Option<RuleHit> {
        let body = ctx.block.body();
        let path = words_with_punctuation(&body)
            .into_iter()
            .find(|w| looks_like_source_path(w, ctx.source_extensions))?;
        Some(RuleHit::new(
            ctx.block.span.clone(),
            format!("reference a symbol, not a file: replace \"{path}\" with the name it defines"),
        ))
    }
}

/// Whitespace-separated words. Punctuation around a word is the trimmer's job — splitting on it
/// here as well was redundant, and mutation testing found the redundancy by changing it with no
/// test noticing.
fn words_with_punctuation(body: &str) -> Vec<&str> {
    body.split_whitespace().collect()
}

fn looks_like_source_path(word: &str, extensions: &[&'static str]) -> bool {
    // Every punctuation mark that can sit against a path in prose, on either side: quotes and
    // brackets open as well as close.
    let candidate = word.trim_matches(['.', '(', ')', '[', ']', ':', ',', ';', '"', '\'', '`']);
    let Some((stem, ext)) = candidate.rsplit_once('.') else {
        return false;
    };
    !stem.is_empty() && extensions.contains(&ext)
}

/// The seven rules whose whole input is the comment body.
pub fn text_rules() -> Vec<Box<dyn BlockRule>> {
    vec![
        Box::new(Narration),
        Box::new(Banner),
        Box::new(Attribution),
        Box::new(Hedging),
        Box::new(Vague),
        Box::new(Task),
        Box::new(FileRef),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::matching::words;

    #[test]
    fn banner_matches_rules_steps_and_labels() {
        assert!(banner_reason(" =====", true).is_some());
        assert!(banner_reason(" ---------------", true).is_some());
        assert!(banner_reason(" Step 1:", true).is_some());
        assert!(banner_reason(" STEP 2 -", true).is_some());
        assert!(banner_reason(" HELPERS", true).is_some());
        assert!(banner_reason(" ---- SETUP ----", true).is_some());
    }

    #[test]
    fn banner_leaves_prose_and_task_markers_alone() {
        assert!(banner_reason(" a real comment", true).is_none());
        assert!(banner_reason(" -- but this is prose", true).is_none());
        assert!(
            banner_reason(" TODO", true).is_none(),
            "task asks the other question"
        );
        assert!(
            banner_reason(" --", true).is_none(),
            "two characters is not a rule"
        );
        assert!(banner_reason(" Step by step", true).is_none());
    }

    #[test]
    fn task_passes_a_marker_that_carries_a_reference() {
        assert!(task_without_reference("TODO fix this", "TODO"));
        assert!(!task_without_reference("TODO(#456) fix this", "TODO"));
        assert!(!task_without_reference("TODO(KAT-123) fix this", "TODO"));
        assert!(task_without_reference("TODO() fix this", "TODO"));
        assert!(
            !task_without_reference("MYTODO list", "TODO"),
            "a marker inside a longer word is not a marker"
        );
    }

    #[test]
    fn etc_fires_as_a_sentence_ender_only() {
        assert!(ends_with_etc(" handles ints, floats, etc."));
        assert!(ends_with_etc(" ints, floats etc"));
        assert!(!ends_with_etc(" etc.d is a directory we scan"));
        assert!(!ends_with_etc(" a real comment"));
    }

    #[test]
    fn fileref_needs_an_extension_the_pack_claims() {
        assert!(looks_like_source_path("parser.go", &["go"]));
        assert!(looks_like_source_path("internal/parser.go,", &["go"]));
        assert!(!looks_like_source_path("parser.go", &["rs"]));
        assert!(!looks_like_source_path("e.g.", &["go"]));
        assert!(!looks_like_source_path("config.json", &["go"]));
    }

    /// Pins a `cargo mutants` survivor; each says so on itself, since a doc claiming a *range* is
    /// falsified by anything inserted into it. Every fence width, because under three rule
    /// characters only `is_section_label` runs — testing one width asserted the exclusion where the
    /// other predicate never runs.
    #[test]
    fn a_decorated_task_marker_is_not_a_section_label() {
        assert!(banner_reason(" -- TODO --", true).is_none());
        assert!(banner_reason(" == FIXME ==", true).is_none());
        assert!(banner_reason(" --- TODO ---", true).is_none());
        assert!(banner_reason(" === FIXME ===", true).is_none());
        assert!(banner_reason(" -- HELPERS --", true).is_some());
        assert!(banner_reason(" --- HELPERS ---", true).is_some());
    }

    /// A decorated marker carrying a reference and a sentence is the destructive case: `banner`
    /// is the only Delete rule besides `narration` shipping autofix on, so classifying this as a
    /// label deletes the sentence and the issue number with it.
    #[test]
    fn a_decorated_task_marker_carrying_content_is_not_a_label() {
        assert!(
            banner_reason(
                "--- TODO(#42) the retry path drops the last batch ---",
                true
            )
            .is_none()
        );
        assert!(banner_reason("### HACK until the upstream fix lands ###", true).is_none());
    }

    /// A backticked path is a path. The carve-out that silenced this was applied by where the
    /// matcher lived rather than by what it matches, which made the rule evadable by backticks.
    #[test]
    fn a_backticked_path_is_still_a_path() {
        let found = words_with_punctuation("Mirrors the shape in `internal/parser.go`.")
            .into_iter()
            .find(|w| looks_like_source_path(w, &["go"]));
        assert_eq!(found, Some("`internal/parser.go`."));
    }

    /// An unterminated `(` is not a reference. Same shape as the backtick parity rule: a delimiter
    /// scan that accepts an unclosed opener silences the rule instead of firing it.
    #[test]
    fn an_unclosed_parenthesis_is_not_an_issue_reference() {
        assert!(task_without_reference("TODO( never closed", "TODO"));
        assert!(task_without_reference("TODO()", "TODO"));
        assert!(!task_without_reference("TODO(#42) the retry path", "TODO"));
    }

    /// Pins a `cargo mutants` survivor.
    /// Two letters is a label; one is not. The boundary is the whole content of the rule.
    #[test]
    fn a_two_letter_label_is_the_shortest_one() {
        assert!(is_section_label("OK"));
        assert!(!is_section_label("X"));
    }

    /// Pins a `cargo mutants` survivor.
    /// `etc` must be a word, not a suffix. Without the boundary check any word ending in those
    /// three letters trips the rule.
    #[test]
    fn etc_must_be_its_own_word() {
        assert!(ends_with_etc(" ints, floats, etc."));
        assert!(
            !ends_with_etc(" the netc"),
            "must end the sentence as its own word"
        );
        assert!(!ends_with_etc(" returns the netc value"));
        assert!(!ends_with_etc(" the codec"));
    }

    /// Pins a `cargo mutants` survivor.
    /// A marker inside an identifier is not a marker. `_TODO` is a name, and firing on it would
    /// ask the reader to add an issue reference to a variable.
    #[test]
    fn an_underscore_prefixed_marker_is_an_identifier() {
        assert!(!task_without_reference("_TODO is the field name", "TODO"));
        assert!(task_without_reference("TODO is the field name", "TODO"));
    }

    /// Pins a `cargo mutants` survivor.
    /// A path is found whatever punctuation sits against it. The trimming is the predicate's alone,
    /// so this is what notices if the trim set loses a mark.
    #[test]
    fn a_path_followed_by_punctuation_is_still_a_path() {
        for body in [
            " see parser.go, then lexer.go",
            " see parser.go; then lexer.go",
            " see \"parser.go\" for the shape",
            " see 'parser.go' for the shape",
        ] {
            let found = words_with_punctuation(body)
                .into_iter()
                .find(|w| looks_like_source_path(w, &["go"]));
            assert!(found.is_some(), "no path found in {body:?}");
        }
    }

    #[test]
    fn words_lowercases_and_drops_punctuation() {
        assert_eq!(words("Step 1: DO IT"), ["step", "1", "do", "it"]);
    }
}
