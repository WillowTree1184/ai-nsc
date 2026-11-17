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
