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
use crate::{ast, codegen, lexer, parser, sema, HOST_TRIPLE, LLVM_VERSION, VERSION};

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
    /// Cross-compilation sysroot passed to clang (`--sysroot`).
    pub sysroot: Option<PathBuf>,
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
  ai-nsc <command> [options] <file.ns>...

COMMANDS:
  build     Compile NovaSharp sources and link an executable
  run       Compile and immediately run the program
  check     Parse and type-check only; no code generation
  emit-ir   Emit LLVM IR as textual .ll
  emit-obj  Emit a native object file (.o)

OPTIONS:
  -o <path>           Output path
  --target <triple>   Target triple (default: host)
  --sysroot <path>    Target sysroot used when cross-linking
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
            "--sysroot" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "missing value for '--sysroot'".to_string())?;
                options.sysroot = Some(PathBuf::from(value));
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
                } else if let Some(path) = argument.strip_prefix("--sysroot=") {
                    options.sysroot = Some(PathBuf::from(path));
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

    let uefi_target = options
        .target
        .as_deref()
        .is_some_and(|target| target.ends_with("-uefi"));
    if uefi_target && matches!(action, Action::Run) {
        eprintln!(
            "ai-nsc: UEFI applications cannot be run directly; build a .efi and boot it with UEFI firmware (for example qemu-system-x86_64 with OVMF)"
        );
        return 1;
    }
    // Cross-compilation produces objects but cannot link an executable for a
    // foreign target without a target sysroot (§14).
    if let Some(target) = &options.target {
        let runnable_32_bit =
            target.starts_with("i686") && crate::HOST_TRIPLE.starts_with("x86_64");
        let cross_build_with_sysroot = options.sysroot.is_some() && matches!(action, Action::Build);
        let uefi_build = uefi_target && matches!(action, Action::Build);
        if target != crate::HOST_TRIPLE
            && !runnable_32_bit
            && !cross_build_with_sysroot
            && !uefi_build
            && matches!(action, Action::Build | Action::Run)
        {
            eprintln!(
                "ai-nsc: linking for a foreign target '{target}' is not supported; use 'emit-obj'"
            );
            return 1;
        }
    }

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
    let uefi_target = options
        .target
        .as_deref()
        .is_some_and(|target| target.ends_with("-uefi"));
    if options.inputs.is_empty() {
        eprintln!("ai-nsc: no input files");
        return 1;
    }
    let first_input = &options.inputs[0];

    let mut sources = SourceManager::new();
    let mut diagnostics = DiagnosticEngine::new();
    match options.color {
        Some(color) => diagnostics.set_color(color),
        None => diagnostics.set_color(io::stderr().is_terminal()),
    }

    // Multiple input files form one program: each file is lexed and parsed into
    // the shared AST in input order (§11.2).
    let mut ast = ast::Ast::new(crate::source::FileId::INVALID);
    let mut failed = false;
    for input in &options.inputs {
        // Source text files must have the `.ns` extension (§2.1).
        if input.extension().and_then(std::ffi::OsStr::to_str) != Some("ns") {
            eprintln!(
                "ai-nsc: source file '{}' must have the '.ns' extension (§2.1)",
                input.display()
            );
            return 1;
        }
        let file = match sources.load_file(input) {
            Ok(file) => file,
            Err(error) => {
                eprintln!("ai-nsc: cannot read '{}': {error}", input.display());
                return 1;
            }
        };
        let source = sources.text(file).unwrap_or_default().to_string();

        // ---- lexer -----------------------------------------------------
        let lexed = lexer::tokenize(file, &source);
        for error in &lexed.errors {
            diagnostics.report_stderr(&sources, Severity::Error, Some(error.span), &error.message);
        }
        if !lexed.errors.is_empty() {
            failed = true;
            continue;
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

        // ---- parser ----------------------------------------------------
        let errors = parser::parse_into(&mut ast, file, &source, &lexed.tokens);
        for error in &errors {
            diagnostics.report_stderr(&sources, Severity::Error, Some(error.span), &error.message);
        }
        if !errors.is_empty() {
            failed = true;
        }
    }
    if failed {
        return 1;
    }
    if options.dump_ast {
        println!("{ast:#?}");
    }

    // ---- semantic analysis --------------------------------------------
    let mut parsed = ast;
    let target_layout = match options
        .target
        .as_deref()
        .and_then(|triple| triple.split('-').next())
    {
        Some("i686" | "i386") => sema::types::TargetLayout::I686,
        Some("arm" | "armv7") => sema::types::TargetLayout::ARM32,
        _ => sema::types::TargetLayout::HOST64,
    };
    let jit_allowed = options.target.is_none();
    let freestanding = uefi_target;
    let sema_result = sema::analyze_with_options(
        &mut parsed,
        &sources,
        &mut diagnostics,
        target_layout,
        jit_allowed,
        freestanding,
    );
    if sema_result.error_count > 0 {
        return 1;
    }
    if options.action == Some(Action::Check) {
        return 0;
    }

    // ---- code generation ----------------------------------------------
    let codegen_options = codegen::CodegenOptions {
        optimization_level: options.optimization_level,
        target: options.target.clone(),
        emit_launcher: true,
    };
    let ir = match codegen::generate(&parsed, &sema_result, &codegen_options) {
        Ok(ir) => ir,
        Err(message) => {
            diagnostics.report_stderr(&sources, Severity::Error, None, &message);
            return 1;
        }
    };

    let default_stem = first_input.file_stem().map_or_else(
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
            let output = options.output.clone().unwrap_or_else(|| {
                if uefi_target {
                    PathBuf::from(format!("{default_stem}.efi"))
                } else {
                    PathBuf::from(default_stem)
                }
            });
            let result = if uefi_target {
                link_uefi(&ir, &output, options)
            } else {
                run_clang(&ir, &output, false, options)
            };
            match result {
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
                first_input.file_stem().map_or_else(
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

/// Links `ir` as a freestanding UEFI x86_64 application (§1: no host OS is
/// required). A generated assembly entry thunk calls the generated `main`; on
/// the host test path (`AINSC_UEFI_TEST_EXIT`) it also reports `main`'s result
/// through the qemu ISA debug-exit port so an automated test can observe it.
fn link_uefi(ir: &str, output: &Path, options: &Options) -> Result<(), String> {
    let directory = std::env::temp_dir().join(format!(
        "ainsc-uefi-{}-{}",
        std::process::id(),
        output.file_stem().map_or_else(
            || "program".to_string(),
            |stem| stem.to_string_lossy().into_owned()
        )
    ));
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("cannot create temp dir: {error}"))?;
    let ir_path = directory.join("program.ll");
    let object_path = directory.join("program.obj");
    let stub_source = directory.join("uefi_entry.s");
    let stub_object = directory.join("uefi_entry.obj");
    let cleanup = || {
        let _ = std::fs::remove_dir_all(&directory);
    };
    std::fs::write(&ir_path, ir).map_err(|error| format!("cannot write IR: {error}"))?;
    let test_exit = std::env::var_os("AINSC_UEFI_TEST_EXIT").is_some();
    let entry = if test_exit {
        "padding: .byte 0x90\n.globl efi_main\nefi_main:\n    sub $0x28, %rsp\n    call main\n    movzbl %al, %eax\n    add $1, %al\n    mov $0x501, %dx\n    out %al, %dx\n1:\n    jmp 1b\n"
    } else {
        "padding: .byte 0x90\n.globl efi_main\nefi_main:\n    sub $0x28, %rsp\n    call main\n    xor %eax, %eax\n    add $0x28, %rsp\n    ret\n"
    };
    let stub = format!("{entry}.globl abort\nabort:\n    jmp abort\n");
    std::fs::write(&stub_source, stub)
        .map_err(|error| format!("cannot write UEFI entry: {error}"))?;
    let clang = std::env::var("AINSC_CLANG").unwrap_or_else(|_| "clang".to_string());
    let compile = |input: &Path, out: &Path| -> Result<(), String> {
        let result = Command::new(&clang)
            .arg("--target=x86_64-pc-windows-msvc")
            .arg("-c")
            .arg(format!("-O{}", options.optimization_level))
            .arg(input)
            .arg("-o")
            .arg(out)
            .output()
            .map_err(|error| format!("cannot run '{clang}': {error}"))?;
        if result.status.success() {
            Ok(())
        } else {
            cleanup();
            Err(format!(
                "'{clang}' failed for UEFI:\n{}",
                String::from_utf8_lossy(&result.stderr)
            ))
        }
    };
    compile(&ir_path, &object_path)?;
    compile(&stub_source, &stub_object)?;
    let linker = std::env::var("AINSC_LLD_LINK").unwrap_or_else(|_| "lld-link".to_string());
    let result = Command::new(&linker)
        .arg("/subsystem:efi_application")
        .arg("/entry:efi_main")
        .arg("/nodefaultlib")
        .arg(format!("/out:{}", output.display()))
        .arg(&object_path)
        .arg(&stub_object)
        .output()
        .map_err(|error| format!("cannot run '{linker}': {error}"))?;
    if !result.status.success() {
        cleanup();
        return Err(format!(
            "'{linker}' failed for UEFI:\n{}",
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    cleanup();
    Ok(())
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
        // LLVM 18 on this host cannot lower `-uefi` triples directly (data
        // layout mismatch), but UEFI x86_64 uses the Windows x64 ABI and COFF
        // objects, so the Windows MSVC backend is the practical backend.
        if target.ends_with("-uefi") {
            if !target.starts_with("x86_64") {
                return Err(format!(
                    "UEFI target support is limited to x86_64 in this build (requested '{target}')"
                ));
            }
            command.arg("--target=x86_64-pc-windows-msvc");
        } else {
            command.arg(format!("--target={target}"));
        }
    }
    if let Some(sysroot) = &options.sysroot {
        command.arg(format!("--sysroot={}", sysroot.display()));
    }
    // The host GNU ld cannot link ARM objects; LLVM's lld is target-aware.
    if options
        .target
        .as_deref()
        .is_some_and(|target| target.starts_with("arm"))
    {
        command.arg("-fuse-ld=lld");
    }
    command.arg(format!("-O{}", options.optimization_level));
    command.arg("-o").arg(output);
    if !compile_only {
        // Generated code may call libm helpers (for example float remainder
        // used by out-of-range float-to-integer wrapping, §3.2).
        command.arg("-lm");
    }
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
        let options = parse_args(&args(&["build", "a.ns", "b.ns", "-o", "app"])).unwrap();
        assert_eq!(options.action, Some(Action::Build));
        assert_eq!(
            options.inputs,
            vec![PathBuf::from("a.ns"), PathBuf::from("b.ns")]
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
            "x.ns",
        ]))
        .unwrap();
        assert_eq!(options.action, Some(Action::EmitIr));
        assert_eq!(options.optimization_level, 2);
        assert!(options.debug_info);
        assert_eq!(options.target.as_deref(), Some("x86_64-linux-gnu"));
    }

    #[test]
    fn option_terminator_treats_everything_as_input() {
        let options = parse_args(&args(&["check", "--", "-weird-name.ns"])).unwrap();
        assert_eq!(options.action, Some(Action::Check));
        assert_eq!(options.inputs, vec![PathBuf::from("-weird-name.ns")]);
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
