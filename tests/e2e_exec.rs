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

#[test]
fn interfaces_dispatch_to_the_implementing_class() {
    let source = "\
interface IShape
{
    extension
    {
        public fixed Area(this: &IShape): integer(4);
        public fixed Corners(this: &IShape): integer(4);
    }
}

class Square: IShape
{
    private Side: integer(4);

    extension
    {
        public fixed Construct(this: &Square, side: integer(4)) { this.Side = side; }
        public fixed Area(this: &Square): integer(4) { return this.Side * this.Side; }
        public fixed Corners(this: &Square): integer(4) { return 4; }
    }
}

class Triangle: IShape
{
    extension
    {
        public fixed Area(this: &Triangle): integer(4) { return 6; }
        public fixed Corners(this: &Triangle): integer(4) { return 3; }
    }
}

fixed Main(): integer(4)
{
    local square: Square = Square(5);
    local triangle: Triangle = Triangle();
    local first: IShape = square;
    local second: IShape = triangle;
    return first.Area() + second.Corners();   // 25 + 3
}";
    assert_eq!(run_program(source), 28);
}

#[test]
fn interface_type_checks_use_rtti() {
    let source = "\
interface INamed
{
    extension
    {
        public fixed Id(this: &INamed): integer(4);
    }
}

class Thing: INamed
{
    extension
    {
        public fixed Id(this: &Thing): integer(4) { return 7; }
    }
}

class Other { }

fixed Main(): integer(4)
{
    local thing: Thing = Thing();
    local named: INamed = thing;
    local result: integer(4) = 0;
    if (named is INamed) { result += 1; }
    if (named is Thing) { result += 10; }
    if (named is Other) { result += 100; }
    return result + named.Id();   // 11 + 7
}";
    assert_eq!(run_program(source), 18);
}

#[test]
fn missing_interface_member_is_reported() {
    let source = "\
interface IShape
{
    extension
    {
        public fixed Area(this: &IShape): integer(4);
    }
}

class Broken: IShape
{
    extension
    {
        public fixed Perimeter(this: &Broken): integer(4) { return 0; }
    }
}

fixed Main(): integer(4) { return 0; }";
    let path = write_program(source);
    let output = ai_nsc()
        .args(["check", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(path.parent().expect("temp dir"));
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Area"), "stderr: {stderr}");
}

#[test]
fn accessors_read_and_write_fields() {
    let source = "\
structure Point
{
    private X: integer(4);

    extension
    {
        public fixed Construct(this: &Point, x: integer(4)) { this.X = x; }
        public accessor Value: integer(4) => X { get; set; }
    }
}

class Counter
{
    private Count: integer(4);

    extension
    {
        public fixed Construct(this: &Counter, count: integer(4)) { this.Count = count; }
        public accessor Value: integer(4)
        {
            get() { return this.Count; }
            set(value) { this.Count = value; }
        }
    }
}

fixed Main(): integer(4)
{
    local point: Point = Point(3);
    point.Value = 5;
    local counter: Counter = Counter(10);
    counter.Value = counter.Value + 2;
    return point.Value + counter.Value;   // 5 + 12
}";
    assert_eq!(run_program(source), 17);
}

#[test]
fn interface_accessor_dispatches_virtually() {
    let source = "\
interface IThing
{
    extension
    {
        public accessor Value: integer(4) { get; set; }
    }
}

class Thing: IThing
{
    private Stored: integer(4);

    extension
    {
        public fixed Construct(this: &Thing, value: integer(4)) { this.Stored = value; }
        public accessor Value: integer(4) => Stored { get; set; }
    }
}

fixed Main(): integer(4)
{
    local thing: Thing = Thing(4);
    local first: IThing = thing;
    first.Value = first.Value + 3;
    return first.Value;   // 7
}";
    assert_eq!(run_program(source), 7);
}

#[test]
fn accessor_without_backing_needs_an_implementation() {
    let source = "\
structure Broken
{
    extension
    {
        public accessor Value: integer(4) { get; }
    }
}

fixed Main(): integer(4) { return 0; }";
    let path = write_program(source);
    let output = ai_nsc()
        .args(["check", path.to_str().expect("utf-8 path")])
        .output()
        .expect("run ai-nsc");
    let _ = std::fs::remove_dir_all(path.parent().expect("temp dir"));
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("implementation"), "stderr: {stderr}");
}

#[test]
fn operators_dispatch_to_definitions() {
    let source = "\
structure Vec2
{
    public X: integer(4);
    public Y: integer(4);

    extension
    {
        public fixed Construct(this: &Vec2, x: integer(4), y: integer(4))
        {
            this.X = x;
            this.Y = y;
        }

        operator +(left: Vec2, right: Vec2): Vec2
        {
            return Vec2(left.X + right.X, left.Y + right.Y);
        }

        operator ==(left: Vec2, right: Vec2): boolean
        {
            return left.X == right.X && left.Y == right.Y;
        }

        operator <(left: Vec2, right: Vec2): boolean
        {
            return left.X < right.X;
        }

        operator -(operand: Vec2): Vec2
        {
            return Vec2(-operand.X, -operand.Y);
        }
    }
}

fixed Main(): integer(4)
{
    local a: Vec2 = Vec2(1, 2);
    local b: Vec2 = Vec2(3, 4);
    local c: Vec2 = a + b;
    local negated: Vec2 = -a;

    local score: integer(4) = c.X * 10 + c.Y + negated.Y;   // 46 - 2 = 44
    if (a == a) { score += 1; }
    if (a != b) { score += 2; }
    if (a < b) { score += 4; }
    if (a <= a) { score += 8; }
    return score;   // 44 + 15 = 59
}";
    assert_eq!(run_program(source), 59);
}

#[test]
fn indexer_operator_reads_and_writes() {
    let source = "\
structure Grid
{
    private Data[4]: integer(4);

    extension
    {
        public fixed Construct(this: &Grid, a: integer(4), b: integer(4), c: integer(4), d: integer(4))
        {
            this.Data[0] = a;
            this.Data[1] = b;
            this.Data[2] = c;
            this.Data[3] = d;
        }

        operator [index: integer(4)]: integer(4)
        {
            get(this: &Grid) { return this.Data[index]; }
            set(this: &Grid, value: integer(4)) { this.Data[index] = value; }
        }
    }
}

fixed Main(): integer(4)
{
    local grid: Grid = Grid(10, 20, 30, 40);
    grid[1] = grid[1] + 5;
    return grid[1] + grid[3];   // 25 + 40
}";
    assert_eq!(run_program(source), 65);
}

#[test]
fn conversion_operator_handles_casts() {
    let source = "\
structure Celsius { public Degrees: integer(4); extension {
    public fixed Construct(this: &Celsius, degrees: integer(4)) { this.Degrees = degrees; }
} }

structure Fahrenheit
{
    public Degrees: integer(4);

    extension
    {
        public fixed Construct(this: &Fahrenheit, degrees: integer(4)) { this.Degrees = degrees; }

        operator Fahrenheit(value: Celsius): Fahrenheit
        {
            return Fahrenheit(value.Degrees * 9 / 5 + 32);
        }
    }
}

fixed Main(): integer(4)
{
    local celsius: Celsius = Celsius(100);
    local fahrenheit: Fahrenheit = Cast<Fahrenheit>(celsius);
    return fahrenheit.Degrees;   // 212
}";
    assert_eq!(run_program(source), 212);
}

#[test]
fn throw_is_caught_by_matching_trap() {
    let source = "\
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
}";
    assert_eq!(run_program(source), 42);
}

