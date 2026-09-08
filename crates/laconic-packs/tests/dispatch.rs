//! Dispatch, reconcile and the reporter.
//!
//! The rules here are stubs: this subtask builds the three dispatch paths and the output contract,
//! and the real rules land in the two that follow. A stub that fires on a marker string is enough
//! to prove that the registry decides tier and fix while the rule decides only whether.

use laconic_engine::registry::{FixShape, Tier};
use laconic_engine::rule::{BlockContext, BlockRule, RuleHit, SubjectContext, SubjectRule};
use laconic_engine::{
    Config, FileAnalysis, Registry, Report, Resolved, Rules, analyse, dispatch, is_autofixable,
    resolve,
};
use laconic_packs::all;
use std::path::Path;

/// Fires on any block whose body contains `needle`.
struct OnBody {
    id: &'static str,
    needle: &'static str,
}

impl BlockRule for OnBody {
    fn id(&self) -> &'static str {
        self.id
    }
    fn check(&self, ctx: &BlockContext) -> Option<RuleHit> {
        ctx.block
            .body()
            .contains(self.needle)
            .then(|| RuleHit::new(ctx.block.span.clone(), format!("remove this {}", self.id)))
    }
}

/// Fires on every subject that has a block attached.
struct EverySubject(&'static str);

impl SubjectRule for EverySubject {
    fn id(&self) -> &'static str {
        self.0
    }
    fn check(&self, ctx: &SubjectContext) -> Option<RuleHit> {
        Some(RuleHit::new(
            ctx.subject.span.clone(),
            "rewrite these comments into one".to_string(),
        ))
    }
}

fn analysis(src: &str) -> (FileAnalysis, &'static str) {
    let packs = all();
    let path = Path::new("x.go");
    let (pack, grammar) = resolve(&packs, path).unwrap();
    let a = analyse(pack, grammar, path, src, &Config::unrestricted()).unwrap();
    (a, "x.go")
}

fn report_for(src: &str, rules: Rules) -> Report {
    let (a, name) = analysis(src);
    let registry = Registry::default();
    let (findings, note) = dispatch(
        Path::new(name),
        src,
        &a,
        &Resolved::from(registry.clone()),
        &rules,
    );
    let mut report = Report {
        findings,
        withheld: note.into_iter().collect(),
        unreadable: Vec::new(),
    };
    report.finalise();
    report
}

fn narration_stub() -> Rules {
    Rules {
        block: vec![Box::new(OnBody {
            id: "narration",
            needle: "changed to",
        })],
        subject: Vec::new(),
    }
}

const NARRATION: &str = "package x\n\n// changed to use a map\nfunc F() {}\n";

/// Tier and fix come from the registry per comment kind, not from the rule. The same stub firing on
/// a line comment gates and deletes; on a doc comment it warns and rewrites.
#[test]
fn the_registry_decides_tier_and_fix_per_kind() {
    let line = report_for(
        "package x\n\n// changed to use a map\n\nfunc F() {}\n",
        narration_stub(),
    );
    let f = &line.findings[0];
    assert_eq!((f.tier, f.fix), (Tier::Gate, FixShape::Delete));

    // The same text one line lower is Go's doc comment for the declaration below it.
    let doc = report_for(NARRATION, narration_stub());
    let f = &doc.findings[0];
    assert_eq!((f.tier, f.fix), (Tier::Warn, FixShape::Rewrite));
}

/// The doc-comment deletion invariant, end to end: no finding on a doc comment ever carries a
/// Delete fix, whatever the rule.
#[test]
fn a_doc_comment_never_receives_a_delete_fix() {
    let doc = report_for(NARRATION, narration_stub());
    assert!(doc.findings.iter().all(|f| f.fix != FixShape::Delete));
}

