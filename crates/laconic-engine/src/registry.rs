//! The rule set as declarations.
//!
//! Enabling, disabling, re-tiering and configuring a rule all act on an entry here, so
//! `ignoreReason` and `deadIgnore` are ordinary entries rather than engine behaviour with no
//! configuration surface.

use crate::domain::CommentKind;

/// Gate decides exit status. There is no separate severity: the reporter emits tier directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Gate,
    Warn,
}

/// Only `Delete` is mechanically applicable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixShape {
    Delete,
    Rewrite,
    None,
}

/// What one comment kind gets from one rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Disposition {
    pub tier: Tier,
    pub fix: FixShape,
}

const GATE_DELETE: Disposition = Disposition {
    tier: Tier::Gate,
    fix: FixShape::Delete,
};
const GATE_NONE: Disposition = Disposition {
    tier: Tier::Gate,
    fix: FixShape::None,
};
const WARN_REWRITE: Disposition = Disposition {
    tier: Tier::Warn,
    fix: FixShape::Rewrite,
};

/// Where a rule is evaluated. Three paths, not one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dispatch {
    /// The twelve rules that take a comment block.
    PerBlock,
    /// `density` alone: an aggregate over the blocks inside one subject's span, so its finding
    /// anchors to the subject rather than to a block.
    PerSubject,
    /// `ignoreReason` and `deadIgnore`. Both take surviving ignore directives as input, and
    /// `deadIgnore` additionally needs post-dispatch results, so neither can run during dispatch.
    AtReconcile,
}

/// What §7 does to a rule when the file contains an ERROR node. The partition covers every rule
/// in the set — a rule missing from it is a rule whose behaviour on broken input nobody decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorPolicy {
    /// The test is over comment text, or over the comment body's own nested parse. Neither depends
    /// on the surrounding tree.
    Unaffected,
    /// Correctness depends on subject resolution and attachment, both unreliable in an ERROR
    /// subtree.
    SuppressedInErrorSubtree,
    /// `implInInterface` alone. Its input is file-scoped — concern 11 enumerates every declaration
    /// in the file — so a clean subtree says nothing about whether that enumeration is complete.
    SuppressedForFile,
    /// `deadIgnore` fires only when its named rule ran and did not fire, never when the rule was
    /// suppressed. Conflating the two reports a live directive as dead.
    Conditional,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleEntry {
    pub id: &'static str,
    /// The (tier, fix) pair each kind gets, or `None` where the rule does not apply.
    ///
    /// Per kind rather than per rule because `narration` is gate plus Delete for Line and Block and
    /// warn plus Rewrite for Doc; a single tier per rule cannot express that.
    pub line: Option<Disposition>,
    pub block: Option<Disposition>,
    pub doc: Option<Disposition>,
    pub autofix: bool,
    pub dispatch: Dispatch,
    pub error_policy: ErrorPolicy,
    pub enabled: bool,
}

impl RuleEntry {
    pub fn disposition(&self, kind: CommentKind) -> Option<Disposition> {
        match kind {
            CommentKind::Line => self.line,
            CommentKind::Block => self.block,
            CommentKind::Doc => self.doc,
        }
    }
}

const fn entry(
    id: &'static str,
    line: Option<Disposition>,
    block: Option<Disposition>,
    doc: Option<Disposition>,
    autofix: bool,
    dispatch: Dispatch,
    error_policy: ErrorPolicy,
) -> RuleEntry {
    RuleEntry {
        id,
        line,
        block,
        doc,
        autofix,
        dispatch,
        error_policy,
        enabled: true,
    }
}

