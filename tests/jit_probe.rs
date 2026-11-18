//! Probes whether the LLVM 18 MCJIT backend is usable for compile-time
//! execution (the legacy LLVM interpreter was removed from LLVM).

use inkwell::context::Context;
use inkwell::targets::{InitializationConfig, Target};
use inkwell::OptimizationLevel;

#[test]
#[allow(unsafe_code)]
fn mcjit_can_execute_a_trivial_function() {
    Target::initialize_native(&InitializationConfig::default()).expect("native target");
    let context = Context::create();
    let module = context.create_module("jit-probe");
    let builder = context.create_builder();
    let i32_type = context.i32_type();
    let function_type = i32_type.fn_type(&[], false);
    let function = module.add_function("answer", function_type, None);
    let entry = context.append_basic_block(function, "entry");
    builder.position_at_end(entry);
    builder
        .build_return(Some(&i32_type.const_int(42, false)))
        .expect("return");
    let engine = module
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit engine");
    let function = engine.get_function_value("answer").expect("jit function");
    let value = unsafe { engine.run_function(function, &[]) };
    assert_eq!(value.as_int(false), 42);
}
