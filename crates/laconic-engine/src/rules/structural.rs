//! The six rules that need more than the comment's text.
//!
//! These are the rules §7 withholds on broken input, because their correctness depends on subject
//! resolution and attachment. `commentedOutCode` is the exception in the group: its test is over
//! the comment body's own nested parse, which does not depend on the surrounding tree at all.

use crate::domain::{Attachment, CommentKind};
use crate::pack::split_identifier;
use crate::rule::{BlockContext, BlockRule, RuleHit, SubjectContext, SubjectRule};
use crate::rules::matching::words;
use crate::rules::text::is_banner;

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

/// `docbloat`'s ratio, and `density`'s below it. Both carried over unchanged when the denominator
/// stopped being rows of text; the corpus run is what calibrates them.
///
/// Coupled to [`ABSOLUTE_DOC_LINES`]: two members already reach the cap, so this decides firing at
/// one member and nowhere else. Raising it to loosen the ratio disables the only band it has.
pub const DOC_LINES_PER_MEMBER: usize = 3;

/// `density` measures non-doc commentary, so it needs both a floor and a ratio: a long run is
/// unremarkable in a long function and damning in a short one.
///
/// The floor is the largest count that stays *silent*; the rule first fires one line above it. Its
/// sibling below is the opposite polarity, firing *above* its value.
pub const DENSITY_MIN_COMMENT_LINES: usize = 8;
pub const DENSITY_MAX_RATIO: f64 = 0.5;

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
    /// A banner is reported by `banner` alone: a label is detached by construction, and there is no
    /// code a `--- helpers ---` divider could be moved onto.
    fn check(&self, ctx: &BlockContext) -> Option<RuleHit> {
        if is_banner(&ctx.block.body(), ctx.block.attachment) {
            return None;
        }
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
        let tree = ctx.grammar.parser().parse(&body, None)?;
        let root = tree.root_node();

        // `has_error` rather than a walk counting ERROR nodes. A body can parse to a clean-looking
        // tree with a MISSING node that no traversal of `children` reaches — a bare identifier does
        // exactly that — and only this reports it.
        if root.has_error() {
            return None;
        }
        if named_nodes(root) < MIN_CODE_NODES {
            return None;
        }
        Some(RuleHit::new(
            ctx.block.span.clone(),
            "delete this commented-out code: version control already has it",
        ))
    }
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
        // Which test fired decides everything after the line count, the naming of the denominator
        // included, because they are different failures. Neither sentence says "move the rest into
        // the body" any more: that instruction taught a repairing agent to relocate prose into the
        // function it documented, where `density` then reported it — one rule instructing what
        // another punishes. Neither invites restructuring the subject either: the denominator is
        // the agent's to change, and inflating it reaches green with the comment untouched.
        Some(RuleHit::new(
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
        ))
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
        let body = words(&ctx.block.body());
        let leaked = ctx.declared.iter().find(|d| {
            !d.visibility.is_exported()
                && d.name.chars().count() >= MIN_SYMBOL_LEN
                && body.contains(&d.name.to_lowercase())
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

    /// Past [`DENSITY_MIN_COMMENT_LINES`] **and** above [`DENSITY_MAX_RATIO`]. Both are the
    /// specification's stated values with no measurement behind them. The ratio is the weaker of the
    /// two by a wide margin: with the floor where it is, suppressing a finding by ratio alone needs
    /// a subject declaring at least eighteen members, so on ordinary code the floor decides.
    fn check(&self, ctx: &SubjectContext) -> Option<RuleHit> {
        // Doc kind is excluded, and the reason is not positional: `docbloat` is the rule that
        // measures a doc comment against its subject, so counting one here would measure the same
        // lines twice under two rules with two different instructions. In Python the docstring sits
        // *inside* the scope it documents, so a filter keyed on position rather than kind would
        // count it there and not elsewhere.
        let comment_lines: usize = ctx
            .blocks
            .iter()
            .filter(|b| b.kind != CommentKind::Doc)
            .map(|b| b.line_count())
            .sum();
        if comment_lines <= ctx.thresholds.density_min_comment_lines {
            return None;
        }
        // A subject declaring no members has no denominator, and this rule is the ratio: unlike
        // `docbloat` it carries no absolute test to fall back on.
        let members = ctx.subject.member_count;
        if members == 0 {
            return None;
        }
        let ratio = comment_lines as f64 / members as f64;
        if ratio <= ctx.thresholds.density_max_ratio {
            return None;
        }
        Some(RuleHit::new(
            ctx.subject.span.clone(),
            format!(
                "reduce the commentary here: {comment_lines} comment lines against {} — keep the ones a reader could not derive and delete the rest",
                members_phrase(members)
            ),
        ))
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
        assert_eq!(DENSITY_MIN_COMMENT_LINES, 8, "silent at 8, fires at 9");
        assert_eq!(DENSITY_MAX_RATIO, 0.5, "carried over, uncalibrated");
    }
}
