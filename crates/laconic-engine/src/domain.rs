//! What a rule receives. Nothing here knows any language.

use std::ops::Range;

/// Line, Block or Doc. The pack assigns it, and every rule declares a disposition per kind — this
/// is the whole mechanism by which language asymmetries stay out of the rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommentKind {
    Line,
    Block,
    Doc,
}

/// What a block relates to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attachment {
    /// The next non-comment node follows with no blank line between.
    AttachedBelow,
    /// Code precedes the comment on its own line.
    AttachedTrailing,
    /// A blank line intervenes, or nothing follows.
    Detached,
}

/// One comment node, after extraction.
#[derive(Debug, Clone)]
pub struct Comment {
    pub span: Range<usize>,
    /// 0-based row of the comment's first line.
    pub start_row: usize,
    /// 0-based row of the comment's last line; a block comment can span rows.
    pub end_row: usize,
    /// The body with markers stripped — pack concern 7.
    pub body: String,
    /// Whether code precedes this comment on its first line.
    pub trailing: bool,
}

/// How visible a subject is. Not a boolean: `pub(crate)`, a TypeScript class member and a
/// package-level Go identifier are different questions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Visibility {
    /// Reachable from outside the file by any importer.
    Exported,
    /// Reachable outside the file but only within the same crate, package or module tree.
    Restricted(String),
    /// Not reachable outside the file.
    Private,
}

impl Visibility {
    pub fn is_exported(&self) -> bool {
        matches!(self, Visibility::Exported)
    }
}

/// A declaration somewhere in the file — pack concern 11. `implInInterface` resolves names in a
/// doc comment against this, which concern 8 cannot do: concern 8 describes one subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredSymbol {
    pub name: String,
    /// The declaration form, in the language's own vocabulary — `func`, `type`, `class`, `def`.
    pub form: &'static str,
    pub visibility: Visibility,
}

/// What a block is attached to, described by the pack rather than handed over as a raw syntax
/// node. Handing a rule a raw node puts a per-language match arm inside three rules and ends the
/// pack abstraction.
#[derive(Debug, Clone)]
pub struct Subject {
    pub span: Range<usize>,
    /// The identifiers this subject binds, already split on camelCase and snake_case — `restate`.
    pub bound_identifiers: Vec<String>,
    /// The members this subject declares — pack concern 9, and the denominator `docbloat` measures
    /// a doc comment against. Zero means the subject declares none, so the ratio does not apply and
    /// the absolute threshold stands alone.
    pub member_count: usize,
    /// Lines in the span carrying a byte that is neither whitespace nor inside a comment —
    /// `density`'s denominator.
    ///
    /// Not `member_count`, which a pack derives from the parse tree and which collapses wherever a
    /// language nests code inside an expression: a Go function whose body is one
    /// `f(func(){ …50 lines… })` declares a single statement however long the closure is, so the
    /// ratio measured a 935-line subject as though it were a one-liner. Counted in the engine so a
    /// pack cannot get it wrong.
    pub code_lines: usize,
    /// Whether this subject is the file itself rather than a declaration inside it.
    ///
    /// The Rust and Python packs make the parse root a subject so that a module docstring and an
    /// inner `//!` doc have something to document; Go, Java and TypeScript declare no root. That
    /// makes the file a denominator for `density`, whose budget is calibrated for a declaration —
    /// and a file's commentary grows with its length while the budget grows with its square root,
    /// so every long file exceeded it and the finding spanned the whole file, naming no repair.
    ///
    /// Decided in the engine from the parse root, like [`Self::code_lines`], so a pack cannot get
    /// it wrong.
    pub file_scope: bool,
    pub visibility: Visibility,
}

/// laconic's own suppression, `laconic:ignore <rule> — <reason>`.
///
/// Not a machine directive and never stripped: `ignoreReason` fires on one lacking a reason and
/// `deadIgnore` fires on one whose named rule ran and did not fire, so both take it as input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IgnoreDirective {
    pub rule: String,
    /// `None` is itself a finding — `ignoreReason` at gate tier.
    pub reason: Option<String>,
    pub span: Range<usize>,
    pub start_row: usize,
}

/// The unit a rule receives. Consecutive comment nodes separated by whitespace only, each alone on
/// its line, form one block; a comment with code before it on the same line is always its own
/// block and never merges.
#[derive(Debug, Clone)]
pub struct CommentBlock {
    pub comments: Vec<Comment>,
    pub kind: CommentKind,
    pub attachment: Attachment,
    pub span: Range<usize>,
    /// Index into the file's subjects, when the block documents a declaration.
    ///
    /// A block attached to an ordinary statement has none: a statement is not a subject. What
    /// `restate` needs from that statement is [`CommentBlock::attached_identifiers`].
    pub subject: Option<usize>,
    /// The identifiers bound by whatever this block attaches to — `restate`, which fires when the
    /// block's content tokens are a subset of these.
    pub attached_identifiers: Vec<String>,
    /// The directive protecting this block, lifted out of it during grouping. Lifting rather than
    /// merging matters: otherwise the directive's own text joins the block and changes what
    /// `narration` and `restate` match against, so a suppression would alter the finding it
    /// suppresses.
    pub ignore: Option<IgnoreDirective>,
    /// The block sits inside a subtree containing an ERROR node, so subject resolution and
    /// attachment are unreliable for it and the rules that depend on them are withheld.
    pub in_error_subtree: bool,
}

impl CommentBlock {
    /// Rows the block covers, counting a multi-row block comment once per row.
    pub fn line_count(&self) -> usize {
        self.comments
            .iter()
            .map(|c| c.end_row - c.start_row + 1)
            .sum()
    }

    /// Every comment body joined by newline — what the deny-list rules match against.
    pub fn body(&self) -> String {
        self.comments
            .iter()
            .map(|c| c.body.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}
