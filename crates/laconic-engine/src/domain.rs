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
    /// Rows the subject's body spans — `docbloat`.
    pub body_rows: usize,
    /// Statements in the subject's body — `density`'s denominator.
    pub statement_count: usize,
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
    /// Index into the file's subjects, when the block is attached to one.
    pub subject: Option<usize>,
    /// The directive protecting this block, lifted out of it during grouping. Lifting rather than
    /// merging matters: otherwise the directive's own text joins the block and changes what
    /// `narration` and `restate` match against, so a suppression would alter the finding it
    /// suppresses.
    pub ignore: Option<IgnoreDirective>,
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
