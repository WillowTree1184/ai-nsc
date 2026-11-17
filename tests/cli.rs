//! End-to-end tests for the `ai-nsc` command line interface.

use std::process::Command;

fn ai_nsc() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ai-nsc"))
}

#[test]
fn version_reports_toolchain() {
    let output = ai_nsc().arg("--version").output().expect("run ai-nsc");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "stderr: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("ai-nsc version"), "{stdout}");
    assert!(stdout.contains("LLVM:"), "{stdout}");
}

#[test]
fn help_lists_commands_and_options() {
    let output = ai_nsc().arg("--help").output().expect("run ai-nsc");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success());
    for needle in [
        "USAGE:", "build", "run", "check", "emit-ir", "emit-obj", "--target",
    ] {
        assert!(
            stdout.contains(needle),
            "help should mention {needle:?}:\n{stdout}"
        );
    }
}

#[test]
fn no_arguments_prints_usage_to_stderr_and_fails() {
    let output = ai_nsc().output().expect("run ai-nsc");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(stderr.contains("USAGE:"), "{stderr}");
}

#[test]
fn missing_input_file_reports_the_path() {
    let output = ai_nsc()
        .args(["check", "definitely-not-here.nova"])
        .output()
        .expect("run ai-nsc");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(
        stderr.contains("cannot read 'definitely-not-here.nova'"),
        "{stderr}"
    );
}

#[test]
fn unknown_option_fails_with_hint() {
    let output = ai_nsc()
        .arg("--not-an-option")
        .output()
        .expect("run ai-nsc");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(
        stderr.contains("unknown option '--not-an-option'"),
        "{stderr}"
    );
    assert!(stderr.contains("ai-nsc --help"), "{stderr}");
}

#[test]
fn invalid_optimization_level_fails() {
    let output = ai_nsc()
        .args(["-O9", "x.nova"])
        .output()
        .expect("run ai-nsc");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(stderr.contains("invalid optimization level"), "{stderr}");
}

#[test]
fn check_accepts_a_valid_program() {
    let directory = std::env::temp_dir().join(format!("ainsc-cli-ok-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let source = directory.join("main.ns");
    std::fs::write(&source, "fixed Main(): integer(4) { return 0; }\n").expect("write source");

    let output = ai_nsc()
        .args(["check", source.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "stderr: {stderr}");
    assert!(!stderr.contains("error:"), "stderr: {stderr}");

    std::fs::remove_dir_all(&directory).expect("clean temp dir");
}

#[test]
fn check_rejects_an_invalid_program() {
    let directory = std::env::temp_dir().join(format!("ainsc-cli-bad-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let source = directory.join("main.ns");
    std::fs::write(&source, "fixed Main(): integer(4) { return true; }\n").expect("write source");

    let output = ai_nsc()
        .args(["check", source.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(stderr.contains("error:"), "stderr: {stderr}");

    std::fs::remove_dir_all(&directory).expect("clean temp dir");
}
