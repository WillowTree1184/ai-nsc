//! Proves the LLVM back end is usable end to end: inkwell builds IR, the IR
//! prints correctly, and the LLVM assembler inside `clang` turns it into a real
//! ELF object file.

use std::path::{Path, PathBuf};
use std::process::Command;

use inkwell::context::Context;

fn clang_available() -> bool {
    Command::new("clang")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn scratch_dir(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("ainsc-codegen-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("create scratch directory");
    directory
}

#[test]
fn builds_and_prints_llvm_ir() {
    let context = Context::create();
    let module = ainsc::codegen::answer_module(&context);
    let ir = module.print_to_string().to_string();

    assert!(
        ir.contains("define i32 @answer()"),
        "expected a function definition in:\n{ir}"
    );
    assert!(
        ir.contains("ret i32 42"),
        "expected a return of 42 in:\n{ir}"
    );
    assert!(
        ir.contains(&format!(
            "source_filename = \"{}\"",
            ainsc::codegen::SMOKE_MODULE_NAME
        )) || ir.contains(ainsc::codegen::SMOKE_MODULE_NAME),
        "module name missing from:\n{ir}"
    );
}

#[test]
fn clang_compiles_generated_ir_to_elf_object() {
    if !clang_available() {
        eprintln!("skipping: clang is not available");
        return;
    }

    let directory = scratch_dir("object");
    let ir_path = directory.join("answer.ll");
    let object_path = directory.join("answer.o");

    let context = Context::create();
    let module = ainsc::codegen::answer_module(&context);
    module
        .print_to_file(&ir_path)
        .expect("write textual IR to disk");

    let output = Command::new("clang")
        .args(["-c", "-o"])
        .arg(&object_path)
        .arg(&ir_path)
        .output()
        .expect("run clang");
    assert!(
        output.status.success(),
        "clang failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let bytes = std::fs::read(&object_path).expect("read object file");
    assert!(bytes.len() > 4, "object file is suspiciously small");
    assert_eq!(&bytes[..4], b"\x7fELF", "expected an ELF object file");

    remove_dir(&directory);
}

fn remove_dir(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}
