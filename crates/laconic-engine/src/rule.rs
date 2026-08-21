//! What a rule is, and what it receives.
//!
//! A rule decides **whether**. The registry decides the tier and the fix shape, per comment kind.
//! Keeping those apart is what lets `narration` gate and delete a line comment while rewriting a
//! doc comment at warn tier, without the rule knowing either number.

use crate::domain::{CommentBlock, DeclaredSymbol, Subject};
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
    pub src: &'a str,
}

/// The twelve rules that take a comment block.
pub trait BlockRule: Send + Sync {
    fn id(&self) -> &'static str;
    fn check(&self, ctx: &BlockContext) -> Option<RuleHit>;
}

pub struct SubjectContext<'a> {
    pub subject: &'a Subject,
    /// Every block attached to this subject.
    pub blocks: Vec<&'a CommentBlock>,
}

/// `density` alone: an aggregate over every block attached to one subject, so its finding anchors
/// to the subject rather than to a block.
pub trait SubjectRule: Send + Sync {
    fn id(&self) -> &'static str;
    fn check(&self, ctx: &SubjectContext) -> Option<RuleHit>;
}
