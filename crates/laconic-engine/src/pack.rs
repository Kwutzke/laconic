//! The pack interface: eleven concerns, four declarations and seven strategies.
//!
//! What protects charter constraint 4 is this enumeration plus the trait being open — not a claim
//! that per-language strategies collapse. A language whose strategy fits nothing already here
//! implements the trait directly rather than forcing a new variant into the engine.

use crate::domain::{CommentKind, DeclaredSymbol, Visibility};
use laconic_grammars::Grammar;
use tree_sitter::Node;

/// A doc comment together with the subject it documents — concern 6.
///
/// The pair travels together because the relation is not derivable from either half: Rust reads a
/// grammar field, Java and TS/JS read marker text, Go and Python read position. Python's points the
/// other way from the rest — a docstring documents the scope that contains it, not what follows.
#[derive(Debug, Clone, Copy)]
pub struct DocComment<'t> {
    pub node: Node<'t>,
    pub subject: Node<'t>,
}

/// What `fix` does to the whitespace a removed block leaves behind — concern 10.
///
/// The policy is the pack's rather than `fix`'s: a per-language switch inside `fix` is the same
/// constraint-4 violation as one inside the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlankLinePolicy {
    /// Remove the block's lines and leave surrounding blank lines as they were.
    LeaveSurrounding,
    /// Remove the block's lines and collapse a resulting run of blank lines to one.
    CollapseRun,
}

pub trait Pack {
    fn name(&self) -> &'static str;

