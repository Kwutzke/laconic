use laconic_grammars::Grammar;
use std::collections::BTreeSet;

fn main() {
    for g in Grammar::ALL {
        let (name, src) = g.probe_fixture();
        let mut parser = g.parser();
        let tree = parser.parse(src, None).expect("parse");
        let root = tree.root_node();
        println!(
            "======== {} ({}) abi={}",
            g.name(),
            name,
            g.language().abi_version()
        );

        let mut extras = BTreeSet::new();
        let mut errors = 0usize;
        let mut cursor = root.walk();
        let mut stack = vec![root];
        while let Some(n) = stack.pop() {
            if n.is_extra() {
                extras.insert(n.kind());
            }
            if n.is_error() || n.is_missing() {
                errors += 1;
            }
            for c in n.children(&mut cursor) {
                stack.push(c);
            }
        }
        println!("extras: {:?}  errors: {}", extras, errors);
        println!("{}", sexp(root, src, 0));
    }
}

fn sexp(node: tree_sitter::Node, src: &str, depth: usize) -> String {
    let pad = "  ".repeat(depth);
    let mut cursor = node.walk();
    let mut out = format!("{pad}({}", node.kind());
    if node.is_extra() {
        out.push_str(" *extra");
    }
    let children: Vec<_> = node.children(&mut cursor).collect();
    if children.is_empty() {
        let text = &src[node.byte_range()];
        let text = text.replace('\n', "\\n");
        let shown = if text.chars().count() > 40 {
            format!("{}…", text.chars().take(40).collect::<String>())
        } else {
            text
        };
        out.push_str(&format!(" {:?}", shown));
    }
    let mut c2 = node.walk();
    for child in node.children(&mut c2) {
        let fname = {
            let mut fc = node.walk();
            fc.goto_first_child();
            let mut found = None;
            loop {
                if fc.node().id() == child.id() {
                    found = fc.field_name();
                    break;
                }
                if !fc.goto_next_sibling() {
                    break;
                }
            }
            found
        };
        out.push('\n');
        if let Some(f) = fname {
            out.push_str(&format!("{pad}  {f}:"));
            out.push('\n');
        }
        out.push_str(&sexp(child, src, depth + 1));
    }
    out.push(')');
    out
}