#[test]
fn gate_findings_exit_one_and_a_clean_run_exits_zero() {
    let gated = report_for(
        "package x\n\n// changed to use a map\n\nfunc F() {}\n",
        narration_stub(),
    );
    assert_eq!(gated.exit_code(), laconic_engine::EXIT_GATE);

    let clean = report_for(
        "package x\n\n// a plain comment\n\nfunc F() {}\n",
        narration_stub(),
    );
    assert!(clean.findings.is_empty());
    assert_eq!(clean.exit_code(), laconic_engine::EXIT_CLEAN);
}

/// AC10. Ordering is imposed by the reporter — file path then byte offset — so the walk's order is
/// not the output's order and a CI diff means something.
#[test]
fn two_runs_produce_byte_identical_machine_output() {
    let src = "package x\n\n// changed to use a map\n\n// changed to use a slice\n\nfunc F() {}\n";
    let first = report_for(src, narration_stub()).machine();
    let second = report_for(src, narration_stub()).machine();
    assert_eq!(first, second);
    assert!(!first.is_empty());
}

#[test]
fn findings_are_ordered_by_byte_offset() {
    let src = "package x\n\n// changed to use a map\n\n// changed to use a slice\n\nfunc F() {}\n";
    let report = report_for(src, narration_stub());
    let offsets: Vec<usize> = report.findings.iter().map(|f| f.span.start).collect();
    let mut sorted = offsets.clone();
    sorted.sort_unstable();
    assert_eq!(offsets, sorted);
    assert_eq!(offsets.len(), 2);
}

/// The two forms are one source: the same findings and the same withheld notes, differently
/// rendered.
#[test]
fn both_formats_report_the_same_findings() {
    let src = "package x\n\n// changed to use a map\n\nfunc F() {}\n";
    let report = report_for(src, narration_stub());
    let human = report.human();
    let machine = report.machine();
    for f in report.active() {
        assert!(
            human.contains(&f.instruction),
            "human carries the instruction"
        );
        assert!(
            machine.contains(&f.instruction),
            "machine carries the instruction"
        );
        assert!(
            machine.contains(&format!("{}\t{}", f.span.start, f.span.end)),
            "machine carries the span an agent applies without re-deriving it"
        );
    }
    assert_eq!(human.lines().count(), report.active().count());
}

/// A protected block is dispatched, not skipped: its rules run and their findings are recorded as
/// suppressed. That is what lets `deadIgnore` know whether the named rule would have fired.
#[test]
fn an_ignore_directive_suppresses_rather_than_skips() {
    let src = "package x\n\n// laconic:ignore narration — the log line matters\n// changed to use a map\n\nfunc F() {}\n";
    let report = report_for(src, narration_stub());
    let narration: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule == "narration")
        .collect();
    assert_eq!(narration.len(), 1, "the rule ran");
    assert!(narration[0].suppressed, "and its finding is suppressed");
    assert_eq!(
        report.exit_code(),
        laconic_engine::EXIT_CLEAN,
        "a suppressed finding never gates"
    );
    assert!(!report.human().contains("remove this narration"));
}

/// Gate tier, and never autofixable: the only deletion that makes it pass is deleting the
/// directive, which silently re-enables the rule it suppressed.
#[test]
fn a_directive_without_a_reason_is_a_gate_finding() {
    let src = "package x\n\n// laconic:ignore narration\n// changed to use a map\n\nfunc F() {}\n";
    let report = report_for(src, narration_stub());
    let f = report
        .findings
        .iter()
        .find(|f| f.rule == "ignoreReason")
        .expect("ignoreReason fired");
    assert_eq!((f.tier, f.fix), (Tier::Gate, FixShape::None));
    assert_eq!(report.exit_code(), laconic_engine::EXIT_GATE);
}