    /// Concern 1 — the source extensions this pack claims, each with the grammar that parses it.
    ///
    /// Per extension rather than per pack: TypeScript, TSX and JavaScript are three grammars behind
    /// one TS/JS pack.
    fn extensions(&self) -> &[(&'static str, Grammar)];

    /// Concern 2 — which node types carry comments in this grammar. No type is common to all
    /// grammars, so this cannot be an engine constant.
    fn comment_node_kinds(&self, grammar: Grammar) -> &[&'static str];

    /// Concern 3 — comment prefixes some other tool reads. Stripped before any rule runs.
    fn machine_directive_prefixes(&self) -> &[&'static str];

    /// Concern 4 — markers identifying a generated file, which is excluded whole.
    fn generated_file_markers(&self) -> &[&'static str];

    /// Concern 5 — Line or Block for one comment node.
    ///
    /// Doc is not answered here: concern 6 owns it, because for three of the five languages Doc is
    /// positional or structural and cannot be decided from a comment node alone.
    fn comment_kind(&self, node: Node, src: &str) -> CommentKind;

    /// Concern 6 — every doc comment in the file, with the subject it documents.
    ///
    /// A returned node need not be a comment node: a Python docstring is a `string`, which is why
    /// this enumerates rather than classifying what concern 2 already found.
    fn doc_comments<'t>(&self, root: Node<'t>, src: &str) -> Vec<DocComment<'t>>;

    /// Concern 6's other half — every node that *can* be documented, whether or not one is.
    ///
    /// `density` measures a function whether or not anyone documented it, and `docbloat` and
    /// `implInInterface` need the declaration rather than whatever a comment happened to sit above.
    /// A pack that can answer `doc_comments` already knows this set; both read one piece of
    /// knowledge, which is why this is not a twelfth concern.
    fn subject_nodes<'t>(&self, root: Node<'t>) -> Vec<Node<'t>>;

    /// Concern 7 — the comment body with its markers removed.
    ///
    /// Rules match content and must never see `//`, `#`, `/**` or `*` continuation leaders: a
    /// raw-line test for `banner` misses `// =====` entirely.
    fn comment_body(&self, node: Node, src: &str) -> String;

    /// Concern 8 — a subject's own visibility.
    fn visibility(&self, subject: Node, src: &str) -> Visibility;

    /// Concern 9 — statements in a subject's body, for `density`'s ratio.
    fn statement_count(&self, subject: Node, src: &str) -> usize;

    /// Concern 9's other half — the rows that body spans, for `docbloat`'s relative test.
    ///
    /// `None` when the subject has no body at all: a Go `package_clause`, a const, a file root.
    /// There is nothing to measure a doc comment against, and treating the declaration's own
    /// extent as a body made every four-line package comment a finding.
    ///
    /// **Answered by the pack, not defaulted in the engine.** A default reading the `body` field
    /// asked whether the grammar happens to name a field, which is not the same question: Go's
    /// `type_declaration` names no fields at all yet a struct literal has a real extent, while
    /// Rust's `enum_variant` does name one and a tuple variant's spans a single row. Every pack
    /// answers this beside `statement_count`, which already locates the same node.
    fn body_rows(&self, subject: Node, src: &str) -> Option<usize>;

    /// Concern 10 — what happens to the whitespace around a removed block.
    fn blank_line_policy(&self) -> BlankLinePolicy;

    /// Concern 11 — every declaration in the file with its visibility, so `implInInterface` can
    /// resolve a name in a doc comment. Concern 8 describes one subject and cannot answer this.
    fn declared_symbols(&self, root: Node, src: &str) -> Vec<DeclaredSymbol>;

    /// The identifiers a node binds, split on camelCase and snake_case — `restate`.
    ///
    /// Called with whatever a block attaches to, which is a declaration for a block above one and
    /// an ordinary statement for a trailing block — not only with a subject.
    ///
    /// The node's **header**, not its body: a function that prints `x` would otherwise bind every
    /// identifier its body mentions, and `restate` — which fires when a comment's tokens are a
    /// subset of these — would match almost any comment above almost any function. The exclusion
    /// is keyed on the `body` field, so it protects only nodes that have one; `preceding_code_node`
    /// returns the outermost node ending at the offset for that reason, since the inner `block` of
    /// a loop carries no `body` field and would leak the whole loop body.
    ///
    /// Provided, because every pinned grammar names identifier nodes with an `identifier` suffix
    /// and names the body field `body`. A language that does neither overrides this, which is the
    /// open-trait escape hatch rather than an engine change.
    fn bound_identifiers(&self, subject: Node, src: &str) -> Vec<String> {
        let body = subject.child_by_field_name("body").map(|b| b.id());
        let mut cursor = subject.walk();
        let mut stack = vec![subject];
        let mut out = Vec::new();
        while let Some(n) = stack.pop() {
            if n.is_named() && n.kind().ends_with("identifier") {
                out.extend(split_identifier(&src[n.byte_range()]));
            }
            for c in n.named_children(&mut cursor) {
                if Some(c.id()) != body {
                    stack.push(c);
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }
}

/// `parseHTTPResponse` and `parse_http_response` both become `parse`, `http`, `response`.
///
/// Runs of capitals are one word up to the last, which then starts the next: `HTTPResponse` is
/// `http` and `response`, not `h`, `t`, `t`, `p`, `response`.
pub fn split_identifier(ident: &str) -> Vec<String> {
    let mut words = Vec::new();
    for part in ident.split(['_', '-']) {
        let chars: Vec<char> = part.chars().collect();
        let mut start = 0;
        for i in 1..chars.len() {
            let boundary = if chars[i].is_uppercase() {
                !chars[i - 1].is_uppercase()
            } else {
                chars[i - 1].is_uppercase() && i >= 2 && chars[i - 2].is_uppercase()
            };
            if boundary {
                let cut = if chars[i].is_uppercase() { i } else { i - 1 };
                if cut > start {
                    words.push(chars[start..cut].iter().collect::<String>().to_lowercase());
                    start = cut;
                }
            }
        }
        if start < chars.len() {
            words.push(chars[start..].iter().collect::<String>().to_lowercase());
        }
    }
    words.retain(|w| !w.is_empty());
    words
}

#[cfg(test)]
mod tests {
    use super::split_identifier;

    #[test]
    fn splits_camel_snake_and_capital_runs() {
        assert_eq!(
            split_identifier("parseHTTPResponse"),
            ["parse", "http", "response"]
        );
        assert_eq!(
            split_identifier("parse_http_response"),
            ["parse", "http", "response"]
        );
        assert_eq!(split_identifier("Exported"), ["exported"]);
        assert_eq!(split_identifier("x"), ["x"]);
        assert_eq!(split_identifier("HTTP"), ["http"]);
        assert_eq!(split_identifier("_default_name"), ["default", "name"]);
    }
}
