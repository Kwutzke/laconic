//! The file set `--since <ref>` scans, taken from git.
//!
//! Selection is by file and reporting is whole-file, which is why a comment on a line nobody
//! touched still reports. The set is repository-wide rather than relative to the working directory:
//! scoping it to the latter was measured to examine no part of the branch and exit 0.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Files changed since `reference`, deduplicated, and only those that still exist.
///
/// Paths come back relative to the working directory where that is possible and absolute where it
/// is not, so a run from the repository root reads like any other run.
pub fn changed_files(reference: &str) -> Result<Vec<PathBuf>, String> {
    let root = git(Path::new("."), &["rev-parse", "--show-toplevel"])
        .map_err(|e| format!("laconic: --since needs a git repository: {e}"))?;
    let root = PathBuf::from(root.trim());

    // Resolved before it is used in a revision range, so a typo in a ref names itself rather than
    // arriving as an empty diff that would read as "the branch changed nothing".
    git(
        &root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{reference}^{{commit}}"),
        ],
    )
    .map_err(|_| format!("laconic: --since: no such commit {reference:?}"))?;

    let mut names = Vec::new();

    // An unborn HEAD has no committed side and no tracked modifications; only the untracked sweep
    // below applies. Asking git for a diff against it is an error, not an empty set.
    if git(&root, &["rev-parse", "--verify", "--quiet", "HEAD"]).is_ok() {
        // Three dots: what this side added since the merge base, not everything that has happened
        // on the other side as well.
        let range = format!("{reference}...HEAD");
        names.push(git(
            &root,
            &["diff", "--name-only", "--diff-filter=ACMR", &range],
        )?);
        names.push(git(
            &root,
            &["diff", "--name-only", "--diff-filter=ACMR", "HEAD"],
        )?);
    }
    names.push(git(&root, &["ls-files", "--others", "--exclude-standard"])?);

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut files: Vec<PathBuf> = names
        .iter()
        .flat_map(|block| block.lines())
        .filter(|line| !line.is_empty())
        .map(|line| root.join(line))
        // A path git reports can be gone by the time it is read: deleted after it was modified,
        // or an untracked file removed mid-run. It has no content to lint either way.
        .filter(|path| path.is_file())
        .map(|path| display_path(&path, &cwd))
        .collect();
    files.sort();
    files.dedup();
    Ok(files)
}

fn display_path(path: &Path, cwd: &Path) -> PathBuf {
    match path.strip_prefix(cwd) {
        Ok(relative) => relative.to_path_buf(),
        Err(_) => path.to_path_buf(),
    }
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(|e| format!("git could not be run: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
