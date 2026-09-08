//! Every node kind a pack names, checked against the grammar it names it for.
//!
//! Nothing read the pinned grammars these hand-written lists describe, so a list was an assertion
//! no test could fail. Each kind must resolve in a grammar its pack claims, and each
//! `MEMBER_CONTAINERS` entry must be reachable as some node's `body` field, shown by a probe
//! written by hand so it cannot assert the list against itself.

use crate::{go, java, python, rust, typescript};
use tree_sitter::{Language, Node, Parser};

/// Every list a pack names node kinds in, with its owning pack. Hand-enumerated, which is the one
/// gap this module cannot close: a list added to a pack and not added here is unchecked.
fn lists() -> Vec<(&'static str, &'static str, &'static [&'static str])> {
    vec![
        ("go", "COMMENT_KINDS", go::COMMENT_KINDS),
        ("go", "DOCUMENTABLE_ANYWHERE", go::DOCUMENTABLE_ANYWHERE),
        (
            "go",
            "DOCUMENTABLE_AT_FILE_SCOPE",
            go::DOCUMENTABLE_AT_FILE_SCOPE,
        ),
        ("python", "COMMENT_KINDS", python::COMMENT_KINDS),
        ("python", "SCOPES", python::SCOPES),
        ("rust", "COMMENT_KINDS", rust::COMMENT_KINDS),
        ("rust", "ITEMS", rust::ITEMS),
        ("rust", "NON_STATEMENTS", rust::NON_STATEMENTS),
        ("java", "COMMENT_KINDS", java::COMMENT_KINDS),
        ("java", "DECLARATIONS", java::DECLARATIONS),
        ("typescript", "COMMENT_KINDS", typescript::COMMENT_KINDS),
        ("typescript", "DECLARATIONS", typescript::DECLARATIONS),
        (
            "typescript",
            "MEMBER_CONTAINERS",
            typescript::MEMBER_CONTAINERS,
        ),
    ]
}

/// The grammars a pack claims, taken from the pack rather than from a table here.
fn languages_of(pack_name: &str) -> Vec<(&'static str, Language)> {
    let packs = crate::all();
    let pack = packs
        .iter()
        .find(|p| p.name() == pack_name)
        .unwrap_or_else(|| panic!("no pack named {pack_name}"));
    let mut seen: Vec<(&'static str, Language)> = Vec::new();
    for (_, grammar) in pack.extensions() {
        if !seen.iter().any(|(n, _)| *n == grammar.name()) {
            seen.push((grammar.name(), grammar.language()));
        }
    }
    seen
}

/// Guarantee 1.
///
/// "In at least one" rather than "in every" grammar the pack claims, deliberately: the TS/JS pack
/// spans three grammars and `interface_body` exists in only two of them. A kind absent from all of
/// them names nothing at all, which is the failure worth an assertion.
#[test]
fn every_node_kind_a_pack_names_exists_in_a_grammar_it_claims() {
    let mut unknown: Vec<String> = Vec::new();
    for (pack_name, list_name, kinds) in lists() {
        let languages = languages_of(pack_name);
        for kind in kinds {
            let found = languages
                .iter()
                .any(|(_, lang)| lang.id_for_node_kind(kind, true) != 0);
            if !found {
                let grammars: Vec<&str> = languages.iter().map(|(n, _)| *n).collect();
                unknown.push(format!("{pack_name}::{list_name} {kind:?} in {grammars:?}"));
            }
        }
    }
    assert!(
        unknown.is_empty(),
        "node kinds that name nothing in any grammar their pack claims: {unknown:#?}"
    );
}

/// A hand-written source per `MEMBER_CONTAINERS` entry, in which some node names that kind as its
/// `body`. Never generated from the list — see the module doc.
const MEMBER_CONTAINER_PROBES: &[(&str, &str)] = &[
    ("statement_block", "function f() { const a = 1; }"),
    ("class_body", "class C { m(): void {} }"),
    ("interface_body", "interface I { m(): void; }"),
    ("enum_body", "enum E { A, B }"),
];

/// Guarantee 2, both directions.
///
/// A probe with no list entry is as much a defect as an entry with no probe: the first means the
/// pack stopped counting a container someone demonstrated is real, the second means the entry
/// excludes nothing because nothing can reach it.
#[test]
fn every_member_container_is_reachable_as_a_body_field() {
    let listed = typescript::MEMBER_CONTAINERS;
    let probed: Vec<&str> = MEMBER_CONTAINER_PROBES.iter().map(|(k, _)| *k).collect();

    let unprobed: Vec<&&str> = listed.iter().filter(|k| !probed.contains(k)).collect();
    assert!(
        unprobed.is_empty(),
        "listed as a member container with no source demonstrating one: {unprobed:?}"
    );
    let unlisted: Vec<&&str> = probed.iter().filter(|k| !listed.contains(k)).collect();
    assert!(
        unlisted.is_empty(),
        "demonstrated as a member container but not counted as one: {unlisted:?}"
    );

    let language = languages_of("typescript")
        .into_iter()
        .find(|(n, _)| *n == "typescript")
        .expect("the TS pack claims the typescript grammar")
        .1;
    for (kind, src) in MEMBER_CONTAINER_PROBES {
        let mut parser = Parser::new();
        parser.set_language(&language).expect("pinned grammar");
        let tree = parser.parse(src, None).expect("probe parses");
        assert!(
            !tree.root_node().has_error(),
            "{kind}: probe does not parse: {src}"
        );
        assert!(
            names_as_body(tree.root_node(), kind),
            "{kind}: no node in {src:?} names one as its `body` field"
        );
    }
}

fn names_as_body(node: Node, kind: &str) -> bool {
    if node
        .child_by_field_name("body")
        .is_some_and(|b| b.kind() == kind)
    {
        return true;
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|c| names_as_body(c, kind))
}
