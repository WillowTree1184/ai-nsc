//! End-to-end execution tests: compile NovaSharp programs and check the process
//! exit code, which is `Main`'s return value (§14.2).

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn ai_nsc() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ai-nsc"))
}

fn write_program(source: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "ainsc-e2e-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create temp dir");
    let path = directory.join("main.ns");
    std::fs::write(&path, source).expect("write program");
    path
}

fn run_program(source: &str) -> i32 {
    let path = write_program(source);
    let output = ai_nsc()
        .args(["run", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(path.parent().expect("temp dir"));
    assert!(
        output.status.success() || output.status.code().is_some(),
        "ai-nsc failed to run: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    if output.status.success() {
        output.status.code().unwrap_or(0)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("error:"),
            "compilation failed:\n{stderr}\n--- program ---\n{source}"
        );
        output.status.code().unwrap_or(1)
    }
}

#[test]
fn returns_a_constant() {
    assert_eq!(run_program("fixed Main(): integer(4) { return 42; }"), 42);
}

#[test]
fn evaluates_arithmetic_with_precedence() {
    assert_eq!(
        run_program("fixed Main(): integer(4) { return 1 + 2 * 3; }"),
        7
    );
}

#[test]
fn if_else_and_comparisons() {
    let source = "\
fixed Main(): integer(4)
{
    local x: integer(4) = 5;
    if (x > 10) { return 1; } else if (x == 5) { return 2; } else { return 3; }
}";
    assert_eq!(run_program(source), 2);
}

#[test]
fn while_loop_accumulates() {
    let source = "\
fixed Main(): integer(4)
{
    local sum: integer(4) = 0;
    local i: integer(4) = 1;
    while (i <= 5)
    {
        sum += i;
        i += 1;
    }
    return sum;
}";
    assert_eq!(run_program(source), 15);
}

#[test]
fn for_range_accumulates() {
    let source = "\
fixed Main(): integer(4)
{
    local sum: integer(4) = 0;
    for i: integer(4) in 1..5
    {
        sum += i;
    }
    return sum;
}";
    // 1..5 is the inclusive range 1,2,3,4,5 (§5.9.5).
    assert_eq!(run_program(source), 15);
}

#[test]
fn recursion_works() {
    let source = "\
fixed Factorial(n: integer(4)): integer(4)
{
    if (n <= 1) { return 1; }
    return n * Factorial(n - 1);
}

fixed Main(): integer(4)
{
    return Factorial(5);
}";
    assert_eq!(run_program(source), 120);
}

#[test]
fn arrays_and_indexes() {
    let source = "\
fixed Main(): integer(4)
{
    local values[3]: integer(4) = { 10, 20, 12 };
    local sum: integer(4) = 0;
    for value: integer(4) in values
    {
        sum += value;
    }
    return sum;
}";
    assert_eq!(run_program(source), 42);
}

#[test]
fn loops_can_break_and_continue() {
    let source = "\
fixed Main(): integer(4)
{
    local sum: integer(4) = 0;
    local i: integer(4) = 0;
    while (true)
    {
        i += 1;
        if (i == 3) { continue; }
        if (i > 5) { break; }
        sum += i;
    }
    return sum;
}";
    // 1 + 2 + 4 + 5 = 12
    assert_eq!(run_program(source), 12);
}

#[test]
fn match_statement_selects_a_case() {
    let source = "\
fixed Main(): integer(4)
{
    local x: integer(4) = 7;
    match x
    {
        case 1 { return 10; }
        case 7 { return 70; }
        case wildcard { return 99; }
    }
    return 0;
}";
    assert_eq!(run_program(source), 70);
}

#[test]
fn empty_main_returns_zero() {
    assert_eq!(run_program("fixed Main() { return; }"), 0);
}

#[test]
fn emit_ir_prints_a_verified_module() {
    let path = write_program("fixed Main(): integer(4) { return 7; }");
    let output = ai_nsc()
        .args(["emit-ir", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc emit-ir");
    let _ = std::fs::remove_dir_all(path.parent().expect("temp dir"));
    assert!(
        output.status.success(),
        "emit-ir failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let ir = String::from_utf8_lossy(&output.stdout);
    assert!(ir.contains("define"), "expected IR definitions:\n{ir}");
    assert!(ir.contains("@main"), "expected a launcher:\n{ir}");
    assert!(
        ir.contains("ret i32"),
        "expected the launcher return:\n{ir}"
    );
}

#[test]
fn structure_construction_and_methods() {
    let source = "\
structure Vector3
{
    public X: integer(4);
    public Y: integer(4);

    extension
    {
        public fixed Construct(this: &Vector3, x: integer(4), y: integer(4))
        {
            this.X = x;
            this.Y = y;
        }

        public fixed Sum(this: &Vector3): integer(4)
        {
            return this.X + this.Y;
        }
    }
}

fixed Main(): integer(4)
{
    local v: Vector3 = Vector3(3, 4);
    local w: Vector3 = Vector3(10, 20);
    return v.Sum() + w.Sum();
}";
    assert_eq!(run_program(source), 37);
}

#[test]
fn class_construction_and_methods() {
    let source = "\
class Counter
{
    private Count: integer(4);

    extension
    {
        public fixed Construct(this: &Counter, start: integer(4))
        {
            this.Count = start;
        }

        public fixed Increment(this: &Counter)
        {
            this.Count = this.Count + 1;
        }

        public fixed Value(this: &Counter): integer(4)
        {
            return this.Count;
        }
    }
}

fixed Main(): integer(4)
{
    local c: Counter = Counter(10);
    c.Increment();
    c.Increment();
    return c.Value();
}";
    assert_eq!(run_program(source), 12);
}

#[test]
fn heap_structure_and_dispose() {
    let source = "\
structure Pair
{
    public A: integer(4);

    extension
    {
        public fixed Construct(this: &Pair, a: integer(4))
        {
            this.A = a;
        }
    }
}

fixed Main(): integer(4)
{
    heap p: Pair = Pair(21);
    local value: integer(4) = p.A;
    dispose p;
    return value * 2;
}";
    assert_eq!(run_program(source), 42);
}

#[test]
fn heap_class_and_dispose() {
    let source = "\
class Box
{
    private Value: integer(4);

    extension
    {
        public fixed Construct(this: &Box, value: integer(4))
        {
            this.Value = value;
        }

        public fixed Get(this: &Box): integer(4)
        {
            return this.Value;
        }
    }
}

fixed Main(): integer(4)
{
    heap box: Box = Box(42);
    local result: integer(4) = box.Get();
    dispose box;
    return result;
}";
    assert_eq!(run_program(source), 42);
}

#[test]
fn field_initializers_run_before_the_constructor() {
    let source = "\
structure Point
{
    public X: integer(4) = 5;
    public Y: integer(4) = 7;

    extension
    {
        public fixed Sum(this: &Point): integer(4)
        {
            return this.X + this.Y;
        }
    }
}

fixed Main(): integer(4)
{
    local p: Point = Point();
    return p.Sum();
}";
    assert_eq!(run_program(source), 12);
}

#[test]
fn virtual_dispatch_uses_the_dynamic_type() {
    let source = "\
class Animal
{
    extension
    {
        public virtual fixed Sound(this: &Animal): integer(4)
        {
            return 1;
        }
    }
}

class Dog: Animal
{
    extension
    {
        public override fixed Sound(this: &Dog): integer(4)
        {
            return 2;
        }
    }
}

fixed Main(): integer(4)
{
    local dog: Dog = Dog();
    local animal: Animal = dog;
    return animal.Sound();
}";
    assert_eq!(run_program(source), 2);
}

#[test]
fn base_calls_bypass_virtual_dispatch() {
    let source = "\
class A
{
    extension
    {
        public virtual fixed Value(this: &A): integer(4)
        {
            return 10;
        }
    }
}

class B: A
{
    extension
    {
        public override fixed Value(this: &B): integer(4)
        {
            return 20;
        }

        public fixed Total(this: &B): integer(4)
        {
            return base.Value() + this.Value();
        }
    }
}

fixed Main(): integer(4)
{
    local b: B = B();
    return b.Total();
}";
    assert_eq!(run_program(source), 30);
}

#[test]
fn is_expression_checks_runtime_type() {
    let source = "\
class Base { }
class Derived: Base { }
class Other { }

fixed Main(): integer(4)
{
    local derived: Derived = Derived();
    local value: Base = derived;

    local result: integer(4) = 0;
    if (value is Derived) { result += 1; }
    if (value is Base) { result += 10; }
    if (value is Other) { result += 100; }
    return result;
}";
    assert_eq!(run_program(source), 11);
}

#[test]
fn sealed_class_cannot_be_inherited() {
    let source = "\
class Final
{
    sealed;
}

class Child: Final { }

fixed Main(): integer(4)
{
    return 0;
}";
    let path = write_program(source);
    let output = ai_nsc()
        .args(["check", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(path.parent().expect("temp dir"));
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("sealed"), "stderr: {stderr}");
}

#[test]
fn variants_and_match_is_patterns() {
    let source = "\
variant Status
{
    Idle;
    Loading: integer(4);
    Done;
}

fixed Describe(status: Status): integer(4)
{
    match status
    {
        case is Status.Idle { return 1; }
        case is value:Status.Loading { return value; }
        case is Status.Done { return 3; }
        case wildcard { return 0; }
    }
    return 0;
}

fixed Main(): integer(4)
{
    local idle: Status = Status.Idle;
    local loading: Status = Status.Loading(42);
    local done: Status = Status.Done;
    return Describe(idle) + Describe(loading) + Describe(done);
}";
    assert_eq!(run_program(source), 46);
}

#[test]
fn union_member_construction_and_punning() {
    let source = "\
union Pixel
{
    Packed: cardinal(4);
    Channels[4]: integer(1);
}

fixed Main(): integer(4)
{
    local pixel: Pixel = Pixel.Packed(0x01020304);
    local packed: cardinal(4) = pixel.Packed;
    local first: integer(4) = Cast<integer(4)>(pixel.Channels[0]);
    local low: integer(4) = Cast<integer(4)>(packed % 256);
    return low + first;
}";
    // Little-endian: the first channel is the low byte 0x04 = 4, so 4 + 4 = 8.
    assert_eq!(run_program(source), 8);
}

#[test]
fn variant_is_expression() {
    let source = "\
variant State
{
    Empty;
    Value: integer(4);
}

fixed Main(): integer(4)
{
    local empty: State = State.Empty;
    local with_value: State = State.Value(7);

    local result: integer(4) = 0;
    if (empty is State.Empty) { result += 1; }
    if (with_value is State.Value) { result += 10; }
    if (with_value is State.Empty) { result += 100; }
    return result;
}";
    assert_eq!(run_program(source), 11);
}