/// A directive protecting nothing is the one a reader most needs told about, and it reaches
/// `ignoreReason` through `unbound_directives` rather than through a block. Nothing exercised that
/// path: deleting the loop, or inverting its `reason.is_some()` guard, left the suite green.
#[test]
fn a_reasonless_directive_binding_to_no_block_still_fires_ignore_reason() {
    let src = "package x\n\n// laconic:ignore narration\n\nfunc F() {}\n";
    let report = report_for(src, narration_stub());
    let f = report
        .findings
        .iter()
        .find(|f| f.rule == "ignoreReason")
        .expect("ignoreReason fired for an unbound directive");
    assert_eq!((f.tier, f.fix), (Tier::Gate, FixShape::None));
}

/// Fires only when the named rule ran and did not fire.
#[test]
fn dead_ignore_fires_when_its_rule_ran_and_did_not_fire() {
    let src = "package x\n\n// laconic:ignore narration — kept for later\n// a plain comment\n\nfunc F() {}\n";
    let report = report_for(src, narration_stub());
    let f = report
        .findings
        .iter()
        .find(|f| f.rule == "deadIgnore")
        .expect("deadIgnore fired");
    assert_eq!(f.tier, Tier::Warn);
    assert_eq!(report.exit_code(), laconic_engine::EXIT_CLEAN);
}

#[test]
fn dead_ignore_stays_quiet_when_its_rule_fired() {
    let src = "package x\n\n// laconic:ignore narration — the log line matters\n// changed to use a map\n\nfunc F() {}\n";
    let report = report_for(src, narration_stub());
    assert!(report.findings.iter().all(|f| f.rule != "deadIgnore"));
}

/// Suppressed is not the same as did-not-fire. Conflating them reports a live directive as dead —
/// whereupon deleting it, as the diagnostic instructs, makes the withheld rule fire the moment the
/// syntax error is fixed.
#[test]
fn dead_ignore_stays_quiet_when_its_rule_was_withheld() {
    let src =
        "package x\n\n// laconic:ignore detached — structural\n// a plain comment\n\nfunc F( {}\n";
    let rules = Rules {
        block: vec![Box::new(OnBody {
            id: "detached",
            needle: "never matches this",
        })],
        subject: Vec::new(),
    };
    let report = report_for(src, rules);
    assert!(
        report.findings.iter().all(|f| f.rule != "deadIgnore"),
        "the named rule was withheld, not evaluated"
    );
    let note = report
        .withheld
        .first()
        .expect("a withheld note is recorded");
    assert!(note.rules.contains(&"detached"));
}

/// A file laconic could not fully analyse must be distinguishable from a clean one in the output
/// itself, which is why the note is a record rather than a log line.
#[test]
fn a_withheld_note_reaches_both_formats() {
    // The block must be Line kind: `detached` has no Doc disposition, and a rule that does not
    // apply to a kind is inapplicable rather than withheld.
    let src = "package x\n\n// a plain comment\n\nfunc F( {}\n";
    let rules = Rules {
        block: vec![Box::new(OnBody {
            id: "detached",
            needle: "never matches this",
        })],
        subject: Vec::new(),
    };
    let report = report_for(src, rules);
    assert!(report.human().contains("rules withheld"));
    assert!(report.machine().contains("#withheld"));
    assert_eq!(
        report.exit_code(),
        laconic_engine::EXIT_CLEAN,
        "a parse failure never by itself produces a non-zero exit"
    );
}

/// The only per-subject dispatch in the set. Its finding anchors to the subject, not to a block.
#[test]
fn density_dispatches_once_per_subject() {
    // Comments inside the function bodies: `density` is per function, and these attach to the
    // statements below them rather than to the functions.
    let src = "package x\n\nfunc F() {\n\t// one\n\t// two\n\tx := 1\n\t_ = x\n}\n\nfunc G() {\n\t// three\n\ty := 2\n\t_ = y\n}\n";
    let rules = Rules {
        block: Vec::new(),
        subject: vec![Box::new(EverySubject("density"))],
    };
    let report = report_for(src, rules);
    assert_eq!(
        report.findings.len(),
        2,
        "one per subject, not one per block"
    );
    let (a, _) = analysis(src);
    for f in &report.findings {
        assert!(
            a.subjects.iter().any(|s| s.span.start == f.span.start),
            "the finding anchors to a subject span, not to a block"
        );
        assert_eq!(f.tier, Tier::Warn);
    }
}

