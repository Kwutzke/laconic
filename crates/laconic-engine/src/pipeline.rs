//! The per-file pipeline, from a path to blocks that are ready to dispatch.
//!
//! `resolve pack by extension → parse → extract comment nodes → classify and strip → group into
//! blocks → lift out ignore directives → resolve attachment, kind and subject → drop excluded
//! regions`.
//!
//! The design states the order as group-then-strip. Stripping a machine directive out of a block
//! it had already joined would mean splitting that block and rejoining its remainder, so grouping
//! here runs over the comments that survive classification instead. Same blocks, no split-and-
//! rejoin path to get wrong.

use crate::domain::{
    Attachment, Comment, CommentBlock, CommentKind, DeclaredSymbol, IgnoreDirective, Subject,
};
use crate::exclude::Config;
use crate::pack::Pack;
use laconic_grammars::Grammar;
use std::path::Path;
use tree_sitter::Node;

/// Everything the dispatch stage needs about one file.
#[derive(Debug)]
pub struct FileAnalysis {
    pub blocks: Vec<CommentBlock>,
    pub subjects: Vec<Subject>,
    /// Every declaration in the file — pack concern 11, for `implInInterface`.
    pub declared: Vec<DeclaredSymbol>,
    /// `implInInterface` is suppressed for the whole file when this is set, because its input is
    /// file-scoped: a clean subtree says nothing about whether the enumeration is complete.
    pub has_error_nodes: bool,
}

/// Why a file produced no analysis. None of these is an error: laconic runs over whole
/// repositories, and warning on every `.json` makes the output unusable.
#[derive(Debug, PartialEq, Eq)]
pub enum Skipped {
    NoPackClaimsExtension,
    ExcludedPath,
    GeneratedFile,
}

/// Resolve a pack for a path — concern 1, per extension rather than per pack.
pub fn resolve<'p>(packs: &'p [Box<dyn Pack>], path: &Path) -> Option<(&'p dyn Pack, Grammar)> {
    let ext = path.extension()?.to_str()?;
    packs.iter().find_map(|p| {
        p.extensions()
            .iter()
            .find(|(e, _)| *e == ext)
            .map(|(_, g)| (p.as_ref(), *g))
    })
}

pub fn analyse(
    pack: &dyn Pack,
    grammar: Grammar,
    path: &Path,
    src: &str,
    config: &Config,
) -> Result<FileAnalysis, Skipped> {
    if config.is_excluded_path(path) {
        return Err(Skipped::ExcludedPath);
    }

    let tree = grammar
        .parser()
        .parse(src, None)
        .expect("a parser with a language set always returns a tree");
    let root = tree.root_node();

    if is_generated(pack, root, src) {
        return Err(Skipped::GeneratedFile);
    }

    let docs = pack.doc_comments(root, src);
    let comment_nodes = collect_comment_nodes(pack, grammar, root);

    // Doc comments that are not comment nodes — a Python docstring is a `string` — join the set
    // here rather than being missed by concern 2.
    let mut sources: Vec<Node> = comment_nodes;
    for d in &docs {
        if !sources.iter().any(|n| n.id() == d.node.id()) {
            sources.push(d.node);
        }
    }
    sources.sort_by_key(|n| n.start_byte());

    let mut ordinary: Vec<(Node, Comment)> = Vec::new();
    let mut directives: Vec<IgnoreDirective> = Vec::new();
    for node in sources {
        let body = pack.comment_body(node, src);
        if is_machine_directive(pack, &body) {
            continue;
        }
        let comment = Comment {
            span: node.byte_range(),
            start_row: node.start_position().row,
            end_row: node.end_position().row,
            trailing: has_code_before(src, node.start_byte()),
            body,
        };
        match parse_ignore_directive(&comment) {
            Some(directive) => directives.push(directive),
            None => ordinary.push((node, comment)),
        }
    }

    let runs = group(&ordinary);

    // Every documentable declaration becomes a subject, whether or not a comment sits above it:
    // `density` measures a function nobody documented, and a subject that exists only where a
    // block attached would make that rule unable to see one.
    let subject_nodes = pack.subject_nodes(root);
    let subjects: Vec<Subject> = subject_nodes
        .iter()
        .map(|n| build_subject(pack, *n, src))
        .collect();
    let subject_index = |node: Node| subject_nodes.iter().position(|s| s.id() == node.id());

    let mut blocks: Vec<CommentBlock> = Vec::new();

    for run in runs {
        let nodes: Vec<Node> = run.iter().map(|i| ordinary[*i].0).collect();
        let comments: Vec<Comment> = run.iter().map(|i| ordinary[*i].1.clone()).collect();

        let doc = docs
            .iter()
            .find(|d| nodes.iter().any(|n| n.id() == d.node.id()));
        let kind = match doc {
            Some(_) => CommentKind::Doc,
            None => pack.comment_kind(nodes[0], src),
        };

        let last = *nodes.last().expect("a run is never empty");
        let following = next_code_node(root, last.end_byte());
        let attachment = if comments[0].trailing {
            Attachment::AttachedTrailing
        } else {
            match following {
                Some(n) if n.start_position().row == comments.last().unwrap().end_row + 1 => {
                    Attachment::AttachedBelow
                }
                _ => Attachment::Detached,
            }
        };

        let subject = match doc {
            Some(d) => subject_index(d.subject),
            None if attachment == Attachment::AttachedBelow => following.and_then(subject_index),
            None => None,
        };
        let attached_identifiers = match attachment {
            Attachment::AttachedBelow => following
                .map(|n| pack.bound_identifiers(n, src))
                .unwrap_or_default(),
            _ => Vec::new(),
        };

        let span = comments[0].span.start..comments.last().unwrap().span.end;
        blocks.push(CommentBlock {
            kind,
            attachment,
            span,
            subject,
            ignore: None,
            in_error_subtree: following.is_some_and(|n| n.has_error()),
            attached_identifiers,
            comments,
        });
    }

    bind_ignore_directives(&mut blocks, directives);

    // The licence carve-out is top-of-file only. A mid-file notice stays, because `attribution`
    // is the rule that must still see it and no deterministic test separates a required notice
    // from vanity.
    let header_is_licence = blocks
        .first()
        .is_some_and(|b| b.comments[0].start_row < 5 && config.is_licence_header(b));
    if header_is_licence {
        blocks.remove(0);
    }

    Ok(FileAnalysis {
        blocks,
        subjects,
        declared: pack.declared_symbols(root, src),
        has_error_nodes: has_error_nodes(root),
    })
}

