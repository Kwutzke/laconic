//! Dispatch and reconcile.
//!
//! Dispatch is per block for twelve rules and per subject for `density`. **Reconcile** is where
//! the other two live: findings covered by an ignore directive are marked suppressed rather than
//! discarded, and only then can `deadIgnore` ask whether a named rule ran and did not fire, and
//! `ignoreReason` report a directive carrying no reason.

use crate::config::Resolved;
use crate::domain::CommentKind;
use crate::pipeline::FileAnalysis;
use crate::registry::{Dispatch, Disposition, ErrorPolicy, FixShape, Registry};
use crate::report::{Finding, WithheldNote, line_column};
use crate::rule::{BlockContext, BlockRule, SubjectContext, SubjectRule};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Default)]
pub struct Rules {
    pub block: Vec<Box<dyn BlockRule>>,
    pub subject: Vec<Box<dyn SubjectRule>>,
}

/// A finding still carrying the block whose ignore directive reconcile must match it against.
///
/// For a per-block rule that is the block the finding came from. For `density` it is not: a
/// per-subject finding comes from no block, and this holds whichever of the subject's blocks
/// carries the directive covering it. Suppression is all the field is for — anything wanting the
/// finding's origin needs a different one.
struct Pending {
    block: Option<usize>,
    finding: Finding,
}

