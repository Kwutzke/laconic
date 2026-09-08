//! `laconic fix`: autofixable findings become byte-range edits, applied back-to-front.
//!
//! Delete findings only, from rules whose registry entry enables autofix. The other four Delete
//! rules ship it off, so the only ways past one are a hand fix or a reasoned ignore directive —
//! intended friction. Deletion is all-or-nothing per block; no fix shape keeps line three of four.

use crate::dispatch::is_autofixable;
use crate::domain::CommentBlock;
use crate::pack::{BlankLinePolicy, Pack};
use crate::pipeline::FileAnalysis;
use crate::registry::Registry;
use crate::report::Finding;
use std::ops::Range;

/// The source with every autofixable finding's block removed.
///
/// Ranges are computed against the original text and applied in descending order, so an earlier
/// offset is still valid when its edit runs.
pub fn fix(
    src: &str,
    analysis: &FileAnalysis,
    findings: &[Finding],
    registry: &Registry,
    pack: &dyn Pack,
) -> String {
    let mut ranges: Vec<Range<usize>> = findings
        .iter()
        .filter(|f| is_autofixable(registry, f))
        .filter_map(|f| block_at(analysis, &f.span))
        .map(|block| deletion_range(src, block, pack.blank_line_policy()))
        .collect();

    ranges.sort_by_key(|r| r.start);
    let merged = merge(ranges);

    let mut out = src.to_string();
    for range in merged.into_iter().rev() {
        out.replace_range(range, "");
    }
    out
}

/// The block a finding was reported on, matched on the span the rule carried. A subject rule's span
/// matches no block, which is correct: the one subject rule's fix shape is Rewrite.
fn block_at<'a>(analysis: &'a FileAnalysis, span: &Range<usize>) -> Option<&'a CommentBlock> {
    analysis.blocks.iter().find(|b| b.span == *span)
}

/// What one block's removal takes with it.
///
/// A trailing block keeps its line, because the code before it on that line is not the finding.
/// Every other block goes by whole lines, together with the directive protecting it — a directive
/// outliving its block is a `deadIgnore` finding this function would have manufactured.
fn deletion_range(src: &str, block: &CommentBlock, policy: BlankLinePolicy) -> Range<usize> {
    if block.comments.iter().any(|c| c.trailing) {
        let start = src[..block.span.start].trim_end_matches([' ', '\t']).len();
        return start..block.span.end;
    }

    let mut start = line_start(src, block.span.start);
    let mut end = line_end(src, block.span.end);
    if let Some(directive) = &block.ignore {
        start = start.min(line_start(src, directive.span.start));
        end = end.max(line_end(src, directive.span.end));
    }

    // Collapsing takes the blank line *below*, never the one above, so a removal directly under a
    // declaration cannot pull that declaration up against the previous one.
    if policy == BlankLinePolicy::CollapseRun && blank_above(src, start) && blank_below(src, end) {
        end = line_end(src, end);
    }
    start..end
}

fn line_start(src: &str, at: usize) -> usize {
    src[..at].rfind('\n').map_or(0, |i| i + 1)
}

/// One past the newline ending the line `at` sits on, so a whole-line removal takes its terminator.
fn line_end(src: &str, at: usize) -> usize {
    src[at..].find('\n').map_or(src.len(), |i| at + i + 1)
}

fn blank_above(src: &str, start: usize) -> bool {
    start != 0 && src[line_start(src, start - 1)..start].trim().is_empty()
}

fn blank_below(src: &str, end: usize) -> bool {
    end < src.len() && src[end..line_end(src, end)].trim().is_empty()
}

/// Overlapping and touching ranges become one edit.
///
/// Two rules reporting one block give two findings with the same span, and whole-line expansion can
/// make neighbouring blocks meet. Splicing both would corrupt the offsets the descending order
/// exists to keep valid.
fn merge(sorted: Vec<Range<usize>>) -> Vec<Range<usize>> {
    let mut out: Vec<Range<usize>> = Vec::with_capacity(sorted.len());
    for range in sorted {
        match out.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => out.push(range),
        }
    }
    out
}
