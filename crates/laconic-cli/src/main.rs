//! `laconic` — the command line. Exit `0` clean, `1` a gate finding, `2` the run never started.
//!
//! Arguments are parsed by hand: three commands and four options would be the workspace's largest
//! dependency. That refusal is not blanket — `laconic.toml` goes through the `toml` crate, because
//! a config the tool half understands is the failure exit 2 exists to prevent.

use laconic_engine::config::{ConfigFile, defaults_toml, discover};
use laconic_engine::report::{EXIT_CLEAN, EXIT_USAGE};
use laconic_engine::run::{Run, languages};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
laconic — remove low-value comments deterministically, across languages

usage:
  laconic check [PATH...]     report findings; exit 1 if any gates
  laconic fix [PATH...]       apply every autofixable finding, then report what remains
  laconic defaults            print the shipped defaults as a laconic.toml

options:
  --format human|machine      output shape (default: human)
  --config PATH               use this config file rather than discovering one
  --no-config                 run on the shipped defaults, ignoring any laconic.toml
  -h, --help                  this text

With no PATH, laconic reads the current directory. Exit 2 means the run never started: a usage
error, or a config laconic could not fully understand.
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
}

enum ConfigSource {
    Discover,
    File(PathBuf),
    None,
}

fn run() -> Result<i32, String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USAGE}");
        return Ok(EXIT_CLEAN);
    }
    let args = parse(&argv)?;

    if let Command::Defaults = args.command {
        print!("{}", defaults_toml());
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

    print!(
        "{}",
        match args.format {
            Format::Human => report.human(),
            Format::Machine => report.machine(),
        }
    );
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
            "--no-config" => config = ConfigSource::None,
            other if other.starts_with("--") => {
                return Err(format!("laconic: unknown option {other:?}\n\n{USAGE}"));
            }
            other => paths.push(PathBuf::from(other)),
        }
    }

    if paths.is_empty() {
        paths.push(PathBuf::from("."));
    }
    Ok(Args {
        command,
        paths,
        format,
        config,
    })
}
