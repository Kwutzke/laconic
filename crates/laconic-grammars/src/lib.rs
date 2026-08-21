//! The pinned tree-sitter grammars, behind one enum.
//!
//! A pack selects a grammar per source extension rather than owning one: TypeScript, TSX and
//! JavaScript are three grammars behind a single TS/JS pack.

use tree_sitter::Language;

/// Declares the grammar set once, and derives the enum, [`Grammar::ALL`], the names, the language
/// functions and the probe fixtures from that one list.
///
/// A grammar added and never probed is the failure this crate exists to prevent, and the guard has
/// to be structural: a hand-written `ALL` beside a hand-written `match` drifts silently, because
/// adding a variant breaks the matches while leaving `ALL` compiling — and every `for g in ALL`
/// test then skips the new grammar and still reports green.
macro_rules! grammars {
    ($($variant:ident => $name:literal, $language:expr, $fixture:literal;)*) => {
        /// A grammar laconic can parse with. Exactly one parse tree shape per variant.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum Grammar {
            $($variant,)*
        }

        impl Grammar {
            /// Every variant. Derived from the same list as the enum, so it cannot fall behind it.
            pub const ALL: &'static [Grammar] = &[$(Grammar::$variant,)*];

            pub fn name(self) -> &'static str {
                match self {
                    $(Grammar::$variant => $name,)*
                }
            }

            pub fn language(self) -> Language {
                match self {
                    $(Grammar::$variant => $language.into(),)*
                }
            }

            /// The probe fixture for this grammar, as `(file name, contents)`.
            ///
            /// Lives here rather than in the probe because the test suite and the dump example both
            /// need it, and a second copy of this map is a second list to forget a grammar in.
            pub fn probe_fixture(self) -> (&'static str, &'static str) {
                match self {
                    $(Grammar::$variant => (
                        $fixture,
                        include_str!(concat!("../probe/", $fixture)),
                    ),)*
                }
            }
        }
    };
}

grammars! {
    Go => "go", tree_sitter_go::LANGUAGE, "go.go";
    Python => "python", tree_sitter_python::LANGUAGE, "python.py";
    Rust => "rust", tree_sitter_rust::LANGUAGE, "rust.rs";
    Java => "java", tree_sitter_java::LANGUAGE, "java.java";
    TypeScript => "typescript", tree_sitter_typescript::LANGUAGE_TYPESCRIPT, "typescript.ts";
    Tsx => "tsx", tree_sitter_typescript::LANGUAGE_TSX, "tsx.tsx";
    JavaScript => "javascript", tree_sitter_javascript::LANGUAGE, "javascript.js";
}

impl Grammar {
    pub fn parser(self) -> tree_sitter::Parser {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&self.language())
            .expect("pinned grammar ABI is incompatible with the pinned tree-sitter runtime");
        parser
    }

    /// Every named node kind this grammar defines, deduplicated.
    ///
    /// Grammar truth, independent of what any fixture happens to instantiate — a fixture-derived
    /// set can only confirm a claim that is too narrow. Deduplicated because the id table repeats
    /// kinds: tree-sitter-rust lists `primitive_type` seventeen times, so the raw sequence is a bag
    /// and a caller reading `.len()` as "how many kinds" gets the wrong number.
    pub fn named_node_kinds(self) -> Vec<&'static str> {
        let lang = self.language();
        let mut kinds: Vec<&'static str> = (0..lang.node_kind_count())
            .map(|id| id as u16)
            .filter(|id| lang.node_kind_is_named(*id))
            .filter_map(|id| lang.node_kind_for_id(id))
            .collect();
        kinds.sort_unstable();
        kinds.dedup();
        kinds
    }
}

#[cfg(test)]
mod tests {
    use super::Grammar;

    /// The list the macro derives everything from. Adding a variant without a fixture fails to
    /// compile at `include_str!`, and adding one without a name or language fails at its match arm.
    #[test]
    fn all_carries_every_variant_exactly_once() {
        let mut seen: Vec<Grammar> = Grammar::ALL.to_vec();
        let before = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), before, "no duplicates");
        assert_eq!(before, 7);
    }

    #[test]
    fn named_node_kinds_is_a_set() {
        for g in Grammar::ALL.iter().copied() {
            let kinds = g.named_node_kinds();
            let mut deduped = kinds.clone();
            deduped.dedup();
            assert_eq!(kinds.len(), deduped.len(), "{} repeats a kind", g.name());
        }
    }
}
