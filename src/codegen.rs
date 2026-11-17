//! LLVM IR generation.
//!
//! This is the only module that depends on LLVM/inkwell; the rest of the
//! compiler stays testable without a code generator.
//!
//! Currently it exposes the smoke-test module used to prove that the LLVM
//! toolchain is wired up correctly. The real AST/HIR -> LLVM lowering (Phase 4)
//! replaces it once the NovaSharp grammar and type system are known. Generated
//! modules are always passed through LLVM's verifier before being written out.

use inkwell::context::Context;
use inkwell::module::Module;

/// Name of the module used by the smoke tests.
pub const SMOKE_MODULE_NAME: &str = "ainsc_smoke";

/// Builds the minimal module `int answer() { return 42; }`.
///
/// This deliberately generates real LLVM IR through inkwell rather than
/// textual templates: it proves the link against `libLLVM-18.so` works and that
/// IR can be produced, printed and consumed by the back end.
#[must_use]
pub fn answer_module<'ctx>(context: &'ctx Context) -> Module<'ctx> {
    let module = context.create_module(SMOKE_MODULE_NAME);
    let builder = context.create_builder();
    let i32_type = context.i32_type();

    let function_type = i32_type.fn_type(&[], false);
    let function = module.add_function("answer", function_type, None);
    let entry = context.append_basic_block(function, "entry");

    builder.position_at_end(entry);
    builder
        .build_return(Some(&i32_type.const_int(42, false)))
        .expect("building a return instruction cannot fail for a valid block");

    module
}