#[test]
fn catch_matches_base_classes_and_runs_finally() {
    let source = "\
class AppError
{
    extension
    {
        public virtual fixed Code(this: &AppError): integer(4) { return 1; }
    }
}

class NotFound: AppError
{
    extension
    {
        public override fixed Code(this: &NotFound): integer(4) { return 7; }
    }
}

fixed Main(): integer(4)
{
    local result: integer(4) = 0;
    trap
    {
        local problem: NotFound = NotFound();
        throw problem;
    }
    catch error: AppError
    {
        result = error.Code();
    }
    catch wildcard
    {
        result = 99;
    }
    finally
    {
        result += 1;
    }
    return result;
}";
    assert_eq!(run_program(source), 8);
}

#[test]
fn finally_runs_on_normal_and_handled_paths() {
    let source = "\
fixed Main(): integer(4)
{
    local log: integer(4) = 0;
    trap
    {
        throw 1;
    }
    catch wildcard
    {
        log += 10;
    }
    finally
    {
        log += 20;
    }
    trap
    {
        log += 1;
    }
    catch wildcard
    {
        log += 100;
    }
    finally
    {
        log += 4;
    }
    return log;
}";
    assert_eq!(run_program(source), 35);
}

#[test]
fn unmatched_exception_propagates_to_outer_trap() {
    let source = "\
fixed Main(): integer(4)
{
    local result: integer(4) = 0;
    local code: integer(4) = 7;
    trap
    {
        trap
        {
            throw code;
        }
        catch value: boolean
        {
            result = 100;
        }
    }
    catch value: integer(4)
    {
        result = value;
    }
    return result;
}";
    assert_eq!(run_program(source), 7);
}

