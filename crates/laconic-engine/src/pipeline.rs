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
    /// The extensions the resolved pack claims — concern 1, which `fileref` needs to tell a source
    /// path from an ordinary dotted word.
    pub source_extensions: Vec<&'static str>,
    /// The grammar this file was parsed with, for `commentedOutCode`.
    pub grammar: Grammar,
    /// Directives that bound to no block.
    ///
    /// Kept rather than dropped: `ignoreReason` fires on a directive lacking a reason, and a
    /// directive protecting nothing is exactly the one a reader most needs told about. Dropping
    /// them made the rule's completeness depend on the binding step, and its misses silent.
    pub unbound_directives: Vec<IgnoreDirective>,
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

    if is_generated(pack, src) {
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

    // **Grouping runs over every comment, before anything is removed.** An earlier version stripped
    // machine directives first, which broke row adjacency wherever a directive sat inside a run: the
    // comments above and below it stopped being one block, and a narration block split by a
    // `//nolint:` line became two gate-tier findings where the source has one comment.
    let all: Vec<(Node, Comment)> = sources
        .into_iter()
        .map(|node| {
            let comment = Comment {
                span: node.byte_range(),
                start_row: node.start_position().row,
                end_row: node.end_position().row,
                trailing: has_code_before(src, node.start_byte()),
                body: pack.comment_body(node, src),
            };
            (node, comment)
        })
        .collect();
    let runs = group(&all);

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

    let mut directives: Vec<IgnoreDirective> = Vec::new();

    for run in runs {
        // Machine directives are dropped and ignore directives lifted out **within** the run, so
        // neither changes which comments are one block. A directive left in the block would join
        // its text to what `narration` and `restate` match against, so a suppression would alter
        // the finding it suppresses.
        let mut nodes: Vec<Node> = Vec::new();
        let mut comments: Vec<Comment> = Vec::new();
        for i in run {
            let (node, comment) = &all[i];
            if is_machine_directive(pack, &comment.body) {
                continue;
            }
            match parse_ignore_directive(comment) {
                Some(directive) => directives.push(directive),
                None => {
                    nodes.push(*node);
                    comments.push(comment.clone());
                }
            }
        }
        if comments.is_empty() {
            continue;
        }

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
        // A trailing comment attaches to the code beside it, and `i++ // increment i` is the
        // commonest restatement there is. Leaving this empty for trailing blocks exempted that
        // whole shape from `restate` silently.
        let attached_identifiers = match attachment {
            Attachment::AttachedBelow => following
                .map(|n| pack.bound_identifiers(n, src))
                .unwrap_or_default(),
            Attachment::AttachedTrailing => preceding_code_node(root, comments[0].span.start)
                .map(|n| pack.bound_identifiers(n, src))
                .unwrap_or_default(),
            Attachment::Detached => Vec::new(),
        };

        let span = comments[0].span.start..comments.last().unwrap().span.end;
        let in_error_subtree = in_error_subtree(root, &span, following);
        blocks.push(CommentBlock {
            kind,
            attachment,
            span,
            subject,
            ignore: None,
            in_error_subtree,
            attached_identifiers,
            comments,
        });
    }

    let unbound_directives = bind_ignore_directives(&mut blocks, directives);

    // The licence carve-out is top-of-file only. A mid-file notice stays, because `attribution`
    // is the rule that must still see it and no deterministic test separates a required notice
    // from vanity.
    //
    // Two guards beyond the text test. The block must be the first one and must not be a doc
    // comment: a package comment is a documented public surface, and deleting it from the analysis
    // would hide every finding on it. And it must begin within the file's preamble — machine
    // directives and blank lines can push a real notice down a few rows, but a notice fifty rows in
    // is a mid-file notice and belongs to `attribution`.
    const PREAMBLE_ROWS: usize = 10;
    let header_is_licence = blocks.first().is_some_and(|b| {
        b.kind != CommentKind::Doc
            && b.comments[0].start_row < PREAMBLE_ROWS
            && config.is_licence_header(b)
    });
    if header_is_licence {
        blocks.remove(0);
    }

    Ok(FileAnalysis {
        blocks,
        subjects,
        declared: pack.declared_symbols(root, src),
        has_error_nodes: has_error_nodes(root),
        unbound_directives,
        source_extensions: pack.extensions().iter().map(|(e, _)| *e).collect(),
        grammar,
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

/// Generated-file markers, looked for in the file's opening bytes.
///
/// The cut is taken at a character boundary, not at byte 2048: slicing mid-character panics, and a
/// panic in a pre-commit hook or in CI is the only output anyone sees.
fn is_generated(pack: &dyn Pack, src: &str) -> bool {
    let mut cut = src.len().min(2048);
    while cut > 0 && !src.is_char_boundary(cut) {
        cut -= 1;
    }
    let head = &src[..cut];
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
/// Returns the directives that bound to nothing.
fn bind_ignore_directives(
    blocks: &mut [CommentBlock],
    directives: Vec<IgnoreDirective>,
) -> Vec<IgnoreDirective> {
    let mut unbound = Vec::new();
    for directive in directives {
        let target = blocks.iter_mut().find(|b| {
            b.comments.first().is_some_and(|c| {
                c.start_row == directive.start_row + 1 || c.start_row == directive.start_row
            })
        });
        match target {
            Some(block) => block.ignore = Some(directive),
            None => unbound.push(directive),
        }
    }
    unbound
}

/// The innermost node ending at or before `offset` on the same line — what a trailing comment sits
/// beside.
fn preceding_code_node(root: Node<'_>, offset: usize) -> Option<Node<'_>> {
    let mut cursor = root.walk();
    let mut stack = vec![root];
    let mut best: Option<Node> = None;
    while let Some(n) = stack.pop() {
        for c in n.named_children(&mut cursor) {
            if c.is_extra() {
                continue;
            }
            if c.end_byte() <= offset {
                let better = best.is_none_or(|b| {
                    (c.end_byte(), c.start_byte()) > (b.end_byte(), b.start_byte())
                });
                if better {
                    best = Some(c);
                }
            }
            stack.push(c);
        }
    }
    best
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

/// Whether a block sits in a subtree that does not parse — §7's suppression scope.
///
/// The subtree is the **top-level declaration** containing the block, not the root: the root of any
/// file with a single syntax error reports `has_error`, so testing the root would make subtree scope
/// mean file scope and collapse two of §7's four cases into one.
///
/// The second test covers a doc comment above a broken declaration. It sits outside that
/// declaration's span, so containment alone would call it clean while its subject is exactly the
/// node that failed to parse.
fn in_error_subtree(
    root: Node<'_>,
    span: &std::ops::Range<usize>,
    following: Option<Node>,
) -> bool {
    if following.is_some_and(|n| n.has_error()) {
        return true;
    }
    let mut cursor = root.walk();
    root.named_children(&mut cursor).any(|decl| {
        decl.start_byte() <= span.start && decl.end_byte() >= span.end && decl.has_error()
    })
}
