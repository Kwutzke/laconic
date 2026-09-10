//! The six rules that need more than the comment's text.
//!
//! These are the rules §7 withholds on broken input, because their correctness depends on subject
//! resolution and attachment. `commentedOutCode` is the exception in the group: its test is over
//! the comment body's own nested parse, which does not depend on the surrounding tree at all.

use crate::domain::Attachment;
use crate::pack::split_identifier;
use crate::rule::{BlockContext, BlockRule, RuleHit, SubjectContext, SubjectRule};
use crate::rules::matching::words;

/// Words carrying no content for `restate`'s subset test. Without removal, every comment contains
/// `the` and no comment is ever a subset of anything.
const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "for", "from", "if", "in", "is", "it", "its",
    "of", "on", "or", "that", "the", "then", "this", "to", "was", "we", "will", "with",
];

/// Named nodes a comment body must parse to before `commentedOutCode` will call it code.
///
/// A starting value, not a measurement: AC7's corpus run is what calibrates it. Three excludes a
/// bare identifier — which parses cleanly as an expression statement in several grammars — while
/// admitting `return nil`, the shortest real statement in the fixtures.
const MIN_CODE_NODES: usize = 3;

/// Length below which a declared name is too short to resolve against prose — `implInInterface`.
/// Also a starting value: a private symbol named `x` would otherwise match any comment mentioning
/// a variable called x.
const MIN_SYMBOL_LEN: usize = 3;

/// `docbloat`'s cap, applied whatever the subject declares. Six is the owner's ruling against the
/// corpus; the specified 15 had no measurement behind it.
///
/// A comment that genuinely earns more carries `laconic:ignore docbloat — <reason>`. Spelled out
/// rather than referenced, because the reader who needs the escape has no tracker access.
pub const ABSOLUTE_DOC_LINES: usize = 6;

/// `docbloat`'s ratio. Carried over unchanged when the denominator stopped being rows of text; the
/// corpus run is what calibrates it.
///
/// Coupled to [`ABSOLUTE_DOC_LINES`]: two members already reach the cap, so this decides firing at
/// one member and nowhere else. Raising it to loosen the ratio disables the only band it has.
pub const DOC_LINES_PER_MEMBER: usize = 3;

/// Allowed comment lines are this many times the **square root** of a subject's code lines.
///
/// A constant ratio grants a long subject a proportional budget, and nothing needs one: at 0.2 a
/// 250-line function was allowed 50 comment lines, which is how a concentrated six-line block hid
/// inside thirty-four lines of switch and logger setup and the rule reported nothing. The square
/// root grants that function 15 and a four-line one 2 — looser than the old ratio below 25 code
/// lines, stricter above, crossing over exactly there.
///
/// The two directions are the two failures the ratio produced, in one curve. Its false positives
/// were single `why` comments in short subjects, which are now inside the budget; its false
/// negatives were dense blocks diluted by length, which are now outside it.
///
/// At 1.0 there is no free parameter: the rule is `comment_lines² > code_lines`. Measured over
/// `business-platform-backend/internal`, 370 of 2979 commented subjects fire against 305 under the
/// old ratio, 218 of them shared — a different selection rather than a larger one.
///
/// **The curve buys recall, not precision.** A hand-judged fifteen ran 8/15 against 7/14 for the
/// ratio it replaced, and the misses changed shape rather than thinning: the ratio's were single
/// `why` comments in short subjects, and these are long functions carrying dense measured
/// rationale — WAL semantics, search scoring, cross-system field names — which is the population a
/// curve stricter on length was always going to surface. 1.0 itself is fitted to two cases: 1.2
/// misses both, so the value is sensitive and remains uncalibrated.
pub const DENSITY_ALLOWANCE: f64 = 1.0;

/// The hint `density` and `docbloat` add once a subject is well past the threshold that bound it.
///
/// Ordered after the instruction for a reason: the comment is what gets repaired, and only what
/// survives that repair is evidence about the code. It is a note rather than part of the
/// instruction because the consumer executes instructions — told to restructure, an agent reaches
/// green fastest by adding code, which grows the denominator and leaves every comment in place.
const RESTRUCTURE_NOTE: &str = "most code needs no comment; one that earns its place says why, not \
what. If it still seems necessary here, that is usually naming or structure asking to be fixed — \
repair the comment first, and report the restructuring rather than doing it to satisfy this \
finding.";

