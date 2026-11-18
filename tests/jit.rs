//! JIT-based compile-time evaluation tests (LLVM 18's MCJIT backend).

use ainsc::ast::Ast;
use ainsc::codegen;
use ainsc::diagnostics::DiagnosticEngine;
use ainsc::lexer;
use ainsc::parser;
use ainsc::sema::{self, SemaResult};
use ainsc::source::SourceManager;

fn compile(source: &str) -> (Ast, SemaResult) {
    let mut sources = SourceManager::new();
    let file = sources.add_buffer("jit.ns", source);
    let lexed = lexer::tokenize(file, source);
    assert!(lexed.errors.is_empty(), "{:?}", lexed.errors);
    let mut parsed = parser::parse(file, source, &lexed.tokens);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let mut engine = DiagnosticEngine::new();
    let result = sema::analyze(&mut parsed.ast, &sources, &mut engine);
    assert_eq!(result.error_count, 0, "semantic errors");
    (parsed.ast, result)
}

fn function_named(sema: &SemaResult, name: &str) -> u32 {
    let index = sema
        .functions
        .iter()
        .position(|function| function.name == name && function.body.is_some())
        .unwrap_or_else(|| panic!("function '{name}' not found"));
    u32::try_from(index).expect("index")
}

const PROGRAM: &str = "\
fixed Double(value: integer(4)): integer(4)
{
    return value * 2;
}

fixed Fibonacci(value: integer(4)): integer(4)
{
    if (value < 2)
    {
        return value;
    }
    return Fibonacci(value - 1) + Fibonacci(value - 2);
}

fixed Main(): integer(4)
{
    return 0;
}";

#[test]
fn jit_evaluates_an_integer_function() {
    let (ast, sema) = compile(PROGRAM);
    let index = function_named(&sema, "Double");
    let result =
        codegen::jit_evaluate_integer_function(&ast, &sema, index, &[21]).expect("jit evaluation");
    assert_eq!(result, Some(42));
}

#[test]
fn jit_evaluates_recursive_compile_time_functions() {
    let (ast, sema) = compile(PROGRAM);
    let index = function_named(&sema, "Fibonacci");
    let result =
        codegen::jit_evaluate_integer_function(&ast, &sema, index, &[10]).expect("jit evaluation");
    assert_eq!(result, Some(55));
}
