//! One run: paths in, a [`Report`] out.
//!
//! **A file that cannot be read is reported and the run continues.** Only a bad config stops a run,
//! and it stops it before this module is reached — one is data, the other is instructions.
//!
//! The packs stay a parameter, so the engine still holds no list of languages.

use crate::config::{ConfigFile, Resolved};
use crate::dispatch::{Rules, dispatch};
use crate::exclude::Config;
use crate::fix::fix;
use crate::pack::Pack;
use crate::pipeline::{analyse, resolve};
use crate::report::{Report, UnreadableFile};
use crate::rules::{all_block_rules, all_subject_rules};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Everything a run needs, resolved once.
pub struct Run<'p> {
    packs: &'p [Box<dyn Pack>],
    rules: Rules,
    exclude: Config,
    /// Keyed by pack name. Resolved at construction because the pack set is known then, and per
    /// file it would re-apply every override for every file.
    resolved: BTreeMap<&'static str, Resolved>,
}

/// Every language name the packs claim — what a config's `[languages.<name>]` keys are checked
/// against.
pub fn languages(packs: &[Box<dyn Pack>]) -> Vec<&'static str> {
    packs.iter().map(|p| p.name()).collect()
}

impl<'p> Run<'p> {
    pub fn new(packs: &'p [Box<dyn Pack>], config: &ConfigFile) -> Self {
        let exclude = match config.excluded_paths() {
            Some(paths) => Config {
                excluded_paths: paths,
            },
            None => Config::default(),
        };
        Self {
            packs,
            rules: Rules {
                block: all_block_rules(),
                subject: all_subject_rules(),
            },
            exclude,
            resolved: packs
                .iter()
                .map(|p| (p.name(), config.resolve(p.name())))
                .collect(),
        }
    }

    /// Findings over every file the paths reach.
    pub fn check(&self, paths: &[PathBuf]) -> Report {
        let mut report = Report::default();
        for file in self.files(paths) {
            self.check_file(&file, &mut report);
        }
        report.finalise();
        report
    }

    /// Apply every autofixable finding, then report what the files now say.
    ///
    /// The residual report comes from a second pass over the rewritten text rather than from
    /// subtracting the applied findings from the first. A fix changes what the remaining rules see,
    /// and a hook whose output does not describe the file on disk is the failure this whole mode
    /// exists to prevent.
    pub fn fix(&self, paths: &[PathBuf]) -> (Report, Vec<PathBuf>) {
        let files = self.files(paths);
        let mut changed = Vec::new();
        for file in &files {
            if self.fix_file(file).is_some_and(|written| written) {
                changed.push(file.clone());
            }
        }
        (self.check(paths), changed)
    }

    /// `Some(true)` when the file was rewritten, `Some(false)` when it needed nothing, `None` when
    /// it was skipped or unreadable — all three of which the following check pass reports on.
    fn fix_file(&self, file: &Path) -> Option<bool> {
        let (pack, grammar) = resolve(self.packs, file)?;
        let resolved = self.resolved.get(pack.name())?;
        let src = std::fs::read_to_string(file).ok()?;
        let analysis = analyse(pack, grammar, file, &src, &self.exclude).ok()?;
        let (findings, _) = dispatch(file, &src, &analysis, resolved, &self.rules);
        let fixed = fix(&src, &analysis, &findings, &resolved.registry, pack);
        if fixed == src {
            return Some(false);
        }
        std::fs::write(file, &fixed).ok()?;
        Some(true)
    }

    fn check_file(&self, file: &Path, report: &mut Report) {
        // An extension no pack claims is not an error and produces no output. laconic runs over
        // whole repositories, and a line per `.json` and `.md` makes the findings unreadable.
        let Some((pack, grammar)) = resolve(self.packs, file) else {
            return;
        };
        let Some(resolved) = self.resolved.get(pack.name()) else {
            return;
        };
        // Read failure and non-UTF-8 are one case here: both mean this file yielded no text, both
        // are per file, and neither stops the run.
        let src = match std::fs::read_to_string(file) {
            Ok(src) => src,
            Err(e) => {
                report.unreadable.push(UnreadableFile {
                    file: file.to_path_buf(),
                    reason: e.to_string(),
                });
                return;
            }
        };
        let Ok(analysis) = analyse(pack, grammar, file, &src, &self.exclude) else {
            return;
        };
        let (findings, note) = dispatch(file, &src, &analysis, resolved, &self.rules);
        report.findings.extend(findings);
        report.withheld.extend(note);
    }

    /// Every file the given paths reach, in a deterministic order.
    ///
    /// Exclusion applies however a file was reached, named or walked to. The pre-commit hook names
    /// its files, so a named path overriding exclusion would mean a staged file under `testdata/`
    /// is linted by the hook and not by CI — the one place the two must agree.
    fn files(&self, paths: &[PathBuf]) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for path in paths {
            if path.is_dir() {
                self.walk(path, &mut out);
            } else if !self.exclude.is_excluded_path(path) {
                out.push(path.clone());
            }
        }
        out.sort();
        out.dedup();
        out
    }

    fn walk(&self, dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let mut children: Vec<PathBuf> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        children.sort();
        for child in children {
            let name = child.file_name().and_then(|n| n.to_str()).unwrap_or("");
            // Dot directories are skipped whole: `.git` holds packed objects that parse as nothing
            // and cost a read each, and no dot directory holds source anyone lints.
            if child.is_dir() {
                if !name.starts_with('.') && !self.exclude.is_excluded_path(&child) {
                    self.walk(&child, out);
                }
            } else if !self.exclude.is_excluded_path(&child) {
                out.push(child);
            }
        }
    }
}