/// How far past its threshold a subject must be before [`RESTRUCTURE_NOTE`] is attached.
///
/// A subject a line over is evidence of nothing, and a hint on every finding is a hint nobody
/// reads. Doubling is the coarsest band separating "slightly over" from "the commentary is the
/// thing being read", and it is a starting value with no measurement behind it.
///
/// Both rules multiply in `f64` against it. Stated once because a second integer copy drifted from
/// this one silently, with nothing tying the two and nothing covering `docbloat`'s gate — see
/// `the_docbloat_note_follows_the_binding_threshold`, which covers it now.
const NOTE_MULTIPLE: f64 = 2.0;

pub struct Restate;

impl BlockRule for Restate {
    fn id(&self) -> &'static str {
        "restate"
    }

    /// Fires when the comment's content tokens are a subset of the identifiers bound by the code
    /// it is attached to — below it, or beside it for a trailing comment. A comment naming the
    /// same identifiers while saying *why* is a subset by this test and is the most valuable
    /// comment in the file, which is why this rule ships autofix off.
    fn check(&self, ctx: &BlockContext) -> Option<RuleHit> {
        // Below the code or beside it. A detached block attaches to nothing, so there is nothing
        // for its tokens to be a subset of.
        if ctx.block.attachment == Attachment::Detached {
            return None;
        }
        let bound = &ctx.block.attached_identifiers;
        if bound.is_empty() {
            return None;
        }
        // Comment words are camel- and snake-split too, because the identifiers they are compared
        // against already are: without it `// userName is the login` yields the single token
        // `username`, which is in no split identifier set and the subset test never fires.
        let tokens: Vec<String> = words(&ctx.block.body())
            .into_iter()
            .flat_map(|w| split_identifier(&w))
            .filter(|w| !STOPWORDS.contains(&w.as_str()))
            .collect();
        if tokens.is_empty() {
            return None;
        }
        if !tokens.iter().all(|t| bound.contains(t)) {
            return None;
        }
        Some(RuleHit::new(
            ctx.block.span.clone(),
            "remove this comment: it names only what the code it is attached to already says",
        ))
    }
}

pub struct Detached;

impl BlockRule for Detached {
    fn id(&self) -> &'static str {
        "detached"
    }

    /// Purely structural, with no text test at all, which is why it ships autofix off. *Detached*
    /// includes "nothing follows", so `// intentionally empty` closing a block is an orphan here.
    ///
    /// This rule yields to `banner` and to `commentedOutCode`, and says so in the registry's
    /// precedence table rather than here. It used to ask `banner`'s own predicate whether it would
    /// fire, which is a rule holding another rule's test — two copies to keep in step, and the
    /// precedence graph readable only from the rule that loses.
    fn check(&self, ctx: &BlockContext) -> Option<RuleHit> {
        (ctx.block.attachment == Attachment::Detached).then(|| {
            RuleHit::new(
                ctx.block.span.clone(),
                "move this comment onto the line above the code it describes, or delete it",
            )
        })
    }
}

pub struct CommentedOutCode;