/// The fifteen rules: thirteen from the specification, plus `ignoreReason` and `deadIgnore`, which
/// its own text requires but does not list.
pub fn default_rules() -> Vec<RuleEntry> {
    vec![
        // Gate, Delete, autofix on. The only two that `laconic fix` touches by default.
        entry(
            "narration",
            Some(GATE_DELETE),
            Some(GATE_DELETE),
            Some(WARN_REWRITE),
            true,
            Dispatch::PerBlock,
            ErrorPolicy::Unaffected,
        ),
        entry(
            "banner",
            Some(GATE_DELETE),
            Some(GATE_DELETE),
            Some(WARN_REWRITE),
            true,
            Dispatch::PerBlock,
            ErrorPolicy::Unaffected,
        ),
        // Gate, Delete, autofix off. Each has a stated false-positive reason; the friction is the
        // price of not deleting good comments unattended.
        entry(
            "restate",
            Some(GATE_DELETE),
            Some(GATE_DELETE),
            None,
            false,
            Dispatch::PerBlock,
            ErrorPolicy::SuppressedInErrorSubtree,
        ),
        entry(
            "commentedOutCode",
            Some(GATE_DELETE),
            Some(GATE_DELETE),
            None,
            false,
            Dispatch::PerBlock,
            ErrorPolicy::Unaffected,
        ),
        entry(
            "attribution",
            Some(GATE_DELETE),
            Some(GATE_DELETE),
            None,
            false,
            Dispatch::PerBlock,
            ErrorPolicy::Unaffected,
        ),
        entry(
            "detached",
            Some(GATE_DELETE),
            Some(GATE_DELETE),
            None,
            false,
            Dispatch::PerBlock,
            ErrorPolicy::SuppressedInErrorSubtree,
        ),
        // Gate, no fix. A human decides.
        entry(
            "fileref",
            Some(GATE_NONE),
            Some(GATE_NONE),
            Some(GATE_NONE),
            false,
            Dispatch::PerBlock,
            ErrorPolicy::Unaffected,
        ),
        // Never autofixable at any tier: the only deletion that makes it pass is deleting the
        // directive, which silently re-enables the rule it suppressed.
        entry(
            "ignoreReason",
            Some(GATE_NONE),
            Some(GATE_NONE),
            Some(GATE_NONE),
            false,
            Dispatch::AtReconcile,
            ErrorPolicy::Unaffected,
        ),
        // Warn, Rewrite, never autofixable.
        entry(
            "density",
            Some(WARN_REWRITE),
            Some(WARN_REWRITE),
            None,
            false,
            Dispatch::PerSubject,
            ErrorPolicy::SuppressedInErrorSubtree,
        ),
        entry(
            "docbloat",
            None,
            None,
            Some(WARN_REWRITE),
            false,
            Dispatch::PerBlock,
            ErrorPolicy::SuppressedInErrorSubtree,
        ),
        // The only rule suppressed per file rather than per subtree: decision 23 made its input
        // file-scoped, and a clean subtree says nothing about whether concern 11's enumeration is
        // complete.
        entry(
            "implInInterface",
            None,
            None,
            Some(WARN_REWRITE),
            false,
            Dispatch::PerBlock,
            ErrorPolicy::SuppressedForFile,
        ),
        entry(
            "hedging",
            Some(WARN_REWRITE),
            Some(WARN_REWRITE),
            Some(WARN_REWRITE),
            false,
            Dispatch::PerBlock,
            ErrorPolicy::Unaffected,
        ),
        entry(
            "vague",
            Some(WARN_REWRITE),
            Some(WARN_REWRITE),
            Some(WARN_REWRITE),
            false,
            Dispatch::PerBlock,
            ErrorPolicy::Unaffected,
        ),
        entry(
            "task",
            Some(WARN_REWRITE),
            Some(WARN_REWRITE),
            Some(WARN_REWRITE),
            false,
            Dispatch::PerBlock,
            ErrorPolicy::Unaffected,
        ),
        entry(
            "deadIgnore",
            Some(WARN_REWRITE),
            Some(WARN_REWRITE),
            Some(WARN_REWRITE),
            false,
            Dispatch::AtReconcile,
            ErrorPolicy::Conditional,
        ),
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registry {
    entries: Vec<RuleEntry>,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            entries: default_rules(),
        }
    }
}

impl Registry {
    pub fn get(&self, id: &str) -> Option<&RuleEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn entries(&self) -> &[RuleEntry] {
        &self.entries
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        self.get(id).is_some_and(|e| e.enabled)
    }

    /// An unknown id is a configuration error rather than a no-op: a config the tool half
    /// understands silently disables rules the author believed were on.
    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<(), UnknownRule> {
        match self.entries.iter_mut().find(|e| e.id == id) {
            Some(e) => {
                e.enabled = enabled;
                Ok(())
            }
            None => Err(UnknownRule(id.to_string())),
        }
    }

