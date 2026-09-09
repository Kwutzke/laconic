//! What a caller receives.
//!
//! A unit rather than a formatting detail, because the consumer is a machine acting on one
//! diagnostic: the output is a contract.

use crate::registry::{FixShape, Tier};
use std::fmt::Write as _;
use std::ops::Range;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub rule: &'static str,
    pub file: PathBuf,
    pub span: Range<usize>,
    /// 1-based, so an editor and a human agree with the output.
    pub line: usize,
    pub column: usize,
    pub tier: Tier,
    pub fix: FixShape,
    pub instruction: String,
    /// A hint for the reader, never an instruction for the agent.
    ///
    /// Separate from `instruction` because the consumer executes instructions: a restructuring hint
    /// folded into one becomes an order, and the cheapest way to satisfy "restructure" is to add
    /// code — which grows a ratio's denominator and reaches green with every comment still in
    /// place. Rendered after the instruction, because the comment is what gets repaired first.
    pub note: Option<String>,
    /// Covered by an ignore directive naming this rule. Recorded rather than discarded, which is
    /// what lets `deadIgnore` tell "ran and did not fire" from "was suppressed".
    pub suppressed: bool,
}

/// A file laconic could not fully analyse must be distinguishable from a clean one in the output
/// itself, which is why this is a record rather than a log line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithheldNote {
    pub file: PathBuf,
    pub rules: Vec<&'static str>,
}

/// A file that could not be read at all. Reported per file; the run continues over the rest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnreadableFile {
    pub file: PathBuf,
    pub reason: String,
}

#[derive(Debug, Default, Clone)]
pub struct Report {
    pub findings: Vec<Finding>,
    pub withheld: Vec<WithheldNote>,
    pub unreadable: Vec<UnreadableFile>,
}

/// `0` no gate findings, `1` gate findings present, `2` usage or configuration error. CI must be
/// able to distinguish a failing gate from a broken install, which one non-zero code cannot do.
pub const EXIT_CLEAN: i32 = 0;
pub const EXIT_GATE: i32 = 1;
pub const EXIT_USAGE: i32 = 2;

impl Report {
    /// Ordering is imposed here, by file path then byte offset then rule id. The walk's order is
    /// not the output's order: two runs over the same tree must produce identical output, or a CI
    /// diff means nothing.
    pub fn finalise(&mut self) {
        self.findings
            .sort_by(|a, b| (&a.file, a.span.start, a.rule).cmp(&(&b.file, b.span.start, b.rule)));
        // Two findings a consumer cannot tell apart are one finding. A per-subject rule reports
        // once per enclosing subject, so a Python module whose only statement is a class measures
        // the same comment run twice — and the spans differ by the closing byte, which is why
        // sorting alone leaves both. Only exact renders collapse: distinct spans that render
        // differently are distinct problems and stay.
        self.findings.dedup_by(|a, b| {
            (&a.file, a.rule, a.line, a.column, &a.instruction)
                == (&b.file, b.rule, b.line, b.column, &b.instruction)
        });
        self.withheld.sort_by(|a, b| a.file.cmp(&b.file));
        self.unreadable.sort_by(|a, b| a.file.cmp(&b.file));
    }

    /// A suppressed finding never affects exit status — that is what suppressing it means. A parse
    /// failure never by itself produces a non-zero exit either, so withheld notes are not counted.
    pub fn exit_code(&self) -> i32 {
        let gated = self
            .findings
            .iter()
            .any(|f| f.tier == Tier::Gate && !f.suppressed);
        if gated { EXIT_GATE } else { EXIT_CLEAN }
    }

    /// Findings a caller acts on: everything not suppressed.
    pub fn active(&self) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(|f| !f.suppressed)
    }

    pub fn human(&self) -> String {
        let mut out = String::new();
        for f in self.active() {
            let _ = writeln!(
                out,
                "{}:{}:{}: {} [{}] {}",
                f.file.display(),
                f.line,
                f.column,
                tier_str(f.tier),
                f.rule,
                f.instruction
            );
            if let Some(note) = &f.note {
                let _ = writeln!(out, "  note: {note}");
            }
        }
        for w in &self.withheld {
            let _ = writeln!(
                out,
                "{}: not fully analysed; rules withheld: {}",
                w.file.display(),
                w.rules.join(", ")
            );
        }
        for u in &self.unreadable {
            let _ = writeln!(out, "{}: not read: {}", u.file.display(), u.reason);
        }
        out
    }

    /// One record per line, tab-separated, with a `#` leader on the note kinds.
    ///
    /// A *Delete* fix is carried as its byte span, so an agent applies it without re-deriving it
    /// from the instruction text.
    pub fn machine(&self) -> String {
        let mut out = String::new();
        for f in self.active() {
            let _ = writeln!(
                out,
                "finding\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                f.rule,
                f.file.display(),
                f.span.start,
                f.span.end,
                f.line,
                f.column,
                tier_str(f.tier),
                fix_str(f.fix),
            );
            let _ = writeln!(out, "\tinstruction\t{}", f.instruction);
            if let Some(note) = &f.note {
                let _ = writeln!(out, "\tnote\t{note}");
            }
        }
        for w in &self.withheld {
            let _ = writeln!(
                out,
                "#withheld\t{}\t{}",
                w.file.display(),
                w.rules.join(",")
            );
        }
        for u in &self.unreadable {
            let _ = writeln!(out, "#unreadable\t{}\t{}", u.file.display(), u.reason);
        }
        out
    }
}

pub fn tier_str(t: Tier) -> &'static str {
    match t {
        Tier::Gate => "gate",
        Tier::Warn => "warn",
    }
}

pub fn fix_str(f: FixShape) -> &'static str {
    match f {
        FixShape::Delete => "delete",
        FixShape::Rewrite => "rewrite",
        FixShape::None => "none",
    }
}

/// 1-based line and **character** column for a byte offset.
///
/// Characters, not bytes: `x := "日本" // changed to use a map` would otherwise report the comment
/// four columns right of where any editor puts it, with nothing in the output to say the units
/// differ.
pub fn line_column(src: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(src.len());
    let head = &src[..offset];
    let line = head.matches('\n').count() + 1;
    let line_start = head.rfind('\n').map_or(0, |i| i + 1);
    let column = src[line_start..offset].chars().count() + 1;
    (line, column)
}