impl BlockRule for CommentedOutCode {
    fn id(&self) -> &'static str {
        "commentedOutCode"
    }

    /// Parses the body in the file's own language. "Parses" is weaker than "is code" by a
    /// different amount per grammar, which is what [`MIN_CODE_NODES`] exists to bound.
    fn check(&self, ctx: &BlockContext) -> Option<RuleHit> {
        let body = ctx.block.body();
        if body.trim().is_empty() {
            return None;
        }
        // Code names something. A body of digits and operators names nothing, and the grammars are
        // looser here than the languages are: Go parses `1.2 + 0.7 = 1.9` as an `assignment_statement`
        // without asking whether the left side can be assigned to, so arithmetic worked out in a
        // comment came back as clean code with seven named nodes.
        if !body.chars().any(char::is_alphabetic) {
            return None;
        }
        // Two attempts: the body alone, then the body wrapped as a statement fragment.
        //
        // The bare parse is what catches a commented-out declaration, and it is tried first so a
        // body that is already a compilation unit is judged as one. It cannot catch a fragment of
        // statements: `if err != nil { … }` is not a Go file, not a Rust item and not a Java class,
        // so the commonest leftover of all parsed to ERROR and this rule never saw it.
        //
        // The scaffold is the pack's, not this rule's — concern 12. Where a pack states none,
        // statements already parse at the top level and the first attempt was the whole test.
        let counted = parses_as_code(ctx.grammar, &body).or_else(|| {
            // The scaffold path is gated on punctuation, and the gate is not optional. Inside a
            // function body almost any bare word is a valid expression statement, so wrapping makes
            // the grammar *more* permissive than the bare parse rather than less: `// HELPERS`,
            // `// counter` and a two-line `// alpha / // bravo` all came back as clean code, and
            // `MIN_CODE_NODES` does not bound it because two words are already four nodes.
            //
            // Real statements carry an operator or a bracket; prose does not. This is the whole
            // difference between the fragment the rule is here for and a section label.
            has_code_punctuation(&body)
                .then(|| scaffolded(ctx.grammar, &body, ctx.statement_scaffold?))
                .flatten()
        })?;
        if counted < MIN_CODE_NODES {
            return None;
        }
        Some(RuleHit::new(
            ctx.block.span.clone(),
            "delete this commented-out code: version control already has it",
        ))
    }
}

/// Whether a body carries punctuation that statements have and sentences do not.
///
/// Deliberately not `.`, `,`, `:` or `-`: prose is full of them. Brackets and `=` are what a
/// statement fragment cannot avoid and a section label never has.
fn has_code_punctuation(body: &str) -> bool {
    body.contains(['=', '(', ')', '{', '}', '[', ']', ';'])
}

/// The named-node count of `text`, or `None` where it does not parse cleanly.
///
/// `has_error` rather than a walk counting ERROR nodes. A body can parse to a clean-looking tree
/// with a MISSING node that no traversal of `children` reaches — a bare identifier does exactly
/// that — and only this reports it.
fn parses_as_code(grammar: laconic_grammars::Grammar, text: &str) -> Option<usize> {
    let tree = grammar.parser().parse(text, None)?;
    let root = tree.root_node();
    (!root.has_error()).then(|| named_nodes(root))
}

/// The same count for a body wrapped in its pack's scaffold, minus what the scaffold itself
/// contributes.
///
/// Subtracting the wrapper is what keeps [`MIN_CODE_NODES`] meaning the same thing in both
/// attempts. Go's scaffold alone brings a package clause and a function declaration, which would
/// otherwise carry a one-token body over the threshold on their own.
fn scaffolded(
    grammar: laconic_grammars::Grammar,
    body: &str,
    scaffold: (&'static str, &'static str),
) -> Option<usize> {
    let (prefix, suffix) = scaffold;
    let empty = parses_as_code(grammar, &format!("{prefix}{suffix}"))?;
    let full = parses_as_code(grammar, &format!("{prefix}{body}{suffix}"))?;
    Some(full.saturating_sub(empty))
}

fn named_nodes(root: tree_sitter::Node) -> usize {
    let mut cursor = root.walk();
    let mut stack = vec![root];
    let mut count = 0;
    while let Some(n) = stack.pop() {
        if n.is_named() && n.id() != root.id() {
            count += 1;
        }
        for c in n.children(&mut cursor) {
            stack.push(c);
        }
    }
    count
}

pub struct DocBloat;

