//! Scratch driver: walk a tree, run the pipeline, print the report. Stands in for the CLI that
//! laconic#9azt builds. Not part of the deliverable.

use laconic_engine::{
    Config, Registry, Report, Rules, UnreadableFile, all_block_rules, all_subject_rules, analyse,
    dispatch, resolve,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name == ".git" || name == "target" {
            continue;
        }
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn main() {
    let root: PathBuf = std::env::args().nth(1).unwrap_or_else(|| ".".into()).into();
    let unrestricted = std::env::args().any(|a| a == "--unrestricted");

    let packs = laconic_packs::all();
    let registry = Registry::default();
    let rules = Rules {
        block: all_block_rules(),
        subject: all_subject_rules(),
    };
    let config = if unrestricted {
        Config::unrestricted()
    } else {
        Config::default()
    };

    let mut files = Vec::new();
    walk(&root, &mut files);
    files.sort();

    let mut report = Report::default();
    let mut scanned = 0usize;
    for path in &files {
        let Some((pack, grammar)) = resolve(&packs, path) else {
            continue;
        };
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                report.unreadable.push(UnreadableFile {
                    file: path.clone(),
                    reason: e.to_string(),
                });
                continue;
            }
        };
        match analyse(pack, grammar, path, &src, &config) {
            Ok(a) => {
                scanned += 1;
                let (findings, withheld) = dispatch(path, &src, &a, &registry, &rules);
                report.findings.extend(findings);
                if let Some(w) = withheld {
                    report.withheld.push(w);
                }
            }
            Err(_) => continue,
        }
    }
    report.finalise();
    print!("{}", report.human());

    let mut by_rule: BTreeMap<(&str, String), usize> = BTreeMap::new();
    for f in report.active() {
        *by_rule
            .entry((f.rule, format!("{:?}", f.tier)))
            .or_default() += 1;
    }
    eprintln!(
        "\n--- {scanned} files scanned, {} findings ---",
        by_rule.values().sum::<usize>()
    );
    for ((rule, tier), n) in &by_rule {
        eprintln!("{n:>5}  {rule} ({tier})");
    }
    eprintln!("exit {}", report.exit_code());
}