#[test]
fn throw_outside_trap_is_reported() {
    let source = "\
fixed Main(): integer(4)
{
    throw 1;
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
    assert!(stderr.contains("trap"), "stderr: {stderr}");
}

#[test]
fn division_by_zero_raises_builtin_exception() {
    let source = "\
fixed Main(): integer(4)
{
    local result: integer(4) = 0;
    local zero: integer(4) = 0;
    trap
    {
        result = 10 / zero;
        result = 1;
    }
    catch error: DivideByZeroException
    {
        result = 42;
    }
    return result;
}";
    assert_eq!(run_program(source), 42);
}

#[test]
fn out_of_range_index_raises_catchable_exception() {
    let source = "\
fixed Main(): integer(4)
{
    local result: integer(4) = 0;
    local values[3]: integer(4);
    values[0] = 1;
    values[1] = 2;
    values[2] = 3;
    local position: integer(4) = 5;
    trap
    {
        result = values[position];
    }
    catch error: OutOfRangeException
    {
        result = 7;
    }
    return result;
}";
    assert_eq!(run_program(source), 7);
}

#[test]
fn builtin_exceptions_match_their_base_class() {
    let source = "\
fixed Main(): integer(4)
{
    local result: integer(4) = 0;
    local zero: integer(4) = 0;
    trap
    {
        result = 1 / zero;
    }
    catch error: Exception
    {
        result = 9;
    }
    return result;
}";
    assert_eq!(run_program(source), 9);
}

#[test]
fn compile_time_if_selects_a_branch() {
    let source = "\
fixed UseExtra: boolean = true;
fixed Mode: integer(4) = 2;

fixed Main(): integer(4)
{
    local result: integer(4) = 0;

    #if UseExtra
    {
        result += 5;
    }
    else
    {
        result += 100;
    }

    #if Mode == 2
    {
        result += 7;
    }
    else
    {
        result += 1000;
    }

    #if !false && (Mode > 1)
    {
        result += 1;
    }

    return result;
}";
    assert_eq!(run_program(source), 13);
}

#[test]
fn untaken_compile_time_branch_is_not_checked() {
    let source = "\
fixed Main(): integer(4)
{
    local result: integer(4) = 1;
    #if false
    {
        local broken: boolean = 12345;
    }
    else
    {
        result += 2;
    }
    return result;
}";
    assert_eq!(run_program(source), 3);
}

// ----------------------------------------------------------------------
// Generics and monomorphization (§10)
// ----------------------------------------------------------------------

#[test]
fn generic_structure_method_works_for_two_instantiations() {
    let source = "\
structure Box<T>
{
    public Value: T;

    extension
    {
        public fixed Get(this: &Box<T>): T
        {
            return this.Value;
        }
    }
}

fixed Main(): integer(4)
{
    local box: Box<integer(4)>;
    box.Value = 42;
    local other: Box<cardinal(8)>;
    other.Value = 7;
    local sum: integer(8) = Cast<integer(8)>(other.Value);
    return box.Get() + Cast<integer(4)>(sum);
}";
    assert_eq!(run_program(source), 49);
}

#[test]
fn generic_function_instantiations_coexist() {
    let source = "\
fixed Echo<T>(value: T): T
{
    return value;
}

fixed Main(): integer(4)
{
    local a: integer(4) = Echo<integer(4)>(7);
    local b: cardinal(8) = Echo<cardinal(8)>(5);
    return a + Cast<integer(4)>(b);
}";
    assert_eq!(run_program(source), 12);
}

#[test]
fn generic_class_construction_and_fields() {
    let source = "\
class Box<T>
{
    public Value: T;

    extension
    {
        public fixed Construct(this: &Box<T>, value: T)
        {
            this.Value = value;
        }
    }
}

fixed Main(): integer(4)
{
    local box: Box<integer(4)> = Box<integer(4)>(33);
    local other: Box<cardinal(8)> = Box<cardinal(8)>(9);
    return box.Value;
}";
    assert_eq!(run_program(source), 33);
}

#[test]
fn generic_extension_target_adds_members() {
    let source = "\
structure Box<T>
{
    public Value: T;
}

extension for Box<T>
{
    public fixed Get(this: &Box<T>): T
    {
        return this.Value;
    }

    public fixed Set(this: &Box<T>, value: T)
    {
        this.Value = value;
    }
}

fixed Main(): integer(4)
{
    local box: Box<integer(4)>;
    box.Set(11);
    return box.Get();
}";
    assert_eq!(run_program(source), 11);
}

#[test]
fn generic_interface_dispatches_to_the_implementation() {
    let source = "\
interface IComparable<T>
{
    extension
    {
        public fixed Compare(this: &IComparable<T>, other: T): integer(4);
    }
}

class IntBox : IComparable<integer(4)>
{
    public Value: integer(4);

    extension
    {
        public fixed Construct(this: &IntBox, value: integer(4))
        {
            this.Value = value;
        }

        public fixed Compare(this: &IntBox, other: integer(4)): integer(4)
        {
            return this.Value - other;
        }
    }
}

fixed Describe(value: IComparable<integer(4)>): integer(4)
{
    return value.Compare(5);
}

fixed Main(): integer(4)
{
    local box: IntBox = IntBox(9);
    return Describe(box);
}";
    assert_eq!(run_program(source), 4);
}

#[test]
fn generic_variant_construction_and_match() {
    let source = "\
variant Maybe<T>
{
    Nothing;
    Some: T;

    extension
    {
        public fixed UnwrapOr(this: &Maybe<T>, fallback: T): T
        {
            match this
            {
                case is value:Maybe<T>.Some { return value; }
                case wildcard { return fallback; }
            }
            return fallback;
        }
    }
}

fixed Main(): integer(4)
{
    local value: Maybe<integer(4)> = Maybe<integer(4)>.Some(12);
    return value.UnwrapOr(3);
}";
    assert_eq!(run_program(source), 12);
}

#[test]
fn generic_base_class_dispatches_virtually() {
    let source = "\
class Holder<T>
{
    public Value: T;

    extension
    {
        public virtual fixed Describe(this: &Holder<T>): integer(4)
        {
            return 1;
        }
    }
}

class IntHolder : Holder<integer(4)>
{
    extension
    {
        public override fixed Describe(this: &IntHolder): integer(4)
        {
            return 2;
        }
    }
}

fixed Main(): integer(4)
{
    local derived: IntHolder = IntHolder();
    local base_value: Holder<integer(4)> = derived;
    return base_value.Describe();
}";
    assert_eq!(run_program(source), 2);
}

#[test]
fn nested_generic_instantiations() {
    let source = "\
structure Pair<T; U>
{
    public First: T;
    public Second: U;
}

structure Nested<T>
{
    public Inner: Pair<T; integer(4)>;
}

fixed Main(): integer(4)
{
    local pair: Nested<integer(4)>;
    pair.Inner.First = 5;
    pair.Inner.Second = 6;
    return pair.Inner.First + pair.Inner.Second;
}";
    assert_eq!(run_program(source), 11);
}

#[test]
fn generic_constraint_kind_accepts_matching_argument() {
    let source = "\
structure Point
{
    public X: integer(4);
}

structure Box<T: structure>
{
    public Value: T;
}

fixed Main(): integer(4)
{
    local box: Box<Point>;
    box.Value.X = 8;
    return box.Value.X;
}";
    assert_eq!(run_program(source), 8);
}

#[test]
fn generic_constraint_named_type_accepts_implementations() {
    let source = "\
interface IValue
{
    extension
    {
        public fixed Get(this: &IValue): integer(4);
    }
}

class Thing : IValue
{
    extension
    {
        public fixed Get(this: &Thing): integer(4) { return 5; }
    }
}

structure Wrapper<T: IValue>
{
    public Value: T;
}

fixed Main(): integer(4)
{
    local thing: Thing = Thing();
    local wrapper: Wrapper<Thing>;
    wrapper.Value = thing;
    return wrapper.Value.Get();
}";
    assert_eq!(run_program(source), 5);
}

// ----------------------------------------------------------------------
// Defect regressions (see docs/AUDIT.md)
// ----------------------------------------------------------------------

#[test]
fn enumeration_values_and_casts_round_trip() {
    let source = "\
enumeration Status: cardinal(2) { Idle; Running; Done = 7; }

fixed ReturnShort(): cardinal(2)
{
    return Status.Running;
}

fixed Main(): integer(4)
{
    local state: Status = Status.Done;
    local raw: cardinal(2) = state;
    local back: Status = Cast<Status>(raw);
    return Cast<integer(4)>(ReturnShort())
        + Cast<integer(4)>(Status.Idle)
        + Cast<integer(4)>(back);
}";
    // Running = 1, Idle = 0, back = Done = 7.
    assert_eq!(run_program(source), 8);
}

#[test]
fn match_break_and_continue_target_the_match() {
    let source = "\
fixed Main(): integer(4)
{
    local result: integer(4) = 0;
    for i: integer(4) in 1..4
    {
        match i
        {
            case 2 { continue; }
            case 3 { break; }
            case wildcard { result += i; }
        }
        result += 10;
    }
    return result;
}";
    // continue retries the following cases; break leaves the match, not the for.
    assert_eq!(run_program(source), 47);
}

#[test]
fn named_match_label_continues_to_the_next_case() {
    let source = "\
fixed Main(): integer(4)
{
    local result: integer(4) = 0;
    match 1 : Pick
    {
        case 1 { result += 1; continue Pick; }
        case 2 { result += 2; }
        case wildcard { result += 4; }
    }
    return result;
}";
    assert_eq!(run_program(source), 5);
}

#[test]
fn match_patterns_use_user_defined_operators() {
    let source = "\
structure Vec2
{
    public X: integer(4);
    public Y: integer(4);

    extension
    {
        public fixed Construct(this: &Vec2, x: integer(4), y: integer(4))
        {
            this.X = x;
            this.Y = y;
        }

        operator ==(left: Vec2, right: Vec2): boolean { return left.X == right.X; }
        operator <(left: Vec2, right: Vec2): boolean { return left.X < right.X; }
    }
}

fixed Main(): integer(4)
{
    local target: Vec2 = Vec2(2, 9);
    local result: integer(4) = 0;
    match target
    {
        case Vec2(1, 1) { result = 1; }
        case Vec2(2, 7) { result = 2; }
        case < Vec2(5, 0) { result += 10; }
        case wildcard { result += 100; }
    }
    return result;
}";
    assert_eq!(run_program(source), 2);
}

#[test]
fn derived_match_comparison_uses_ordering_and_equality() {
    let source = "\
structure Vec2
{
    public X: integer(4);
    public Y: integer(4);

    extension
    {
        public fixed Construct(this: &Vec2, x: integer(4), y: integer(4))
        {
            this.X = x;
            this.Y = y;
        }

        operator ==(left: Vec2, right: Vec2): boolean { return left.X == right.X; }
        operator <(left: Vec2, right: Vec2): boolean { return left.X < right.X; }
    }
}

fixed Main(): integer(4)
{
    local target: Vec2 = Vec2(2, 9);
    local result: integer(4) = 0;
    match target
    {
        case < Vec2(1, 0) { result = 1; }
        case <= Vec2(2, 0) { result = 2; }
        case wildcard { result = 3; }
    }
    return result;
}";
    assert_eq!(run_program(source), 2);
}

#[test]
fn unmatched_inner_catch_propagates_to_the_outer_trap() {
    let source = "\
fixed Main(): integer(4)
{
    local result: integer(4) = 0;
    trap
    {
        trap
        {
            local value: integer(4) = 6;
            throw value;
        }
        catch divided: DivideByZeroException
        {
            result = 1;
        }
    }
    catch value: integer(4)
    {
        result = value;
    }
    catch wildcard
    {
        result = 99;
    }
    return result;
}";
    assert_eq!(run_program(source), 6);
}

#[test]
fn array_pointer_casts_reference_the_same_storage() {
    let source = "\
fixed Main(): integer(4)
{
    local values[3]: integer(4) = { 1, 2, 3 };
    local p: pointer integer(4) = Cast<pointer integer(4)>(values);
    p[0] = 42;
    Cast<array(3) integer(4)>(p)[1] = 7;
    return values[0] + values[1] + p[2];
}";
    assert_eq!(run_program(source), 52);
}

#[test]
fn float_to_integer_cast_wraps_out_of_range_values() {
    let source = "\
fixed Main(): integer(4)
{
    local wide: float(8) = Cast<float(8)>(300);
    return Cast<integer(4)>(Cast<integer(1)>(wide));
}";
    // 300 wraps modulo 256 to 44 (§3.2).
    assert_eq!(run_program(source), 44);
}

#[test]
fn byte_escapes_are_raw_bytes() {
    let source = "\
fixed Main(): integer(4)
{
    local bytes: array(2) cardinal(1) = \"\\xFF\\x41\";
    return Cast<integer(4)>(bytes[0]) + Cast<integer(4)>(bytes[1]);
}";
    // 0xFF + 0x41 = 320, truncated to the process exit code.
    assert_eq!(run_program(source), 320 % 256);
}

#[test]
fn pointer_arithmetic_uses_the_pointee_size() {
    let source = "\
fixed Main(): integer(4)
{
    local values[4]: integer(4) = { 10, 20, 30, 40 };
    local first: pointer integer(4) = Cast<pointer integer(4)>(values);
    local second: pointer integer(4) = first + 1;
    local third: pointer integer(4) = 1 + second;
    local back: pointer integer(4) = third - 1;
    local count: integer(8) = third - first;
    return Cast<integer(4)>(second[0])
        + Cast<integer(4)>(third[0])
        + Cast<integer(4)>(back[0])
        + Cast<integer(4)>(count);
}";
    // 20 + 30 + 20 + 2
    assert_eq!(run_program(source), 72);
}

#[test]
fn derived_constructor_calls_the_base_constructor() {
    let source = "\
class Base
{
    public X: integer(4);

    extension
    {
        public fixed Construct(this: &Base, x: integer(4))
        {
            this.X = x;
        }
    }
}

class Derived : Base
{
    public Y: integer(4);

    extension
    {
        public fixed Construct(this: &Derived, y: integer(4))
        {
            base.Construct(y * 2);
            this.Y = y;
        }
    }
}

fixed Main(): integer(4)
{
    local d: Derived = Derived(5);
    return d.X + d.Y;
}";
    // Base constructor sets X = 10, derived sets Y = 5.
    assert_eq!(run_program(source), 15);
}

#[test]
fn owner_accesses_shadowed_outer_scopes() {
    let source = "\
local a: integer(4) = 1;

fixed Main(): integer(4)
{
    local a: integer(4) = 2;
    local b: integer(4) = a;
    local c: integer(4) = owner.a;
    {
        local a: integer(8) = 3;
        local d: integer(4) = owner.owner.a;
        b = b + Cast<integer(4)>(owner.a);
        return b + c * 10 + d * 100;
    }
}";
    // b = 2 + 2, c = global 1, d = global 1.
    assert_eq!(run_program(source), 114);
}

#[test]
fn owner_is_a_valid_assignment_target() {
    let source = "\
local total: integer(4) = 1;

fixed Main(): integer(4)
{
    local total: integer(4) = 2;
    owner.total = total + 5;
    return owner.total * 10 + total;
}";
    // The global becomes 7, the local stays 2.
    assert_eq!(run_program(source), 72);
}

#[test]
fn use_imports_module_members_into_the_scope() {
    let source = "\
module Settings
{
    local Flag: boolean = true;
    local Limit: integer(4) = 9;
}

module Maths
{
    fixed Double(value: integer(4)): integer(4)
    {
        return value * 2;
    }

    structure Point
    {
        public X: integer(4);
        public Y: integer(4);
    }
}

fixed Main(): integer(4)
{
    use Settings, Maths;

    local point: Point;
    point.X = Double(3);
    point.Y = Double(2);

    if (Flag && Limit == 9)
    {
        return point.X + point.Y;
    }
    return 0;
}";
    assert_eq!(run_program(source), 10);
}

#[test]
fn use_works_at_module_level_for_later_declarations() {
    let source = "\
module Types
{
    structure Item
    {
        public Value: integer(4);
    }
}

module Container
{
    use Types;

    structure Holder
    {
        public Item: Item;
    }
}

fixed Main(): integer(4)
{
    use Container;
    local holder: Holder;
    holder.Item.Value = 11;
    return holder.Item.Value;
}";
    assert_eq!(run_program(source), 11);
}

#[test]
fn match_and_if_branches_satisfy_the_return_judgement() {
    let source = "\
fixed Pick(value: integer(4)): integer(4)
{
    match value
    {
        case 1 { return 10; }
        case wildcard { return 20; }
    }
}

fixed Choose(flag: boolean): integer(4)
{
    if (flag) { return 1; } else { return 2; }
}

fixed Main(): integer(4) { return Pick(1) + Choose(false); }";
    assert_eq!(run_program(source), 12);
}

#[test]
fn dispose_runs_the_destructor_of_a_scoped_value() {
    let source = "\
local destroyed: integer(4) = 0;

structure Token
{
    public Value: integer(4);

    extension
    {
        public fixed Destruct(this: &Token)
        {
            destroyed = 1;
        }
    }
}

fixed Main(): integer(4)
{
    local made: Token;
    made.Value = 5;
    dispose made;
    return destroyed * 10 + made.Value;
}";
    assert_eq!(run_program(source), 15);
}

#[test]
fn uninitialized_is_a_valid_storage_initial_value() {
    let source = "\
fixed Main(): integer(4)
{
    local buffer: integer(4) = uninitialized;
    buffer = 7;
    return buffer;
}";
    assert_eq!(run_program(source), 7);
}

#[test]
fn nested_fixed_arrays_flatten_to_combined_dimensions() {
    let source = "\
fixed Main(): integer(4)
{
    local matrix: array(2) array(3) integer(4);
    local flat: array(2, 3) integer(4);
    matrix[1][2] = 5;
    flat = matrix;
    matrix[0][1] = 4;
    return flat[1][2] + flat[0][1];
}";
    // The assignment copies the whole flattened array; the later write to
    // `matrix` is not visible through `flat`.
    assert_eq!(run_program(source), 5);
}

#[test]
fn variant_state_names_elide_the_type_in_context() {
    let source = "\
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
        case is Status.Idle { return 3; }
        case is Status.Busy { return 5; }
        case wildcard { return 7; }
    }
}