impl BlockRule for DocBloat {
    fn id(&self) -> &'static str {
        "docbloat"
    }

    /// Two tests that catch different failures. **The cap is unconditional** — it applies whether
    /// or not the subject declares members, and it is the only test a subject declaring none ever
    /// meets. The ratio reaches below the cap at exactly one member, which is the four-line comment
    /// on a one-member interface no defensible cap would catch; above one member the cap has
    /// already fired. Both values live on the constants, not in this sentence.
    fn check(&self, ctx: &BlockContext) -> Option<RuleHit> {
        let subject = ctx.subject?;
        let lines = ctx.block.line_count();
        let members = subject.member_count;
        let over_absolute = lines > ctx.thresholds.absolute_doc_lines;
        // `members > 0` is load-bearing beyond the arithmetic: it is what keeps `members_phrase(0)`
        // — "documenting 0 members" — off the relative arm below.
        let over_relative =
            members > 0 && lines > members.saturating_mul(ctx.thresholds.doc_lines_per_member);
        if !over_absolute && !over_relative {
            return None;
        }
        // Which test fired decides the sentence, the naming of the denominator included, because
        // they are different failures. It does not decide the note below, which measures against
        // the threshold that bound rather than the arm that named it.
        //
        // Neither sentence says "move the rest into
        // the body" any more: that instruction taught a repairing agent to relocate prose into the
        // function it documented, where `density` then reported it — one rule instructing what
        // another punishes. Restructuring reaches the reader through [`RESTRUCTURE_NOTE`] instead,
        // which is a note rather than an instruction for exactly the reason it was once left out
        // altogether: the denominator is the agent's to change, and inflating it reaches green with
        // the comment untouched.
        let hit = RuleHit::new(
            ctx.block.span.clone(),
            if over_relative {
                format!(
                    "shorten this doc comment: {lines} lines documenting {} — keep only what a caller cannot derive from the code",
                    members_phrase(members)
                )
            } else {
                format!(
                    "shorten this doc comment: {lines} lines — keep only what a reader cannot derive; the rest belongs in a document if it belongs anywhere"
                )
            },
        );
        // The smallest threshold the comment broke, not the arm that named it. Selecting by arm
        // measured "well past" against a budget the comment never had to satisfy: above two members
        // the relative budget passes the cap, so thirteen lines on four members sat inside twice
        // twelve and carried no note, while the same thirteen on five members fell to the cap arm
        // and carried one. That made the hint non-monotonic in members — adding one to an unchanged
        // comment could attach it — and withheld it from the longest comments, which are the ones
        // it exists for.
        let cap = ctx.thresholds.absolute_doc_lines;
        let per_member = members.saturating_mul(ctx.thresholds.doc_lines_per_member);
        let budget = match (over_absolute, over_relative) {
            (true, true) => cap.min(per_member),
            (true, false) => cap,
            (false, _) => per_member,
        };
        #[expect(
            clippy::cast_precision_loss,
            reason = "line counts are far inside f64's exact integer range"
        )]
        let well_past = lines as f64 > budget as f64 * NOTE_MULTIPLE;
        Some(if well_past {
            hit.with_note(RESTRUCTURE_NOTE)
        } else {
            hit
        })
    }
}

/// The consumer is an agent acting on the sentence, and "documenting 1 members" reads as a
/// generated string rather than a claim about its code.
fn members_phrase(members: usize) -> String {
    match members {
        1 => "1 member".to_string(),
        n => format!("{n} members"),
    }
}

/// Same reason as [`members_phrase`].
fn code_lines_phrase(lines: usize) -> String {
    match lines {
        1 => "1 line of code".to_string(),
        n => format!("{n} lines of code"),
    }
}

/// Same reason as [`members_phrase`]. Reached only under a configured `density_allowance` below
/// 1.0: at the shipped 1.0 the budget is at least `sqrt(1)`, so one comment line never exceeds it.
fn comment_lines_phrase(lines: usize) -> String {
    match lines {
        1 => "1 comment line".to_string(),
        n => format!("{n} comment lines"),
    }
}

pub struct ImplInInterface;

impl BlockRule for ImplInInterface {
    fn id(&self) -> &'static str {
        "implInInterface"
    }

    /// Decision 23's narrow definition: a public subject's doc comment naming an identifier that is
    /// **declared in this file and not exported**. A reference to another exported symbol is an
    /// ordinary cross-reference and is not a finding.
    fn check(&self, ctx: &BlockContext) -> Option<RuleHit> {
        let subject = ctx.subject?;
        if !subject.visibility.is_exported() {
            return None;
        }
        let text = ctx.block.body();
        let body = crate::rules::matching::cased_words(&text);
        let leaked = ctx.declared.iter().find(|d| {
            if d.visibility.is_exported() || d.name.chars().count() < MIN_SYMBOL_LEN {
                return false;
            }
            // Marked as code by the author — `normalize()`, a code span, or a selector — is a
            // reference whatever the name is, so this arm runs before the word test.
            if crate::rules::matching::referenced_as_code(&text, &d.name) {
                return true;
            }
            // A bare occurrence counts only where the name is not also an ordinary English word.
            // `buildSearchInfos` in plain prose is unambiguous; "each lock acquisition" is a
            // sentence that happens to contain an identifier's letters.
            body.contains(&d.name.to_string()) && !crate::rules::matching::is_common_word(&d.name)
        })?;
        Some(RuleHit::new(
            ctx.block.span.clone(),
            format!(
                "rewrite this doc comment without `{}`: it is not exported, so a caller reading this cannot use it",
                leaked.name
            ),
        ))
    }
}