    pub fn set_autofix(&mut self, id: &str, autofix: bool) -> Result<(), UnknownRule> {
        match self.entries.iter_mut().find(|e| e.id == id) {
            Some(e) => {
                e.autofix = autofix;
                Ok(())
            }
            None => Err(UnknownRule(id.to_string())),
        }
    }

    /// Re-tier every kind the rule applies to, leaving each fix shape alone. A kind the rule does
    /// not apply to stays inapplicable — re-tiering never switches a rule on where it never ran.
    pub fn set_tier(&mut self, id: &str, tier: Tier) -> Result<(), UnknownRule> {
        match self.entries.iter_mut().find(|e| e.id == id) {
            Some(e) => {
                for disposition in [&mut e.line, &mut e.block, &mut e.doc]
                    .into_iter()
                    .flatten()
                {
                    disposition.tier = tier;
                }
                Ok(())
            }
            None => Err(UnknownRule(id.to_string())),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct UnknownRule(pub String);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rule_set_is_fifteen() {
        assert_eq!(default_rules().len(), 15);
    }

    /// The doc-comment deletion invariant. A doc comment is a public surface — godoc, rustdoc,
    /// javadoc, JSDoc — and in Python a runtime value on `__doc__`, so deleting one is an API
    /// change wearing a cleanup's clothes and the agent cannot tell the difference.
    #[test]
    fn no_delete_fix_applies_to_doc_kind() {
        for e in default_rules() {
            assert_ne!(
                e.doc.map(|d| d.fix),
                Some(FixShape::Delete),
                "{} carries a Delete fix on Doc kind",
                e.id
            );
        }
    }

    /// Twelve per block, one per subject, two at reconcile. The design said fourteen per block,
    /// which counted the two reconcile rules as dispatched.
    #[test]
    fn dispatch_paths_partition_the_rule_set() {
        let rules = default_rules();
        let count = |d: Dispatch| rules.iter().filter(|e| e.dispatch == d).count();
        assert_eq!(count(Dispatch::PerBlock), 12);
        assert_eq!(count(Dispatch::PerSubject), 1);
        assert_eq!(count(Dispatch::AtReconcile), 2);
    }

    /// Only `narration` and `banner` autofix by default. The other four Delete rules ship off,
    /// each for a stated false-positive reason.
    #[test]
    fn two_rules_autofix_by_default() {
        let on: Vec<&str> = default_rules()
            .iter()
            .filter(|e| e.autofix)
            .map(|e| e.id)
            .collect();
        assert_eq!(on, vec!["narration", "banner"]);
    }

    /// A rule with a Delete fix but autofix off still gates. Tier and autofix are independent axes
    /// and there are exactly two.
    #[test]
    fn delete_rules_gate_whether_or_not_they_autofix() {
        let rules = default_rules();
        for id in ["restate", "commentedOutCode", "attribution", "detached"] {
            let e = rules.iter().find(|e| e.id == id).unwrap();
            assert_eq!(e.line, Some(GATE_DELETE), "{id}");
            assert!(!e.autofix, "{id} ships autofix off");
        }
    }

    /// §7 asserts the partition covers every rule in the set. A rule missing from it is a rule
    /// whose behaviour on broken input nobody decided.
    #[test]
    fn the_error_policy_partition_covers_every_rule() {
        let rules = default_rules();
        let count = |p: ErrorPolicy| rules.iter().filter(|e| e.error_policy == p).count();
        assert_eq!(count(ErrorPolicy::Unaffected), 9);
        assert_eq!(count(ErrorPolicy::SuppressedInErrorSubtree), 4);
        assert_eq!(count(ErrorPolicy::SuppressedForFile), 1);
        assert_eq!(count(ErrorPolicy::Conditional), 1);

        let only_file_scoped: Vec<&str> = rules
            .iter()
            .filter(|e| e.error_policy == ErrorPolicy::SuppressedForFile)
            .map(|e| e.id)
            .collect();
        assert_eq!(only_file_scoped, vec!["implInInterface"]);
    }

    #[test]
    fn an_unknown_rule_id_is_an_error_not_a_no_op() {
        let mut r = Registry::default();
        assert!(r.set_enabled("narration", false).is_ok());
        assert!(!r.is_enabled("narration"));
        assert_eq!(
            r.set_enabled("nosuchrule", false),
            Err(UnknownRule("nosuchrule".to_string()))
        );
    }
}