/// A disabled rule is not dispatched at all, so it cannot fire and cannot make a directive dead.
#[test]
fn a_disabled_rule_is_not_dispatched() {
    let (a, name) = analysis(NARRATION);
    let mut registry = Registry::default();
    registry.set_enabled("narration", false).unwrap();
    let (findings, _) = dispatch(
        Path::new(name),
        NARRATION,
        &a,
        &Resolved::from(registry.clone()),
        &narration_stub(),
    );
    assert!(findings.is_empty());
}

/// `is_autofixable` is the only gate on the destructive half of the tool, and its three conjuncts
/// are three independent protections. Covered as a product rather than by sampling: drop the
/// suppressed check and `fix` deletes a comment someone protected with a reasoned directive; drop
/// the autofix check and it deletes `restate`, `detached`, `attribution` and `commentedOutCode`
/// hits, each of which ships autofix-off for a stated false-positive reason.
#[test]
fn only_unsuppressed_delete_findings_from_autofix_rules_are_applicable() {
    let registry = Registry::default();
    let finding = |rule: &'static str, fix: FixShape, suppressed: bool| laconic_engine::Finding {
        rule,
        file: Path::new("x.go").to_path_buf(),
        span: 0..1,
        line: 1,
        column: 1,
        tier: Tier::Gate,
        fix,
        instruction: String::new(),
        suppressed,
    };

    // The only combination `fix` may touch.
    assert!(is_autofixable(
        &registry,
        &finding("narration", FixShape::Delete, false)
    ));

    // Each conjunct, removed one at a time.
    assert!(
        !is_autofixable(&registry, &finding("narration", FixShape::Delete, true)),
        "a suppressed finding is never applied"
    );
    for rule in ["restate", "detached", "attribution", "commentedOutCode"] {
        assert!(
            !is_autofixable(&registry, &finding(rule, FixShape::Delete, false)),
            "{rule} ships autofix off"
        );
    }
    for shape in [FixShape::Rewrite, FixShape::None] {
        assert!(
            !is_autofixable(&registry, &finding("narration", shape, false)),
            "only Delete is mechanically applicable"
        );
    }

    // A rule that is not in the registry at all cannot be applied either.
    assert!(!is_autofixable(
        &registry,
        &finding("nosuchrule", FixShape::Delete, false)
    ));
}

/// A directive on a block inside the subject silences the subject's finding. Without this there is
/// no way to quiet `density` short of disabling the rule in config, and §4 grants no exemption for
/// the one per-subject rule.
#[test]
fn a_per_subject_finding_can_be_suppressed() {
    let src = "package x\n\nfunc F() {\n\t// laconic:ignore density — the state machine needs it\n\t// one\n\tq := 1\n\t_ = q\n}\n";
    let rules = Rules {
        block: Vec::new(),
        subject: vec![Box::new(EverySubject("density"))],
    };
    let report = report_for(src, rules);
    let density: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.rule == "density")
        .collect();
    assert_eq!(density.len(), 1, "the rule ran");
    assert!(density[0].suppressed, "and its finding is suppressed");
}

/// The column is a character column. Reporting bytes would put a finding after a multi-byte string
/// several columns right of where any editor shows it, with nothing in the output saying so.
#[test]
fn the_column_counts_characters_not_bytes() {
    let src = "package x\n\nfunc F() {\n\tq := \"日本語\" // changed to use a map\n\t_ = q\n}\n";
    let report = report_for(src, narration_stub());
    let f = report.active().next().expect("narration fired");
    let line = src.lines().nth(f.line - 1).expect("the reported line");
    let at: String = line.chars().skip(f.column - 1).take(2).collect();
    assert_eq!(at, "//", "the column lands on the comment marker");
}
