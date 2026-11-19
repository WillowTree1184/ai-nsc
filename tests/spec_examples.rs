//! Conformance test: every NovaSharp code example in the language specification
//! must parse without errors.
//!
//! The specification is the source of truth, so this test reads it directly.
//! Examples that are deliberately invalid are commented out in the spec, so a
//! clean parse of an uncommented block means the parser covers that syntax.

use std::path::PathBuf;

use ainsc::lexer;
use ainsc::parser;
use ainsc::source::SourceManager;

fn spec_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join("grammar")
        .join("Draft 1.153.md")
}

/// Returns the contents of every ```novasharp fenced block, with its 1-based
/// starting line in the spec.
fn nova_examples(spec: &str) -> Vec<(usize, String)> {
    let mut examples = Vec::new();
    let mut lines = spec.lines().enumerate();
    while let Some((index, line)) = lines.next() {
        if line.trim_start().starts_with("```novasharp") {
            let start_line = index + 2; // 1-based line of the first content line
            let mut body = String::new();
            for (_, content) in lines.by_ref() {
                if content.trim_start().starts_with("```") {
                    break;
                }
                body.push_str(content);
                body.push('\n');
            }
            examples.push((start_line, body));
        }
    }
    examples
}

#[test]
fn every_spec_example_parses() {
    let spec = std::fs::read_to_string(spec_path()).expect("read the language specification");
    let examples = nova_examples(&spec);
    assert!(
        examples.len() > 80,
        "expected to find the spec's examples, found {}",
        examples.len()
    );

    // Examples that are syntax fragments rather than whole programs.
    const NON_PROGRAM_FRAGMENTS: &[usize] = &[9220, 9278, 9441, 9454, 9674];

    let mut failures = Vec::new();
    for (line, source) in &examples {
        if NON_PROGRAM_FRAGMENTS.contains(line) {
            continue;
        }
        let mut sources = SourceManager::new();
        let file = sources.add_buffer(format!("spec-{line}.ns"), source.clone());
        let lexed = lexer::tokenize(file, source);
        if let Some(error) = lexed.errors.first() {
            let (spec_line, spec_column) = sources
                .line_col(error.span.start_location())
                .unwrap_or((0, 0));
            failures.push(format!(
                "spec line {line} (+{spec_line}:{spec_column}): lex error: {}\n--- source ---\n{source}",
                error.message
            ));
            continue;
        }
        let parsed = parser::parse(file, source, &lexed.tokens);
        if let Some(error) = parsed.errors.first() {
            let (spec_line, spec_column) = sources
                .line_col(error.span.start_location())
                .unwrap_or((0, 0));
            failures.push(format!(
                "spec line {line} (+{spec_line}:{spec_column}): parse error: {}\n--- source ---\n{source}",
                error.message
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {} spec examples failed to parse:\n\n{}",
        failures.len(),
        examples.len(),
        failures.join("\n\n")
    );
}

/// Whole-program spec examples that pass semantic analysis are compiled end to
/// end and must produce an executable without diagnostics.
#[test]
fn sema_clean_spec_programs_compile() {
    use ainsc::diagnostics::DiagnosticEngine;
    use ainsc::sema;
    use std::process::Command;

    let spec = std::fs::read_to_string(spec_path()).expect("read the language specification");
    let examples = nova_examples(&spec);
    let directory =
        std::env::temp_dir().join(format!("ainsc-spec-programs-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("create temp dir");

    let mut checked = 0usize;
    let mut failures = Vec::new();
    for (line, source) in &examples {
        if !source.contains("fixed Main") {
            continue;
        }
        let mut sources = SourceManager::new();
        let file = sources.add_buffer(format!("spec-{line}.ns"), source.clone());
        let lexed = lexer::tokenize(file, source);
        if !lexed.errors.is_empty() {
            continue;
        }
        let mut parsed = parser::parse(file, source, &lexed.tokens);
        if !parsed.errors.is_empty() {
            continue;
        }
        let mut diagnostics = DiagnosticEngine::new();
        let result = sema::analyze(&mut parsed.ast, &sources, &mut diagnostics);
        if result.error_count > 0 {
            // Examples with elided bodies ("// ...") or demonstration-only code.
            continue;
        }
        let path = directory.join(format!("spec-{line}.ns"));
        std::fs::write(&path, source).expect("write spec program");
        let executable = directory.join(format!("spec-{line}"));
        let output = Command::new(env!("CARGO_BIN_EXE_ai-nsc"))
            .args([
                "build",
                path.to_str().expect("utf-8 path"),
                "-o",
                executable.to_str().expect("utf-8 path"),
            ])
            .output()
            .expect("run ai-nsc");
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !output.status.success() || stderr.contains("error") {
            failures.push(format!("spec line {line}: {stderr}"));
        }
        checked += 1;
    }
    let _ = std::fs::remove_dir_all(&directory);

    assert!(
        checked >= 15,
        "expected at least 15 sema-clean whole-program examples, found {checked}"
    );
    assert!(
        failures.is_empty(),
        "{} sema-clean spec programs failed to compile:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// The set of `Main`-containing examples that are *not* complete programs is
/// documented in `docs/spec-notes/_inconsistencies.md`: elided/omitted returns,
/// a top-level loop in a declaration-shorthand example, and the launcher example
/// with three `Main` declarations. This test keeps that set
/// closed so no new semantic regression can hide among skipped examples.
#[test]
fn only_documented_spec_programs_fail_semantics() {
    use ainsc::diagnostics::DiagnosticEngine;
    use ainsc::sema;

    const DOCUMENTED: &[usize] = &[3802, 4748, 4816, 6581, 6650, 6713, 6892, 8869, 10508];
    let spec = std::fs::read_to_string(spec_path()).expect("read the language specification");
    let examples = nova_examples(&spec);
    let mut failing = Vec::new();
    for (line, source) in &examples {
        if !source.contains("fixed Main") {
            continue;
        }
        let mut sources = SourceManager::new();
        let file = sources.add_buffer(format!("spec-{line}.ns"), source.clone());
        let lexed = lexer::tokenize(file, source);
        if !lexed.errors.is_empty() {
            continue;
        }
        let mut parsed = parser::parse(file, source, &lexed.tokens);
        if !parsed.errors.is_empty() {
            continue;
        }
        let mut diagnostics = DiagnosticEngine::new();
        let result = sema::analyze(&mut parsed.ast, &sources, &mut diagnostics);
        if result.error_count > 0 {
            failing.push(*line);
        }
    }
    assert_eq!(
        failing, DOCUMENTED,
        "undocumented semantic failures among spec programs; update docs/spec-notes/_inconsistencies.md if intentional"
    );
}