fixed Main(): integer(4)
{
    local idle: Status = Idle;
    local done: Status = Done;
    return Describe(idle) + Describe(done);
}";
    assert_eq!(run_program(source), 10);
}

#[test]
fn variant_payload_state_names_elide_the_type() {
    let source = "\
variant Maybe
{
    Nothing;
    Some: integer(4);
}

fixed UnwrapOr(value: Maybe, fallback: integer(4)): integer(4)
{
    match value
    {
        case is payload: Maybe.Some { return payload; }
        case wildcard { return fallback; }
    }
}

fixed Main(): integer(4)
{
    local present: Maybe = Some(9);
    local empty: Maybe = Nothing;
    return UnwrapOr(present, 1) + UnwrapOr(empty, 2);
}";
    assert_eq!(run_program(source), 11);
}

#[test]
fn uninitialized_is_allowed_as_a_global_initial_value() {
    let source = "\
local buffer: integer(4) = uninitialized;

fixed Main(): integer(4)
{
    buffer = 4;
    return buffer;
}";
    assert_eq!(run_program(source), 4);
}

#[test]
fn pointer_compound_assignment_scales_by_the_pointee_size() {
    let source = "\
fixed Main(): integer(4)
{
    local values[4]: integer(4) = { 10, 20, 30, 40 };
    local p: pointer integer(4) = Cast<pointer integer(4)>(values);
    p += 2;
    p -= 1;
    return Cast<integer(4)>(p[0]);
}";
    assert_eq!(run_program(source), 20);
}

#[test]
fn heap_type_is_inferred_from_the_initializer() {
    let source = "\
structure Item
{
    public Value: integer(4);
}

fixed Main(): integer(4)
{
    heap item = Item();
    item.Value = 7;
    local result: integer(4) = item.Value;
    dispose item;
    return result;
}";
    assert_eq!(run_program(source), 7);
}

#[test]
fn type_size_of_reports_memory_sizes() {
    let source = "\
structure Vector3
{
    public X: float(4);
    public Y: float(4);
    public Z: float(4);
}

fixed Main(): integer(4)
{
    local size: cardinal(8) = Type.SizeOf<Vector3>();
    local int_size: cardinal(8) = Type.SizeOf<integer(4)>();
    return Cast<integer(4)>(size) + Cast<integer(4)>(int_size);
}";
    assert_eq!(run_program(source), 16);
}

#[test]
fn compile_time_expressions_evaluate_constants() {
    let source = "\
structure Vector3
{
    public X: float(4);
    public Y: float(4);
    public Z: float(4);
}

fixed Main(): integer(4)
{
    local size: cardinal(8) = #(Type.SizeOf<Vector3>());
    local sum: integer(4) = #(1 + 2 * 3);
    return Cast<integer(4)>(size) + sum;
}";
    assert_eq!(run_program(source), 19);
}

#[test]
fn fixed_array_shapes_convert_explicitly() {
    let source = "\
fixed Main(): integer(4)
{
    local words[2]: cardinal(4) = { 7, 0 };
    local bytes: array(4) cardinal(2) = Cast<array(4) cardinal(2)>(words);
    return Cast<integer(4)>(bytes[0]) + Cast<integer(4)>(bytes[1]);
}";
    assert_eq!(run_program(source), 7);
}

#[test]
fn compile_time_block_locals_are_constants() {
    let source = "\
fixed Main(): integer(4)
{
    #{
        local size: integer(4) = 3;
        local values: array(size) integer(4);
        values[0] = 5;
        values[size - 1] = 2;
        return values[0] * 10 + values[size - 1];
    }
}";
    assert_eq!(run_program(source), 52);
}

