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

#[test]
fn operator_accessibility_modifier_is_rejected() {
    let directory = std::env::temp_dir().join(format!("ainsc-cli-access-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("main.ns");
    std::fs::write(
        &path,
        "structure Left { public X: integer(4); }\n\nextension for Left\n{\n    public operator +(left: Left, right: Left): Left { return Left(left.X + right.X); }\n}\n\nfixed Main(): integer(4) { return 0; }\n",
    )
    .expect("write program");
    let output = ai_nsc()
        .args(["check", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("accessibility"), "stderr: {stderr}");
}

// ----------------------------------------------------------------------
// Generics diagnostics (§10)
// ----------------------------------------------------------------------

fn check_source(source: &str) -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-generics-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("main.ns");
    std::fs::write(&path, source).expect("write program");
    let output = ai_nsc()
        .args(["check", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    assert!(!output.status.success(), "expected a diagnostic");
    String::from_utf8_lossy(&output.stderr).to_string()
}

#[test]
fn generic_constraint_violation_is_reported() {
    let stderr = check_source(
        "\
structure Bad<T: class>
{
    public Value: T;
}

fixed Main(): integer(4)
{
    local bad: Bad<integer(4)>;
    return 0;
}",
    );
    assert!(
        stderr.contains("does not satisfy constraint 'class'"),
        "{stderr}"
    );
}

#[test]
fn generic_type_requires_type_arguments() {
    let stderr = check_source(
        "\
structure Box<T>
{
    public Value: T;
}

fixed Main(): integer(4)
{
    local box: Box;
    return 0;
}",
    );
    assert!(stderr.contains("requires type arguments"), "{stderr}");
}

#[test]
fn generic_argument_count_mismatch_is_reported() {
    let stderr = check_source(
        "\
structure Box<T>
{
    public Value: T;
}

fixed Main(): integer(4)
{
    local box: Box<integer(4); integer(4)>;
    return 0;
}",
    );
    assert!(stderr.contains("expects 1 type argument(s)"), "{stderr}");
}

#[test]
fn sealed_base_class_is_rejected_without_fields() {
    let stderr = check_source(
        "\
sealed class Root { }
class Child: Root { }
fixed Main(): integer(4) { return 0; }",
    );
    assert!(stderr.contains("sealed"), "{stderr}");
}

#[test]
fn abstract_class_cannot_be_instantiated() {
    let stderr = check_source(
        "\
abstract class Service { }
fixed Main(): integer(4)
{
    local service: Service = Service();
    return 0;
}",
    );
    assert!(stderr.contains("abstract"), "{stderr}");
}

#[test]
fn duplicate_entry_point_is_reported() {
    let stderr = check_source(
        "\
fixed Main(): integer(4) { return 1; }
fixed Main(value: integer(4)): integer(4) { return 2; }",
    );
    assert!(stderr.contains("exactly one 'Main'"), "{stderr}");
}

#[test]
fn duplicate_field_is_reported() {
    let stderr = check_source(
        "\
structure Point
{
    public X: integer(4);
    public X: integer(4);
}
fixed Main(): integer(4) { return 0; }",
    );
    assert!(stderr.contains("duplicate field"), "{stderr}");
}
