//! The command line as a caller meets it: exit codes, and what reaches stdout.
//!
//! Driven through the built binary rather than through `run()`, because the exit code is the
//! contract and a function returning an integer is not evidence about a process.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A Go file with one gate finding: a rule of punctuation, which `banner` reports and `fix` takes.
///
/// Detached from the function on purpose. Adjacent to it the two comments group into one block,
/// that block resolves as the function's doc comment, and `banner` on a doc comment is warn plus
/// Rewrite — no gate and nothing to fix, which is the invariant rather than a quirk of the fixture.
const BANNERED: &str = "package p\n\n// ==============\n\n// Add returns the sum.\nfunc Add(a, b int) int { return a + b }\n";

/// The same file with nothing to say about it.
const CLEAN: &str =
    "package p\n\n// Add returns the sum.\nfunc Add(a, b int) int { return a + b }\n";

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

fn laconic(dir: &Path, args: &[&str]) -> Run {
    let output: Output = Command::new(env!("CARGO_BIN_EXE_laconic"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("the binary runs");
    Run {
        code: output.status.code().expect("exited rather than signalled"),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

#[test]
fn a_gate_finding_exits_one_and_a_clean_tree_exits_zero() {
    let dir = tempdir("gate");
    write(&dir, "x.go", BANNERED);
    let run = laconic(&dir, &["check", "--no-config"]);
    assert_eq!(run.code, 1, "stdout was {}", run.stdout);
    assert!(run.stdout.contains("banner"), "{}", run.stdout);

    write(&dir, "x.go", CLEAN);
    let run = laconic(&dir, &["check", "--no-config"]);
    assert_eq!(run.code, 0, "stdout was {}", run.stdout);
    assert_eq!(run.stdout, "");
}

/// The machine format is a contract, so its shape is asserted field by field.
///
/// The consumer is an agent acting on one diagnostic with no surrounding context. The byte span is
/// the field that earns this test: an agent applies a Delete from `start..end` without re-deriving
/// it from the instruction text, so a column shifted by one is a corrupted file rather than a
/// misaligned caret.
#[test]
fn the_machine_format_carries_the_span_an_agent_edits_with() {
    let dir = tempdir("machine");
    write(&dir, "x.go", BANNERED);
    let run = laconic(&dir, &["check", "--no-config", "--format", "machine"]);
    assert_eq!(run.code, 1);

    let mut lines = run.stdout.lines();
    let record: Vec<&str> = lines
        .next()
        .expect("a finding record")
        .split('\t')
        .collect();
    assert_eq!(record.len(), 9, "got {record:?}");
    assert_eq!(record[0], "finding");
    assert_eq!(record[1], "banner");
    assert_eq!(record[2], "./x.go");
    assert_eq!(record[7], "gate");
    assert_eq!(record[8], "delete");

    // The span, read back against the source it was reported on.
    let (start, end) = (parse_field(record[3]), parse_field(record[4]));
    let src = std::fs::read_to_string(dir.join("x.go")).expect("read");
    assert_eq!(
        &src[start..end],
        "// ==============",
        "the span does not cover the comment it reported"
    );

    // Line and column are 1-based, and point at the same place.
    assert_eq!(parse_field(record[5]), 3);
    assert_eq!(parse_field(record[6]), 1);

    let instruction = lines.next().expect("an instruction line");
    assert!(
        instruction.starts_with("\tinstruction\t"),
        "got {instruction:?}"
    );
}

fn parse_field(field: &str) -> usize {
    field
        .parse()
        .unwrap_or_else(|_| panic!("{field:?} is not a number"))
}

/// `--version` and `--help` answer without a command in front of them, and exit 0.
#[test]
fn version_and_help_need_no_command() {
    let dir = tempdir("version");
    let run = laconic(&dir, &["--version"]);
    assert_eq!(run.code, 0, "stderr was {}", run.stderr);
    assert!(run.stdout.starts_with("laconic "), "{}", run.stdout);
    assert!(
        run.stdout.trim().len() > "laconic ".len(),
        "no version number: {}",
        run.stdout
    );

    let run = laconic(&dir, &["--help"]);
    assert_eq!(run.code, 0);
    assert!(run.stdout.contains("laconic check"), "{}", run.stdout);
}

/// An extension no pack claims produces nothing at all — not a warning, not a line.
///
/// laconic runs over whole repositories. A note per `.json` and `.md` is what makes the findings
/// unreadable, so silence here is the behaviour rather than the absence of one.
#[test]
fn an_unclaimed_extension_is_silent() {
    let dir = tempdir("unclaimed");
    write(&dir, "data.json", "{\"//\": \"////////\"}\n");
    write(&dir, "notes.md", "//////// not code\n");
    let run = laconic(&dir, &["check", "--no-config"]);
    assert_eq!(run.code, 0);
    assert_eq!(run.stdout, "", "an unclaimed extension produced output");
}

/// A file laconic cannot read is named, and the run carries on over the rest.
///
/// The opposite of the config aborts above, deliberately: a file is data and a config is
/// instructions. So this reports per file rather than stopping, and the finding on the file beside
/// it still has to appear.
#[test]
fn an_unreadable_file_is_reported_and_the_run_continues() {
    let dir = tempdir("unreadable");
    write(&dir, "good.go", BANNERED);
    // Not UTF-8, and a `.go` file, so a pack claims it and the read is what fails.
    std::fs::write(dir.join("bad.go"), [0x70, 0x6b, 0x67, 0xff, 0xfe, 0x0a]).expect("write");

    let run = laconic(&dir, &["check", "--no-config"]);
    assert!(run.stdout.contains("bad.go"), "{}", run.stdout);
    assert!(run.stdout.contains("not read"), "{}", run.stdout);
    assert!(
        run.stdout.contains("banner"),
        "the run stopped at the unreadable file: {}",
        run.stdout
    );
    assert_eq!(
        run.code, 1,
        "the exit code is the gate finding's, not the unreadable file's"
    );
}

/// A malformed config aborts before any file is read. The bannered file below would exit 1 on its
/// own; exit 2 with no finding printed is what says nothing was read.
#[test]
fn a_malformed_config_exits_two_before_reading_anything() {
    let dir = tempdir("malformed");
    write(&dir, "x.go", BANNERED);
    write(&dir, "laconic.toml", "[rules\n");
    let run = laconic(&dir, &["check"]);
    assert_eq!(run.code, 2, "stderr was {}", run.stderr);
    assert_eq!(run.stdout, "", "a file was read despite the bad config");
    assert!(run.stderr.contains("no files were read"), "{}", run.stderr);
}

/// An unknown rule id is the same abort: a config the tool half understands silently disables rules
/// its author believed were on.
#[test]
fn an_unknown_rule_id_exits_two() {
    let dir = tempdir("unknown-rule");
    write(&dir, "x.go", BANNERED);
    write(
        &dir,
        "laconic.toml",
        "[rules.nosuchrule]\nenabled = false\n",
    );
    let run = laconic(&dir, &["check"]);
    assert_eq!(run.code, 2, "stderr was {}", run.stderr);
    assert_eq!(run.stdout, "");
    assert!(run.stderr.contains("nosuchrule"), "{}", run.stderr);
}

/// And an override naming a language no pack claims.
#[test]
fn an_override_for_an_unclaimed_language_exits_two() {
    let dir = tempdir("unknown-language");
    write(&dir, "x.go", BANNERED);
    write(
        &dir,
        "laconic.toml",
        "[languages.cobol.rules.banner]\nenabled = false\n",
    );
    let run = laconic(&dir, &["check"]);
    assert_eq!(run.code, 2, "stderr was {}", run.stderr);
    assert!(run.stderr.contains("cobol"), "{}", run.stderr);
}

/// A usage error is the same class as a bad config: the run never started.
#[test]
fn an_unknown_command_exits_two() {
    let dir = tempdir("usage");
    assert_eq!(laconic(&dir, &["lint"]).code, 2);
    assert_eq!(laconic(&dir, &[]).code, 2);
    assert_eq!(laconic(&dir, &["check", "--nosuchflag"]).code, 2);
}

/// A mistyped **short** flag is an error too, and this is the case that was not.
///
/// `-q` fell through to the path arm; a path with no extension resolves no pack and is skipped in
/// silence, which is right for `data.json` and wrong for a flag. The run scanned nothing and exited
/// 0, so a hook or a CI step with a typo in it reported a clean tree.
#[test]
fn a_mistyped_short_flag_exits_two_rather_than_reporting_clean() {
    let dir = tempdir("short-flag");
    write(&dir, "x.go", BANNERED);
    assert_eq!(
        laconic(&dir, &["check", "--no-config"]).code,
        1,
        "the fixture must gate, or this test proves nothing"
    );

    let run = laconic(&dir, &["check", "--no-config", "-q"]);
    assert_eq!(run.code, 2, "stdout was {:?}", run.stdout);
    assert!(run.stderr.contains("-q"), "{}", run.stderr);
}

/// AC9, end to end: `laconic defaults` written to `laconic.toml` changes no finding.
///
/// The file states every rule and every threshold explicitly, so a value it got wrong would move a
/// finding. Asserted through the binary rather than by comparing two registries, because AC9's
/// claim is about a run.
#[test]
fn a_config_of_only_defaults_is_a_default_run() {
    let dir = tempdir("defaults");
    write(&dir, "x.go", BANNERED);
    write(&dir, "long.go", &over_the_doc_cap());

    let bare = laconic(&dir, &["check", "--no-config"]);
    let defaults = laconic(&dir, &["defaults"]);
    assert_eq!(defaults.code, 0);
    write(&dir, "laconic.toml", &defaults.stdout);
    let configured = laconic(&dir, &["check"]);

    assert_eq!(configured.stdout, bare.stdout);
    assert_eq!(configured.code, bare.code);
    assert!(!bare.stdout.is_empty(), "the comparison found nothing");
}

/// Seven doc lines over a one-member struct: over the shipped cap and over the ratio, so the
/// defaults file getting either threshold wrong would show up above.
fn over_the_doc_cap() -> String {
    let mut src = String::from("package p\n\n");
    for i in 0..7 {
        src.push_str(&format!(
            "// Widget line {i} of prose that earns nothing.\n"
        ));
    }
    src.push_str("type Widget struct {\n\tName string\n}\n");
    src
}

/// `fix` rewrites the file and then reports what the file now says.
#[test]
fn fix_applies_the_autofixable_findings_and_reports_the_residue() {
    let dir = tempdir("fix");
    write(&dir, "x.go", BANNERED);
    let run = laconic(&dir, &["fix", "--no-config"]);
    assert_eq!(run.code, 0, "stdout was {}", run.stdout);

    let after = std::fs::read_to_string(dir.join("x.go")).expect("read");
    assert!(
        !after.contains("=============="),
        "the banner survived:\n{after}"
    );
    assert!(
        after.contains("// Add returns the sum."),
        "the fix took more than the banner:\n{after}"
    );
    assert!(run.stderr.contains("fixed"), "{}", run.stderr);
}

/// Excluded paths replace the default set rather than extending it.
#[test]
fn the_excluded_set_is_replaced() {
    let dir = tempdir("excluded");
    std::fs::create_dir_all(dir.join("testdata")).expect("mkdir");
    std::fs::create_dir_all(dir.join("probe")).expect("mkdir");
    write(&dir, "testdata/x.go", BANNERED);
    write(&dir, "probe/y.go", BANNERED);

    // Default set: `testdata` is excluded, `probe` is not.
    let run = laconic(&dir, &["check", "--no-config"]);
    assert!(run.stdout.contains("probe"), "{}", run.stdout);
    assert!(!run.stdout.contains("testdata"), "{}", run.stdout);

    write(&dir, "laconic.toml", "excluded_paths = [\"probe\"]\n");
    let run = laconic(&dir, &["check"]);
    assert!(
        run.stdout.contains("testdata"),
        "the list extended rather than replaced: {}",
        run.stdout
    );
    assert!(!run.stdout.contains("probe"), "{}", run.stdout);
}

/// The break this catches is the one that makes `--since` worthless: a scope that quietly widens to
/// the whole repository, or quietly narrows to nothing. Both exit 0 on a clean-looking run.
#[test]
fn since_reads_the_changed_files_whole_and_leaves_the_rest_alone() {
    let dir = tempdir("since");
    repo(&dir);
    write(&dir, "untouched.go", BANNERED);
    write(&dir, "changed.go", CLEAN);
    vcs(&dir, &["add", "."]);
    vcs(&dir, &["commit", "-m", "base"]);
    let base = vcs(&dir, &["rev-parse", "HEAD"]).trim().to_string();

    // Committed on top of the base, and carrying its finding on a line the second commit did not
    // write — whole-file reporting is the only way it surfaces.
    write(&dir, "changed.go", BANNERED);
    vcs(&dir, &["commit", "-am", "touch changed.go"]);

    let run = laconic(&dir, &["check", "--no-config", "--since", &base]);
    assert_eq!(run.code, 1, "stdout was {}", run.stdout);
    assert!(run.stdout.contains("changed.go"), "{}", run.stdout);
    assert!(
        !run.stdout.contains("untouched.go"),
        "a file the branch never touched was scanned: {}",
        run.stdout
    );
}

/// Untracked files are in scope. Left out, a hook reports nothing on the commonest state there is —
/// a new file written this session and not yet added.
#[test]
fn since_covers_files_that_are_not_tracked_yet() {
    let dir = tempdir("since-untracked");
    repo(&dir);
    write(&dir, "seed.go", CLEAN);
    vcs(&dir, &["add", "."]);
    vcs(&dir, &["commit", "-m", "base"]);
    let base = vcs(&dir, &["rev-parse", "HEAD"]).trim().to_string();

    write(&dir, "fresh.go", BANNERED);
    let run = laconic(&dir, &["check", "--no-config", "--since", &base]);
    assert_eq!(run.code, 1, "stdout was {}", run.stdout);
    assert!(run.stdout.contains("fresh.go"), "{}", run.stdout);
}

/// A ref that does not resolve must exit 2, not 0. An empty diff from a typo is indistinguishable
/// from a clean branch, which is the failure mode that would let a broken hook report success.
#[test]
fn since_exits_two_on_a_ref_that_does_not_resolve() {
    let dir = tempdir("since-badref");
    repo(&dir);
    write(&dir, "x.go", CLEAN);
    vcs(&dir, &["add", "."]);
    vcs(&dir, &["commit", "-m", "base"]);

    let run = laconic(&dir, &["check", "--no-config", "--since", "nosuchref"]);
    assert_eq!(run.code, 2, "stdout was {}", run.stdout);
    assert!(run.stderr.contains("nosuchref"), "{}", run.stderr);
}

#[test]
fn since_outside_a_repository_exits_two() {
    let dir = tempdir("since-norepo");
    write(&dir, "x.go", BANNERED);
    let run = laconic(&dir, &["check", "--no-config", "--since", "HEAD"]);
    assert_eq!(run.code, 2, "stdout was {}", run.stdout);
}

#[test]
fn since_and_named_paths_together_exit_two() {
    let dir = tempdir("since-both");
    repo(&dir);
    let run = laconic(&dir, &["check", "--no-config", "--since", "HEAD", "."]);
    assert_eq!(run.code, 2, "stdout was {}", run.stdout);
}

fn write(dir: &Path, name: &str, content: &str) {
    std::fs::write(dir.join(name), content).expect("write");
}

/// Identity and hooks are set per repository rather than read from the machine, so the suite passes
/// on a host with no git identity and cannot fire a developer's own hooks.
fn repo(dir: &Path) {
    vcs(dir, &["init", "--initial-branch=main"]);
    vcs(dir, &["config", "user.email", "laconic@example.invalid"]);
    vcs(dir, &["config", "user.name", "laconic tests"]);
    vcs(dir, &["config", "commit.gpgsign", "false"]);
    vcs(dir, &["config", "core.hooksPath", "/dev/null"]);
}

fn vcs(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn tempdir(tag: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!("laconic-cli-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).expect("mkdir");
    base
}
