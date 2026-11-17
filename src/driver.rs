//! Command line parsing and action dispatch for `ai-nsc`.
//!
//! Parsing is hand-written: the option set is small and the exact error
//! messages and exit codes matter for tests. `parse_args` is pure (it takes the
//! raw argument list) so it is unit-testable without spawning a process.

use std::ffi::OsString;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

use crate::diagnostics::{DiagnosticEngine, Severity};
use crate::source::SourceManager;
use crate::{HOST_TRIPLE, LLVM_VERSION, VERSION};

/// What the user asked the compiler to do.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    /// Print usage and exit successfully.
    Help,
    /// Print version information and exit successfully.
    Version,
    /// Parse and type-check only; no code generation.
    Check,
    /// Emit LLVM IR as textual `.ll`.
    EmitIr,
    /// Emit a native object file.
    EmitObj,
    /// Compile and link an executable.
    Build,
    /// Compile, link, then execute the program.
    Run,
}

impl Action {
    /// Maps a command name to an action.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Action> {
        match name {
            "build" => Some(Action::Build),
            "run" => Some(Action::Run),
            "check" => Some(Action::Check),
            "emit-ir" => Some(Action::EmitIr),
            "emit-obj" => Some(Action::EmitObj),
            _ => None,
        }
    }
}

/// Fully parsed command line.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// Requested action; `None` means "infer from the inputs".
    pub action: Option<Action>,
    /// Input source files, in command line order.
    pub inputs: Vec<PathBuf>,
    /// Value of `-o`, if given.
    pub output: Option<PathBuf>,
    /// Value of `--target`, if given.
    pub target: Option<String>,
    /// Optimization level from `-O0..-O3`.
    pub optimization_level: u8,
    /// Emit debug information (`-g`).
    pub debug_info: bool,
    /// Print the token stream (`--dump-tokens`).
    pub dump_tokens: bool,
    /// Print the AST after parsing (`--dump-ast`).
    pub dump_ast: bool,
    /// `Some(true)` forces colors on, `Some(false)` off, `None` auto-detects.
    pub color: Option<bool>,
}

const USAGE: &str = "\
ai-nsc - the NovaSharp compiler (LLVM backend)

USAGE:
  ai-nsc <command> [options] <file.nova>...

COMMANDS:
  build     Compile NovaSharp sources and link an executable
  run       Compile and immediately run the program
  check     Parse and type-check only; no code generation
  emit-ir   Emit LLVM IR as textual .ll
  emit-obj  Emit a native object file (.o)

OPTIONS:
  -o <path>           Output path
  --target <triple>   Target triple (default: host)
  -O0/-O1/-O2/-O3     Optimization level (default: -O0)
  -g                  Emit debug information
  --dump-tokens       Print the token stream
  --dump-ast          Print the AST after parsing
  --no-color          Disable colored diagnostics
  -h, --help          Show this help
  --version           Show version information
";

/// Parses raw command line arguments (without the program name).
///
/// # Errors
///
/// Returns a human-readable message for any malformed invocation.
pub fn parse_args(args: &[OsString]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut only_inputs = false;
    let mut index = 0;

    while index < args.len() {
        let argument = args[index].to_string_lossy().into_owned();

        if only_inputs {
            options.inputs.push(PathBuf::from(&args[index]));
            index += 1;
            continue;
        }

        match argument.as_str() {
            "--" => only_inputs = true,
            "-h" | "--help" => options.action = Some(Action::Help),
            "--version" => options.action = Some(Action::Version),
            "--dump-tokens" => options.dump_tokens = true,
            "--dump-ast" => options.dump_ast = true,
            "--no-color" => options.color = Some(false),
            "--color" => options.color = Some(true),
            "-g" => options.debug_info = true,
            "-o" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for '-o'".to_string())?;
                options.output = Some(PathBuf::from(value));
                index += 1;
            }
            "--target" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for '--target'".to_string())?;
                options.target = Some(value.to_string_lossy().into_owned());
                index += 1;
            }
            _ => {
                if let Some(level) = argument.strip_prefix("-O") {
                    options.optimization_level = parse_optimization_level(level)?;
                } else if let Some(triple) = argument.strip_prefix("--target=") {
                    options.target = Some(triple.to_string());
                } else if argument.starts_with('-') {
                    return Err(format!("unknown option '{argument}'"));
                } else if options.action.is_none() && Action::from_name(&argument).is_some() {
                    options.action = Action::from_name(&argument);
                } else {
                    options.inputs.push(PathBuf::from(&args[index]));
                }
            }
        }

        index += 1;
    }

    Ok(options)
}

