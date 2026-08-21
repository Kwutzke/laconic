//! Dispatch and reconcile.
//!
//! Dispatch is per block for twelve rules and per subject for `density`. **Reconcile** is where
//! the other two live: findings covered by an ignore directive are marked suppressed rather than
//! discarded, and only then can `deadIgnore` ask whether a named rule ran and did not fire, and
//! `ignoreReason` report a directive carrying no reason.

use crate::pipeline::FileAnalysis;
use crate::registry::{Disposition, ErrorPolicy, FixShape, Registry, Tier};
use crate::report::{Finding, WithheldNote, line_column};
use crate::rule::{BlockContext, BlockRule, SubjectContext, SubjectRule};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Default)]
pub struct Rules {
    pub block: Vec<Box<dyn BlockRule>>,
    pub subject: Vec<Box<dyn SubjectRule>>,
}

/// A finding still carrying the block it came from, so reconcile can match it against that block's
/// ignore directive.
struct Pending {
    block: Option<usize>,
    finding: Finding,
}

pub fn dispatch(
    file: &Path,
    src: &str,
    analysis: &FileAnalysis,
    registry: &Registry,
    rules: &Rules,
) -> (Vec<Finding>, Option<WithheldNote>) {
    let mut withheld: BTreeSet<&'static str> = BTreeSet::new();
    let mut pending: Vec<Pending> = Vec::new();
    // (block index, rule id) pairs that were evaluated. `deadIgnore` needs "ran and did not fire",
    // which is not the same as "did not fire".
    let mut ran: BTreeSet<(usize, &'static str)> = BTreeSet::new();
    let mut fired: BTreeSet<(usize, &'static str)> = BTreeSet::new();

    for (bi, block) in analysis.blocks.iter().enumerate() {
        for rule in &rules.block {
            let Some(entry) = registry.get(rule.id()).filter(|e| e.enabled) else {
                continue;
            };
            let Some(disposition) = entry.disposition(block.kind) else {
                continue;
            };
            if withhold(
                entry.error_policy,
                analysis.has_error_nodes,
                block.in_error_subtree,
            ) {
                withheld.insert(entry.id);
                continue;
            }
            ran.insert((bi, entry.id));

            let subject = block.subject.map(|i| &analysis.subjects[i]);
            let ctx = BlockContext {
                block,
                subject,
                declared: &analysis.declared,
                src,
            };
            if let Some(hit) = rule.check(&ctx) {
                fired.insert((bi, entry.id));
                pending.push(Pending {
                    block: Some(bi),
                    finding: build(file, src, entry.id, disposition, hit),
                });
            }
        }
    }

    // `density` is the only per-subject dispatch in the set: an aggregate over every block attached
    // to one subject, anchored to the subject rather than to a block.
    for rule in &rules.subject {
        let Some(entry) = registry.get(rule.id()).filter(|e| e.enabled) else {
            continue;
        };
        for subject in analysis.subjects.iter() {
            // Blocks **inside** the subject's span, not blocks attached to it. `density` is per
            // function, and a comment inside a function body attaches to the statement below it —
            // so grouping by attachment would give every statement its own denominator and no
            // function any comments at all. The subject's own doc comment sits above its span and
            // is correctly excluded: `docbloat` measures that one.
            let blocks: Vec<_> = analysis
                .blocks
                .iter()
                .filter(|b| b.span.start >= subject.span.start && b.span.end <= subject.span.end)
                .collect();
            if blocks.is_empty() {
                continue;
            }
            let in_error = blocks.iter().any(|b| b.in_error_subtree);
            if withhold(entry.error_policy, analysis.has_error_nodes, in_error) {
                withheld.insert(entry.id);
                continue;
            }
            // Its disposition is read from the kind of the blocks it aggregates; a subject has no
            // kind of its own.
            let Some(disposition) = blocks.iter().find_map(|b| entry.disposition(b.kind)) else {
                continue;
            };
            let ctx = SubjectContext { subject, blocks };
            if let Some(hit) = rule.check(&ctx) {
                pending.push(Pending {
                    block: None,
                    finding: build(file, src, entry.id, disposition, hit),
                });
            }
        }
    }

    reconcile(file, src, analysis, registry, &mut pending, &ran, &fired);

    let findings = pending.into_iter().map(|p| p.finding).collect();
    let note = (!withheld.is_empty()).then(|| WithheldNote {
        file: file.to_path_buf(),
        rules: withheld.into_iter().collect(),
    });
    (findings, note)
}

/// A rule is withheld when §7 says its inputs are unreliable here.
fn withhold(policy: ErrorPolicy, file_has_errors: bool, block_in_error_subtree: bool) -> bool {
    match policy {
        ErrorPolicy::Unaffected | ErrorPolicy::Conditional => false,
        ErrorPolicy::SuppressedInErrorSubtree => block_in_error_subtree,
        ErrorPolicy::SuppressedForFile => file_has_errors,
    }
}

/// Mark what an ignore directive covers, then run the two rules that take directives as input.
fn reconcile(
    file: &Path,
    src: &str,
    analysis: &FileAnalysis,
    registry: &Registry,
    pending: &mut Vec<Pending>,
    ran: &BTreeSet<(usize, &'static str)>,
    fired: &BTreeSet<(usize, &'static str)>,
) {
    for p in pending.iter_mut() {
        let Some(bi) = p.block else { continue };
        if let Some(directive) = &analysis.blocks[bi].ignore
            && directive.rule == p.finding.rule
        {
            p.finding.suppressed = true;
        }
    }

    for (bi, block) in analysis.blocks.iter().enumerate() {
        let Some(directive) = &block.ignore else {
            continue;
        };

        if directive.reason.is_none()
            && let Some(entry) = registry.get("ignoreReason").filter(|e| e.enabled)
            && let Some(disposition) = entry.disposition(block.kind)
        {
            pending.push(Pending {
                block: None,
                finding: build(
                    file,
                    src,
                    entry.id,
                    disposition,
                    crate::rule::RuleHit::new(
                        directive.span.clone(),
                        format!(
                            "add a reason to this directive: `laconic:ignore {} — <why this comment stays>`",
                            directive.rule
                        ),
                    ),
                ),
            });
        }

        // Fires only when the named rule **ran and did not fire**. Suppressed is not the same as
        // did-not-fire: conflating them reports a live directive as dead, and deleting it as the
        // diagnostic instructs makes the suppressed rule fire the moment the file is repaired.
        //
        // A directive naming a rule that is not in the registry, or is disabled, never ran — so it
        // is not dead either, and this stays quiet.
        let named_ran = ran.contains(&(bi, resolve_id(registry, &directive.rule).unwrap_or("")));
        let named_fired =
            fired.contains(&(bi, resolve_id(registry, &directive.rule).unwrap_or("")));
        if named_ran
            && !named_fired
            && let Some(entry) = registry.get("deadIgnore").filter(|e| e.enabled)
            && let Some(disposition) = entry.disposition(block.kind)
        {
            pending.push(Pending {
                block: None,
                finding: build(
                    file,
                    src,
                    entry.id,
                    disposition,
                    crate::rule::RuleHit::new(
                        directive.span.clone(),
                        format!(
                            "delete this directive: `{}` no longer fires on the comment below it",
                            directive.rule
                        ),
                    ),
                ),
            });
        }
    }
}

fn resolve_id(registry: &Registry, name: &str) -> Option<&'static str> {
    registry.get(name).map(|e| e.id)
}

fn build(
    file: &Path,
    src: &str,
    rule: &'static str,
    disposition: Disposition,
    hit: crate::rule::RuleHit,
) -> Finding {
    let (line, column) = line_column(src, hit.span.start);
    Finding {
        rule,
        file: file.to_path_buf(),
        span: hit.span,
        line,
        column,
        tier: disposition.tier,
        fix: disposition.fix,
        instruction: hit.instruction,
        suppressed: false,
    }
}

/// Whether `fix` may apply this finding: a Delete shape from a rule whose autofix is on.
pub fn is_autofixable(registry: &Registry, finding: &Finding) -> bool {
    finding.fix == FixShape::Delete
        && !finding.suppressed
        && registry.get(finding.rule).is_some_and(|e| e.autofix)
}

/// Whether any finding gates. Kept beside dispatch so the tier's one consequence is stated once.
pub fn gates(findings: &[Finding]) -> bool {
    findings
        .iter()
        .any(|f| f.tier == Tier::Gate && !f.suppressed)
}
