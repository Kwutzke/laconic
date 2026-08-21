//! The six rules that need more than the comment's text.
//!
//! These are the rules §7 withholds on broken input, because their correctness depends on subject
//! resolution and attachment. `commentedOutCode` is the exception in the group: its test is over
//! the comment body's own nested parse, which does not depend on the surrounding tree at all.

use crate::domain::{Attachment, CommentKind};
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

/// Rows a body needs before `docbloat` measures a doc comment against it.
const MIN_BODY_ROWS: usize = 2;

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

    /// Purely structural, with no text test at all — which makes it the most destructive rule in
    /// the set to run unattended, and is why it ships autofix off. *Detached* includes "nothing
    /// follows", so `// intentionally empty` at the end of a block is structurally identical to an
    /// orphan.
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

    /// The relative test needs a body with room in it. Below two rows it is not a ratio: a
    /// one-row body puts the threshold at three lines, which is shorter than an ordinary doc
    /// comment in every language, so a Go `package_clause`, a Rust tuple variant and a newtype
    /// struct all reported "documenting 1 lines of code". Those subjects are measured by the
    /// absolute threshold alone.
    fn check(&self, ctx: &BlockContext) -> Option<RuleHit> {
        let subject = ctx.subject?;
        let lines = ctx.block.line_count();
        let measurable = subject.body_rows.filter(|rows| *rows >= MIN_BODY_ROWS);
        let over_absolute = lines > 15;
        let over_relative = measurable.is_some_and(|rows| lines > rows.saturating_mul(3));
        if !over_absolute && !over_relative {
            return None;
        }
        Some(RuleHit::new(
            ctx.block.span.clone(),
            match measurable {
                Some(rows) => format!(
                    "shorten this doc comment: {lines} lines documenting {rows} lines of code — keep what a caller needs and move the rest into the body"
                ),
                // Reached only by the absolute threshold, and the subject may be a file root, so
                // the sentence names neither a declaration nor a body to move text into.
                None => format!(
                    "shorten this doc comment: {lines} lines — keep what a caller needs in the summary and move the detail below it"
                ),
            },
        ))
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

    /// More than 8 comment lines **and** a comment-to-statement ratio above 0.5. Both thresholds
    /// are the specification's stated values with no measurement behind them.
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
        if comment_lines <= 8 {
            return None;
        }
        let statements = ctx.subject.statement_count;
        if statements == 0 {
            return None;
        }
        let ratio = comment_lines as f64 / statements as f64;
        if ratio <= 0.5 {
            return None;
        }
        Some(RuleHit::new(
            ctx.subject.span.clone(),
            format!(
                "reduce the commentary here: {comment_lines} comment lines against {statements} statements — keep the ones a reader could not derive and delete the rest"
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
