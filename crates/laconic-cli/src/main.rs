//! `laconic` — the command line. Exit `0` clean, `1` a gate finding, `2` the run never started.
//!
//! Arguments are parsed by hand: three commands and four options would be the workspace's largest
//! dependency. That refusal is not blanket — `laconic.toml` goes through the `toml` crate, because
//! a config the tool half understands is the failure exit 2 exists to prevent.

use laconic_engine::config::{ConfigFile, defaults_toml, discover};
use laconic_engine::report::{EXIT_CLEAN, EXIT_USAGE};
use laconic_engine::run::{Run, languages};
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

mod since;

const USAGE: &str = "\
laconic — remove low-value comments deterministically, across languages

usage:
  laconic check [PATH...]     report findings; exit 1 if any gates
  laconic fix [PATH...]       apply every autofixable finding, then report what remains
  laconic defaults            print the shipped defaults as a laconic.toml

options:
  --since REF                 scan the files changed since REF, whole; needs git
  --format human|machine      output shape (default: human)
  --config PATH               use this config file rather than discovering one
  --no-config                 run on the shipped defaults, ignoring any laconic.toml
  -h, --help                  this text
  --version                   the version of this binary

With no PATH, laconic reads the current directory. `--since` selects the paths itself and cannot be
combined with them; it reads the whole of every file changed since REF, anywhere in the repository.
Exit 2 means the run never started: a usage error, a config laconic could not fully understand, or a
`--since` with no git to answer it.
";

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code as u8),
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(EXIT_USAGE as u8)
        }
    }
}

/// Write to stdout, treating a closed pipe as an ordinary end rather than a failure.
///
/// `print!` panics when the write fails, and the reader going away first is not a failure of this
/// program: `laconic check . | head` closes the pipe by design. The panic it produced was a Rust
/// backtrace on stderr where a linter's output belongs, which is the worst possible answer in the
/// two places this tool runs — a pre-commit hook and a CI log.
fn emit(text: &str) -> Result<(), std::io::Error> {
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    match handle
        .write_all(text.as_bytes())
        .and_then(|()| handle.flush())
    {
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        other => other,
    }
}

enum Command {
    Check,
    Fix,
    Defaults,
}

enum Format {
    Human,
    Machine,
}

struct Args {
    command: Command,
    paths: Vec<PathBuf>,
    format: Format,
    config: ConfigSource,
    since: Option<String>,
}

enum ConfigSource {
    Discover,
    File(PathBuf),
    None,
}

fn run() -> Result<i32, String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    // Both before `parse`, so neither needs a command in front of it.
    if argv.iter().any(|a| a == "-h" || a == "--help") {
        emit(USAGE).map_err(|e| format!("laconic: {e}"))?;
        return Ok(EXIT_CLEAN);
    }
    if argv.iter().any(|a| a == "--version") {
        emit(&format!("laconic {}\n", env!("CARGO_PKG_VERSION")))
            .map_err(|e| format!("laconic: {e}"))?;
        return Ok(EXIT_CLEAN);
    }
    let mut args = parse(&argv)?;
    if let Some(reference) = &args.since {
        args.paths = since::changed_files(reference)?;
    }

    if let Command::Defaults = args.command {
        emit(&defaults_toml()).map_err(|e| format!("laconic: {e}"))?;
        return Ok(EXIT_CLEAN);
    }

    let packs = laconic_packs::all();
    let config = load_config(&args.config)?;
    // Before a single file is read. Every problem at once, so a config with three typos takes one
    // round trip to fix rather than three.
    if let Err(errors) = config.validate(&languages(&packs)) {
        let mut message = String::new();
        for error in errors {
            message.push_str(&format!("laconic: {error}\n"));
        }
        message.push_str("laconic: no files were read");
        return Err(message);
    }

    let run = Run::new(&packs, &config);
    let report = match args.command {
        Command::Check => run.check(&args.paths),
        Command::Fix => {
            let (report, changed) = run.fix(&args.paths);
            for path in &changed {
                eprintln!("fixed {}", path.display());
            }
            report
        }
        Command::Defaults => unreachable!("handled before any file is read"),
    };

    let output = match args.format {
        Format::Human => report.human(),
        Format::Machine => report.machine(),
    };
    emit(&output).map_err(|e| format!("laconic: {e}"))?;
    Ok(report.exit_code())
}

/// A named file that does not exist is an error; no discovered file is a default run. What
/// separates them is that one of the two is the caller saying where it is.
fn load_config(source: &ConfigSource) -> Result<ConfigFile, String> {
    match source {
        ConfigSource::None => Ok(ConfigFile::default()),
        ConfigSource::File(path) => {
            ConfigFile::load(path).map_err(|e| format!("laconic: {e}\nlaconic: no files were read"))
        }
        ConfigSource::Discover => {
            let cwd = std::env::current_dir().map_err(|e| format!("laconic: {e}"))?;
            match discover(&cwd) {
                Some(path) => ConfigFile::load(&path)
                    .map_err(|e| format!("laconic: {e}\nlaconic: no files were read")),
                None => Ok(ConfigFile::default()),
            }
        }
    }
}

fn parse(argv: &[String]) -> Result<Args, String> {
    let mut argv = argv.iter();
    let command = match argv.next().map(String::as_str) {
        Some("check") => Command::Check,
        Some("fix") => Command::Fix,
        Some("defaults") => Command::Defaults,
        Some(other) => return Err(format!("laconic: unknown command {other:?}\n\n{USAGE}")),
        None => return Err(format!("laconic: no command given\n\n{USAGE}")),
    };

    let mut paths = Vec::new();
    let mut format = Format::Human;
    let mut config = ConfigSource::Discover;
    let mut since = None;
    while let Some(arg) = argv.next() {
        match arg.as_str() {
            "--format" => {
                format = match argv.next().map(String::as_str) {
                    Some("human") => Format::Human,
                    Some("machine") => Format::Machine,
                    Some(other) => return Err(format!("laconic: unknown format {other:?}")),
                    None => return Err("laconic: --format takes human or machine".to_string()),
                }
            }
            "--config" => {
                config = match argv.next() {
                    Some(path) => ConfigSource::File(PathBuf::from(path)),
                    None => return Err("laconic: --config takes a path".to_string()),
                }
            }
            "--since" => {
                since = match argv.next() {
                    Some(reference) => Some(reference.clone()),
                    None => return Err("laconic: --since takes a git ref".to_string()),
                }
            }
            "--no-config" => config = ConfigSource::None,
            // Any leading dash, not just two. A mistyped short flag used to fall through to the
            // path arm, and a path with no extension resolves no pack and is skipped in silence —
            // right for `data.json`, wrong for `-q`. `laconic check -q` scanned nothing and exited
            // 0, so a hook or CI step with a typo in it reported a clean tree.
            other if other.starts_with('-') && other.len() > 1 => {
                return Err(format!("laconic: unknown option {other:?}\n\n{USAGE}"));
            }
            other => paths.push(PathBuf::from(other)),
        }
    }

    // Rejected rather than intersected: both narrow the scan, so a caller passing the two together
    // believes one of them does something it does not.
    if since.is_some() && !paths.is_empty() {
        return Err("laconic: --since selects the paths itself; do not also name them".to_string());
    }
    if since.is_none() && paths.is_empty() {
        paths.push(PathBuf::from("."));
    }
    Ok(Args {
        command,
        paths,
        format,
        config,
        since,
    })
}