fn parse_optimization_level(level: &str) -> Result<u8, String> {
    match level {
        "0" => Ok(0),
        "1" => Ok(1),
        "2" => Ok(2),
        "3" => Ok(3),
        "" => Err("missing level for '-O'; expected -O0, -O1, -O2 or -O3".to_string()),
        other => Err(format!(
            "invalid optimization level '-O{other}'; expected -O0, -O1, -O2 or -O3"
        )),
    }
}

/// Entry point used by `main`: parses `std::env::args_os` and returns an exit
/// code.
#[must_use]
pub fn run() -> i32 {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    run_with_args(&args)
}

/// Like [`run`], but with an explicit argument list.
#[must_use]
pub fn run_with_args(args: &[OsString]) -> i32 {
    let options = match parse_args(args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("ai-nsc: {message}");
            eprintln!("Run 'ai-nsc --help' for usage.");
            return 1;
        }
    };

    let action = match (options.action, options.inputs.is_empty()) {
        (Some(action), _) => action,
        (None, true) => {
            let _ = io::stderr().write_all(USAGE.as_bytes());
            return 1;
        }
        (None, false) => Action::Build,
    };

    match action {
        Action::Help => {
            print!("{USAGE}");
            0
        }
        Action::Version => {
            print_version();
            0
        }
        Action::Check | Action::EmitIr | Action::EmitObj | Action::Build | Action::Run => {
            compile(&options)
        }
    }
}

/// Shared entry point for every action that consumes source files.
///
/// The front end and the LLVM lowering are added once the NovaSharp grammar
/// specification is available; for now this validates argument handling, loads
/// the inputs, and reports that the pipeline is not implemented yet.
fn compile(options: &Options) -> i32 {
    if options.inputs.is_empty() {
        eprintln!("ai-nsc: no input files");
        return 1;
    }

    let mut sources = SourceManager::new();
    let mut loaded_all = true;
    for input in &options.inputs {
        if let Err(error) = sources.load_file(input) {
            eprintln!("ai-nsc: cannot read '{}': {error}", input.display());
            loaded_all = false;
        }
    }
    if !loaded_all {
        return 1;
    }

    let mut diagnostics = DiagnosticEngine::new();
    if let Some(color) = options.color {
        diagnostics.set_color(color);
    } else {
        diagnostics.set_color(io::stderr().is_terminal());
    }
    diagnostics.report_stderr(
        &sources,
        Severity::Error,
        None,
        "the NovaSharp front end is not implemented yet: the language grammar \
         specification is still pending",
    );
    1
}

fn print_version() {
    println!("ai-nsc version {VERSION}");
    println!("  LLVM: {LLVM_VERSION}");
    println!("  host triple: {HOST_TRIPLE}");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<OsString> {
        items.iter().map(OsString::from).collect()
    }

    #[test]
    fn parses_a_command_and_inputs() {
        let options = parse_args(&args(&["build", "a.nova", "b.nova", "-o", "app"])).unwrap();
        assert_eq!(options.action, Some(Action::Build));
        assert_eq!(
            options.inputs,
            vec![PathBuf::from("a.nova"), PathBuf::from("b.nova")]
        );
        assert_eq!(options.output, Some(PathBuf::from("app")));
    }

    #[test]
    fn parses_flags() {
        let options = parse_args(&args(&[
            "emit-ir",
            "-O2",
            "-g",
            "--target=x86_64-linux-gnu",
            "x.nova",
        ]))
        .unwrap();
        assert_eq!(options.action, Some(Action::EmitIr));
        assert_eq!(options.optimization_level, 2);
        assert!(options.debug_info);
        assert_eq!(options.target.as_deref(), Some("x86_64-linux-gnu"));
    }

    #[test]
    fn option_terminator_treats_everything_as_input() {
        let options = parse_args(&args(&["check", "--", "-weird-name.nova"])).unwrap();
        assert_eq!(options.action, Some(Action::Check));
        assert_eq!(options.inputs, vec![PathBuf::from("-weird-name.nova")]);
    }

    #[test]
    fn rejects_unknown_options() {
        assert!(parse_args(&args(&["--nope"])).is_err());
        assert!(parse_args(&args(&["-O9"])).is_err());
        assert!(parse_args(&args(&["-o"])).is_err());
    }

    #[test]
    fn no_arguments_means_no_action() {
        let options = parse_args(&args(&[])).unwrap();
        assert!(options.action.is_none());
        assert!(options.inputs.is_empty());
    }
}