fn collect_comment_nodes<'t>(pack: &dyn Pack, grammar: Grammar, root: Node<'t>) -> Vec<Node<'t>> {
    let kinds = pack.comment_node_kinds(grammar);
    let mut cursor = root.walk();
    let mut stack = vec![root];
    let mut out = Vec::new();
    while let Some(n) = stack.pop() {
        if kinds.contains(&n.kind()) {
            out.push(n);
        }
        for c in n.children(&mut cursor) {
            stack.push(c);
        }
    }
    out
}

fn is_generated(pack: &dyn Pack, root: Node, src: &str) -> bool {
    let head = &src[..src.len().min(2048)];
    let _ = root;
    pack.generated_file_markers()
        .iter()
        .any(|m| head.contains(m))
}

fn is_machine_directive(pack: &dyn Pack, body: &str) -> bool {
    let trimmed = body.trim_start();
    pack.machine_directive_prefixes()
        .iter()
        .any(|p| trimmed.starts_with(p))
}

fn has_code_before(src: &str, start: usize) -> bool {
    let line_start = src[..start].rfind('\n').map_or(0, |i| i + 1);
    !src[line_start..start].trim().is_empty()
}

/// Consecutive comments separated by whitespace only, each alone on its line, form one run. A
/// trailing comment is always its own run and never merges — with the code beside it or with the
/// comment below it.
fn group(comments: &[(Node, Comment)]) -> Vec<Vec<usize>> {
    let mut runs: Vec<Vec<usize>> = Vec::new();
    for (i, (_, c)) in comments.iter().enumerate() {
        let merges = !c.trailing
            && runs.last().is_some_and(|run| {
                let prev = &comments[*run.last().unwrap()].1;
                !prev.trailing && prev.end_row + 1 == c.start_row
            });
        if merges {
            runs.last_mut().unwrap().push(i);
        } else {
            runs.push(vec![i]);
        }
    }
    runs
}

/// `laconic:ignore <rule>` or `laconic:ignore <rule> — <reason>`.
fn parse_ignore_directive(comment: &Comment) -> Option<IgnoreDirective> {
    let rest = comment.body.trim().strip_prefix("laconic:ignore")?.trim();
    let (rule, reason) = match rest.split_once(['—', '-']) {
        Some((rule, reason)) => {
            let reason = reason.trim_start_matches(['-', '—']).trim();
            (
                rule.trim(),
                (!reason.is_empty()).then(|| reason.to_string()),
            )
        }
        None => (rest, None),
    };
    Some(IgnoreDirective {
        rule: rule.trim().to_string(),
        reason,
        span: comment.span.clone(),
        start_row: comment.start_row,
    })
}

/// A directive binds to the block below it, or to the block on its own line when trailing.
fn bind_ignore_directives(blocks: &mut [CommentBlock], directives: Vec<IgnoreDirective>) {
    for directive in directives {
        let target = blocks.iter_mut().find(|b| {
            b.comments
                .first()
                .is_some_and(|c| c.start_row == directive.start_row + 1)
                || b.comments
                    .first()
                    .is_some_and(|c| c.start_row == directive.start_row)
        });
        if let Some(block) = target {
            block.ignore = Some(directive);
        }
    }
}

/// The outermost node that begins at or after `offset` and is not a comment.
fn next_code_node(root: Node<'_>, offset: usize) -> Option<Node<'_>> {
    let mut cursor = root.walk();
    let mut stack = vec![root];
    let mut best: Option<Node> = None;
    while let Some(n) = stack.pop() {
        for c in n.named_children(&mut cursor) {
            if c.is_extra() {
                continue;
            }
            if c.start_byte() >= offset {
                let better = match best {
                    None => true,
                    Some(b) => (c.start_byte(), b.end_byte()) < (b.start_byte(), c.end_byte()),
                };
                if better {
                    best = Some(c);
                }
            } else if c.end_byte() > offset {
                stack.push(c);
            }
        }
    }
    best
}

fn build_subject(pack: &dyn Pack, node: Node, src: &str) -> Subject {
    Subject {
        span: node.byte_range(),
        bound_identifiers: pack.bound_identifiers(node, src),
        body_rows: node.end_position().row - node.start_position().row + 1,
        statement_count: pack.statement_count(node, src),
        visibility: pack.visibility(node, src),
    }
}

fn has_error_nodes(root: Node) -> bool {
    root.has_error()
}
