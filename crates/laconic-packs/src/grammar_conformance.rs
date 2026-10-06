//! The node kinds packs name in their lists and container tables, checked against the grammars they
//! claim. Kinds a pack names only inside its code are not covered.
//!
//! Each listed kind must resolve in a grammar its pack claims; each `MEMBER_CONTAINERS` entry must be
//! reachable as some node's `body` field, shown by a probe written by hand so it cannot assert the
//! list against itself; and each container table must classify every child its container can hold.

use crate::common::Container;
use crate::{bash, go, java, python, rust, swift, typescript};
use laconic_grammars::Grammar;
use serde_json::Value;
use std::collections::BTreeSet;
use tree_sitter::{Language, Node, Parser};

/// Every list a pack names node kinds in, with its owning pack. Hand-enumerated, which is the one
/// gap this module cannot close: a list added to a pack and not added here is unchecked.
fn lists() -> Vec<(&'static str, &'static str, &'static [&'static str])> {
    vec![
        ("go", "COMMENT_KINDS", go::COMMENT_KINDS),
        ("python", "COMMENT_KINDS", python::COMMENT_KINDS),
        ("rust", "COMMENT_KINDS", rust::COMMENT_KINDS),
        ("rust", "NON_STATEMENTS", rust::NON_STATEMENTS),
        ("java", "COMMENT_KINDS", java::COMMENT_KINDS),
        ("swift", "COMMENT_KINDS", swift::COMMENT_KINDS),
        ("bash", "COMMENT_KINDS", bash::COMMENT_KINDS),
        ("typescript", "COMMENT_KINDS", typescript::COMMENT_KINDS),
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

/// Every pack's container table. Hand-enumerated like [`lists`], with the same gap.
fn tables() -> Vec<(&'static str, &'static [Container])> {
    vec![
        ("go", go::CONTAINERS),
        ("java", java::CONTAINERS),
        ("python", python::CONTAINERS),
        ("rust", rust::CONTAINERS),
        ("swift", swift::CONTAINERS),
        ("bash", bash::CONTAINERS),
        ("typescript", typescript::CONTAINERS),
    ]
}

/// The named child kinds `node-types.json` allows under `container`, supertypes expanded to the
/// concrete kinds a parse tree carries. `None` when the grammar has no such kind.
fn allowed_children(node_types: &str, container: &str) -> Option<BTreeSet<String>> {
    let types: Vec<Value> = serde_json::from_str(node_types).expect("node-types.json parses");
    let named: Vec<&Value> = types.iter().filter(|t| t["named"] == true).collect();
    let entry = named.iter().find(|t| t["type"] == container)?;
    let mut refs: Vec<&Value> = Vec::new();
    if let Some(children) = entry["children"]["types"].as_array() {
        refs.extend(children);
    }
    if let Some(fields) = entry["fields"].as_object() {
        for field in fields.values() {
            refs.extend(field["types"].as_array().into_iter().flatten());
        }
    }
    let mut out = BTreeSet::new();
    while let Some(r) = refs.pop() {
        if r["named"] != true {
            continue;
        }
        let kind = r["type"].as_str().expect("a type name");
        match named
            .iter()
            .find(|t| t["type"] == kind && t.get("subtypes").is_some())
        {
            Some(supertype) => refs.extend(supertype["subtypes"].as_array().into_iter().flatten()),
            None => {
                out.insert(kind.to_string());
            }
        }
    }
    Some(out)
}

/// Every child kind a container can hold is classified exactly once, and nothing is classified that
/// the container cannot hold — checked against every grammar the pack claims.
#[test]
fn every_child_of_a_container_is_classified_exactly_once() {
    let mut problems: Vec<String> = Vec::new();
    for (pack_name, table) in tables() {
        let grammars: Vec<Grammar> = crate::all()
            .iter()
            .find(|p| p.name() == pack_name)
            .expect("a pack with this name")
            .extensions()
            .iter()
            .map(|(_, g)| *g)
            .collect();
        for (i, container) in table.iter().enumerate() {
            if table[..i]
                .iter()
                .any(|c| c.kind == container.kind && c.within == container.within)
            {
                problems.push(format!(
                    "{pack_name}: {} is listed twice under the same parents",
                    container.kind
                ));
            }
            let classified: Vec<&str> = container
                .declarations
                .iter()
                .chain(container.wrappers)
                .chain(container.excluded.iter().flat_map(|e| e.kinds))
                .copied()
                .collect();
            let unique: BTreeSet<&str> = classified.iter().copied().collect();
            if unique.len() != classified.len() {
                problems.push(format!(
                    "{pack_name}: {} classifies a kind more than once",
                    container.kind
                ));
            }
            for excluded in container.excluded {
                if excluded.reason.trim().is_empty() {
                    problems.push(format!(
                        "{pack_name}: {} excludes {:?} without a reason",
                        container.kind, excluded.kinds
                    ));
                }
            }

            let mut allowed_anywhere: BTreeSet<String> = BTreeSet::new();
            for grammar in &grammars {
                let Some(allowed) = allowed_children(grammar.node_types(), container.kind) else {
                    continue;
                };
                for kind in &allowed {
                    if !unique.contains(kind.as_str()) {
                        problems.push(format!(
                            "{pack_name}: {} in {} can hold {kind:?}, which is not classified",
                            container.kind,
                            grammar.name()
                        ));
                    }
                }
                allowed_anywhere.extend(allowed);
            }
            if allowed_anywhere.is_empty() {
                problems.push(format!(
                    "{pack_name}: {} is a container in no grammar the pack claims",
                    container.kind
                ));
                continue;
            }
            for kind in &unique {
                if !allowed_anywhere.contains(*kind) {
                    problems.push(format!(
                        "{pack_name}: {} classifies {kind:?}, which it can never hold",
                        container.kind
                    ));
                }
            }
        }
    }
    problems.sort();
    problems.dedup();
    assert!(problems.is_empty(), "{problems:#?}");
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
