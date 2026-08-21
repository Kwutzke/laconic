//! Helpers several packs share.
//!
//! Shared because the mechanism is identical, not to make packs look alike. Where two languages
//! answer a concern differently — and most of the time they do — each pack says so itself.

use laconic_engine::domain::Visibility;
use tree_sitter::Node;

/// Whether only whitespace precedes the comment on its line.
///
/// A trailing comment documents nothing: without this test, `x = 1 // note` becomes the doc comment
/// of whatever declaration happens to follow it.
pub fn alone_on_its_line(node: Node, src: &str) -> bool {
    let start = node.start_byte();
    let line_start = src[..start].rfind('\n').map_or(0, |i| i + 1);
    src[line_start..start].trim().is_empty()
}

/// Doc kind for the languages with no grammar field to read — Java and TS/JS.
///
/// Not `starts_with("/**")`: that also matches the empty comment `/**/` and every `/****…****/`
/// banner, which would make Doc kind out of exactly the input `banner` exists to catch and cost
/// that rule its gate tier and its Delete fix. The fourth character must be neither `*` nor `/`.
/// The known loss is `/***`-opened doc comments, which no generator emits.
pub fn is_marker_doc(body: &str) -> bool {
    body.strip_prefix("/**")
        .is_some_and(|rest| !rest.starts_with(['*', '/']))
}

/// A C-family comment with its markers removed.
///
/// Leading whitespace after the marker stays: `commentedOutCode` parses the body in the file's own
/// language, and trimming each line would destroy the relative indentation an indentation-
/// significant language needs to parse at all.
pub fn strip_c_markers(raw: &str) -> String {
    if let Some(rest) = raw.strip_prefix("//") {
        return rest.trim_end().to_string();
    }
    let inner = raw
        .strip_prefix("/*")
        .and_then(|r| r.strip_suffix("*/"))
        .unwrap_or(raw);
    inner
        .lines()
        .map(strip_continuation_leader)
        .collect::<Vec<_>>()
        .join("\n")
        .trim_matches('\n')
        .to_string()
}

/// `  * text` becomes ` text`; a line with no leader is returned as it stands.
fn strip_continuation_leader(line: &str) -> &str {
    let trimmed = line.trim_end();
    let indent = trimmed.len() - trimmed.trim_start().len();
    match trimmed[indent..].strip_prefix('*') {
        Some(rest) => rest,
        None => trimmed,
    }
}

/// The named descendants of `root` matching any of `kinds`, in document order.
pub fn descendants_of_kind<'t>(root: Node<'t>, kinds: &[&str]) -> Vec<Node<'t>> {
    let mut cursor = root.walk();
    let mut stack = vec![root];
    let mut out = Vec::new();
    while let Some(n) = stack.pop() {
        for child in n.named_children(&mut cursor) {
            if kinds.contains(&child.kind()) {
                out.push(child);
            }
            stack.push(child);
        }
    }
    out.sort_by_key(|n| n.start_byte());
    out
}

/// The comment directly above `subject`, when one is there and is alone on its line.
pub fn preceding_comment<'t>(
    subject: Node<'t>,
    src: &str,
    comment_kinds: &[&str],
) -> Option<Node<'t>> {
    let prev = subject.prev_sibling()?;
    let adjacent = prev.end_position().row + 1 == subject.start_position().row;
    (comment_kinds.contains(&prev.kind()) && adjacent && alone_on_its_line(prev, src))
        .then_some(prev)
}

/// Statements in a body node, excluding comments and any node kind that is not a statement.
pub fn count_statements(body: Node, skip: &[&str]) -> usize {
    let mut cursor = body.walk();
    body.named_children(&mut cursor)
        .filter(|n| !n.is_extra() && !skip.contains(&n.kind()))
        .count()
}

/// A keyword child of a `modifiers`-style node — Java's shape.
///
/// A child, not a substring of the node's text: an annotation carries its arguments inside that
/// text, so `@SuppressWarnings("public-api") private void hidden()` reads as public under a
/// substring test and `implInInterface` then treats a private method as a subject.
pub fn has_modifier_keyword(node: Node, keyword: &str) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor).any(|m| {
        m.kind() == "modifiers" && {
            let mut mc = m.walk();
            m.children(&mut mc).any(|k| k.kind() == keyword)
        }
    })
}

/// The name a declaration binds, when it carries a `name` field.
pub fn declared_name<'a>(node: Node, src: &'a str) -> Option<&'a str> {
    node.child_by_field_name("name")
        .map(|n| &src[n.byte_range()])
}

/// Whether a declaration is wrapped in an `export` statement — the TS/JS shape.
pub fn is_exported_declaration(node: Node) -> bool {
    node.parent()
        .is_some_and(|p| p.kind() == "export_statement")
}

pub fn visibility_from_export(node: Node) -> Visibility {
    if is_exported_declaration(node) {
        Visibility::Exported
    } else {
        Visibility::Restricted("module".to_string())
    }
}