pub struct Density;

impl SubjectRule for Density {
    fn id(&self) -> &'static str {
        "density"
    }

    /// Past [`DENSITY_ALLOWANCE`] times the square root of a **declaration's** code lines, and
    /// nothing else. A floor exempts by absolute count, which is exactly the short heavily-commented
    /// subject the budget exists to catch.
    ///
    /// A file is not a declaration and is not measured. Its commentary is the sum of what its
    /// declarations carry plus whatever sits between them, so it grows with the file while the
    /// budget grows with the square root, and every long file fails a test it cannot pass. The
    /// finding also spanned the whole file, which names nothing to repair.
    fn check(&self, ctx: &SubjectContext) -> Option<RuleHit> {
        if ctx.subject.file_scope {
            return None;
        }
        // Commentary that documents no declaration. A block resolving to a subject is that
        // subject's documentation and belongs to `docbloat`, which measures it against what it
        // documents — counting it here would measure the same lines twice under two rules with two
        // different instructions. This subsumes the Doc-kind test it replaces, since a doc comment
        // always resolves to a subject, and it additionally excludes a member's documentation that
        // the language writes as a trailing comment.
        let comment_lines: usize = ctx
            .blocks
            .iter()
            .filter(|b| b.subject.is_none())
            .map(|b| b.line_count())
            .sum();
        // A subject holding no code has no denominator, and this rule is the ratio: unlike
        // `docbloat` it carries no absolute test to fall back on.
        let code_lines = ctx.subject.code_lines;
        if code_lines == 0 {
            return None;
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "line counts are far inside f64's exact integer range"
        )]
        let allowed = ctx.thresholds.density_allowance * (code_lines as f64).sqrt();
        #[expect(
            clippy::cast_precision_loss,
            reason = "line counts are far inside f64's exact integer range"
        )]
        let lines = comment_lines as f64;
        if lines <= allowed {
            return None;
        }
        let hit = RuleHit::new(
            ctx.subject.span.clone(),
            format!(
                "reduce the commentary here: {} against {} — keep the ones a reader could not derive and delete the rest",
                comment_lines_phrase(comment_lines),
                code_lines_phrase(code_lines)
            ),
        );
        Some(if lines > allowed * NOTE_MULTIPLE {
            hit.with_note(RESTRUCTURE_NOTE)
        } else {
            hit
        })
    }
}

/// The five block rules that need subject facts or a nested parse.
pub fn structural_block_rules() -> Vec<Box<dyn BlockRule>> {
    vec![
        Box::new(Restate),
        Box::new(Detached),
        Box::new(CommentedOutCode),
        Box::new(DocBloat),
        Box::new(ImplInInterface),
    ]
}

/// `density` alone.
pub fn structural_subject_rules() -> Vec<Box<dyn SubjectRule>> {
    vec![Box::new(Density)]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped values, in the one place that states them.
    ///
    /// Sources built *from* a constant pin the behaviour and move with any change, which leaves the
    /// value itself unpinned — the cap could be set back to what it was measured away from with the
    /// suite green. So the digits are asserted once, here, and changing one is a decision rather
    /// than an edit. Restating them elsewhere is what went stale four times when the cap moved.
    #[test]
    fn the_shipped_thresholds_are_the_calibrated_defaults() {
        assert_eq!(ABSOLUTE_DOC_LINES, 6, "owner's ruling against the corpus");
        assert_eq!(DOC_LINES_PER_MEMBER, 3, "carried over, uncalibrated");
        assert_eq!(
            DENSITY_ALLOWANCE, 1.0,
            "fitted to two cases, not calibrated"
        );
    }
}
