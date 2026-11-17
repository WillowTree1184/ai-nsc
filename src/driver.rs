//! Command line parsing and action dispatch for `ai-nsc`.
//!
//! Parsing is hand-written: the option set is small and the exact error
//! messages and exit codes matter for tests. `parse_args` is pure (it takes the
//! raw argument list) so it is unit-testable without spawning a process.

use std::ffi::OsString;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::diagnostics::{DiagnosticEngine, Severity};
use crate::source::SourceManager;
use crate::{codegen, lexer, parser, sema, HOST_TRIPLE, LLVM_VERSION, VERSION};

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
    if options.inputs.len() > 1 {
        eprintln!("ai-nsc: multiple input files are not supported yet");
        return 1;
    }
    let input = &options.inputs[0];

    let mut sources = SourceManager::new();
    let file = match sources.load_file(input) {
        Ok(file) => file,
        Err(error) => {
            eprintln!("ai-nsc: cannot read '{}': {error}", input.display());
            return 1;
        }
    };
    let source = sources.text(file).unwrap_or_default().to_string();

    let mut diagnostics = DiagnosticEngine::new();
    match options.color {
        Some(color) => diagnostics.set_color(color),
        None => diagnostics.set_color(io::stderr().is_terminal()),
    }

    // ---- lexer ---------------------------------------------------------
    let lexed = lexer::tokenize(file, &source);
    for error in &lexed.errors {
        diagnostics.report_stderr(&sources, Severity::Error, Some(error.span), &error.message);
    }
    if !lexed.errors.is_empty() {
        return 1;
    }
    if options.dump_tokens {
        for token in &lexed.tokens {
            println!(
                "{:?} {}..{} {:?}",
                token.kind,
                token.span.start,
                token.span.end,
                token.text(&source)
            );
        }
    }

    // ---- parser --------------------------------------------------------
    let mut parsed = parser::parse(file, &source, &lexed.tokens);
    for error in &parsed.errors {
        diagnostics.report_stderr(&sources, Severity::Error, Some(error.span), &error.message);
    }
    if !parsed.errors.is_empty() {
        return 1;
    }
    if options.dump_ast {
        println!("{:#?}", parsed.ast);
    }

    // ---- semantic analysis --------------------------------------------
    let sema_result = sema::analyze(&mut parsed.ast, &sources, &mut diagnostics);
    if sema_result.error_count > 0 {
        return 1;
    }
    if options.action == Some(Action::Check) {
        return 0;
    }

    // ---- code generation ----------------------------------------------
    let codegen_options = codegen::CodegenOptions {
        optimization_level: options.optimization_level,
    };
    let ir = match codegen::generate(&parsed.ast, &sema_result, &codegen_options) {
        Ok(ir) => ir,
        Err(message) => {
            diagnostics.report_stderr(&sources, Severity::Error, None, &message);
            return 1;
        }
    };

    let default_stem = input.file_stem().map_or_else(
        || "a".to_string(),
        |stem| stem.to_string_lossy().into_owned(),
    );

    match options.action {
        Some(Action::EmitIr) => match &options.output {
            Some(path) => match std::fs::write(path, ir.as_bytes()) {
                Ok(()) => 0,
                Err(error) => {
                    eprintln!("ai-nsc: cannot write '{}': {error}", path.display());
                    1
                }
            },
            None => {
                print!("{ir}");
                0
            }
        },
        Some(Action::EmitObj) => {
            let output = options
                .output
                .clone()
                .unwrap_or_else(|| PathBuf::from(format!("{default_stem}.o")));
            match run_clang(&ir, &output, true, options) {
                Ok(()) => 0,
                Err(message) => {
                    eprintln!("ai-nsc: {message}");
                    1
                }
            }
        }
        Some(Action::Build) => {
            let output = options
                .output
                .clone()
                .unwrap_or_else(|| PathBuf::from(default_stem));
            match run_clang(&ir, &output, false, options) {
                Ok(()) => 0,
                Err(message) => {
                    eprintln!("ai-nsc: {message}");
                    1
                }
            }
        }
        Some(Action::Run) => {
            let executable = std::env::temp_dir().join(format!(
                "ainsc-run-{}-{}",
                std::process::id(),
                input.file_stem().map_or_else(
                    || "program".to_string(),
                    |stem| stem.to_string_lossy().into_owned()
                )
            ));
            if let Err(message) = run_clang(&ir, &executable, false, options) {
                eprintln!("ai-nsc: {message}");
                return 1;
            }
            let status = Command::new(&executable).status();
            let _ = std::fs::remove_file(&executable);
            match status {
                Ok(status) => status.code().unwrap_or(1),
                Err(error) => {
                    eprintln!("ai-nsc: cannot run '{}': {error}", executable.display());
                    1
                }
            }
        }
        Some(Action::Check | Action::Help | Action::Version) | None => 0,
    }
}

/// Assembles/links `ir` with clang.
fn run_clang(ir: &str, output: &Path, compile_only: bool, options: &Options) -> Result<(), String> {
    let directory = std::env::temp_dir();
    let unique = format!(
        "ainsc-ir-{}-{}.ll",
        std::process::id(),
        output
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
            .replace(['/', '\\'], "_")
    );
    let ir_path = directory.join(unique);
    std::fs::write(&ir_path, ir).map_err(|error| format!("cannot write IR: {error}"))?;

    let clang = std::env::var("AINSC_CLANG").unwrap_or_else(|_| "clang".to_string());
    let mut command = Command::new(&clang);
    command.arg(&ir_path);
    if compile_only {
        command.arg("-c");
    }
    if let Some(target) = &options.target {
        command.arg(format!("--target={target}"));
    }
    command.arg(format!("-O{}", options.optimization_level));
    command.arg("-o").arg(output);
    let result = command.output();
    let _ = std::fs::remove_file(&ir_path);
    let output_result = result.map_err(|error| format!("cannot run '{clang}': {error}"))?;
    if !output_result.status.success() {
        return Err(format!(
            "'{clang}' failed:\n{}",
            String::from_utf8_lossy(&output_result.stderr)
        ));
    }
    Ok(())
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