#[test]
fn variants_compare_by_state_and_payload() {
    let source = "\
variant Maybe
{
    Nothing;
    Some: integer(4);
}

fixed Main(): integer(4)
{
    local a: Maybe = Some(5);
    local b: Maybe = Some(5);
    local c: Maybe = Some(6);
    local d: Maybe = Nothing;
    local score: integer(4) = 0;
    if (a == b) { score += 1; }
    if (a != c) { score += 2; }
    if (a != d) { score += 4; }
    if (d == d) { score += 8; }
    return score;
}";
    assert_eq!(run_program(source), 15);
}

#[test]
fn by_reference_parameters_write_back_to_the_caller() {
    let source = "\
fixed Scale(value: &integer(4), factor: integer(4))
{
    value = value * factor;
}

fixed Main(): integer(4)
{
    local number: integer(4) = 6;
    Scale(number, 7);
    return number;
}";
    assert_eq!(run_program(source), 42);
}

#[test]
fn method_by_reference_parameters_alias_the_caller() {
    let source = "\
structure Counter
{
    public Value: integer(4);

    extension
    {
        public fixed Bump(this: &Counter, amount: &integer(4))
        {
            this.Value += amount;
            amount += 1;
        }
    }
}

fixed Main(): integer(4)
{
    local counter: Counter;
    counter.Value = 0;
    local step: integer(4) = 3;
    counter.Bump(step);
    counter.Bump(step);
    return counter.Value * 10 + step;
}";
    assert_eq!(run_program(source), 75);
}

#[test]
fn for_by_reference_bindings_write_the_elements() {
    let source = "\
fixed Main(): integer(4)
{
    local values[3]: integer(4) = { 1, 2, 3 };
    for &value: integer(4) in values
    {
        value = value * 10;
    }
    return values[0] + values[1] + values[2];
}";
    assert_eq!(run_program(source), 60);
}

#[test]
fn compile_time_expressions_accept_non_integer_constants() {
    let source = "\
fixed Flag: boolean = true;
fixed Pi: float(8) = 3.5;

fixed Main(): integer(4)
{
    local mark: boolean = #(Flag);
    local ratio: float(8) = #(Pi * 2.0);
    local casted: integer(4) = #(Cast<integer(4)>(3.9));
    if (mark) { return casted + Cast<integer(4)>(ratio); }
    return 0;
}";
    assert_eq!(run_program(source), 10);
}

#[test]
fn array_parameter_sugar_expands_to_pointer_and_length() {
    let source = "\
fixed FirstByte(message[length: cardinal(4)]: cardinal(1)): integer(4)
{
    if (length < 1) { return 0; }
    return Cast<integer(4)>(message[0]);
}

fixed Main(): integer(4)
{
    local text[3]: cardinal(1) = { 97, 98, 99 };
    return FirstByte(Cast<pointer cardinal(1)>(text), 3);
}";
    assert_eq!(run_program(source), 97);
}

#[test]
fn compiler_api_diagnostics_run_at_compile_time() {
    let source = "\
fixed Main(): integer(4)
{
    #{
        Compiler.Assert(true);
        Compiler.LogNote(\"checked\");
    }
    return 0;
}";
    assert_eq!(run_program(source), 0);
}

#[test]
fn static_fields_and_methods_share_class_level_storage() {
    let source = "\
class Counter
{
    public static Total: integer(4) = 10;

    extension
    {
        public static fixed Bump(amount: integer(4)): integer(4)
        {
            Total += amount;
            return Total;
        }
    }
}

fixed Main(): integer(4)
{
    Counter.Bump(1);
    Counter.Bump(2);
    return Counter.Total;
}";
    assert_eq!(run_program(source), 13);
}

#[test]
fn static_fields_are_read_and_written_through_the_class() {
    let source = "\
class Config
{
    public static Limit: integer(4) = 7;
}

fixed Main(): integer(4)
{
    Config.Limit = Config.Limit + 1;
    return Config.Limit;
}";
    assert_eq!(run_program(source), 8);
}

#[test]
fn static_constants_are_class_level_values() {
    let source = "\
class Limits
{
    extension
    {
        public static fixed Max: integer(4) = 9;
    }
}

fixed Main(): integer(4)
{
    return Limits.Max;
}";
    assert_eq!(run_program(source), 9);
}

#[test]
fn static_class_members_and_accessor_alias() {
    let source = "\
static class Counter
{
    public static Total: integer(4) = 5;

    extension
    {
        public static accessor Count: integer(4) => Total;

        public static fixed Add(amount: integer(4))
        {
            Total += amount;
        }
    }
}

fixed Main(): integer(4)
{
    Counter.Add(3);
    return Counter.Count;
}";
    assert_eq!(run_program(source), 8);
}

#[test]
fn optional_coalescing_takes_value_or_alternative() {
    let source = "\
fixed Main(): integer(4)
{
    local present: Optional<integer(4)> = Optional<integer(4)>.Value(42);
    local empty: Optional<integer(4)> = Optional<integer(4)>.Null;
    local a: integer(4) = present ?? 7;
    local b: integer(4) = empty ?? 7;
    return a + b;
}";
    assert_eq!(run_program(source), 49);
}

#[test]
fn result_coalescing_supports_throw_and_return() {
    let throw_source = "\
fixed Unwrap(result: Result<integer(4); integer(4)>): integer(4)
{
    trap
    {
        return result ?? throw;
    }
    catch error: integer(4)
    {
        return 100 + error;
    }
}

fixed Main(): integer(4)
{
    local ok: Result<integer(4); integer(4)> = Result<integer(4); integer(4)>.Value(4);
    local bad: Result<integer(4); integer(4)> = Result<integer(4); integer(4)>.Alternative(7);
    return Unwrap(ok) + Unwrap(bad);
}";
    assert_eq!(run_program(throw_source), 111);

    let return_source = "\
fixed Scale(result: Result<integer(4); Exception>): Result<integer(4); Exception>
{
    local value: integer(4) = result ?? return;
    return Result<integer(4); Exception>.Value(value * 10);
}

fixed Main(): integer(4)
{
    local ok: Result<integer(4); Exception> = Result<integer(4); Exception>.Value(5);
    local bad: Result<integer(4); Exception> = Result<integer(4); Exception>.Alternative(Exception());
    local scaled: Result<integer(4); Exception> = Scale(ok);
    local a: integer(4) = scaled ?? 0;
    local b: integer(4) = Scale(bad) ?? -1;
    return a - b;
}";
    assert_eq!(run_program(return_source), 51);
}

#[test]
fn user_defined_decision_operator_drives_coalescing() {
    let source = "\
structure Gauge
{
    public Count: integer(4);

    extension
    {
        operator ??(value: Gauge): Result<integer(4); integer(4)>
        {
            if (value.Count > 0)
            {
                return Result<integer(4); integer(4)>.Value(value.Count);
            }
            return Result<integer(4); integer(4)>.Alternative(0);
        }
    }
}

fixed Main(): integer(4)
{
    local filled: Gauge;
    filled.Count = 6;
    local empty: Gauge;
    empty.Count = 0;
    local a: integer(4) = filled ?? 99;
    local b: integer(4) = empty ?? 99;
    return a + b;
}";
    assert_eq!(run_program(source), 105);
}

#[test]
fn builtin_object_root_and_platform_queries() {
    let source = "\
class Animal
{
    extension
    {
        public virtual fixed Sound(this: &Animal): integer(4) { return 1; }
    }
}

class Dog: Animal { }

fixed Main(): integer(4)
{
    local dog: Dog = Dog();
    local value: Object = dog;
    local score: integer(4) = 0;
    if (value is Animal) { score += 1; }
    if (value is Dog) { score += 2; }
    if (value is Object) { score += 4; }

    local kinds: integer(4) = 0;
    if (Platform.Is(PlatformKind.Windows)) { kinds += 1; }
    if (Platform.Is(PlatformKind.Linux)) { kinds += 1; }
    if (Platform.Is(PlatformKind.MacOS)) { kinds += 1; }
    if (Platform.Is(PlatformKind.FreeBSD)) { kinds += 1; }
    if (kinds == 1) { score += 8; }

    #if (Platform.Is(PlatformKind.Other))
    {
        score += 100;
    }
    else
    {
        score += 16;
    }
    return score;
}";
    assert_eq!(run_program(source), 31);
}

