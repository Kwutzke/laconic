//! What a rule is, and what it receives.
//!
//! A rule decides **whether**. The registry decides the tier and the fix shape, per comment kind.
//! Keeping those apart is what lets `narration` gate and delete a line comment while rewriting a
//! doc comment at warn tier, without the rule knowing either number.

use crate::config::Thresholds;
use crate::domain::{CommentBlock, DeclaredSymbol, Subject};
use laconic_grammars::Grammar;
use std::ops::Range;

/// A rule fired. The instruction states the change to make, never the defect: the consumer is a
/// machine acting on one diagnostic with no surrounding context, and it will read
/// `narration detected` and write a comment explaining that narration was detected.
#[derive(Debug, Clone)]
pub struct RuleHit {
    pub span: Range<usize>,
    pub instruction: String,
}

impl RuleHit {
    pub fn new(span: Range<usize>, instruction: impl Into<String>) -> Self {
        Self {
            span,
            instruction: instruction.into(),
        }
    }
}

pub struct BlockContext<'a> {
    pub block: &'a CommentBlock,
    pub subject: Option<&'a Subject>,
    /// Every declaration in the file — pack concern 11, for `implInInterface`.
    pub declared: &'a [DeclaredSymbol],
    /// The extensions the pack claims — concern 1, for `fileref`. A list, not the pack: a rule that
    /// held a pack would be a rule with a per-language branch in it.
    pub source_extensions: &'a [&'static str],
    /// The grammar this file was parsed with, so `commentedOutCode` can parse a comment body in the
    /// file's own language. Opaque to the rule: it parses, it does not branch on which language.
    pub grammar: Grammar,
    /// The numeric thresholds for **this file's language**, already narrowed by any override.
    ///
    /// Per file rather than per run, and that is the language-override mechanism rather than a
    /// convenience: a threshold retuned for one pack is not a property of the run.
    pub thresholds: Thresholds,
    pub src: &'a str,
}

/// The twelve rules that take a comment block.
pub trait BlockRule: Send + Sync {
    fn id(&self) -> &'static str;
    fn check(&self, ctx: &BlockContext) -> Option<RuleHit>;
}

pub struct SubjectContext<'a> {
    pub subject: &'a Subject,
    /// Every block **inside** this subject's span — not the blocks attached to it.
    ///
    /// The two differ and the difference is the point: a comment inside a function body attaches to
    /// the statement below it, so grouping by attachment would give every statement its own
    /// denominator and no function any comments at all. The subject's own doc comment is excluded
    /// by `density` itself, since `docbloat` is the rule that measures that one.
    pub blocks: Vec<&'a CommentBlock>,
    /// The thresholds for this file's language — see [`BlockContext::thresholds`].
    pub thresholds: Thresholds,
}

/// `density` alone: an aggregate over every block attached to one subject, so its finding anchors
/// to the subject rather than to a block.
pub trait SubjectRule: Send + Sync {
    fn id(&self) -> &'static str;
    fn check(&self, ctx: &SubjectContext) -> Option<RuleHit>;
}
