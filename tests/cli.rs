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
        "USAGE:",
        "build",
        "run",
        "check",
        "emit-ir",
        "emit-obj",
        "--target",
        "<file.ns>",
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
        .args(["check", "definitely-not-here.ns"])
        .output()
        .expect("run ai-nsc");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(
        stderr.contains("cannot read 'definitely-not-here.ns'"),
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

fn check_source_ok(source: &str) -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-ok-{}-{}",
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
    assert!(
        output.status.success(),
        "expected success: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stderr).to_string()
}

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

#[test]
fn derived_class_must_declare_a_constructor_for_a_non_trivial_base() {
    let stderr = check_source(
        "\
class Base
{
    public X: integer(4);
    extension { public fixed Construct(this: &Base, x: integer(4)) { this.X = x; } }
}
class Derived : Base { }
fixed Main(): integer(4) { return 0; }",
    );
    assert!(stderr.contains("must declare a constructor"), "{stderr}");
}

#[test]
fn constructor_must_start_by_calling_the_base_constructor() {
    let stderr = check_source(
        "\
class Base
{
    public X: integer(4);
    extension { public fixed Construct(this: &Base, x: integer(4)) { this.X = x; } }
}
class Derived : Base
{
    extension
    {
        public fixed Construct(this: &Derived) { this.X = 1; }
    }
}
fixed Main(): integer(4) { return 0; }",
    );
    assert!(stderr.contains("base.Construct"), "{stderr}");
}

#[test]
fn use_only_affects_the_rest_of_its_scope() {
    let stderr = check_source(
        "\
module A
{
    fixed Value(): integer(4) { return 5; }
}

fixed Main(): integer(4)
{
    {
        use A;
        local inside: integer(4) = Value();
    }
    return Value();
}",
    );
    assert!(stderr.contains("unknown name 'Value'"), "{stderr}");
}

#[test]
fn constant_requires_an_initializer() {
    let stderr = check_source(
        "\
fixed Bad: integer(4);
fixed Main(): integer(4) { return 0; }",
    );
    assert!(stderr.contains("must be initialized"), "{stderr}");
}

#[test]
fn illegal_value_literal_cannot_be_compared() {
    let stderr = check_source(
        "\
fixed Main(): integer(4)
{
    local ratio: float(8) = 1.5;
    if (ratio == illegal) { return 1; }
    return 0;
}",
    );
    assert!(
        stderr.contains("must not be used as a comparison operand"),
        "{stderr}"
    );
}

#[test]
fn return_is_rejected_in_a_finally_block() {
    let stderr = check_source(
        "\
fixed Main(): integer(4)
{
    trap
    {
        return 1;
    }
    finally
    {
        return 2;
    }
}",
    );
    assert!(stderr.contains("'return' must not appear"), "{stderr}");
}

#[test]
fn break_and_continue_are_rejected_in_a_finally_block() {
    let stderr = check_source(
        "\
fixed Main(): integer(4)
{
    for i: integer(4) in 1..3
    {
        trap
        {
            break;
        }
        finally
        {
            continue;
        }
    }
    return 0;
}",
    );
    assert!(stderr.contains("'continue' must not appear"), "{stderr}");
}

#[test]
fn function_must_return_on_every_path() {
    let stderr = check_source(
        "\
fixed Missing(flag: boolean): integer(4)
{
    if (flag) { return 1; }
}
fixed Main(): integer(4) { return 0; }",
    );
    assert!(stderr.contains("not all code paths"), "{stderr}");
}

#[test]
fn dispose_requires_a_class_or_structure_variable() {
    let stderr = check_source(
        "\
fixed Main(): integer(4)
{
    local number: integer(4) = 3;
    dispose number;
    return number;
}",
    );
    assert!(stderr.contains("class or structure"), "{stderr}");
}

#[test]
fn uninitialized_is_rejected_outside_an_initializer() {
    let stderr = check_source(
        "\
fixed Main(): integer(4)
{
    local value: integer(4) = 1;
    value = uninitialized;
    return value;
}",
    );
    assert!(stderr.contains("storage initial value"), "{stderr}");
}

#[test]
fn payload_variant_state_must_be_constructed() {
    let stderr = check_source(
        "\
variant Maybe
{
    Some: integer(4);
    Nothing;
}

fixed Main(): integer(4)
{
    local value: Maybe = Some;
    return 0;
}",
    );
    assert!(stderr.contains("carries a payload"), "{stderr}");
}

#[test]
fn constant_out_of_bounds_index_warns_without_failing() {
    let directory = std::env::temp_dir().join(format!("ainsc-cli-warn-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("main.ns");
    std::fs::write(
        &path,
        "\
fixed Main(): integer(4)
{
    local values[3]: integer(4) = { 1, 2, 3 };
    return values[5];
}",
    )
    .expect("write program");
    let output = ai_nsc()
        .args(["check", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "warnings must not fail the build: {stderr}"
    );
    assert!(
        stderr.contains("warning") && stderr.contains("out of bounds"),
        "{stderr}"
    );
}

#[test]
fn array_pointer_conversion_warns_without_failing() {
    let directory = std::env::temp_dir().join(format!("ainsc-cli-warn2-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("main.ns");
    std::fs::write(
        &path,
        "\
fixed Main(): integer(4)
{
    local values[3]: integer(4) = { 1, 2, 3 };
    local p: pointer integer(4) = (values as pointer integer(4));
    return (p[1] as integer(4));
}",
    )
    .expect("write program");
    let output = ai_nsc()
        .args(["check", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "warnings must not fail the build: {stderr}"
    );
    assert!(
        stderr.contains("warning") && stderr.contains("bounds-check"),
        "{stderr}"
    );
}

#[test]
fn non_exhaustive_variant_match_warns_without_failing() {
    let directory = std::env::temp_dir().join(format!("ainsc-cli-warn3-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("main.ns");
    std::fs::write(
        &path,
        "\
variant Status
{
    Idle;
    Busy;
    Done;
}

fixed Describe(state: Status): integer(4)
{
    match state
    {
        case is Status.Idle { return 1; }
        case is Status.Busy { return 2; }
    }
    return 0;
}

fixed Main(): integer(4) { return Describe(Status.Idle); }",
    )
    .expect("write program");
    let output = ai_nsc()
        .args(["check", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "warnings must not fail the build: {stderr}"
    );
    assert!(stderr.contains("not exhaustive"), "{stderr}");
}

#[test]
fn heap_allocator_must_be_an_allocator() {
    let stderr = check_source(
        "\
fixed Main(): integer(4)
{
    local allocator: integer(4) = 1;
    heap(allocator) value: integer(4) = 1;
    return value;
}",
    );
    assert!(stderr.contains("must be an IAllocator"), "{stderr}");
}

#[test]
fn compile_time_expression_rejects_runtime_values() {
    let stderr = check_source(
        "\
fixed Main(): integer(4)
{
    local value: integer(4) = 4;
    local derived: integer(4) = #(value + 1);
    return derived;
}",
    );
    assert!(stderr.contains("compile-time expressions"), "{stderr}");
}

#[test]
fn double_dispose_warns_without_failing() {
    let directory = std::env::temp_dir().join(format!("ainsc-cli-warn4-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("main.ns");
    std::fs::write(
        &path,
        "\
structure Token
{
    public Value: integer(4);
}

fixed Main(): integer(4)
{
    heap made: Token = Token();
    dispose made;
    dispose made;
    return 0;
}",
    )
    .expect("write program");
    let output = ai_nsc()
        .args(["check", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "warnings must not fail the build: {stderr}"
    );
    assert!(stderr.contains("disposed more than once"), "{stderr}");
}

#[test]
fn pointer_to_pointer_conversion_warns_without_failing() {
    let directory = std::env::temp_dir().join(format!("ainsc-cli-warn5-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("main.ns");
    std::fs::write(
        &path,
        "\
fixed Main(): integer(4)
{
    local values[2]: integer(4) = { 1, 2 };
    local p: pointer integer(4) = (values as pointer integer(4));
    local raw: pointer = (p as pointer);
    return ((raw as pointer integer(4))[1] as integer(4));
}",
    )
    .expect("write program");
    let output = ai_nsc()
        .args(["check", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "warnings must not fail the build: {stderr}"
    );
    assert!(stderr.contains("changes only the target type"), "{stderr}");
}

#[test]
fn array_cast_between_different_byte_sizes_is_diagnosed() {
    let stderr = check_source(
        "\
fixed Main(): integer(4)
{
    local words[2]: cardinal(4) = { 7, 0 };
    local bytes: array(3) cardinal(2) = (words as array(3) cardinal(2));
    return (bytes[0] as integer(4));
}",
    );
    assert!(stderr.contains("different byte sizes"), "{stderr}");
}

#[test]
fn compile_time_assertion_failure_is_reported() {
    let stderr = check_source(
        "\
fixed Main(): integer(4)
{
    #{
        Compiler.Assert(false, \"must be true\");
    }
    return 0;
}",
    );
    assert!(stderr.contains("compile-time assertion failed"), "{stderr}");
}

#[test]
fn compiler_api_is_rejected_outside_compile_time_code() {
    let stderr = check_source(
        "\
fixed Main(): integer(4)
{
    Compiler.Assert(true);
    return 0;
}",
    );
    assert!(
        stderr.contains("only available in compile-time code"),
        "{stderr}"
    );
}

#[test]
fn static_class_cannot_be_instantiated() {
    let stderr = check_source(
        "\
static class Utility
{
    extension
    {
        public static fixed Twice(value: integer(4)): integer(4) { return value * 2; }
    }
}

fixed Main(): integer(4)
{
    local instance: Utility = Utility();
    return instance.Twice(4);
}",
    );
    assert!(
        stderr.contains("cannot instantiate the static class"),
        "{stderr}"
    );
}

#[test]
fn missing_required_argument_is_reported() {
    let stderr = check_source(
        "\
fixed Sum(a: integer(4), b: integer(4)): integer(4)
{
    return a + b;
}

fixed Main(): integer(4)
{
    return Sum(a: 1);
}",
    );
    assert!(stderr.contains("missing argument"), "{stderr}");
}

#[test]
fn compile_time_parameter_requires_a_constant_argument() {
    let stderr = check_source(
        "\
fixed Runtime(): integer(4) { return 7; }

fixed Apply(#value: integer(4)): integer(4)
{
    #{
        return value + 1;
    }
}

fixed Main(): integer(4)
{
    return Apply(Runtime());
}",
    );
    assert!(stderr.contains("compile-time constants"), "{stderr}");
}

#[test]
fn multiple_input_files_form_one_program() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-multi-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let library = directory.join("library.ns");
    let program = directory.join("program.ns");
    std::fs::write(
        &library,
        "fixed Triple(value: integer(4)): integer(4)\n{\n    return value * 3;\n}",
    )
    .expect("write library");
    std::fs::write(
        &program,
        "fixed Main(): integer(4)\n{\n    return Triple(4);\n}",
    )
    .expect("write program");
    let output = ai_nsc()
        .args([
            "run",
            program.to_str().expect("utf-8 path"),
            library.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    // The exit code is Main's return value (12), so a non-zero status is expected.
    assert_eq!(
        output.status.code(),
        Some(12),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn nolaa_disposes_heap_units_on_return_without_warning() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-nolaa-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    std::fs::write(
        &path,
        "structure Node\n{\n    public Value: integer(4);\n}\n\nfixed Main(): integer(4)\n{\n    heap leaked: Node = Node();\n    leaked.Value = 4;\n    return leaked.Value;\n}",
    )
    .expect("write program");
    let output = ai_nsc()
        .args(["check", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    // NOLAA now inserts the dispose at the return safe point (§12.2).
    assert!(
        !stderr.contains("never disposed"),
        "return is a safe point, so no warning is expected: {stderr}"
    );
}

#[test]
fn use_after_dispose_warns_about_early_dispose() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-early-dispose-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    std::fs::write(
        &path,
        "structure Node\n{\n    public Value: integer(4);\n}\n\nfixed Main(): integer(4)\n{\n    heap early: Node = Node();\n    early.Value = 2;\n    dispose(early);\n    return early.Value;\n}",
    )
    .expect("write program");
    let output = ai_nsc()
        .args(["check", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    assert!(
        stderr.contains("early dispose"),
        "expected an early-dispose warning: {stderr}"
    );
}

#[test]
fn private_members_are_not_accessible_outside_the_type() {
    let stderr = check_source(
        "\
class Secret
{
    private Hidden: integer(4);

    extension
    {
        public fixed Construct(this: &Secret) { this.Hidden = 1; }
        private fixed HiddenMethod(this: &Secret): integer(4) { return this.Hidden; }
    }
}

fixed Main(): integer(4)
{
    local secret: Secret = Secret();
    local value: integer(4) = secret.Hidden;
    return value + secret.HiddenMethod();
}",
    );
    assert!(stderr.contains("'Hidden' is not accessible"), "{stderr}");
    assert!(
        stderr.contains("'HiddenMethod' is not accessible"),
        "{stderr}"
    );
}

#[test]
fn protected_members_are_accessible_in_derived_types_only() {
    let stderr = check_source(
        "\
class Animal
{
    protected Name: integer(4);
}

class Cat: Animal
{
    extension
    {
        public fixed Read(this: &Cat): integer(4) { return this.Name; }
    }
}

class Stranger
{
    extension
    {
        public fixed Peek(this: &Stranger, target: Animal): integer(4) { return target.Name; }
    }
}

fixed Main(): integer(4) { return 0; }",
    );
    assert!(stderr.contains("'Name' is not accessible"), "{stderr}");
}

#[test]
fn readonly_fields_are_only_assigned_in_constructors() {
    let stderr = check_source(
        "\
class Locked
{
    public readonly Value: integer(4);

    extension
    {
        public fixed Construct(this: &Locked, value: integer(4)) { this.Value = value; }
        public fixed TryChange(this: &Locked) { this.Value = 9; }
    }
}

fixed Main(): integer(4)
{
    local locked: Locked = Locked(3);
    locked.Value = 7;
    return locked.Value;
}",
    );
    assert!(stderr.contains("readonly"), "{stderr}");
}

#[test]
fn function_value_access_right_cannot_be_widened() {
    let stderr = check_source(
        "\
class Container
{
    public Exposed: function(Container) integer(4) = Hidden;

    extension
    {
        private fixed Hidden(this: &Container): integer(4) { return 0; }
    }
}

fixed Main(): integer(4) { return 0; }",
    );
    assert!(stderr.contains("narrower access right"), "{stderr}");
}

#[test]
fn field_initializers_are_type_checked() {
    let stderr = check_source(
        "\
class Broken
{
    public Value: integer(4) = true;
}

fixed Main(): integer(4) { return 0; }",
    );
    assert!(!stderr.is_empty(), "expected a diagnostic");
}

#[test]
fn variable_length_arrays_cannot_be_initialized() {
    let stderr = check_source(
        "\
fixed Main(): integer(4)
{
    local count: integer(4) = 3;
    local values: array(count) integer(4) = 0;
    return values[0];
}",
    );
    assert!(
        stderr.contains("variable-length array") || stderr.contains("initializer"),
        "{stderr}"
    );
}

#[test]
fn unsupported_calling_conventions_are_reported() {
    let stderr = check_source(
        "\
fixed Run(value: integer(4); CallingConvention.FastCall): integer(4)
{
    return value;
}

fixed Main(): integer(4) { return Run(1); }",
    );
    assert!(
        stderr.contains("calling convention is not supported"),
        "{stderr}"
    );
}

#[test]
fn value_type_methods_require_a_by_reference_this() {
    let stderr = check_source(
        "\
structure Point
{
    public X: integer(4);

    extension
    {
        public fixed Read(this: Point): integer(4) { return this.X; }
    }
}

fixed Main(): integer(4) { return 0; }",
    );
    assert!(stderr.contains("must be by-reference"), "{stderr}");
}

#[test]
fn aligned_is_rejected_on_classes() {
    let stderr = check_source(
        "\
aligned(16) class Boxed
{
    public Value: integer(4);
}

fixed Main(): integer(4) { return 0; }",
    );
    assert!(stderr.contains("only allowed on structure"), "{stderr}");
}

#[test]
fn scoped_members_are_not_accessible_from_outside_the_targets() {
    let stderr = check_source(
        "\
class Animal
{
    scoped(sealed Animal) public Visible: integer(4);

    extension
    {
        public fixed Construct(this: Animal, value: integer(4)) { this.Visible = value; }
    }
}

class Cat: Animal
{
    extension
    {
        public fixed Construct(this: Cat, value: integer(4)) { base.Construct(value); }
        public fixed Peek(this: Cat): integer(4) { return this.Visible; }
    }
}

fixed Main(): integer(4) { return 0; }",
    );
    assert!(stderr.contains("'Visible' is not accessible"), "{stderr}");
}

#[test]
fn scoped_member_targets_reject_other_members() {
    let stderr = check_source(
        "\
class Animal
{
    extension
    {
        protected scoped(Speak) fixed Hidden(this: Animal): integer(4) { return 3; }
        public fixed Other(this: Animal): integer(4) { return this.Hidden(); }
        public virtual fixed Speak(this: Animal): integer(4) { return 0; }
    }
}

fixed Main(): integer(4) { return 0; }",
    );
    assert!(stderr.contains("'Hidden' is not accessible"), "{stderr}");
}

#[test]
fn top_level_compile_time_variables_cross_files() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-toplevel-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let first = directory.join("first.ns");
    let second = directory.join("second.ns");
    std::fs::write(&first, "#{ local seed: integer(4) = 7; }").expect("write first");
    std::fs::write(
        &second,
        "#if seed > 3\n{\n    fixed Value: integer(4) = 41;\n}\nelse\n{\n    fixed Value: integer(4) = 99;\n}\n\nfixed Main(): integer(4)\n{\n    return Value + seed;\n}",
    )
    .expect("write second");
    let output = ai_nsc()
        .args([
            "run",
            first.to_str().expect("utf-8 path"),
            second.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    assert_eq!(
        output.status.code(),
        Some(48),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn verbatim_functions_are_not_optimized() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-verbatim-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    std::fs::write(
        &path,
        "fixed Main(): integer(4)\n@{\n    local sum: integer(4) = 1 + 2;\n    return sum;\n}",
    )
    .expect("write program");
    let output = ai_nsc()
        .args(["emit-ir", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{output:?}");
    assert!(
        stdout.contains("optnone"),
        "expected optnone in IR:\n{stdout}"
    );
}

#[test]
fn verbatim_forbids_compile_time_evaluation() {
    let stderr = check_source(
        "\
fixed Main(): integer(4)
@{
    local value: integer(4) = #(1 + 2);
    return 0;
}",
    );
    assert!(
        stderr.contains("no-optimize marker forbids compile-time evaluation"),
        "{stderr}"
    );
}

#[test]
fn generic_methods_require_type_arguments() {
    let stderr = check_source(
        "\
structure Wrap
{
    extension
    {
        public fixed Echo<T>(this: &Wrap, value: T): T { return value; }
    }
}

fixed Main(): integer(4)
{
    local wrap: Wrap;
    return wrap.Echo(5);
}",
    );
    assert!(stderr.contains("requires type arguments"), "{stderr}");
}

#[test]
fn scoped_operator_symbol_targets_restrict_other_operators() {
    let stderr = check_source(
        "\
structure Num
{
    public scoped(operator -) Value: integer(4);

    extension
    {
        operator -(left: Num, right: Num): Num { return Num(); }
        operator +(left: Num, right: Num): integer(4) { return left.Value; }
    }
}

fixed Main(): integer(4) { return 0; }",
    );
    assert!(stderr.contains("'Value' is not accessible"), "{stderr}");
}

#[test]
fn compile_time_expressions_are_folded_in_ir() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-ctfold-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    std::fs::write(
        &path,
        "fixed Main(): integer(4)\n{\n    return #(2 + 3);\n}",
    )
    .expect("write program");
    let output = ai_nsc()
        .args(["emit-ir", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    let ir = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(ir.contains("ret i32 5"), "{ir}");
    assert!(!ir.contains("add i32"), "{ir}");
}

#[test]
fn nolaa_arguments_must_be_trackable_heap_objects() {
    let stderr = check_source(
        "\
fixed Main(): integer(4)
{
    local value: integer(4) = 1;
    local address: pointer integer(4) = &value;
    NOLAA.Bind(address, address);
    return 0;
}",
    );
    assert!(stderr.contains("trackable heap object"), "{stderr}");
}

#[test]
fn compile_time_only_blocks_leave_no_run_time_code() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-ctblock-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    std::fs::write(
        &path,
        "fixed Main(): integer(4)\n{\n    #{\n        local value: integer(4) = 2 + 3;\n        local other: integer(4) = value * 2;\n        Compiler.LogNote(\"compile-time only\");\n    }\n    return 0;\n}",
    )
    .expect("write program");
    let output = ai_nsc()
        .args(["emit-ir", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    let ir = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    // Only inspect the generated Main function: library helpers may allocate.
    let main_start = ir
        .find("define i32 @N04Main")
        .unwrap_or_else(|| panic!("Main not found:\n{ir}"));
    let main_body = &ir[main_start..];
    let main_end = main_body.find("\n}").unwrap_or(main_body.len());
    let main_ir = &main_body[..main_end];
    assert!(main_ir.contains("ret i32 0"), "{main_ir}");
    assert!(
        !main_ir.contains("alloca i64"),
        "compile-time locals should not be emitted:\n{main_ir}"
    );
    assert!(!main_ir.contains("add i32"), "{main_ir}");
}

#[test]
fn emit_obj_supports_aarch64_cross_compilation() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-cross-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    let object = directory.join("program.o");
    std::fs::write(&path, "fixed Main(): integer(4)\n{\n    return 42;\n}").expect("write");
    let output = ai_nsc()
        .args([
            "emit-obj",
            "--target=aarch64-unknown-linux-gnu",
            path.to_str().expect("utf-8 path"),
            "-o",
            object.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run ai-nsc");
    let bytes = std::fs::read(&object).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&directory);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!bytes.is_empty(), "expected a non-empty object file");
    // ELF magic: cross-compiled object is produced for the requested target.
    assert_eq!(&bytes[..4], b"\x7fELF");
}

#[test]
fn cross_target_linking_is_rejected_with_a_clear_message() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-crosslink-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    std::fs::write(&path, "fixed Main(): integer(4)\n{\n    return 0;\n}").expect("write");
    let output = ai_nsc()
        .args([
            "run",
            "--target=aarch64-unknown-linux-gnu",
            path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("not supported"), "{stderr}");
    assert!(stderr.contains("emit-obj"), "{stderr}");
}

#[test]
fn emit_obj_supports_riscv64_cross_compilation() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-riscv-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    let object = directory.join("program.o");
    std::fs::write(&path, "fixed Main(): integer(4)\n{\n    return 7;\n}").expect("write");
    let output = ai_nsc()
        .args([
            "emit-obj",
            "--target=riscv64-unknown-linux-gnu",
            path.to_str().expect("utf-8 path"),
            "-o",
            object.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run ai-nsc");
    let bytes = std::fs::read(&object).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&directory);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(&bytes[..4], b"\x7fELF");
}

#[test]
fn unsupported_target_architectures_are_diagnosed() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-badtarget-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    std::fs::write(&path, "fixed Main(): integer(4)\n{\n    return 0;\n}").expect("write");
    let output = ai_nsc()
        .args([
            "emit-obj",
            "--target=mips-unknown-linux-gnu",
            path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(
        stderr.contains("unsupported target architecture 'mips'"),
        "{stderr}"
    );
}

#[test]
fn compile_time_block_side_effects_warn() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-ctside-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    std::fs::write(
        &path,
        "fixed Helper(): integer(4)\n{\n    return 1;\n}\n\nfixed Main(): integer(4)\n{\n    #{\n        local result: integer(4) = Helper();\n    }\n    return 0;\n}",
    )
    .expect("write");
    let output = ai_nsc()
        .args(["check", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "warnings must not fail compilation: {stderr}"
    );
    assert!(
        stderr.contains("side effect in a compile-time block"),
        "{stderr}"
    );
}

#[test]
fn early_dispose_with_a_still_reachable_alias_warns() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-earlyalias-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    std::fs::write(
        &path,
        "local saved: Token;\n\nstructure Token\n{\n    public Value: integer(4);\n}\n\nfixed Main(): integer(4)\n{\n    heap shared: Token = Token();\n    shared.Value = 1;\n    saved = shared;\n    dispose shared;\n    return 0;\n}",
    )
    .expect("write");
    let output = ai_nsc()
        .args(["check", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "warnings must not fail: {stderr}");
    assert!(stderr.contains("early dispose"), "{stderr}");
}

#[test]
fn array_target_bindings_must_be_cardinal() {
    let stderr = check_source(
        "\
extension for array(length: integer(4)) T
{
    public fixed Size(this: &array(length) T): cardinal(4)
    {
        return (length as cardinal(4));
    }
}

fixed Main(): integer(4)
{
    local values: array(3) integer(4);
    return (values.Size() as integer(4));
}",
    );
    assert!(
        stderr.contains("array target bindings must have a cardinal type"),
        "{stderr}"
    );
}

#[test]
fn external_linkable_declarations_use_the_decoded_symbol() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-linkable-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    std::fs::write(
        &path,
        "linkable(\"abs\") Absolute: function(value: integer(4)) integer(4);\n\nfixed Main(): integer(4)\n{\n    return Absolute(-1);\n}",
    )
    .expect("write");
    let output = ai_nsc()
        .args(["emit-ir", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    let ir = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(ir.contains("declare i32 @abs(i32)"), "{ir}");
    assert!(!ir.contains("\\\"abs\\\""), "{ir}");
}

#[test]
fn free_accessor_with_constant_backing_is_read_only() {
    let stderr = check_source(
        "\
fixed Base: integer(4) = 7;

accessor ReadBase: integer(4) => Base;

fixed Main(): integer(4)
{
    ReadBase = 3;
    return ReadBase;
}",
    );
    assert!(stderr.contains("accessor is read-only"), "{stderr}");
}

#[test]
fn linkable_calls_abandon_nolaa_analysis_with_a_never_disposed_warning() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-linkabandon-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    std::fs::write(
        &path,
        "linkable(\"consume\") Consume: function(value: Token);\n\nclass Token\n{\n    public Value: integer(4);\n}\n\nfixed Main(): integer(4)\n{\n    heap token: Token = Token();\n    token.Value = 1;\n    Consume(token);\n    return 0;\n}",
    )
    .expect("write");
    let output = ai_nsc()
        .args(["check", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(
        stderr.contains("never disposed"),
        "linkable calls must abandon NOLAA analysis: {stderr}"
    );
}

#[test]
fn compile_time_code_may_not_read_runtime_locals() {
    let stderr = check_source(
        "\
fixed Main(): integer(4)
{
    local runtime: integer(4) = 3;
    #{
        local copy: integer(4) = runtime;
    }
    return 0;
}",
    );
    assert!(
        stderr.contains("compile-time code may only reference compile-time constants"),
        "{stderr}"
    );
}

#[test]
fn constants_must_not_be_initialized_with_uninitialized() {
    let stderr = check_source(
        "\
fixed Value: integer(4) = uninitialized;

fixed Main(): integer(4)
{
    return 0;
}",
    );
    assert!(
        stderr.contains("a constant must not be initialized with 'uninitialized'"),
        "{stderr}"
    );
}

#[test]
fn compile_time_expression_function_calls_are_folded_in_ir() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-ctcall-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    std::fs::write(
        &path,
        "fixed Double(value: integer(4)): integer(4)\n{\n    return value * 2;\n}\n\nfixed Main(): integer(4)\n{\n    return #(Double(3));\n}",
    )
    .expect("write");
    let output = ai_nsc()
        .args(["emit-ir", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    let ir = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(ir.contains("ret i32 6"), "{ir}");
    assert!(
        !ir.contains("call i32 @N06Double"),
        "call should be folded:\n{ir}"
    );
}

#[test]
fn i686_target_builds_and_runs_heap_classes() {
    // 32-bit x86 executables run on an x86_64 host when 32-bit libraries exist.
    if std::env::consts::ARCH != "x86_64" || !std::path::Path::new("/usr/lib32/libc.so.6").exists()
    {
        return;
    }
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-i686-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let source_path = directory.join("program.ns");
    std::fs::write(
        &source_path,
        "\
class Box
{
    public Value: integer(4);

    extension
    {
        public fixed Construct(this: &Box, value: integer(4)) { this.Value = value; }
        public fixed Get(this: &Box): integer(4) { return this.Value; }
    }
}

fixed Main(): integer(4)
{
    heap box: Box = Box(40);
    return box.Get() + 2;
}",
    )
    .expect("write");
    let executable = directory.join("program");
    let output = ai_nsc()
        .args([
            "build",
            "--target=i686-unknown-linux-gnu",
            "-o",
            executable.to_str().expect("utf-8 path"),
            source_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run ai-nsc");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let status = std::process::Command::new(&executable)
        .status()
        .expect("run 32-bit program");
    let _ = std::fs::remove_dir_all(&directory);
    assert_eq!(status.code(), Some(42));
}

#[test]
fn i686_target_handles_traps() {
    if std::env::consts::ARCH != "x86_64" || !std::path::Path::new("/usr/lib32/libc.so.6").exists()
    {
        return;
    }
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-i686trap-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let source_path = directory.join("program.ns");
    std::fs::write(
        &source_path,
        "\
fixed Main(): integer(4)
{
    local result: integer(4) = 0;
    local code: integer(4) = 42;
    trap
    {
        throw code;
        result = 1;
    }
    catch value: integer(4)
    {
        result = value;
    }
    return result;
}",
    )
    .expect("write");
    let executable = directory.join("program");
    let output = ai_nsc()
        .args([
            "build",
            "--target=i686-unknown-linux-gnu",
            "-o",
            executable.to_str().expect("utf-8 path"),
            source_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run ai-nsc");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let status = std::process::Command::new(&executable)
        .status()
        .expect("run 32-bit program");
    let _ = std::fs::remove_dir_all(&directory);
    assert_eq!(status.code(), Some(42));
}

#[test]
fn armv7_target_builds_and_runs_heap_classes() {
    let qemu = std::env::var("AINSC_QEMU_ARM").unwrap_or_else(|_| "qemu-arm".to_string());
    let sysroot = std::path::Path::new("/usr/arm-linux-gnueabihf");
    if !sysroot.join("lib/ld-linux-armhf.so.3").exists()
        || !std::path::Path::new("/usr/bin/ld.lld").exists()
        || std::process::Command::new(&qemu)
            .arg("--version")
            .output()
            .is_err()
    {
        return;
    }
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-armv7-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let source_path = directory.join("program.ns");
    std::fs::write(
        &source_path,
        "\
class Box
{
    public Value: integer(4);

    extension
    {
        public fixed Construct(this: &Box, value: integer(4)) { this.Value = value; }
        public fixed Get(this: &Box): integer(4) { return this.Value; }
    }
}

fixed Main(): integer(4)
{
    heap box: Box = Box(40);
    return box.Get() + 2;
}",
    )
    .expect("write");
    let executable = directory.join("program");
    let output = ai_nsc()
        .args([
            "build",
            "--target=armv7-unknown-linux-gnueabihf",
            "--sysroot=/",
            "-o",
            executable.to_str().expect("utf-8 path"),
            source_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run ai-nsc");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let status = std::process::Command::new(&qemu)
        .arg("-L")
        .arg(sysroot)
        .arg(&executable)
        .status()
        .expect("run arm program");
    let _ = std::fs::remove_dir_all(&directory);
    assert_eq!(status.code(), Some(42));
}

#[test]
fn no_optimize_markers_are_rejected_in_compile_time_code() {
    let nested = check_source(
        "\
fixed Main(): integer(4)
@{
    @{
    }
    return 0;
}",
    );
    assert!(
        nested.contains("no-optimize blocks must not nest"),
        "{nested}"
    );
    let in_compile_time = check_source(
        "\
fixed Main(): integer(4)
{
    local x: integer(4) = 0;
    #{
        local value: integer(4) = @(1 + 2);
    }
    return x;
}",
    );
    assert!(
        in_compile_time.contains("compile-time"),
        "{in_compile_time}"
    );
}

#[test]
fn accessor_this_types_follow_target_reference_rules() {
    // §3.8.2: value-type targets require `&`, reference-type targets forbid it,
    // and extension-block accessor blocks must declare `this`.
    let missing_and = check_source(
        "\
structure Holder
{
    public Value: integer(4);

    extension
    {
        public accessor Item: integer(4)
        {
            get(this: Holder) { return this.Value; }
            set(this: Holder, value) { this.Value = value; }
        }
    }
}

fixed Main(): integer(4) { return 0; }",
    );
    assert!(
        missing_and.contains("value-type accessor 'this' must carry '&'"),
        "{missing_and}"
    );
    let extra_and = check_source(
        "\
class Holder
{
    public Value: integer(4);

    extension
    {
        public accessor Item: integer(4)
        {
            get(this: &Holder) { return this.Value; }
            set(this: &Holder, value) { this.Value = value; }
        }
    }
}

fixed Main(): integer(4) { return 0; }",
    );
    assert!(
        extra_and.contains("reference-type accessor 'this' must not carry '&'"),
        "{extra_and}"
    );
    let missing_this = check_source(
        "\
class Holder
{
    public Value: integer(4);

    extension
    {
        public accessor Item: integer(4)
        {
            get() { return this.Value; }
        }
    }
}

fixed Main(): integer(4) { return 0; }",
    );
    assert!(
        missing_this.contains("must declare 'this'"),
        "{missing_this}"
    );
}

#[test]
fn compiler_api_is_restricted_to_compile_time_contexts() {
    let sizeof_direct = check_source(
        "fixed Main(): integer(4)\n{\n    local size: cardinal(8) = Type.SizeOf<integer(4)>();\n    return (size as integer(4));\n}",
    );
    assert!(
        sizeof_direct.contains("through '#(...)' or '#if'"),
        "{sizeof_direct}"
    );
    let platform_direct = check_source(
        "fixed Main(): integer(4)\n{\n    if (Environment.Current.PlatformIs(\"Linux\")) { return 1; }\n    return 0;\n}",
    );
    assert!(
        platform_direct.contains("environment queries must be evaluated at compile time"),
        "{platform_direct}"
    );
    let compiler_direct = check_source(
        "fixed Main(): integer(4)\n{\n    Compiler.LogNote(\"runtime\");\n    return 0;\n}",
    );
    assert!(
        compiler_direct.contains("through '#(...)' or '#if'"),
        "{compiler_direct}"
    );
}

#[test]
fn i686_type_sizeof_returns_a_64_bit_value() {
    if std::env::consts::ARCH != "x86_64" || !std::path::Path::new("/usr/lib32/libc.so.6").exists()
    {
        return;
    }
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-i686-sizeof-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let source_path = directory.join("program.ns");
    std::fs::write(
        &source_path,
        "fixed Main(): integer(4)\n{\n    local size: cardinal(8) = #(Type.SizeOf<integer(4)>());\n    return (size as integer(4));\n}",
    )
    .expect("write");
    let executable = directory.join("program");
    let output = ai_nsc()
        .args([
            "build",
            "--target=i686-unknown-linux-gnu",
            "-o",
            executable.to_str().expect("utf-8 path"),
            source_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run ai-nsc");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let status = std::process::Command::new(&executable)
        .status()
        .expect("run 32-bit program");
    let _ = std::fs::remove_dir_all(&directory);
    assert_eq!(status.code(), Some(4));
}

#[test]
fn source_files_must_have_the_ns_extension() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-ext-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.txt");
    std::fs::write(&path, "fixed Main(): integer(4)\n{\n    return 0;\n}").expect("write");
    let output = ai_nsc()
        .args(["check", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("'.ns' extension"), "{stderr}");
}

#[test]
fn wildcard_functions_validate_only_the_fixed_prefix() {
    let too_many = check_source(
        "\
fixed Need(value: integer(4)): integer(4)
{
    return value;
}

fixed Main(): integer(4)
{
    return Need(1, 2);
}",
    );
    assert!(too_many.contains("too many arguments"), "{too_many}");
    let missing = check_source(
        "\
fixed Need(first: integer(4), wildcard): integer(4)
{
    return first;
}

fixed Main(): integer(4)
{
    return Need();
}",
    );
    assert!(missing.contains("missing argument"), "{missing}");
    let wrong_prefix = check_source(
        "\
fixed Need(first: integer(4), wildcard): integer(4)
{
    return first;
}

fixed Main(): integer(4)
{
    return Need(\"not an integer\");
}",
    );
    assert!(!wrong_prefix.is_empty(), "{wrong_prefix}");
    let accepted = check_source_ok(
        "\
linkable(\"printf\") PrintFormat(wildcard);

fixed Main(): integer(4)
{
    PrintFormat(\"HelloWorld\\n\");
    return 0;
}",
    );
    assert!(!accepted.contains("too many arguments"), "{accepted}");
    assert!(!accepted.contains("no overload"), "{accepted}");
}

#[test]
fn for_range_expression_types_must_be_consistent() {
    let mismatch = check_source(
        "\
fixed Main(): integer(4)
{
    local x: integer(4) = 3;
    for 1..\"text\"
    {
    }
    return x;
}",
    );
    assert!(
        mismatch.contains("not compatible")
            || mismatch.contains("range expression types must be consistent")
            || mismatch.contains("requires an integer range"),
        "{mismatch}"
    );
    // Mixed integer/cardinal non-literal range expressions are also rejected.
    let mixed = check_source(
        "\
fixed Main(): integer(4)
{
    local x: integer(4) = 3;
    local y: cardinal(4) = 4;
    for x..y
    {
    }
    return x;
}",
    );
    assert!(
        mixed.contains("range expression types must be consistent"),
        "{mixed}"
    );
}

#[test]
fn printf_scanf_with_c_declaration_calling_convention_run() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-cdecl-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    std::fs::write(
        &path,
        "\
linkable(\"printf\") PrintFormat(wildcard; CDeclaration);
linkable(\"scanf\") ScanFormat(wildcard; CDeclaration);

structure Vector3
{
    public X: float(4);
    public Y: float(4);
    public Z: float(4);

    extension
    {
        public fixed Construct(this: &Vector3, x: float(4), y: float(4), z: float(4))
        {
            this.X = x;
            this.Y = y;
            this.Z = z;
        }
    }
}

fixed Main(): integer(4)
{
    local o = Vector3(0, 0, 0);
    local p = Vector3(0, 0, 0);
    ScanFormat(\"%f %f %f\\0\", &p.X, &p.Y, &p.Z);
    PrintFormat(\"%f %f %f\\0\", p.X, p.Y, p.Z);
    return 0;
}",
    )
    .expect("write");
    let executable = directory.join("program");
    let output = ai_nsc()
        .args([
            "build",
            "-o",
            executable.to_str().expect("utf-8 path"),
            path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run ai-nsc");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut child = Command::new(&executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("run program");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(b"1.5 2.5 3.5\n")
        .expect("write stdin");
    let result = child.wait_with_output().expect("wait");
    let _ = std::fs::remove_dir_all(&directory);
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert_eq!(result.status.code(), Some(0), "{stdout}");
    assert!(
        stdout.contains("1.500000 2.500000 3.500000"),
        "stdout: {stdout}"
    );
}

#[test]
fn by_reference_operator_operands_must_be_addressable() {
    let stderr = check_source(
        "\
structure V
{
    public X: integer(4);

    extension
    {
        public fixed Construct(this: &V, x: integer(4)) { this.X = x; }
        operator +(a: &V, b: &V): V { return V(a.X + b.X); }
    }
}

fixed Main(): integer(4)
{
    return V(1) + V(2) + 3;
}",
    );
    assert!(
        stderr.contains("requires an addressable operand"),
        "{stderr}"
    );
    assert!(!stderr.contains("must not be by-reference"), "{stderr}");
}

#[test]
fn compile_time_external_linkable_calls_execute_on_the_host() {
    // The host process acts as the compile-time library (§5.5): printf executes
    // while the program is being compiled.
    let stderr = check_source_ok(
        "\
linkable(\"printf\") PrintFormat(wildcard; CDeclaration);

#{
    PrintFormat(\"compile-time text\\n\\0\");
}

fixed Main(): integer(4)
{
    return 0;
}",
    );
    assert!(stderr.is_empty(), "{stderr}");
}

#[test]
fn cross_target_compile_time_external_calls_are_diagnosed() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-ctexternal-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("main.ns");
    std::fs::write(
        &path,
        "linkable(\"printf\") PrintFormat(wildcard; CDeclaration);\n\n#{\n    PrintFormat(\"x\\0\");\n}\n\nfixed Main(): integer(4)\n{\n    return 0;\n}",
    )
    .expect("write");
    let output = ai_nsc()
        .args([
            "check",
            "--target=aarch64-unknown-linux-gnu",
            path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(
        stderr.contains("no compile-time library provides this external symbol"),
        "{stderr}"
    );
}

#[test]
fn compile_time_blocks_may_call_ordinary_functions_as_statements() {
    let stderr = check_source_ok(
        "\
fixed Helper(): integer(4)
{
    return 40 + 2;
}

#{
    local value: integer(4) = 0;
    Helper();
    local ignored: integer(4) = 1;
}

fixed Main(): integer(4)
{
    return 0;
}",
    );
    assert!(stderr.is_empty(), "{stderr}");
}

#[test]
fn compile_time_errors_stop_compile_time_execution() {
    let stderr = check_source(
        "\
#{
    while true
    {
        Compiler.LogError(\"stop now\");
    }
}

fixed Main(): integer(4)
{
    return 0;
}",
    );
    assert_eq!(stderr.matches("error: stop now").count(), 1, "{stderr}");
    assert!(!stderr.contains("iteration limit"), "{stderr}");
}

#[test]
fn uefi_x86_64_builds_a_bootable_application() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let qemu_ok = std::process::Command::new("qemu-system-x86_64")
        .arg("--version")
        .output()
        .is_ok();
    if std::env::consts::ARCH != "x86_64"
        || !std::path::Path::new("/usr/share/qemu/OVMF.fd").exists()
        || !std::path::Path::new("/usr/bin/lld-link").exists()
        || !std::path::Path::new("/usr/bin/mkfs.vfat").exists()
        || !std::path::Path::new("/usr/bin/mcopy").exists()
        || !qemu_ok
    {
        return;
    }
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-uefi-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    std::fs::write(&path, "fixed Main(): integer(4)\n{\n    return 42;\n}").expect("write");
    let executable = directory.join("program.efi");
    let output = ai_nsc()
        .env("AINSC_UEFI_TEST_EXIT", "1")
        .args([
            "build",
            "--target=x86_64-unknown-uefi",
            "-o",
            executable.to_str().expect("utf-8 path"),
            path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run ai-nsc");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let image = directory.join("disk.img");
    assert!(std::process::Command::new("mkfs.vfat")
        .args(["-C", image.to_str().expect("utf-8 path"), "32768"])
        .output()
        .expect("mkfs")
        .status
        .success());
    assert!(std::process::Command::new("mmd")
        .args([
            "-i",
            image.to_str().expect("utf-8 path"),
            "::EFI",
            "::EFI/BOOT"
        ])
        .output()
        .expect("mmd")
        .status
        .success());
    assert!(std::process::Command::new("mcopy")
        .args([
            "-i",
            image.to_str().expect("utf-8 path"),
            executable.to_str().expect("utf-8 path"),
            "::EFI/BOOT/BOOTX64.EFI",
        ])
        .output()
        .expect("mcopy")
        .status
        .success());
    let status = std::process::Command::new("qemu-system-x86_64")
        .args([
            "-machine",
            "q35",
            "-m",
            "128",
            "-bios",
            "/usr/share/qemu/OVMF.fd",
            "-drive",
            &format!("format=raw,file={}", image.display()),
            "-display",
            "none",
            "-serial",
            "none",
            "-device",
            "isa-debug-exit,iobase=0x501,iosize=1",
            "-no-reboot",
        ])
        .status()
        .expect("qemu");
    let _ = std::fs::remove_dir_all(&directory);
    // isa-debug-exit reports ((case + 1) << 1) | 1; main returned 42.
    assert_eq!(status.code(), Some((43 << 1) | 1));
}

#[test]
fn uefi_has_no_platform_default_allocator() {
    // The UEFI target has no platform allocator (the developer must supply one),
    // so heap-with-default-allocator programs are rejected.
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-uefi-heap-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    std::fs::write(
        &path,
        "fixed Main(): integer(4)\n{\n    heap value: integer(4) = 1;\n    return value;\n}",
    )
    .expect("write");
    let output = ai_nsc()
        .args([
            "check",
            "--target=x86_64-unknown-uefi",
            path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert!(
        stderr.contains("DefaultAllocator") || stderr.contains("allocator"),
        "{stderr}"
    );
}

#[test]
fn explicit_range_direction_rejects_contradictory_steps() {
    let ascending = check_source(
        "\
fixed Main(): integer(4)
{
    for index in 1..-2..<5
    {
    }
    return 0;
}",
    );
    assert!(
        ascending.contains("ascending range requires a positive step"),
        "{ascending}"
    );
    let descending = check_source(
        "\
fixed Main(): integer(4)
{
    for index in 5..2..>1
    {
    }
    return 0;
}",
    );
    assert!(
        descending.contains("descending range requires a negative step"),
        "{descending}"
    );
}

#[test]
fn lazy_two_phase_rejects_local_forward_dependencies() {
    let stderr = check_source(
        "\
#if Enabled
{
}

#{
    local Enabled: boolean = true;
}",
    );
    assert!(
        stderr.contains("'local' 'Enabled' may not be a forward dependency"),
        "{stderr}"
    );
}

#[test]
fn lazy_two_phase_reports_mutual_forward_dependencies() {
    let stderr = check_source(
        "\
#if A
{
    #{ fixed B: boolean = true; }
}

#if B
{
    #{ fixed A: boolean = true; }
}",
    );
    assert!(stderr.contains("cannot make progress"), "{stderr}");
}

#[test]
fn compile_time_if_inside_a_block_must_settle_immediately() {
    let stderr = check_source(
        "\
#{
    #if Later
    {
    }
}

#{
    fixed Later: boolean = true;
}",
    );
    assert!(
        stderr.contains("'#if' condition must be a compile-time constant"),
        "{stderr}"
    );
}

#[test]
fn top_level_compile_time_expression_statements_are_accepted() {
    let stderr = check_source_ok(
        "\
#(1 + 2);

#if 1 == 1
{
}

fixed Main(): integer(4)
{
    #(3 * 4);
    return 0;
}",
    );
    assert!(!stderr.contains("unexpected"), "{stderr}");
}

#[test]
fn legacy_cast_syntax_is_rejected() {
    let stderr = check_source(
        "\
fixed Main(): integer(4)
{
    return Cast<integer(4)>(1);
}",
    );
    assert!(
        stderr.contains("expected") || stderr.contains("unknown name 'Cast'"),
        "{stderr}"
    );
}

#[test]
fn shadowed_keyword_in_keyword_position_is_rejected_without_at() {
    let stderr = check_source(
        "\
local class: integer(4) = 1;

class MyClass { }

fixed Main(): integer(4)
{
    return 0;
}",
    );
    assert!(!stderr.is_empty(), "{stderr}");
    let stderr = check_source(
        "\
@class integer { }

fixed Main(): integer(4)
{
    local value: integer(4) = 1;
    return value;
}",
    );
    assert!(!stderr.is_empty(), "{stderr}");
}

#[test]
fn platform_queries_answer_the_selected_target() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-platform-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    std::fs::write(
        &path,
        "\
#if Environment.Target.PlatformIs(\"Windows\")
{
    #{ Compiler.LogError(\"windows target\"); }
}
else
{
    #{ Compiler.LogError(\"non-windows target\"); }
}

fixed Main(): integer(4)
{
    return 0;
}",
    )
    .expect("write");
    let host = ai_nsc()
        .args(["check", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let windows = ai_nsc()
        .args([
            "check",
            "--target=x86_64-pc-windows-msvc",
            path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run ai-nsc");
    let other_path = directory.join("other.ns");
    std::fs::write(
        &other_path,
        "\
#if Environment.Target.Platform == PlatformKind.Other
{
    #{ Compiler.LogError(\"other target\"); }
}
else
{
    #{ Compiler.LogError(\"named target\"); }
}

fixed Main(): integer(4)
{
    return 0;
}",
    )
    .expect("write");
    let other = ai_nsc()
        .args([
            "check",
            "--target=x86_64-unknown-uefi",
            other_path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run ai-nsc");
    let named = ai_nsc()
        .args(["check", other_path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    let host_stderr = String::from_utf8_lossy(&host.stderr);
    let windows_stderr = String::from_utf8_lossy(&windows.stderr);
    let other_stderr = String::from_utf8_lossy(&other.stderr);
    let named_stderr = String::from_utf8_lossy(&named.stderr);
    assert!(!host.status.success(), "{host_stderr}");
    assert!(host_stderr.contains("non-windows target"), "{host_stderr}");
    assert!(
        windows_stderr.contains("windows target"),
        "{windows_stderr}"
    );
    assert!(other_stderr.contains("other target"), "{other_stderr}");
    assert!(named_stderr.contains("named target"), "{named_stderr}");
}

#[test]
fn array_extension_index_accessors_are_rejected() {
    let stderr = check_source(
        "\
extension for array(length: cardinal(4)) T
{
    operator [index: cardinal(4)]: T
    {
        get(this: &array(length) T) { }
    }
}

fixed Main(): integer(4)
{
    local values: array(4) integer(4) = { 1, 2, 3, 4 };
    if (values == values)
    {
        return values[0];
    }
    return 0;
}",
    );
    assert!(
        stderr.contains("index accessor must be declared in a class or structure extension"),
        "{stderr}"
    );
}

#[test]
fn runtime_assignment_to_a_compile_time_block_variable_is_rejected() {
    let stderr = check_source(
        "\
fixed Main(): integer(4)
{
    #{ local value: integer(4) = 1; }
    value = 2;
    return value;
}",
    );
    assert!(
        stderr.contains("may only be assigned at compile time"),
        "{stderr}"
    );
}

#[test]
fn no_optimize_expressions_are_not_constant_folded() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-verbatim-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    std::fs::write(
        &path,
        "fixed Main(): integer(4)\n{\n    local x: integer(4) = @(1 + 2);\n    local @y: integer(4) = 3;\n    return x + y;\n}",
    )
    .expect("write");
    let output = ai_nsc()
        .args(["emit-ir", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    let ir = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let main_start = ir
        .find("define i32 @N04Main")
        .unwrap_or_else(|| panic!("Main not found:\n{ir}"));
    let main_body = &ir[main_start..];
    let main_end = main_body.find("\n}").unwrap_or(main_body.len());
    let main_ir = &main_body[..main_end];
    assert!(
        main_ir.contains("verbatim_temp") && main_ir.contains("add i32"),
        "the marked expression must stay in the IR:\n{main_ir}"
    );
    assert!(
        ir.contains("optnone"),
        "the containing function must be optnone:\n{ir}"
    );
}

#[test]
fn no_optimize_markers_reject_surrounding_whitespace() {
    let stderr = check_source(
        "\
@ class MyClass { }

fixed Main(): integer(4)
{
    local x: @ integer(4) = @ (1 + 2);
    return x;
}",
    );
    assert!(
        stderr
            .matches("whitespace is not allowed between '@'")
            .count()
            >= 3,
        "{stderr}"
    );
}

#[test]
fn nolaa_double_dispose_through_an_alias_warns() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-aliasdd-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    std::fs::write(
        &path,
        "class Token { public Value: integer(4); }\n\nfixed Main(): integer(4)\n{\n    heap token: Token = Token();\n    local alias: Token = token;\n    dispose alias;\n    dispose token;\n    return 0;\n}",
    )
    .expect("write");
    let output = ai_nsc()
        .args(["check", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(stderr.contains("disposed more than once"), "{stderr}");
}

#[test]
fn nolaa_use_after_dispose_through_an_alias_warns() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-aliasuse-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    std::fs::write(
        &path,
        "class Token { public Value: integer(4); }\n\nfixed Main(): integer(4)\n{\n    heap token: Token = Token();\n    local alias: Token = token;\n    dispose alias;\n    return token.Value;\n}",
    )
    .expect("write");
    let output = ai_nsc()
        .args(["check", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(stderr.contains("used after 'dispose'"), "{stderr}");
}

#[test]
fn nested_variable_length_arrays_cannot_have_initializers() {
    let stderr = check_source(
        "\
fixed Main(): integer(4)
{
    local columns: cardinal(4) = 2;
    local matrix: array(2, columns) integer(4) = { 1, 2, 3, 4 };
    return 0;
}",
    );
    assert!(
        stderr.contains("variable-length array cannot have an initializer"),
        "{stderr}"
    );
}

#[test]
fn no_optimize_constants_reject_marked_initializers() {
    let stderr = check_source(
        "\
fixed @constant: integer(4) = @(1 + 2);

fixed Main(): integer(4)
{
    return constant;
}",
    );
    assert!(
        stderr.contains("no-optimize constant must not use a '@' expression"),
        "{stderr}"
    );
}

#[test]
fn no_optimize_locals_forbid_compile_time_evaluation() {
    let stderr = check_source(
        "\
fixed Main(): integer(4)
{
    local @value: integer(4) = #(1 + 2);
    return value;
}",
    );
    assert!(
        stderr.contains("no-optimize marker forbids compile-time evaluation"),
        "{stderr}"
    );
}

#[test]
fn local_no_optimize_marker_applies_to_all_items_only_after_local() {
    let stderr = check_source(
        "\
fixed Main(): integer(4)
{
    local first: integer(4) = 1, @second: integer(4) = 2;
    return first + second;
}",
    );
    assert!(!stderr.is_empty(), "{stderr}");
}

#[test]
fn nolaa_reports_escaped_units_as_never_disposed() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-escaped-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    std::fs::write(
        &path,
        "local saved: Token;\n\nclass Token\n{\n    public Value: integer(4);\n}\n\nfixed Main(): integer(4)\n{\n    heap token: Token = Token();\n    saved = token;\n    return 0;\n}",
    )
    .expect("write");
    let output = ai_nsc()
        .args(["check", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(
        stderr.contains("heap allocation 'token' is never disposed"),
        "{stderr}"
    );
}

#[test]
fn compile_time_heap_of_non_scalar_types_is_supported() {
    let stderr = check_source_ok(
        "\
class Token { public Value: integer(4); }

#{ heap token: Token = Token(); token.Value = 1; }

fixed Main(): integer(4)
{
    return 0;
}",
    );
    assert!(stderr.is_empty(), "{stderr}");
}

#[test]
fn compile_time_heap_units_do_not_enter_runtime() {
    let stderr = check_source(
        "\
#{ heap number: integer(4) = 1; }

fixed Main(): integer(4)
{
    return number;
}",
    );
    assert!(stderr.contains("unknown name 'number'"), "{stderr}");
}

#[test]
fn top_level_statements_are_rejected() {
    let stderr = check_source(
        "\
if true { }
while true { }
for i: integer(4) in 0..5 { }
1 + 2;

fixed Main(): integer(4)
{
    return 0;
}",
    );
    assert!(
        stderr
            .matches("only allowed inside function bodies")
            .count()
            >= 4,
        "{stderr}"
    );
}

#[test]
fn module_level_statements_are_rejected() {
    let stderr = check_source(
        "\
module M
{
    while true { }
}

fixed Main(): integer(4)
{
    return 0;
}",
    );
    assert!(
        stderr.contains("only allowed inside function bodies"),
        "{stderr}"
    );
}

#[test]
fn trailing_commas_are_rejected_in_parameters_and_arguments() {
    let parameter = check_source(
        "\
fixed F(a: integer(4),): integer(4)
{
    return a;
}

fixed Main(): integer(4)
{
    return 0;
}",
    );
    assert!(
        parameter.contains("trailing comma is not allowed in a parameter list"),
        "{parameter}"
    );
    let argument = check_source(
        "\
fixed F(a: integer(4)): integer(4)
{
    return a;
}

fixed Main(): integer(4)
{
    return F(1,);
}",
    );
    assert!(
        argument.contains("trailing comma is not allowed in an argument list"),
        "{argument}"
    );
}

#[test]
fn draft_1_153_overload_and_monomorphization_rules_are_enforced() {
    let contract_only = check_source(
        "\
fixed F(value: integer(4)): integer(4) { return value; }
fixed F(#value: integer(4)): integer(4) { return value; }

fixed Main(): integer(4)
{
    return F(1);
}",
    );
    assert!(
        contract_only.contains("already has an overload with the same parameter signature"),
        "{contract_only}"
    );
    let by_ref = check_source(
        "\
fixed F(value: integer(4)): integer(4) { return value; }
fixed F(value: &integer(4)): integer(4) { return value; }

fixed Main(): integer(4)
{
    return F(1);
}",
    );
    assert!(
        by_ref.contains("already has an overload with the same parameter signature"),
        "{by_ref}"
    );
    let base = check_source(
        "\
fixed F(value: integer(4)): integer(4) { return value; }

fixed Main(): integer(4)
{
    return (1 + 2)<integer(4)>(1);
}",
    );
    assert!(
        base.contains("base of a monomorphization expression must be an identifier"),
        "{base}"
    );
}

#[test]
fn repeated_modifiers_are_rejected() {
    let stderr = check_source(
        "\
class Animal { public fixed Speak() { } }
class Dog : Animal { }

fixed Main(): integer(4)
{
    return 0;
}",
    );
    assert!(!stderr.contains("must not repeat"), "{stderr}");
    let repeated = check_source(
        "\
class Animal { public fixed Speak() { } }

protected scoped(Animal) scoped(Animal) fixed Visible()
{
}

fixed Main(): integer(4)
{
    return 0;
}",
    );
    assert!(repeated.contains("modifier must not repeat"), "{repeated}");
}

#[test]
fn environment_queries_report_host_and_target_facts() {
    let current = check_source(
        "\
#if Environment.Current.PlatformIs(\"Linux\")
{
    #{ Compiler.LogError(\"current linux\"); }
}

fixed Main(): integer(4)
{
    return 0;
}",
    );
    assert!(current.contains("current linux"), "{current}");
    let host = check_source(
        "\
#if Environment.Host.ArchitectureIs(\"X86_64\")
{
    #{ Compiler.LogError(\"host x86_64\"); }
}

fixed Main(): integer(4)
{
    return 0;
}",
    );
    assert!(host.contains("host x86_64"), "{host}");
    let target = check_source(
        "\
#if Environment.Target.PlatformIs(\"Windows\")
{
    #{ Compiler.LogError(\"target windows\"); }
}
else
{
    #{ Compiler.LogError(\"not windows\"); }
}

fixed Main(): integer(4)
{
    return 0;
}",
    );
    assert!(target.contains("not windows"), "{target}");

    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-environment-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    std::fs::write(
        &path,
        "\
#if Environment.Target.ArchitectureIs(\"Arm64\")
{
    #{ Compiler.LogError(\"arm64 target\"); }
}

fixed Main(): integer(4)
{
    return 0;
}",
    )
    .expect("write");
    let output = ai_nsc()
        .args([
            "check",
            "--target=aarch64-unknown-linux-gnu",
            path.to_str().expect("utf-8 path"),
        ])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("arm64 target"), "{stderr}");
}

#[test]
fn platform_namespace_is_not_part_of_draft_1_153() {
    let stderr = check_source(
        "\
fixed Main(): integer(4)
{
    local linux: boolean = #(Platform.Is(\"Linux\"));
    return 0;
}",
    );
    assert!(stderr.contains("unknown name 'Platform'"), "{stderr}");
}

#[test]
fn inline_assembly_diagnostics() {
    let inout = check_source(
        "\
fixed Main(): integer(4)
{
    local value: integer(4) = 1;
    assembly { mov @[value:inout], 0 }
    return 0;
}",
    );
    assert!(
        inout.contains("unknown assembly operand direction"),
        "{inout}"
    );
    let out_target = check_source(
        "\
fixed Main(): integer(4)
{
    assembly { mov @[1:out], 0 }
    return 0;
}",
    );
    assert!(out_target.contains("not assignable"), "{out_target}");
    let duplicate = check_source(
        "\
fixed Main(): integer(4)
{
    local value: integer(4) = 1;
    assembly with clobber(\"memory\", \"memory\") { mov @[value:out], 0 }
    return 0;
}",
    );
    assert!(duplicate.contains("duplicate clobber"), "{duplicate}");
    let top_level = check_source(
        "\
assembly { }
fixed Main(): integer(4) { return 0; }",
    );
    assert!(
        top_level.contains("only allowed inside function bodies"),
        "{top_level}"
    );
    let compile_time = check_source(
        "\
fixed Main(): integer(4)
{
    #{ assembly { } }
    return 0;
}",
    );
    assert!(
        compile_time.contains("must not appear in compile-time code"),
        "{compile_time}"
    );
}

#[test]
fn inline_assembly_escapes_emit_ir() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-asm-escapes-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    std::fs::write(
        &path,
        "fixed Main(): integer(4)\n{\n    assembly\n    {\n        # @@\n    }\n    return 0;\n}",
    )
    .expect("write");
    let output = ai_nsc()
        .args(["emit-ir", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn environment_value_queries_are_compile_time_constants() {
    let platform = check_source(
        "\
#if Environment.Current.Platform == PlatformKind.Linux
{
    #{ Compiler.LogError(\"linux kind\"); }
}

fixed Main(): integer(4) { return 0; }",
    );
    assert!(platform.contains("linux kind"), "{platform}");
    let architecture = check_source(
        "\
#if Environment.Current.Architecture == ArchitectureKind.X86_64
{
    #{ Compiler.LogError(\"x86_64 kind\"); }
}

fixed Main(): integer(4) { return 0; }",
    );
    assert!(architecture.contains("x86_64 kind"), "{architecture}");
    let kind = check_source(
        "\
#if Environment.Kind == EnvironmentKind.Target
{
    #{ Compiler.LogError(\"target kind\"); }
}

fixed Main(): integer(4) { return 0; }",
    );
    assert!(kind.contains("target kind"), "{kind}");
    let other_name = check_source(
        "\
#if Environment.Target.PlatformIs(\"Other\")
{
    #{ Compiler.LogError(\"other named\"); }
}
else
{
    #{ Compiler.LogError(\"other unnamed\"); }
}

fixed Main(): integer(4) { return 0; }",
    );
    assert!(other_name.contains("other unnamed"), "{other_name}");
}

#[test]
fn inline_assembly_emits_objects_for_supported_architectures() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-asm-arch-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    std::fs::write(
        &path,
        "fixed Main(): integer(4)\n{\n    assembly { nop }\n    return 0;\n}",
    )
    .expect("write");
    for target in [
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "riscv64-unknown-linux-gnu",
    ] {
        let output = ai_nsc()
            .args([
                "emit-obj",
                "--target",
                target,
                path.to_str().expect("utf-8 path"),
            ])
            .output()
            .expect("run ai-nsc");
        assert!(
            output.status.success(),
            "{target}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn inline_assembly_text_is_validated_during_check() {
    let stderr = check_source(
        "\
fixed Main(): integer(4)
{
    assembly
    {
        this_is_not_an_instruction
    }
    return 0;
}",
    );
    assert!(
        stderr.contains("error") || stderr.contains("failed"),
        "{stderr}"
    );
    let valid = check_source_ok(
        "\
fixed Main(): integer(4)
{
    local value: integer(4) = 1;
    assembly
    {
        add @[value], 1
    }
    return value;
}",
    );
    assert!(valid.is_empty(), "{valid}");
}

#[test]
fn unannotated_assembly_is_a_memory_and_flag_barrier() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-asm-barrier-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    std::fs::write(
        &path,
        "fixed Main(): integer(4)\n{\n    assembly { nop }\n    return 0;\n}",
    )
    .expect("write");
    let output = ai_nsc()
        .args(["emit-ir", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let ir = String::from_utf8_lossy(&output.stdout);
    for clobber in [
        "~{memory}",
        "~{cc}",
        "~{flags}",
        "~{fpsr}",
        "~{dirflag}",
        "~{rax}",
        "~{xmm15}",
    ] {
        assert!(ir.contains(clobber), "missing {clobber} in\n{ir}");
    }
}

#[test]
fn function_type_contracts_participate_in_equality() {
    let mismatch = check_source(
        "\
fixed Plain(value: integer(4)): integer(4)
{
    return value;
}

fixed Main(): integer(4)
{
    local plain: function(integer(4)) integer(4) = Plain;
    local compile_time: function(#integer(4)) integer(4) = plain;
    return compile_time(1);
}",
    );
    assert!(
        mismatch.contains("expected 'function(#integer(4))integer(4)'"),
        "{mismatch}"
    );
    let same = check_source_ok(
        "\
fixed Main(): integer(4)
{
    local first: function(#integer(4)) integer(4);
    local second: function(#integer(4)) integer(4) = first;
    return 0;
}",
    );
    assert!(same.is_empty(), "{same}");
}

#[test]
fn nolaa_tracks_pointer_aliases() {
    let safe = check_source_ok(
        "\
class Token { public Value: integer(4); }

fixed Main(): integer(4)
{
    heap token: Token = Token();
    local p: pointer Token = &token;
    p->Value = 5;
    return p->Value;
}",
    );
    assert!(safe.is_empty(), "{safe}");
    let use_after = check_source_ok(
        "\
class Token { public Value: integer(4); }

fixed Main(): integer(4)
{
    heap token: Token = Token();
    local p: pointer Token = &token;
    dispose token;
    p->Value = 5;
    return 0;
}",
    );
    assert!(use_after.contains("used after 'dispose'"), "{use_after}");
    let escaped = check_source_ok(
        "\
class Token { public Value: integer(4); }

fixed Main(): integer(4)
{
    local p: pointer Token;
    {
        heap token: Token = Token();
        p = &token;
    }
    return 0;
}",
    );
    assert!(
        escaped.contains("never disposed"),
        "outer-scope pointer alias must prevent automatic disposal: {escaped}"
    );
}

#[test]
fn nolaa_merges_dispose_state_across_branches() {
    let one_branch = check_source_ok(
        "\
class Token { public Value: integer(4); }

fixed Main(flag: boolean): integer(4)
{
    heap token: Token = Token();
    if (flag)
    {
        dispose token;
    }
    return 0;
}",
    );
    assert!(one_branch.contains("never disposed"), "{one_branch}");
    let both_branches = check_source_ok(
        "\
class Token { public Value: integer(4); }

fixed Main(flag: boolean): integer(4)
{
    heap token: Token = Token();
    if (flag)
    {
        dispose token;
    }
    else
    {
        dispose token;
    }
    return 0;
}",
    );
    assert!(both_branches.is_empty(), "{both_branches}");
    let match_cases = check_source_ok(
        "\
class Token { public Value: integer(4); }

fixed Main(flag: boolean): integer(4)
{
    heap token: Token = Token();
    match flag
    {
        case true { dispose token; }
        case wildcard { dispose token; }
    }
    return 0;
}",
    );
    assert!(match_cases.is_empty(), "{match_cases}");
}

#[test]
fn nolaa_disposes_pointer_member_field_aliases() {
    let source = "\
class Token { public Value: integer(4); }
class Holder { public Target: pointer Token; }

fixed Main(): integer(4)
{
    heap token: Token = Token();
    token.Value = 9;
    local holder: Holder = Holder();
    local p: pointer Holder = &holder;
    p->Target = &token;
    local value: integer(4) = p->Target->Value;
    dispose p->Target;
    return value;
}";
    let stderr = check_source_ok(source);
    assert!(stderr.is_empty(), "{stderr}");
}

#[test]
fn nolaa_tracks_container_element_aliases() {
    let safe = check_source_ok(
        "\
class Token { public Value: integer(4); }

fixed Main(): integer(4)
{
    heap token: Token = Token();
    local bucket: array(2) Token = { Token(), Token() };
    bucket[0] = token;
    return bucket[0].Value;
}",
    );
    assert!(safe.is_empty(), "{safe}");
    let use_after = check_source_ok(
        "\
class Token { public Value: integer(4); }

fixed Main(): integer(4)
{
    heap token: Token = Token();
    local bucket: array(2) Token = { Token(), Token() };
    bucket[0] = token;
    dispose token;
    return bucket[0].Value;
}",
    );
    assert!(use_after.contains("used after 'dispose'"), "{use_after}");
}

#[test]
fn nolaa_tracks_direct_aliases() {
    let safe = check_source_ok(
        "\
class Token { public Value: integer(4); }

fixed Main(): integer(4)
{
    heap token: Token = Token();
    local alias: Token = token;
    alias.Value = 4;
    return alias.Value;
}",
    );
    assert!(safe.is_empty(), "{safe}");
    let use_after = check_source_ok(
        "\
class Token { public Value: integer(4); }

fixed Main(): integer(4)
{
    heap token: Token = Token();
    local alias: Token = token;
    dispose token;
    alias.Value = 4;
    return 0;
}",
    );
    assert!(use_after.contains("used after 'dispose'"), "{use_after}");
}

#[test]
fn nolaa_supports_interprocedural_parameter_disposal() {
    let safe = check_source_ok(
        "\
class Token { public Value: integer(4); }

fixed Release(token: Token): integer(4)
{
    dispose token;
    return 0;
}

fixed Main(): integer(4)
{
    heap token: Token = Token();
    Release(token);
    return 0;
}",
    );
    assert!(safe.is_empty(), "{safe}");
    let stack = check_source(
        "\
class Token { public Value: integer(4); }

fixed Release(token: Token): integer(4)
{
    dispose token;
    return 0;
}

fixed Main(): integer(4)
{
    local token: Token = Token();
    Release(token);
    return 0;
}",
    );
    assert!(
        stack.contains("must point to a heap allocation unit"),
        "{stack}"
    );
    let double = check_source_ok(
        "\
class Token { public Value: integer(4); }

fixed Release(token: Token): integer(4)
{
    dispose token;
    return 0;
}

fixed Main(): integer(4)
{
    heap token: Token = Token();
    Release(token);
    dispose token;
    return 0;
}",
    );
    assert!(double.contains("disposed more than once"), "{double}");
}

#[test]
fn nolaa_supports_method_parameter_disposal() {
    let stderr = check_source_ok(
        "\
class Token { public Value: integer(4); }

class Helper
{
    extension
    {
        public fixed Release(this: &Helper, token: Token): integer(4)
        {
            dispose token;
            return 0;
        }
    }
}

fixed Main(): integer(4)
{
    heap token: Token = Token();
    local helper: Helper = Helper();
    helper.Release(token);
    return 0;
}",
    );
    assert!(stderr.is_empty(), "{stderr}");
}

#[test]
fn operand_assembly_pins_operands_and_clobbers_other_registers() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let directory = std::env::temp_dir().join(format!(
        "ainsc-cli-asm-operand-clobbers-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("program.ns");
    std::fs::write(
        &path,
        "fixed Main(): integer(4)\n{\n    local value: integer(4) = 1;\n    assembly { add @[value], 1 }\n    return value;\n}",
    )
    .expect("write");
    let output = ai_nsc()
        .args(["emit-ir", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(&directory);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let ir = String::from_utf8_lossy(&output.stdout);
    assert!(ir.contains("~{memory}"), "{ir}");
    assert!(
        !ir.contains("~{rax}"),
        "operand asm cannot clobber every GPR: {ir}"
    );
}

#[test]
fn nolaa_uses_read_only_parameter_summaries() {
    let read_only = check_source_ok(
        "\
class Token { public Value: integer(4); }

fixed Read(token: Token): integer(4)
{
    return token.Value;
}

fixed Main(): integer(4)
{
    heap token: Token = Token();
    local value: integer(4) = Read(token);
    return value;
}",
    );
    assert!(read_only.is_empty(), "{read_only}");
    let escaping = check_source_ok(
        "\
class Token { public Value: integer(4); }
class Holder { public Target: Token; }

fixed Store(token: Token, holder: Holder)
{
    holder.Target = token;
}

fixed Main(): integer(4)
{
    heap token: Token = Token();
    local holder: Holder = Holder();
    Store(token, holder);
    return 0;
}",
    );
    assert!(
        escaping.contains("never disposed"),
        "stored parameter must be treated as escaping: {escaping}"
    );
}

#[test]
fn environment_queries_require_statically_known_facts() {
    let stderr = check_source(
        "\
class Holder { public Facts: EnvironmentFacts; }

fixed IsLinux(facts: EnvironmentFacts): boolean
{
    return #(facts.PlatformIs(\"Linux\"));
}

fixed Test(holder: Holder): boolean
{
    return IsLinux(holder.Facts);
}

fixed Main(): integer(4)
{
    local holder: Holder = Holder();
    if (Test(holder))
    {
        return 1;
    }
    return 0;
}",
    );
    assert!(
        stderr.contains("requires statically known facts"),
        "{stderr}"
    );
}

#[test]
fn operand_assembly_emits_full_register_barriers_on_supported_architectures() {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let cases = [
        ("x86_64-unknown-linux-gnu", "add @[value], 1"),
        ("aarch64-unknown-linux-gnu", "add @[value], @[value], 1"),
        ("riscv64-unknown-linux-gnu", "addi @[value], @[value], 1"),
        (
            "armv7-unknown-linux-gnueabihf",
            "add @[value], @[value], #1",
        ),
    ];
    for (target, instruction) in cases {
        let directory = std::env::temp_dir().join(format!(
            "ainsc-cli-asm-pinned-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&directory).expect("create temp dir");
        let path = directory.join("program.ns");
        std::fs::write(
            &path,
            format!(
                "fixed Main(): integer(4)\n{{\n    local value: integer(4) = 1;\n    assembly {{ {instruction} }}\n    return value;\n}}"
            ),
        )
        .expect("write");
        let output = ai_nsc()
            .args([
                "emit-obj",
                "--target",
                target,
                path.to_str().expect("utf-8 path"),
            ])
            .output()
            .expect("run ai-nsc");
        let _ = std::fs::remove_dir_all(&directory);
        assert!(
            output.status.success(),
            "{target}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn uncaught_compile_time_throw_is_diagnosed() {
    let stderr = check_source(
        "\
#{ throw Exception(); }

fixed Main(): integer(4)
{
    return 0;
}",
    );
    assert!(
        stderr.contains("uncaught compile-time exception"),
        "{stderr}"
    );
}
