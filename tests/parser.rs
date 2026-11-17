//! Parser shape tests: the AST produced for representative NovaSharp programs.

use ainsc::ast::{Ast, NodeKind};
use ainsc::lexer;
use ainsc::parser;
use ainsc::source::{FileId, SourceManager};

fn parse(source: &str) -> (Ast, Vec<String>) {
    let mut sources = SourceManager::new();
    let file = sources.add_buffer("test.ns", source);
    let lexed = lexer::tokenize(file, source);
    let mut errors: Vec<String> = lexed
        .errors
        .iter()
        .map(|error| error.message.clone())
        .collect();
    assert!(errors.is_empty(), "lex errors: {errors:?}");
    let parsed = parser::parse(file, source, &lexed.tokens);
    errors.extend(parsed.errors.iter().map(|error| error.message.clone()));
    (parsed.ast, errors)
}

fn parse_ok(source: &str) -> Ast {
    let (ast, errors) = parse(source);
    assert!(errors.is_empty(), "parse errors: {errors:?}");
    ast
}

#[test]
fn parses_a_function_definition() {
    let ast = parse_ok("fixed Main(): integer(4) { return 0; }");
    assert_eq!(ast.roots.len(), 1);
    match &ast.node(ast.roots[0]).kind {
        NodeKind::Fixed(declaration) => {
            assert_eq!(declaration.name, "Main");
            assert!(
                declaration.ty.is_some(),
                "function shorthand should expand to a type"
            );
            assert!(matches!(
                declaration.init,
                Some(ainsc::ast::Initializer::Block(_))
            ));
        }
        other => panic!("expected a fixed definition, got {other:?}"),
    }
}

#[test]
fn parses_local_definitions_with_shorthands() {
    let ast = parse_ok(
        "fixed Main(): integer(4) {\n\
             local a: integer(4) = 1;\n\
             local b[5]: integer(4);\n\
             local c = 2;\n\
             return 0;\n\
         }",
    );
    let fixed = &ast.node(ast.roots[0]).kind;
    assert!(matches!(fixed, NodeKind::Fixed(_)));
}

#[test]
fn parses_class_with_fields_and_extension() {
    let ast = parse_ok(
        "class Counter {\n\
             private Count: integer(4);\n\
             extension {\n\
                 public fixed Value(): integer(4) { return Count; }\n\
             }\n\
         }",
    );
    match &ast.node(ast.roots[0]).kind {
        NodeKind::Class(declaration) => {
            assert_eq!(declaration.name, "Counter");
            assert_eq!(declaration.members.len(), 2);
            assert!(matches!(
                ast.node(declaration.members[0]).kind,
                NodeKind::Field(_)
            ));
            assert!(matches!(
                ast.node(declaration.members[1]).kind,
                NodeKind::Extension(_)
            ));
        }
        other => panic!("expected a class, got {other:?}"),
    }
}

#[test]
fn parses_statements_and_expressions() {
    let ast = parse_ok(
        "fixed Main(): integer(4) {\n\
             local x: integer(4) = 1 + 2 * 3;\n\
             if x > 3 { x = x - 1; } else if x == 3 { x += 1; } else { x = 0; }\n\
             while x < 10 { x = x << 1; }\n\
             for i: integer(4) in 0..!10 { x = x + i; }\n\
             match x { case 0 { } case wildcard { } }\n\
             trap { throw 1; } catch e: integer(4) { } finally { }\n\
             return x;\n\
         }",
    );
    assert_eq!(ast.roots.len(), 1);
}

#[test]
fn parses_array_literals_and_ranges() {
    let ast = parse_ok(
        "fixed Main(): integer(4) {\n\
             local a[6]: integer(4) = { 1, 2, [2: 3], [1..5] };\n\
             local b[6]: integer(4) = { [0..2..10] };\n\
             return 0;\n\
         }",
    );
    assert_eq!(ast.roots.len(), 1);
}

#[test]
fn parses_null_coalescing_with_continuations() {
    let ast = parse_ok(
        "fixed Main(): integer(4) {\n\
             local x: integer(4) = 0;\n\
             local y: integer(4) = x ?? throw 5;\n\
             local z: integer(4) = x ?? return 1;\n\
             return y + z;\n\
         }",
    );
    assert_eq!(ast.roots.len(), 1);
}

#[test]
fn parses_operator_definitions() {
    let ast = parse_ok(
        "class Vector {\n\
             extension {\n\
                 public operator +(a: Vector, b: Vector): Vector { return a; }\n\
             }\n\
         }",
    );
    match &ast.node(ast.roots[0]).kind {
        NodeKind::Class(declaration) => {
            assert!(matches!(
                ast.node(declaration.members[0]).kind,
                NodeKind::Extension(_)
            ));
        }
        other => panic!("expected a class, got {other:?}"),
    }
}

#[test]
fn parses_generic_declarations_and_monomorphization() {
    let ast = parse_ok(
        "class Box<T> {\n\
             extension {\n\
                 public fixed Get(): T { return Value; }\n\
             }\n\
         }\n\
         fixed Main(): integer(4) {\n\
             local b: Box<integer(4)> = uninitialized;\n\
             return 0;\n\
         }",
    );
    assert_eq!(ast.roots.len(), 2);
}

#[test]
fn reports_errors_without_panicking() {
    let (_ast, errors) = parse("fixed Main(): integer(4) { local x: = ; }");
    assert!(!errors.is_empty());
}

#[test]
fn eof_after_error_terminates() {
    let (_ast, errors) = parse("class {");
    assert!(!errors.is_empty());
}

#[allow(dead_code)]
fn _file_id_used() -> FileId {
    FileId::from_index(0)
}