#[test]
fn function_values_and_literals_are_callable() {
    let source = "\
fixed Double(value: integer(4)): integer(4)
{
    return value * 2;
}

fixed Apply(f: function(integer(4)) integer(4), value: integer(4)): integer(4)
{
    return f(value);
}

fixed Main(): integer(4)
{
    local named: function(integer(4)) integer(4) = Double;
    local block: function(integer(4)) integer(4) = (value: integer(4)) integer(4) => { return value * 2; };
    local inline: function(integer(4)) integer(4) = (value: integer(4)) integer(4) => value + 1;
    local inferred: function(integer(4)) integer(4) = (value) integer(4) => value * 3;
    return Apply(named, 3) + Apply(block, 4) + Apply(inline, 5) + Apply(inferred, 2);
}";
    // 6 + 8 + 6 + 6
    assert_eq!(run_program(source), 26);
}

#[test]
fn statically_impossible_is_checks_and_interface_patterns() {
    let source = "\
interface INamed
{
    extension
    {
        public fixed Name(this: &INamed): integer(4);
    }
}

class Thing : INamed
{
    extension
    {
        public fixed Name(this: &Thing): integer(4) { return 7; }
    }
}

fixed Main(): integer(4)
{
    local value: integer(4) = 3;
    local never: boolean = value is Thing;
    local thing: Thing = Thing();
    local named: INamed = thing;
    local score: integer(4) = 0;
    if (!never) { score += 1; }
    match named
    {
        case is target: INamed { score += 2; }
        case wildcard { score += 100; }
    }
    return score;
}";
    assert_eq!(run_program(source), 3);
}

#[test]
fn signed_division_overflow_wraps() {
    let source = "\
fixed Check(left: integer(4), right: integer(4)): integer(4)
{
    if (left == right) { return 5; }
    return 6;
}

fixed Main(): integer(4)
{
    local minimum: integer(4) = -2147483647 - 1;
    local wrapped: integer(4) = minimum / -1;
    return Check(wrapped, minimum);
}";
    assert_eq!(run_program(source), 5);
}

#[test]
fn compound_assignment_reads_after_the_rhs() {
    let source = "\
local cell: integer(4) = 10;

fixed Bump(value: integer(4)): integer(4)
{
    cell = value * 2;
    return 1;
}

fixed Main(): integer(4)
{
    cell += Bump(cell);
    return cell;
}";
    assert_eq!(run_program(source), 21);
}

#[test]
fn named_arguments_and_default_values() {
    let source = "\
structure Point
{
    public X: integer(4);
    public Y: integer(4);

    extension
    {
        public fixed Construct(this: &Point, x: integer(4), y: integer(4) = 0)
        {
            this.X = x;
            this.Y = y;
        }

        public fixed Move(this: &Point, dx: integer(4), dy: integer(4) = 1)
        {
            this.X += dx;
            this.Y += dy;
        }
    }
}

fixed Greet(name: integer(4), count: integer(4) = 2, bonus: integer(4) = 3): integer(4)
{
    return name * 10 + count + bonus;
}

fixed Main(): integer(4)
{
    local point: Point = Point(y: 5, x: 4);
    point.Move(dx: 1);
    local sum: integer(4) = Greet(7);
    local sum2: integer(4) = Greet(7, bonus: 9);
    return point.X + point.Y + sum + sum2;
}";
    assert_eq!(run_program(source), 167);
}

#[test]
fn implicitable_conversion_operators_are_invoked_implicitly() {
    let source = "\
structure Celsius
{
    public Degrees: integer(4);
}

structure Fahrenheit
{
    public Degrees: integer(4);

    extension
    {
        public fixed Construct(this: &Fahrenheit, degrees: integer(4))
        {
            this.Degrees = degrees;
        }

        implicitable operator Fahrenheit(value: Celsius): Fahrenheit
        {
            local degrees: integer(4) = value.Degrees * 9 / 5 + 32;
            return Fahrenheit(degrees);
        }
    }
}

fixed Report(entry: Fahrenheit): integer(4)
{
    return entry.Degrees;
}

fixed Main(): integer(4)
{
    local celsius: Celsius;
    celsius.Degrees = 10;
    local fahrenheit: Fahrenheit = celsius;
    return Report(celsius) + fahrenheit.Degrees;
}";
    assert_eq!(run_program(source), 100);
}

#[test]
fn closures_capture_enclosing_variables_by_value() {
    let source = "\
fixed MakeCounter(start: integer(4)): function() integer(4)
{
    local counter: integer(4) = start;
    return () integer(4) => { counter += 1; return counter; };
}

fixed Main(): integer(4)
{
    local limit: integer(4) = 3;
    local adder: function(integer(4)) integer(4) = (value: integer(4)) integer(4) => value + limit;
    local doubled: function(integer(4)) integer(4) = (value: integer(4)) integer(4) => adder(value) * 2;

    local counter: function() integer(4) = MakeCounter(10);
    local a: integer(4) = counter();
    local b: integer(4) = counter();
    return doubled(5) * 10 + a + b;
}";
    // 16 * 10 + 11 + 12
    assert_eq!(run_program(source), 183);
}

#[test]
fn builtin_exceptions_expose_inner_exception() {
    let source = "\
fixed Main(): integer(4)
{
    local score: integer(4) = 0;
    local inner: Exception = Exception(Optional<Exception>.Null);
    local outer: Exception = Exception();
    trap
    {
        throw inner;
    }
    catch error: Exception
    {
        if (error.InnerException is Optional<Exception>.Null) { score += 1; }
    }
    trap
    {
        throw outer;
    }
    catch error: Exception
    {
        if (error.InnerException is Optional<Exception>.Null) { score += 2; }
    }
    trap
    {
        local zero: integer(4) = 0;
        local bad: integer(4) = 5 / zero;
    }
    catch error: DivideByZeroException
    {
        if (error.InnerException is Optional<Exception>.Null) { score += 4; }
    }
    return score;
}";
    assert_eq!(run_program(source), 7);
}