pub fn dispatch(
    file: &Path,
    src: &str,
    analysis: &FileAnalysis,
    resolved: &Resolved,
    rules: &Rules,
) -> (Vec<Finding>, Option<WithheldNote>) {
    let registry = &resolved.registry;
    let mut withheld: BTreeSet<&'static str> = BTreeSet::new();
    let mut pending: Vec<Pending> = Vec::new();
    // (block index, rule id) pairs that were evaluated. `deadIgnore` needs "ran and did not fire",
    // which is not the same as "did not fire".
    let mut ran: BTreeSet<(usize, &'static str)> = BTreeSet::new();
    let mut fired: BTreeSet<(usize, &'static str)> = BTreeSet::new();

    // Reused across blocks rather than allocated per block; drained at the end of each.
    let mut hits: Vec<(&'static str, Disposition, crate::rule::RuleHit)> = Vec::new();
    for (bi, block) in analysis.blocks.iter().enumerate() {
        for rule in &rules.block {
            let Some(entry) = registry.get(rule.id()).filter(|e| e.enabled) else {
                continue;
            };
            // The registry's `dispatch` decides where a rule runs. Without this the field is
            // documentation: a rule placed in the wrong vec would be dispatched anyway, or never,
            // and either way silently.
            debug_assert_eq!(
                entry.dispatch,
                Dispatch::PerBlock,
                "{} is registered for {:?} but was supplied as a block rule",
                entry.id,
                entry.dispatch
            );
            if entry.dispatch != Dispatch::PerBlock {
                continue;
            }
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
                source_extensions: &analysis.source_extensions,
                grammar: analysis.grammar,
                statement_scaffold: analysis.statement_scaffold,
                thresholds: resolved.thresholds,
                src,
            };
            if let Some(hit) = rule.check(&ctx) {
                hits.push((entry.id, disposition, hit));
            }
        }

        // Precedence is resolved here, over the block's own hits, and **before `fired` is
        // recorded**. A superseded rule did not fire: that is what keeps `deadIgnore` correct,
        // since a directive naming the loser is protecting nothing and should be reported as dead
        // exactly as it was when the loser declined by asking the winner's predicate directly.
        let winners: Vec<&'static str> = hits.iter().map(|(id, _, _)| *id).collect();
        for (id, disposition, hit) in hits.drain(..) {
            if crate::registry::superseded_by(id)
                .iter()
                .any(|w| winners.contains(w))
            {
                continue;
            }
            fired.insert((bi, id));
            pending.push(Pending {
                block: Some(bi),
                finding: build(file, src, id, disposition, hit),
            });
        }
    }

    // `density` is the only per-subject dispatch in the set: an aggregate over every block attached
    // to one subject, anchored to the subject rather than to a block.
    for rule in &rules.subject {
        let Some(entry) = registry.get(rule.id()).filter(|e| e.enabled) else {
            continue;
        };
        debug_assert_eq!(
            entry.dispatch,
            Dispatch::PerSubject,
            "{} is registered for {:?} but was supplied as a subject rule",
            entry.id,
            entry.dispatch
        );
        if entry.dispatch != Dispatch::PerSubject {
            continue;
        }
        for (si, subject) in analysis.subjects.iter().enumerate() {
            // Blocks **inside** the subject's span, not blocks attached to it. `density` is per
            // function, and a comment inside a function body attaches to the statement below it —
            // so grouping by attachment would give every statement its own denominator and no
            // function any comments at all. The subject's own doc comment sits above its span and
            // is correctly excluded: `docbloat` measures that one.
            let indexed: Vec<(usize, &crate::domain::CommentBlock)> = analysis
                .blocks
                .iter()
                .enumerate()
                .filter(|(_, b)| {
                    b.span.start >= subject.span.start && b.span.end <= subject.span.end
                })
                .collect();
            if indexed.is_empty() {
                continue;
            }
            let blocks: Vec<_> = indexed.iter().map(|(_, b)| *b).collect();
            let in_error = indexed.iter().any(|(_, b)| b.in_error_subtree);
            if withhold(entry.error_policy, analysis.has_error_nodes, in_error) {
                withheld.insert(entry.id);
                continue;
            }
            // Its disposition is read from the kind of the blocks it aggregates; a subject has no
            // kind of its own.
            let Some(disposition) = indexed.iter().find_map(|(_, b)| entry.disposition(b.kind))
            else {
                continue;
            };
            // A directive on any block inside the subject suppresses the subject's finding.
            // §4 says a protected block is dispatched and its findings recorded as suppressed, with
            // no exemption for the one per-subject rule — and without this there is no way to
            // silence `density` short of disabling it in config.
            //
            // The subject's own doc block counts too, and it is not in `indexed`: a reader puts
            // `// laconic:ignore density — …` above the function, where it binds to the doc
            // comment sitting above the subject's span. Searched only inside, that directive was a
            // silent no-op — the finding reported unsuppressed, `ignoreReason` quiet because the
            // directive has a reason, and `deadIgnore` quiet because `density` never enters `ran`.
            let covering = analysis
                .blocks
                .iter()
                .enumerate()
                .filter(|(i, b)| indexed.iter().any(|(j, _)| j == i) || b.subject == Some(si))
                .find(|(_, b)| b.ignore.as_ref().is_some_and(|d| d.rule == entry.id))
                .map(|(i, _)| i);
            // A directive above an **undocumented** subject binds to no block at all: a run
            // holding only a directive produces none, so it lands in `unbound_directives`. That is
            // the commonest shape `density` fires on, because `Density::check` excludes Doc kind
            // and counts body comments — so searching blocks alone left the reader looking at a
            // finding they had suppressed, with nothing in the output saying why.
            let subject_row = line_column(src, subject.span.start).0;
            let unbound_cover = covering.is_none()
                && analysis
                    .unbound_directives
                    .iter()
                    .any(|d| d.rule == entry.id && d.start_row + 2 == subject_row);
            let ctx = SubjectContext {
                subject,
                blocks,
                thresholds: resolved.thresholds,
            };
            if let Some(hit) = rule.check(&ctx) {
                let mut finding = build(file, src, entry.id, disposition, hit);
                // Recorded here rather than in reconcile, which matches a finding to its block's
                // directive and has no block to match against for this one.
                finding.suppressed = unbound_cover;
                pending.push(Pending {
                    block: covering,
                    finding,
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

    // A directive that bound to no block still lacks a reason, and a directive protecting nothing
    // is the one a reader most needs told about. Reporting only bound directives would make
    // `ignoreReason`'s completeness depend on the binding step, silently.
    for directive in &analysis.unbound_directives {
        if directive.reason.is_some() {
            continue;
        }
        let Some(entry) = registry.get("ignoreReason").filter(|e| e.enabled) else {
            continue;
        };
        let Some(disposition) = entry.disposition(CommentKind::Line) else {
            continue;
        };
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
        note: hit.note,
        suppressed: false,
    }
}

/// Whether `fix` may apply this finding: a Delete shape from a rule whose autofix is on.
pub fn is_autofixable(registry: &Registry, finding: &Finding) -> bool {
    finding.fix == FixShape::Delete
        && !finding.suppressed
        && registry.get(finding.rule).is_some_and(|e| e.autofix)
}
