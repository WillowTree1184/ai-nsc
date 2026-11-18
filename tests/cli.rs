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
    local p: pointer integer(4) = Cast<pointer integer(4)>(values);
    return Cast<integer(4)>(p[1]);
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
    local made: Token;
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
    local p: pointer integer(4) = Cast<pointer integer(4)>(values);
    local raw: pointer = Cast<pointer>(p);
    return Cast<integer(4)>(Cast<pointer integer(4)>(raw)[1]);
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
    local bytes: array(3) cardinal(2) = Cast<array(3) cardinal(2)>(words);
    return Cast<integer(4)>(bytes[0]);
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
fn never_disposed_heap_allocation_warns_without_failing() {
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
    assert!(
        stderr.contains("never disposed"),
        "expected a never-disposed warning: {stderr}"
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