#[test]
fn reference_expressions_nolaa_and_canonical_platform_names() {
    let source = "\
fixed Bump(location: pointer integer(4))
{
    *location = *location + 1;
}

fixed Main(): integer(4)
{
    local counter: integer(4) = 10;
    local address: pointer integer(4) = &counter;
    local score: integer(4) = 0;
    if (*address == 10) { score += 1; }
    Bump(address);
    if (counter == 11) { score += 2; }
    NOLAA.Abandon(address);
    NOLAA.AssumeNoDemandNext(address);
    NOLAA.Bind(address, address);
    if (Platform.Is(\"Linux\")) { score += 4; }
    if (!Platform.Is(\"linux\")) { score += 8; }
    if (!Platform.Is(PlatformKind.Other)) { score += 16; }
    return score;
}";
    assert_eq!(run_program(source), 31);
}

#[test]
fn compile_time_constant_parameters_monomorphize() {
    let source = "\
fixed Apply(#value: integer(4)): integer(4)
{
    #{
        return value + 1;
    }
}

fixed Make(#count: integer(4)): integer(4)
{
    local buffer: array(count) integer(4);
    buffer[count - 1] = count;
    return buffer[count - 1];
}

fixed Scaled(#factor: integer(4), plain: integer(4)): integer(4)
{
    #{
        return factor * plain;
    }
}

fixed Main(): integer(4)
{
    return Apply(1) + Apply(5) + Make(6) + Scaled(2, 4);
}";
    // 2 + 6 + 6 + 8
    assert_eq!(run_program(source), 22);
}

#[test]
fn compile_time_parameters_work_on_methods() {
    let source = "\
structure Bag
{
    public Total: integer(4);

    extension
    {
        public fixed Add(this: &Bag, #amount: integer(4)): integer(4)
        {
            this.Total += amount;
            return this.Total;
        }
    }
}

fixed Main(): integer(4)
{
    local bag: Bag;
    bag.Total = 0;
    local first: integer(4) = bag.Add(4);
    local second: integer(4) = bag.Add(10);
    return bag.Total + first + second;
}";
    assert_eq!(run_program(source), 32);
}

#[test]
fn compile_time_parameters_work_on_operators() {
    let source = "\
fixed Two: integer(4) = 2;
fixed Five: integer(4) = 5;

structure Multiplier
{
    public Factor: integer(4);

    extension
    {
        operator *(left: Multiplier, #scale: integer(4)): integer(4)
        {
            return left.Factor * scale * 10;
        }
    }
}

fixed Main(): integer(4)
{
    local value: Multiplier;
    value.Factor = 3;
    local first: integer(4) = value * Two;
    local second: integer(4) = value * Five;
    return first + second - 200;
}";
    // 3*2*10 + 3*5*10 - 200 = 10
    assert_eq!(run_program(source), 10);
}

#[test]
fn custom_allocator_serves_heap_allocations() {
    let source = "\
class ArenaAllocator : IAllocator
{
    public Buffer: array(64) integer(1);
    public Offset: cardinal(8);

    extension
    {
        public fixed Allocate(this: &ArenaAllocator, size: cardinal(8), alignment: cardinal(8)): Result<pointer; AllocateFailedException>
        {
            local origin: pointer integer(1) = &this.Buffer[0];
            local address: pointer integer(1) = origin + Cast<integer(8)>(this.Offset);
            this.Offset += size;
            return Result<pointer; AllocateFailedException>.Value(Cast<pointer>(address));
        }

        public fixed Deallocate(this: &ArenaAllocator, block: pointer): Result<; DeallocateFailedException>
        {
            return Result<; DeallocateFailedException>.Value;
        }
    }
}

class FailingAllocator : IAllocator
{
    extension
    {
        public fixed Allocate(this: &FailingAllocator, size: cardinal(8), alignment: cardinal(8)): Result<pointer; AllocateFailedException>
        {
            return Result<pointer; AllocateFailedException>.Alternative(AllocateFailedException());
        }

        public fixed Deallocate(this: &FailingAllocator, block: pointer): Result<; DeallocateFailedException>
        {
            return Result<; DeallocateFailedException>.Value;
        }
    }
}

fixed Arena: ArenaAllocator = ArenaAllocator();
fixed Allocator: IAllocator = Arena;
fixed Failing: IAllocator = FailingAllocator();

structure Node
{
    public Value: integer(4);
}

fixed Main(): integer(4)
{
    heap(Allocator) first: Node = Node();
    first.Value = 7;
    heap(Allocator) second: Node = Node();
    second.Value = 4;
    local result: integer(4) = first.Value + second.Value;
    dispose(first);
    dispose(second);

    local caught: integer(4) = 0;
    trap
    {
        heap(Failing) broken: Node = Node();
        broken.Value = 1;
        dispose(broken);
    }
    catch error: AllocateFailedException
    {
        caught = 2;
    }
    return result + Cast<integer(4)>(Arena.Offset) + caught;
}";
    // 7 + 4 + two 4-byte node allocations + the caught failure path
    assert_eq!(run_program(source), 21);
}

#[test]
fn builtin_iterator_interfaces_drive_manual_iteration() {
    let source = "\
class CounterIterator : IIterator<integer(4)>
{
    public Current: integer(4);

    extension
    {
        public fixed Next(this: &CounterIterator): Optional<integer(4)>
        {
            if (this.Current >= 3) { return Optional<integer(4)>.Null; }
            this.Current += 1;
            return Optional<integer(4)>.Value(this.Current);
        }

        public fixed Reset(this: &CounterIterator) { this.Current = 0; }
    }
}

class Counter : IIterable<integer(4)>
{
    private IteratorValue: IIterator<integer(4)>;

    extension
    {
        public fixed Construct(this: &Counter, iterator: IIterator<integer(4)>)
        {
            this.IteratorValue = iterator;
        }

        public accessor Iterator: IIterator<integer(4)> => IteratorValue { get; }
    }
}

fixed Main(): integer(4)
{
    local object: CounterIterator = CounterIterator();
    local counter: Counter = Counter(object);
    local iterator: IIterator<integer(4)> = counter.Iterator;
    local score: integer(4) = 0;
    local running: boolean = true;
    while (running)
    {
        local next: Optional<integer(4)> = iterator.Next();
        if (next is Optional<integer(4)>.Null)
        {
            running = false;
        }
        else
        {
            match next
            {
                case is payload: Optional<integer(4)>.Value { score += payload; }
                case wildcard { }
            }
        }
    }
    return score;
}";
    assert_eq!(run_program(source), 6);
}

#[test]
fn for_loops_traverse_iterators_and_iterables() {
    let source = "\
class CounterIterator : IIterator<integer(4)>
{
    public Current: integer(4);

    extension
    {
        public fixed Next(this: &CounterIterator): Optional<integer(4)>
        {
            if (this.Current >= 5) { return Optional<integer(4)>.Null; }
            this.Current += 1;
            return Optional<integer(4)>.Value(this.Current);
        }

        public fixed Reset(this: &CounterIterator) { this.Current = 0; }
    }
}

class Counter : IIterable<integer(4)>
{
    private IteratorValue: IIterator<integer(4)>;

    extension
    {
        public fixed Construct(this: &Counter, iterator: IIterator<integer(4)>)
        {
            this.IteratorValue = iterator;
        }

        public accessor Iterator: IIterator<integer(4)> => IteratorValue { get; }
    }
}

fixed Main(): integer(4)
{
    local object: CounterIterator = CounterIterator();
    local counter: Counter = Counter(object);
    local score: integer(4) = 0;
    for value: integer(4) in counter
    {
        score += value;
        if (value == 3) { break; }
    }
    for value: integer(4) in object
    {
        score += value;
    }
    return score;
}";
    assert_eq!(run_program(source), 15);
}

#[test]
fn default_allocator_serves_implicit_heap_allocations() {
    let source = "\
structure Node
{
    public Value: integer(4);
}

fixed Main(): integer(4)
{
    heap node: Node = Node();
    node.Value = 7;
    local first: integer(4) = node.Value;
    dispose(node);
    heap other: Node = Node();
    other.Value = 3;
    local second: integer(4) = other.Value;
    dispose(other);
    return first + second;
}";
    assert_eq!(run_program(source), 10);
}

#[test]
fn protected_members_work_in_derived_types() {
    let source = "\
class Animal
{
    protected Name: integer(4);

    extension
    {
        public fixed Construct(this: &Animal, name: integer(4)) { this.Name = name; }
    }
}

class Cat: Animal
{
    extension
    {
        public fixed Construct(this: &Cat, name: integer(4))
        {
            base.Construct(name);
        }

        public fixed Read(this: &Cat): integer(4) { return this.Name; }
    }
}

fixed Main(): integer(4)
{
    local cat: Cat = Cat(6);
    return cat.Read();
}";
    assert_eq!(run_program(source), 6);
}

#[test]
fn nolaa_inserts_dispose_at_the_block_safe_point() {
    let source = "\
static class Arena
{
    public static Buffer: array(256) integer(1);
    public static Offset: cardinal(8);
}

class Stats
{
    public static Deallocations: integer(4);
}

class CountingAllocator : IAllocator
{
    extension
    {
        public fixed Allocate(this: &CountingAllocator, size: cardinal(8), alignment: cardinal(8)): Result<pointer; AllocateFailedException>
        {
            local origin: pointer integer(1) = &Arena.Buffer[0];
            local address: pointer integer(1) = origin + Cast<integer(8)>(Arena.Offset);
            Arena.Offset += size;
            return Result<pointer; AllocateFailedException>.Value(Cast<pointer>(address));
        }

        public fixed Deallocate(this: &CountingAllocator, block: pointer): Result<; DeallocateFailedException>
        {
            Stats.Deallocations += 1;
            return Result<; DeallocateFailedException>.Value;
        }
    }
}

fixed Allocator: IAllocator = CountingAllocator();

structure Node
{
    public Value: integer(4);
}

fixed UseAllocation(): integer(4)
{
    local score: integer(4) = 0;
    {
        heap(Allocator) node: Node = Node();
        node.Value = 5;
        score = node.Value;
    }
    return score + Stats.Deallocations;
}

fixed Main(): integer(4)
{
    return UseAllocation();
}";
    assert_eq!(run_program(source), 6);
}

#[test]
fn readonly_fields_can_be_initialized_in_constructors() {
    let source = "\
class Locked
{
    public readonly Value: integer(4);

    extension
    {
        public fixed Construct(this: &Locked, value: integer(4)) { this.Value = value; }
        public fixed Read(this: &Locked): integer(4) { return this.Value; }
    }
}

fixed Main(): integer(4)
{
    local locked: Locked = Locked(6);
    return locked.Read();
}";
    assert_eq!(run_program(source), 6);
}

#[test]
fn variable_length_arrays_allocate_and_iterate() {
    let source = "\
fixed Fill(count: integer(4)): integer(4)
{
    local values: array(count) integer(4);
    local next: integer(4) = 1;
    for &value: integer(4) in values
    {
        value = next;
        next += 1;
    }
    local total: integer(4) = 0;
    for value: integer(4) in values
    {
        total += value;
    }
    return total;
}

fixed Probe(count: integer(4)): integer(4)
{
    local values: array(count) integer(4);
    trap
    {
        local out: integer(4) = values[count];
        return out;
    }
    catch error: OutOfRangeException
    {
        return 1;
    }
}

fixed Main(): integer(4)
{
    return Fill(4) + Probe(3);
}";
    // 1+2+3+4 plus the caught out-of-range probe
    assert_eq!(run_program(source), 11);
}

#[test]
fn calling_conventions_work_on_functions_and_literals() {
    let source = "\
fixed Add(left: integer(4), right: integer(4); CallingConvention.CDeclaration): integer(4)
{
    return left + right;
}

fixed Main(): integer(4)
{
    local callback: function(integer(4), integer(4); CallingConvention.CDeclaration) integer(4) = Add;
    local literal: function(integer(4); CallingConvention.CDeclaration) integer(4) = (value: integer(4); CallingConvention.CDeclaration) integer(4) => value * 2;
    return callback(2, 3) + literal(4);
}";
    assert_eq!(run_program(source), 13);
}

#[test]
fn extension_functions_and_reference_this_parameters() {
    let source = "\
class Counter
{
    public Value: integer(4);

    extension
    {
        public fixed Bump(this: Counter) { this.Value += 1; }
        public fixed Read(this: Counter): integer(4) { return this.Value; }
    }
}

structure Vec2
{
    public X: integer(4);
    public Y: integer(4);
}

extension for Vec2
{
    public fixed Sum(left: Vec2, right: Vec2): integer(4)
    {
        return left.X + right.X + left.Y + right.Y;
    }
}

fixed Main(): integer(4)
{
    local counter: Counter = Counter();
    counter.Bump();
    counter.Bump();
    local a: Vec2;
    a.X = 1;
    a.Y = 2;
    local b: Vec2;
    b.X = 3;
    b.Y = 4;
    return counter.Read() + Sum(a, b);
}";
    // 2 + 10
    assert_eq!(run_program(source), 12);
}

#[test]
fn aligned_modifier_controls_structure_packing() {
    let source = "\
structure Natural
{
    public A: integer(1);
    public B: integer(4);
}

aligned(0) structure Packed
{
    public A: integer(1);
    public B: integer(4);
}

fixed Main(): integer(4)
{
    local natural: cardinal(8) = Type.SizeOf<Natural>();
    local packed: cardinal(8) = Type.SizeOf<Packed>();
    return Cast<integer(4)>(natural) * 10 + Cast<integer(4)>(packed);
}";
    // natural: 8 bytes; packed: 5 bytes
    assert_eq!(run_program(source), 85);
}

#[test]
fn union_member_aligned_sets_explicit_offset() {
    let source = "\
union Word
{
    A: integer(4);
    aligned(2) B: cardinal(1);
}

fixed Main(): integer(4)
{
    local word: Word;
    word.A = 0x01020304;
    local low_byte: cardinal(1) = word.B;
    word.B = 0xFF;
    local high_byte: integer(4) = (Cast<integer(4)>(word.A) >> 16) & 0xFF;
    local score: integer(4) = 0;
    if (low_byte == 0x02) { score += 1; }
    if (high_byte == 0xFF) { score += 2; }
    return score;
}";
    assert_eq!(run_program(source), 3);
}

#[test]
fn scoped_restricts_members_to_named_type_and_operators() {
    let source = "\
class Animal
{
    scoped(Animal) public Visible: integer(4);

    extension
    {
        public fixed Construct(this: Animal, value: integer(4)) { this.Visible = value; }
        public fixed Read(this: Animal): integer(4) { return this.Visible; }
    }
}

class Cat: Animal
{
    extension
    {
        public fixed Construct(this: Cat, value: integer(4)) { base.Construct(value); }
        public fixed ReadAgain(this: Cat): integer(4) { return this.Visible; }
    }
}

structure Meter
{
    public scoped(operator, Meter) Value: integer(4);

    extension
    {
        public fixed Construct(this: &Meter, value: integer(4)) { this.Value = value; }
        public fixed Read(this: &Meter): integer(4) { return this.Value; }
        operator +(left: Meter, right: Meter): Meter { return Meter(left.Value + right.Value); }
    }
}

fixed Main(): integer(4)
{
    local cat: Cat = Cat(4);
    local left: Meter = Meter(2);
    local right: Meter = Meter(3);
    local sum: Meter = left + right;
    return cat.ReadAgain() + sum.Read();
}";
    // 4 + (2 + 3)
    assert_eq!(run_program(source), 9);
}

#[test]
fn scoped_member_targets_allow_overrides() {
    let source = "\
class Animal
{
    extension
    {
        protected scoped(Speak) fixed Hidden(this: Animal): integer(4) { return 3; }
        public virtual fixed Speak(this: Animal): integer(4) { return this.Hidden(); }
    }
}

class Cat: Animal
{
    extension
    {
        public override fixed Speak(this: Cat): integer(4) { return this.Hidden() + 10; }
    }
}

fixed Main(): integer(4)
{
    local cat: Cat = Cat();
    return cat.Speak();
}";
    assert_eq!(run_program(source), 13);
}

#[test]
fn nested_functions_and_function_pointers() {
    let source = "\
fixed MakeAdder(offset: integer(4)): function(integer(4)) integer(4)
{
    fixed Add(value: integer(4)): integer(4)
    {
        return value + offset;
    }
    return Add;
}

fixed Main(): integer(4)
{
    fixed Local(value: integer(4)): integer(4)
    {
        return value + 1;
    }
    local addTen: function(integer(4)) integer(4) = MakeAdder(10);
    local functionPointer: pointer function(integer(4)) integer(4) = &Local;
    return addTen(2) + (*functionPointer)(3);
}";
    // (2 + 10) + (3 + 1)
    assert_eq!(run_program(source), 16);
}
