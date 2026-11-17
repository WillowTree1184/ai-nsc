//! LLVM IR generation with inkwell.
//!
//! Consumes the AST plus semantic information (types, resolutions, layouts) and
//! produces verified textual LLVM IR. `clang` then assembles and links it.
//!
//! Lowered today: functions/recursion, locals/globals, basic types, arithmetic,
//! comparisons, logical operators, control flow, calls, fixed arrays, string
//! literals, enumerations and structures (value layout, field access).
//! Constructs that are not lowered yet are rejected by semantic analysis first.

use std::collections::HashMap;

use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::{BasicType, BasicTypeEnum, FunctionType, IntType, StructType};
use inkwell::values::{
    BasicValueEnum, FunctionValue, GlobalValue, IntValue, PointerValue, ValueKind,
};
use inkwell::{AddressSpace, FloatPredicate, IntPredicate};

use crate::ast::{
    ArrayElement, Ast, BinaryExpr, BinaryOp, CallExpr, CastExpr, ConditionalExpr, ForStmt,
    ForTarget, IndexExpr, Initializer, MatchStmt, NodeId, NodeKind, Pattern, SetStmt, UnaryExpr,
    UnaryOp,
};
use crate::literal;
use crate::sema::types::{Type, TypeId};
use crate::sema::{ParameterInfo, Resolution, SemaResult, Storage, TypeDefKind};

/// Name of the module used by the workspace smoke test.
pub const SMOKE_MODULE_NAME: &str = "ainsc_smoke";

/// Builds the minimal module `int answer() { return 42; }`.
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
        .expect("building a return instruction cannot fail");
    module
}

/// Options for IR generation.
#[derive(Clone, Debug, Default)]
pub struct CodegenOptions {
    /// Optimization level requested by the user (applied by `clang`).
    pub optimization_level: u8,
}

/// Generates textual LLVM IR for a program.
///
/// # Errors
///
/// Returns a message when a construct cannot be lowered or the module fails
/// LLVM's verifier.
pub fn generate(ast: &Ast, sema: &SemaResult, options: &CodegenOptions) -> Result<String, String> {
    let context = Context::create();
    let module = context.create_module("novasharp");
    let builder = context.create_builder();
    let mut generator = Generator {
        context: &context,
        module,
        builder,
        ast,
        sema,
        options,
        functions: HashMap::new(),
        globals: HashMap::new(),
        local_slots: HashMap::new(),
        struct_types: HashMap::new(),
        loops: Vec::new(),
        current_function: 0,
        current_return: None,
        current_this_slot: None,
        launcher_initializers: Vec::new(),
        malloc_fn: None,
        free_fn: None,
    };
    generator.run()?;
    if let Err(message) = generator.module.verify() {
        return Err(format!("generated invalid LLVM IR: {message}"));
    }
    Ok(generator.module.print_to_string().to_string())
}

struct StructTypeInfo<'ctx> {
    llvm_type: StructType<'ctx>,
    semantic_to_llvm: Vec<u32>,
}

struct LoopInfo<'ctx> {
    label: Option<String>,
    break_block: BasicBlock<'ctx>,
    continue_block: BasicBlock<'ctx>,
}

struct Generator<'ctx, 'a> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    ast: &'a Ast,
    sema: &'a SemaResult,
    options: &'a CodegenOptions,
    functions: HashMap<u32, FunctionValue<'ctx>>,
    globals: HashMap<u32, GlobalValue<'ctx>>,
    local_slots: HashMap<u32, PointerValue<'ctx>>,
    struct_types: HashMap<u32, StructTypeInfo<'ctx>>,
    loops: Vec<LoopInfo<'ctx>>,
    current_function: u32,
    current_return: Option<TypeId>,
    current_this_slot: Option<PointerValue<'ctx>>,
    launcher_initializers: Vec<u32>,
    malloc_fn: Option<FunctionValue<'ctx>>,
    free_fn: Option<FunctionValue<'ctx>>,
}

impl<'ctx, 'a> Generator<'ctx, 'a> {
    fn run(&mut self) -> Result<(), String> {
        self.ensure_libc()?;
        self.prepare_struct_types()?;
        self.declare_functions()?;
        self.declare_globals()?;
        self.generate_functions()?;
        self.generate_launcher()?;
        let _ = self.options;
        Ok(())
    }

    /// Declares the small libc surface used by `heap` and `dispose` (§12.1).
    fn ensure_libc(&mut self) -> Result<(), String> {
        let pointer_type = self.context.ptr_type(AddressSpace::default());
        let malloc_type = pointer_type.fn_type(&[self.context.i64_type().into()], false);
        let malloc = self
            .module
            .get_function("malloc")
            .unwrap_or_else(|| self.module.add_function("malloc", malloc_type, None));
        let free_type = self
            .context
            .void_type()
            .fn_type(&[pointer_type.into()], false);
        let free = self
            .module
            .get_function("free")
            .unwrap_or_else(|| self.module.add_function("free", free_type, None));
        self.malloc_fn = Some(malloc);
        self.free_fn = Some(free);
        Ok(())
    }

    // -- types -------------------------------------------------------------

    fn type_of(&self, node: NodeId) -> TypeId {
        self.sema.node_types[node as usize].unwrap_or(TypeId::ERROR)
    }

    fn llvm_type(&mut self, ty: TypeId) -> Result<BasicTypeEnum<'ctx>, String> {
        let ty_kind = self.sema.types.get(ty).clone();
        Ok(match ty_kind {
            Type::Error | Type::Void | Type::Boolean => self.context.i8_type().into(),
            Type::Integer { bytes, .. } => self.int_type(bytes).into(),
            Type::Float { bytes } => {
                if bytes == 4 {
                    self.context.f32_type().into()
                } else {
                    self.context.f64_type().into()
                }
            }
            Type::Array {
                dimensions,
                element,
            } => {
                let mut current = self.llvm_type(element)?;
                for dimension in dimensions.iter().rev() {
                    let size = u32::try_from(*dimension)
                        .map_err(|_| "array dimension is too large".to_string())?;
                    current = current.array_type(size).into();
                }
                current
            }
            Type::Pointer { .. } | Type::Function { .. } => {
                self.context.ptr_type(AddressSpace::default()).into()
            }
            Type::Named(index) => match self.sema.type_defs[index as usize].kind {
                TypeDefKind::Enumeration => {
                    let underlying = self.sema.type_defs[index as usize]
                        .underlying
                        .ok_or_else(|| "enumeration without underlying type".to_string())?;
                    self.llvm_type(underlying)?
                }
                TypeDefKind::Structure => self.struct_type(index)?.into(),
                TypeDefKind::Class => self.context.ptr_type(AddressSpace::default()).into(),
                _ => return Err("this type cannot be lowered yet".to_string()),
            },
        })
    }

    fn int_type(&self, bytes: u8) -> IntType<'ctx> {
        match bytes {
            1 => self.context.i8_type(),
            2 => self.context.i16_type(),
            4 => self.context.i32_type(),
            16 => self.context.i128_type(),
            _ => self.context.i64_type(),
        }
    }

    fn zero_value(&mut self, ty: TypeId) -> Result<BasicValueEnum<'ctx>, String> {
        let llvm_type = self.llvm_type(ty)?;
        Ok(match llvm_type {
            BasicTypeEnum::IntType(int_type) => int_type.const_zero().into(),
            BasicTypeEnum::FloatType(float_type) => float_type.const_zero().into(),
            BasicTypeEnum::PointerType(pointer_type) => pointer_type.const_zero().into(),
            BasicTypeEnum::ArrayType(array_type) => array_type.const_zero().into(),
            BasicTypeEnum::StructType(struct_type) => struct_type.const_zero().into(),
            _ => return Err("cannot build a zero value of this type".to_string()),
        })
    }

    fn prepare_struct_types(&mut self) -> Result<(), String> {
        let structures: Vec<u32> = self
            .sema
            .type_defs
            .iter()
            .enumerate()
            .filter(|(_, definition)| {
                matches!(definition.kind, TypeDefKind::Structure | TypeDefKind::Class)
            })
            .map(|(index, _)| index as u32)
            .collect();
        for index in structures {
            self.struct_type(index)?;
        }
        Ok(())
    }

    fn struct_type(&mut self, index: u32) -> Result<StructType<'ctx>, String> {
        if let Some(info) = self.struct_types.get(&index) {
            return Ok(info.llvm_type);
        }
        let definition = &self.sema.type_defs[index as usize];
        if !matches!(definition.kind, TypeDefKind::Structure | TypeDefKind::Class) {
            return Err("this type has no instance layout".to_string());
        }
        let fields = definition.fields.clone();
        let size = definition.instance_size;
        let is_class = definition.kind == TypeDefKind::Class;
        let pointer_type = self.context.ptr_type(AddressSpace::default());
        let mut llvm_fields: Vec<BasicTypeEnum<'ctx>> = Vec::new();
        let mut semantic_to_llvm = Vec::with_capacity(fields.len());
        let mut offset: u64 = 0;
        if is_class {
            // Class header: vtable pointer, then type information pointer.
            llvm_fields.push(pointer_type.into());
            llvm_fields.push(pointer_type.into());
            offset = 16;
        }
        for field in &fields {
            if field.offset > offset {
                let padding = u32::try_from(field.offset - offset)
                    .map_err(|_| "padding too large".to_string())?;
                llvm_fields.push(self.context.i8_type().array_type(padding).into());
            }
            let field_type = self.llvm_type(field.ty)?;
            semantic_to_llvm.push(u32::try_from(llvm_fields.len()).unwrap_or(0));
            llvm_fields.push(field_type);
            let (field_size, _) = self.sema.size_align(field.ty);
            offset = field.offset + field_size;
        }
        if size > offset {
            let padding =
                u32::try_from(size - offset).map_err(|_| "padding too large".to_string())?;
            llvm_fields.push(self.context.i8_type().array_type(padding).into());
        }
        let llvm_type = self.context.struct_type(&llvm_fields, true);
        self.struct_types.insert(
            index,
            StructTypeInfo {
                llvm_type,
                semantic_to_llvm,
            },
        );
        Ok(llvm_type)
    }

    fn function_type(&mut self, function_index: u32) -> Result<FunctionType<'ctx>, String> {
        let info = &self.sema.functions[function_index as usize];
        let parameter_types = info.parameters.clone();
        let return_type = info.return_type;
        let mut parameter_metadata = Vec::with_capacity(parameter_types.len());
        for parameter in &parameter_types {
            parameter_metadata.push(self.llvm_parameter_type(parameter)?.into());
        }
        Ok(match return_type {
            Some(ty) => {
                let llvm_return = self.llvm_type(ty)?;
                llvm_return.fn_type(&parameter_metadata, false)
            }
            None => self.context.void_type().fn_type(&parameter_metadata, false),
        })
    }

    fn llvm_parameter_type(
        &mut self,
        parameter: &ParameterInfo,
    ) -> Result<BasicTypeEnum<'ctx>, String> {
        if parameter.by_ref {
            Ok(self.context.ptr_type(AddressSpace::default()).into())
        } else {
            self.llvm_type(parameter.ty)
        }
    }

    // -- declarations ------------------------------------------------------

    fn declare_functions(&mut self) -> Result<(), String> {
        for index in 0..self.sema.functions.len() as u32 {
            let signature = self.function_type(index)?;
            let mangled = self.sema.functions[index as usize].mangled.clone();
            let function = self.module.add_function(&mangled, signature, None);
            self.functions.insert(index, function);
        }
        Ok(())
    }

    fn declare_globals(&mut self) -> Result<(), String> {
        for index in 0..self.sema.globals.len() as u32 {
            let (mangled, ty, fixed, init) = {
                let info = &self.sema.globals[index as usize];
                (info.mangled.clone(), info.ty, info.fixed, info.init)
            };
            let llvm_type = self.llvm_type(ty)?;
            let global = self.module.add_global(llvm_type, None, &mangled);
            global.set_constant(fixed);
            match init.and_then(|node| self.const_scalar(node)) {
                Some(value) => global.set_initializer(&value),
                None => {
                    let zero = self.zero_value(ty)?;
                    global.set_initializer(&zero);
                    if init.is_some() {
                        self.launcher_initializers.push(index);
                    }
                }
            }
            self.globals.insert(index, global);
        }
        Ok(())
    }

    fn const_scalar(&mut self, node: NodeId) -> Option<BasicValueEnum<'ctx>> {
        let ty = self.type_of(node);
        match self.ast.node(node).kind.clone() {
            NodeKind::IntegerLiteral(literal_value) => {
                let value = literal::decode_integer(&literal_value.raw).ok()?;
                let llvm_type = self.llvm_type(ty).ok()?;
                let int_type = llvm_type.into_int_type();
                Some(int_type.const_int(value as u64, false).into())
            }
            NodeKind::FloatLiteral(literal_value) => {
                let value = literal::decode_float(&literal_value.raw).ok()?;
                let llvm_type = self.llvm_type(ty).ok()?;
                Some(llvm_type.into_float_type().const_float(value).into())
            }
            NodeKind::BooleanLiteral(value) => Some(
                self.context
                    .i8_type()
                    .const_int(u64::from(value), false)
                    .into(),
            ),
            NodeKind::CharacterLiteral(literal_value) => {
                let decoded = literal::decode_character(&literal_value.raw).ok()?;
                let llvm_type = self.llvm_type(ty).ok()?;
                Some(
                    llvm_type
                        .into_int_type()
                        .const_int(u64::from(u32::from(decoded.character)), false)
                        .into(),
                )
            }
            _ => None,
        }
    }

    // -- functions ---------------------------------------------------------

    fn generate_functions(&mut self) -> Result<(), String> {
        for index in 0..self.sema.functions.len() as u32 {
            let function = *self
                .functions
                .get(&index)
                .ok_or_else(|| "missing function declaration".to_string())?;
            let (body, locals, return_type, method_of) = {
                let info = &self.sema.functions[index as usize];
                (
                    info.body,
                    info.locals.clone(),
                    info.return_type,
                    info.method_of,
                )
            };
            self.current_function = index;
            self.current_return = return_type;
            self.local_slots.clear();
            self.current_this_slot = None;
            self.loops.clear();

            let entry = self.context.append_basic_block(function, "entry");
            self.builder.position_at_end(entry);
            if method_of.is_some() {
                let pointer_type = self.context.ptr_type(AddressSpace::default());
                let this_slot = self
                    .builder
                    .build_alloca(pointer_type, "this")
                    .map_err(|error| format!("cannot allocate this: {error}"))?;
                let this_value = function
                    .get_nth_param(0)
                    .ok_or_else(|| "method has no this parameter".to_string())?;
                self.builder
                    .build_store(this_slot, this_value)
                    .map_err(|error| format!("cannot store this: {error}"))?;
                self.current_this_slot = Some(this_slot);
            }
            for (local_index, local) in locals.iter().enumerate() {
                let llvm_type = if local.storage == Storage::Heap {
                    self.context.ptr_type(AddressSpace::default()).into()
                } else {
                    self.llvm_type(local.ty)?
                };
                let slot = self
                    .builder
                    .build_alloca(llvm_type, &format!("local{local_index}"))
                    .map_err(|error| format!("cannot allocate local: {error}"))?;
                self.local_slots.insert(local_index as u32, slot);
            }
            for (local_index, local) in locals.iter().enumerate() {
                if let Some(parameter_index) = local.parameter {
                    let value = function
                        .get_nth_param(parameter_index)
                        .ok_or_else(|| "missing function parameter".to_string())?;
                    let slot = *self
                        .local_slots
                        .get(&(local_index as u32))
                        .ok_or_else(|| "missing parameter slot".to_string())?;
                    self.builder
                        .build_store(slot, value)
                        .map_err(|error| format!("cannot store parameter: {error}"))?;
                }
            }
            self.gen_block_contents(body)?;
            if self
                .builder
                .get_insert_block()
                .is_some_and(|block| block.get_terminator().is_none())
            {
                if return_type.is_some() {
                    self.builder
                        .build_unreachable()
                        .map_err(|error| format!("cannot finish function: {error}"))?;
                } else {
                    self.builder
                        .build_return(None)
                        .map_err(|error| format!("cannot return: {error}"))?;
                }
            }
        }
        Ok(())
    }

    fn generate_launcher(&mut self) -> Result<(), String> {
        let main_index = self
            .sema
            .functions
            .iter()
            .position(|function| function.name == "Main" && function.scope.is_empty())
            .ok_or_else(|| "no 'Main' function found".to_string())? as u32;
        let main_function = *self
            .functions
            .get(&main_index)
            .ok_or_else(|| "missing Main declaration".to_string())?;
        let (parameters, return_type) = {
            let info = &self.sema.functions[main_index as usize];
            (info.parameters.clone(), info.return_type)
        };

        let i32_type = self.context.i32_type();
        let launcher_type = i32_type.fn_type(&[], false);
        let launcher = self.module.add_function("main", launcher_type, None);
        let entry = self.context.append_basic_block(launcher, "entry");
        self.builder.position_at_end(entry);
        self.current_function = u32::MAX;
        self.current_return = None;
        self.local_slots.clear();
        self.loops.clear();

        let initializers = self.launcher_initializers.clone();
        for global_index in initializers {
            let (init, global) = {
                let info = &self.sema.globals[global_index as usize];
                (info.init, self.globals.get(&global_index).copied())
            };
            if let (Some(init), Some(global)) = (init, global) {
                let value = self.gen_expr(init)?;
                self.builder
                    .build_store(global.as_pointer_value(), value)
                    .map_err(|error| format!("cannot initialize global: {error}"))?;
            }
        }

        let mut arguments = Vec::with_capacity(parameters.len());
        for parameter in &parameters {
            let value = if parameter.by_ref {
                self.context
                    .ptr_type(AddressSpace::default())
                    .const_zero()
                    .into()
            } else {
                self.zero_value(parameter.ty)?.into()
            };
            arguments.push(value);
        }
        let call = self
            .builder
            .build_call(main_function, &arguments, "main_call")
            .map_err(|error| format!("cannot call Main: {error}"))?;
        let exit_value = match return_type {
            Some(ty) => {
                let value = match call.try_as_basic_value() {
                    ValueKind::Basic(value) => value,
                    ValueKind::Instruction(_) => {
                        return Err("Main did not produce a return value".to_string());
                    }
                };
                self.value_to_i32(value, ty)?
            }
            None => i32_type.const_zero(),
        };
        self.builder
            .build_return(Some(&exit_value))
            .map_err(|error| format!("cannot return from launcher: {error}"))?;
        Ok(())
    }

    fn value_to_i32(
        &self,
        value: BasicValueEnum<'ctx>,
        ty: TypeId,
    ) -> Result<IntValue<'ctx>, String> {
        let i32_type = self.context.i32_type();
        match self.sema.types.get(ty).clone() {
            Type::Integer { bytes, .. } => {
                let value = value.into_int_value();
                let bits = u32::from(bytes) * 8;
                if bits == 32 {
                    Ok(value)
                } else if bits < 32 {
                    self.builder
                        .build_int_z_extend(value, i32_type, "exit")
                        .map_err(|error| format!("cannot extend exit value: {error}"))
                } else {
                    self.builder
                        .build_int_truncate(value, i32_type, "exit")
                        .map_err(|error| format!("cannot truncate exit value: {error}"))
                }
            }
            Type::Boolean => {
                let value = value.into_int_value();
                self.builder
                    .build_int_z_extend(value, i32_type, "exit")
                    .map_err(|error| format!("cannot extend exit value: {error}"))
            }
            _ => Err("Main must return an integer, cardinal, boolean or nothing".to_string()),
        }
    }

    // -- statements --------------------------------------------------------

    fn gen_block_contents(&mut self, block: NodeId) -> Result<(), String> {
        let statements = match self.ast.node(block).kind.clone() {
            NodeKind::Block(statements) => statements,
            NodeKind::VerbatimBlock(inner) => return self.gen_block_contents(inner),
            _ => return Err("expected a block".to_string()),
        };
        for statement in statements {
            if self
                .builder
                .get_insert_block()
                .is_some_and(|block| block.get_terminator().is_some())
            {
                break;
            }
            self.gen_statement(statement)?;
        }
        Ok(())
    }

    fn gen_statement(&mut self, node: NodeId) -> Result<(), String> {
        match self.ast.node(node).kind.clone() {
            NodeKind::Block(_) | NodeKind::VerbatimBlock(_) => self.gen_block_contents(node),
            NodeKind::Local(_) | NodeKind::Fixed(_) => self.gen_local_declaration(node),
            NodeKind::Heap(declaration) => self.gen_heap_statement(node, &declaration),
            NodeKind::Dispose(statement) => self.gen_dispose_statement(&statement),
            NodeKind::Set(statement) => self.gen_set_statement(&statement),
            NodeKind::CompoundSet(statement) => self.gen_compound_set_statement(&statement),
            NodeKind::If(statement) => self.gen_if_statement(&statement),
            NodeKind::While(loop_statement) => self.gen_while_loop(&loop_statement),
            NodeKind::DoWhile(loop_statement) => {
                let condition = loop_statement.condition;
                let body = loop_statement.body;
                let label = loop_statement.label.clone();
                self.gen_do_while(&label, body, condition)
            }
            NodeKind::For(statement) => self.gen_for_statement(node, &statement),
            NodeKind::Match(statement) => self.gen_match_statement(&statement),
            NodeKind::Break(statement) => {
                let target = self.find_loop(statement.label.as_deref())?;
                self.builder
                    .build_unconditional_branch(target.break_block)
                    .map_err(|error| format!("cannot branch: {error}"))?;
                Ok(())
            }
            NodeKind::Continue(statement) => {
                let target = self.find_loop(statement.label.as_deref())?;
                self.builder
                    .build_unconditional_branch(target.continue_block)
                    .map_err(|error| format!("cannot branch: {error}"))?;
                Ok(())
            }
            NodeKind::Return(statement) => self.gen_return_statement(&statement),
            NodeKind::ExpressionStmt(expression) => {
                self.gen_expr(expression)?;
                Ok(())
            }
            NodeKind::Use(_) => Ok(()),
            NodeKind::CompileTimeBlock(block) => self.gen_block_contents(block),
            NodeKind::CompileTimeIf(statement) => {
                let condition = self.gen_expr(statement.condition)?.into_int_value();
                let condition = self.as_boolean_i1(condition);
                let function = self.current_llvm_function()?;
                let then_block = self.context.append_basic_block(function, "ct_then");
                let else_block = self.context.append_basic_block(function, "ct_else");
                let end_block = self.context.append_basic_block(function, "ct_end");
                self.builder
                    .build_conditional_branch(condition, then_block, else_block)
                    .map_err(|error| format!("cannot branch: {error}"))?;
                self.builder.position_at_end(then_block);
                self.gen_block_contents(statement.then_block)?;
                if self
                    .builder
                    .get_insert_block()
                    .is_some_and(|block| block.get_terminator().is_none())
                {
                    self.builder
                        .build_unconditional_branch(end_block)
                        .map_err(|error| format!("cannot branch: {error}"))?;
                }
                self.builder.position_at_end(else_block);
                if let Some(else_block) = statement.else_block {
                    self.gen_block_contents(else_block)?;
                }
                if self
                    .builder
                    .get_insert_block()
                    .is_some_and(|block| block.get_terminator().is_none())
                {
                    self.builder
                        .build_unconditional_branch(end_block)
                        .map_err(|error| format!("cannot branch: {error}"))?;
                }
                self.builder.position_at_end(end_block);
                Ok(())
            }
            _ => Err("unsupported statement in code generation".to_string()),
        }
    }

    fn current_llvm_function(&self) -> Result<FunctionValue<'ctx>, String> {
        self.functions
            .get(&self.current_function)
            .copied()
            .ok_or_else(|| "no current function".to_string())
    }

    fn local_info(&self, local: u32) -> Option<crate::sema::LocalInfo> {
        let function = self.current_function;
        self.sema
            .functions
            .get(function as usize)?
            .locals
            .get(local as usize)
            .cloned()
    }

    fn gen_heap_statement(
        &mut self,
        node: NodeId,
        declaration: &crate::ast::HeapDecl,
    ) -> Result<(), String> {
        let Some(local) = self
            .sema
            .local_declarations
            .get(&node)
            .and_then(|locals| locals.first().copied())
        else {
            return Err("missing heap local".to_string());
        };
        let info = self
            .local_info(local)
            .ok_or_else(|| "missing heap information".to_string())?;
        let slot = *self
            .local_slots
            .get(&local)
            .ok_or_else(|| "missing heap slot".to_string())?;
        let instance_type = self.instance_llvm_type(info.ty)?;
        let size = self.instance_size(info.ty);
        let malloc = self
            .malloc_fn
            .ok_or_else(|| "malloc is not declared".to_string())?;
        let size_value = self.context.i64_type().const_int(size, false);
        let call = self
            .builder
            .build_call(malloc, &[size_value.into()], "allocate")
            .map_err(|error| format!("cannot allocate: {error}"))?;
        let storage = match call.try_as_basic_value() {
            ValueKind::Basic(value) => value.into_pointer_value(),
            ValueKind::Instruction(_) => return Err("allocation produced no pointer".to_string()),
        };
        self.builder
            .build_store(storage, instance_type.const_zero())
            .map_err(|error| format!("cannot initialize heap storage: {error}"))?;
        if let Some(initializer) = declaration.init {
            let is_construction = matches!(
                self.sema.resolutions[initializer as usize],
                Some(Resolution::Construct { .. })
            );
            if is_construction {
                self.gen_construct_into(initializer, storage)?;
            } else {
                let value = self.gen_expr(initializer)?;
                self.builder
                    .build_store(storage, value)
                    .map_err(|error| format!("cannot initialize heap storage: {error}"))?;
            }
        }
        self.builder
            .build_store(slot, storage)
            .map_err(|error| format!("cannot store heap pointer: {error}"))?;
        Ok(())
    }

    fn gen_dispose_statement(&mut self, statement: &crate::ast::DisposeStmt) -> Result<(), String> {
        let NodeKind::Name(name) = self.ast.node(statement.target).kind.clone() else {
            return Err("dispose requires a named heap variable".to_string());
        };
        let Some(Resolution::Local(local)) =
            self.sema.resolutions[statement.target as usize].clone()
        else {
            return Err("dispose requires a heap variable".to_string());
        };
        let info = self
            .local_info(local)
            .ok_or_else(|| "missing heap information".to_string())?;
        let slot = *self
            .local_slots
            .get(&local)
            .ok_or_else(|| "missing heap slot".to_string())?;
        let pointer_type = self.context.ptr_type(AddressSpace::default());
        let storage = self
            .builder
            .build_load(pointer_type, slot, "heap_pointer")
            .map_err(|error| format!("cannot load heap pointer: {error}"))?
            .into_pointer_value();
        if let Some(destructor) = self.find_method(info.ty, "Destruct") {
            let function = *self
                .functions
                .get(&destructor)
                .ok_or_else(|| "missing destructor".to_string())?;
            self.builder
                .build_call(function, &[storage.into()], "destruct")
                .map_err(|error| format!("cannot call destructor: {error}"))?;
        }
        let free = self
            .free_fn
            .ok_or_else(|| "free is not declared".to_string())?;
        self.builder
            .build_call(free, &[storage.into()], "deallocate")
            .map_err(|error| format!("cannot deallocate: {error}"))?;
        let _ = name;
        Ok(())
    }

    fn find_method(&self, ty: TypeId, name: &str) -> Option<u32> {
        let mut current = Some(ty);
        let mut depth = 0;
        while let Some(ty) = current {
            depth += 1;
            if depth > 32 {
                break;
            }
            let Type::Named(index) = self.sema.types.get(ty) else {
                break;
            };
            let definition = &self.sema.type_defs[*index as usize];
            if let Some(list) = definition.methods.get(name) {
                if let Some(first) = list.first() {
                    return Some(*first);
                }
            }
            current = definition.base;
        }
        None
    }

    fn gen_local_declaration(&mut self, node: NodeId) -> Result<(), String> {
        let Some(locals) = self.sema.local_declarations.get(&node).cloned() else {
            return Ok(());
        };
        let initializers: Vec<Option<NodeId>> = match self.ast.node(node).kind.clone() {
            NodeKind::Local(declaration) => declaration
                .items
                .into_iter()
                .map(|item| item.init)
                .collect(),
            NodeKind::Fixed(declaration) => vec![match declaration.init {
                Some(Initializer::Expression(expression)) => Some(expression),
                _ => None,
            }],
            _ => return Ok(()),
        };
        for (local, initializer) in locals.iter().zip(initializers.iter()) {
            let Some(initializer) = initializer else {
                continue;
            };
            if matches!(
                self.ast.node(*initializer).kind,
                NodeKind::UninitializedLiteral
            ) {
                continue;
            }
            let value = self.gen_expr(*initializer)?;
            let slot = *self
                .local_slots
                .get(local)
                .ok_or_else(|| "missing local slot".to_string())?;
            self.builder
                .build_store(slot, value)
                .map_err(|error| format!("cannot initialize local: {error}"))?;
        }
        Ok(())
    }

    fn gen_set_statement(&mut self, statement: &SetStmt) -> Result<(), String> {
        let pointer = self
            .gen_lvalue(statement.target)?
            .ok_or_else(|| "assignment target is not addressable".to_string())?;
        let value = self.gen_expr(statement.value)?;
        self.builder
            .build_store(pointer, value)
            .map_err(|error| format!("cannot assign: {error}"))?;
        Ok(())
    }

    fn gen_compound_set_statement(
        &mut self,
        statement: &crate::ast::CompoundSetStmt,
    ) -> Result<(), String> {
        let pointer = self
            .gen_lvalue(statement.target)?
            .ok_or_else(|| "assignment target is not addressable".to_string())?;
        let target_type = self.type_of(statement.target);
        let llvm_type = self.llvm_type(target_type)?;
        let current = self
            .builder
            .build_load(llvm_type, pointer, "compound_current")
            .map_err(|error| format!("cannot load target: {error}"))?;
        let value = self.gen_expr(statement.value)?;
        let result = match statement.op {
            BinaryOp::Add
            | BinaryOp::Subtract
            | BinaryOp::Multiply
            | BinaryOp::Divide
            | BinaryOp::Remainder
            | BinaryOp::BitwiseAnd
            | BinaryOp::BitwiseOr
            | BinaryOp::BitwiseXor
            | BinaryOp::ShiftLeft
            | BinaryOp::ShiftRight => {
                self.build_arithmetic(statement.op, current, value, target_type, statement.target)?
            }
            _ => return Err("unsupported compound assignment operator".to_string()),
        };
        self.builder
            .build_store(pointer, result)
            .map_err(|error| format!("cannot assign: {error}"))?;
        Ok(())
    }

    fn gen_if_statement(&mut self, statement: &crate::ast::IfStmt) -> Result<(), String> {
        let function = self.current_llvm_function()?;
        let end_block = self.context.append_basic_block(function, "if_end");
        for (index, branch) in statement.branches.iter().enumerate() {
            let condition = self.gen_expr(branch.condition)?.into_int_value();
            let condition = self.as_boolean_i1(condition);
            let then_block = self.context.append_basic_block(function, "if_then");
            let else_block =
                if index + 1 == statement.branches.len() && statement.else_block.is_none() {
                    end_block
                } else {
                    self.context.append_basic_block(function, "if_else")
                };
            self.builder
                .build_conditional_branch(condition, then_block, else_block)
                .map_err(|error| format!("cannot branch: {error}"))?;
            self.builder.position_at_end(then_block);
            self.gen_block_contents(branch.body)?;
            if self
                .builder
                .get_insert_block()
                .is_some_and(|block| block.get_terminator().is_none())
            {
                self.builder
                    .build_unconditional_branch(end_block)
                    .map_err(|error| format!("cannot branch: {error}"))?;
            }
            self.builder.position_at_end(else_block);
        }
        if let Some(else_block) = statement.else_block {
            self.gen_block_contents(else_block)?;
            if self
                .builder
                .get_insert_block()
                .is_some_and(|block| block.get_terminator().is_none())
            {
                self.builder
                    .build_unconditional_branch(end_block)
                    .map_err(|error| format!("cannot branch: {error}"))?;
            }
        }
        self.builder.position_at_end(end_block);
        Ok(())
    }

    fn gen_while_loop(&mut self, statement: &crate::ast::LoopStmt) -> Result<(), String> {
        let function = self.current_llvm_function()?;
        let condition_block = self.context.append_basic_block(function, "while_condition");
        let body_block = self.context.append_basic_block(function, "while_body");
        let end_block = self.context.append_basic_block(function, "while_end");
        self.builder
            .build_unconditional_branch(condition_block)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(condition_block);
        let condition = self.gen_expr(statement.condition)?.into_int_value();
        let condition = self.as_boolean_i1(condition);
        self.builder
            .build_conditional_branch(condition, body_block, end_block)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(body_block);
        self.loops.push(LoopInfo {
            label: statement.label.clone(),
            break_block: end_block,
            continue_block: condition_block,
        });
        self.gen_block_contents(statement.body)?;
        self.loops.pop();
        if self
            .builder
            .get_insert_block()
            .is_some_and(|block| block.get_terminator().is_none())
        {
            self.builder
                .build_unconditional_branch(condition_block)
                .map_err(|error| format!("cannot branch: {error}"))?;
        }
        self.builder.position_at_end(end_block);
        Ok(())
    }

    fn gen_do_while(
        &mut self,
        label: &Option<String>,
        body: NodeId,
        condition: NodeId,
    ) -> Result<(), String> {
        let function = self.current_llvm_function()?;
        let body_block = self.context.append_basic_block(function, "do_body");
        let condition_block = self.context.append_basic_block(function, "do_condition");
        let end_block = self.context.append_basic_block(function, "do_end");
        self.builder
            .build_unconditional_branch(body_block)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(body_block);
        self.loops.push(LoopInfo {
            label: label.clone(),
            break_block: end_block,
            continue_block: condition_block,
        });
        self.gen_block_contents(body)?;
        self.loops.pop();
        if self
            .builder
            .get_insert_block()
            .is_some_and(|block| block.get_terminator().is_none())
        {
            self.builder
                .build_unconditional_branch(condition_block)
                .map_err(|error| format!("cannot branch: {error}"))?;
        }
        self.builder.position_at_end(condition_block);
        let value = self.gen_expr(condition)?.into_int_value();
        let value = self.as_boolean_i1(value);
        self.builder
            .build_conditional_branch(value, body_block, end_block)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(end_block);
        Ok(())
    }

    fn gen_for_statement(&mut self, node: NodeId, statement: &ForStmt) -> Result<(), String> {
        let function = self.current_llvm_function()?;
        let binding_type = self.type_of(node);
        let binding_local = *self
            .sema
            .loop_bindings
            .get(&node)
            .ok_or_else(|| "missing for binding".to_string())?;
        let binding_slot = *self
            .local_slots
            .get(&binding_local)
            .ok_or_else(|| "missing for binding slot".to_string())?;
        let llvm_type = self.llvm_type(binding_type)?;
        let condition_block = self.context.append_basic_block(function, "for_condition");
        let body_block = self.context.append_basic_block(function, "for_body");
        let increment_block = self.context.append_basic_block(function, "for_increment");
        let end_block = self.context.append_basic_block(function, "for_end");

        match &statement.target {
            ForTarget::Range(range) => {
                let start = self.gen_expr(range.start)?;
                self.builder
                    .build_store(binding_slot, start)
                    .map_err(|error| format!("cannot initialize loop variable: {error}"))?;
                let end_value = self.gen_expr(range.end)?.into_int_value();
                let step_value = match range.step {
                    Some(step) => self.gen_expr(step)?.into_int_value(),
                    None => {
                        // Default step: +1 for ascending ranges, -1 otherwise.
                        let current = self
                            .builder
                            .build_load(llvm_type, binding_slot, "current")
                            .map_err(|error| format!("cannot load loop variable: {error}"))?
                            .into_int_value();
                        let predicate = if self.is_signed(binding_type) {
                            IntPredicate::SLT
                        } else {
                            IntPredicate::ULT
                        };
                        let ascending = self
                            .builder
                            .build_int_compare(predicate, current, end_value, "ascending")
                            .map_err(|error| format!("cannot compare: {error}"))?;
                        let one = llvm_type.into_int_type().const_int(1, false);
                        let minus_one = llvm_type.into_int_type().const_int(u64::MAX, true);
                        self.builder
                            .build_select(ascending, one, minus_one, "default_step")
                            .map_err(|error| format!("cannot select step: {error}"))?
                            .into_int_value()
                    }
                };
                self.builder
                    .build_unconditional_branch(condition_block)
                    .map_err(|error| format!("cannot branch: {error}"))?;

                self.builder.position_at_end(condition_block);
                let current = self
                    .builder
                    .build_load(llvm_type, binding_slot, "current")
                    .map_err(|error| format!("cannot load loop variable: {error}"))?
                    .into_int_value();
                let signed = self.is_signed(binding_type);
                // Test `current <= end` for ascending steps and `current >= end`
                // otherwise; both selected with the sign of the step.
                let positive = self
                    .builder
                    .build_int_compare(
                        if signed {
                            IntPredicate::SGT
                        } else {
                            IntPredicate::UGT
                        },
                        step_value,
                        llvm_type.into_int_type().const_zero(),
                        "step_positive",
                    )
                    .map_err(|error| format!("cannot compare: {error}"))?;
                let ascending_test = self
                    .builder
                    .build_int_compare(
                        if range.inclusive {
                            if signed {
                                IntPredicate::SLE
                            } else {
                                IntPredicate::ULE
                            }
                        } else if signed {
                            IntPredicate::SLT
                        } else {
                            IntPredicate::ULT
                        },
                        current,
                        end_value,
                        "ascending_test",
                    )
                    .map_err(|error| format!("cannot compare: {error}"))?;
                let descending_test = self
                    .builder
                    .build_int_compare(
                        if range.inclusive {
                            if signed {
                                IntPredicate::SGE
                            } else {
                                IntPredicate::UGE
                            }
                        } else if signed {
                            IntPredicate::SGT
                        } else {
                            IntPredicate::UGT
                        },
                        current,
                        end_value,
                        "descending_test",
                    )
                    .map_err(|error| format!("cannot compare: {error}"))?;
                let condition = self
                    .builder
                    .build_select(positive, ascending_test, descending_test, "for_condition")
                    .map_err(|error| format!("cannot select condition: {error}"))?;
                let condition = self.as_boolean_i1(condition.into_int_value());
                self.builder
                    .build_conditional_branch(condition, body_block, end_block)
                    .map_err(|error| format!("cannot branch: {error}"))?;

                self.builder.position_at_end(body_block);
                self.loops.push(LoopInfo {
                    label: statement.label.clone(),
                    break_block: end_block,
                    continue_block: increment_block,
                });
                self.gen_block_contents(statement.body)?;
                self.loops.pop();
                if self
                    .builder
                    .get_insert_block()
                    .is_some_and(|block| block.get_terminator().is_none())
                {
                    self.builder
                        .build_unconditional_branch(increment_block)
                        .map_err(|error| format!("cannot branch: {error}"))?;
                }
                self.builder.position_at_end(increment_block);
                let current = self
                    .builder
                    .build_load(llvm_type, binding_slot, "current")
                    .map_err(|error| format!("cannot load loop variable: {error}"))?
                    .into_int_value();
                let next = self
                    .builder
                    .build_int_add(current, step_value, "next")
                    .map_err(|error| format!("cannot increment: {error}"))?;
                self.builder
                    .build_store(binding_slot, next)
                    .map_err(|error| format!("cannot store loop variable: {error}"))?;
                self.builder
                    .build_unconditional_branch(condition_block)
                    .map_err(|error| format!("cannot branch: {error}"))?;
                self.builder.position_at_end(end_block);
            }
            ForTarget::Expression(target) => {
                let target_type = self.type_of(*target);
                let dimensions = match self.sema.types.get(target_type).clone() {
                    Type::Array { dimensions, .. } => dimensions,
                    _ => return Err("for iteration requires an array".to_string()),
                };
                let array_llvm = self.llvm_type(target_type)?;
                let element_llvm = self.llvm_type(binding_type)?;
                let pointer = match self.gen_lvalue(*target)? {
                    Some(pointer) => pointer,
                    None => {
                        let value = self.gen_expr(*target)?;
                        let slot = self
                            .builder
                            .build_alloca(array_llvm, "for_array")
                            .map_err(|error| format!("cannot allocate array: {error}"))?;
                        self.builder
                            .build_store(slot, value)
                            .map_err(|error| format!("cannot store array: {error}"))?;
                        slot
                    }
                };
                let index_slot = self
                    .builder
                    .build_alloca(self.context.i64_type(), "for_index")
                    .map_err(|error| format!("cannot allocate index: {error}"))?;
                self.builder
                    .build_store(index_slot, self.context.i64_type().const_zero())
                    .map_err(|error| format!("cannot store index: {error}"))?;
                self.builder
                    .build_unconditional_branch(condition_block)
                    .map_err(|error| format!("cannot branch: {error}"))?;

                self.builder.position_at_end(condition_block);
                let index = self
                    .builder
                    .build_load(self.context.i64_type(), index_slot, "index")
                    .map_err(|error| format!("cannot load index: {error}"))?
                    .into_int_value();
                let condition = self
                    .builder
                    .build_int_compare(
                        IntPredicate::ULT,
                        index,
                        self.context.i64_type().const_int(dimensions[0], false),
                        "for_test",
                    )
                    .map_err(|error| format!("cannot compare: {error}"))?;
                self.builder
                    .build_conditional_branch(condition, body_block, end_block)
                    .map_err(|error| format!("cannot branch: {error}"))?;

                self.builder.position_at_end(body_block);
                let index = self
                    .builder
                    .build_load(self.context.i64_type(), index_slot, "index")
                    .map_err(|error| format!("cannot load index: {error}"))?
                    .into_int_value();
                let element_pointer = self.element_address(pointer, binding_type, index)?;
                let value = self
                    .builder
                    .build_load(element_llvm, element_pointer, "element")
                    .map_err(|error| format!("cannot load element: {error}"))?;
                self.builder
                    .build_store(binding_slot, value)
                    .map_err(|error| format!("cannot store element: {error}"))?;
                self.loops.push(LoopInfo {
                    label: statement.label.clone(),
                    break_block: end_block,
                    continue_block: increment_block,
                });
                self.gen_block_contents(statement.body)?;
                self.loops.pop();
                if self
                    .builder
                    .get_insert_block()
                    .is_some_and(|block| block.get_terminator().is_none())
                {
                    self.builder
                        .build_unconditional_branch(increment_block)
                        .map_err(|error| format!("cannot branch: {error}"))?;
                }
                self.builder.position_at_end(increment_block);
                let index = self
                    .builder
                    .build_load(self.context.i64_type(), index_slot, "index")
                    .map_err(|error| format!("cannot load index: {error}"))?
                    .into_int_value();
                let next = self
                    .builder
                    .build_int_add(index, self.context.i64_type().const_int(1, false), "next")
                    .map_err(|error| format!("cannot increment: {error}"))?;
                self.builder
                    .build_store(index_slot, next)
                    .map_err(|error| format!("cannot store index: {error}"))?;
                self.builder
                    .build_unconditional_branch(condition_block)
                    .map_err(|error| format!("cannot branch: {error}"))?;
                self.builder.position_at_end(end_block);
            }
        }
        Ok(())
    }

    fn find_loop(&self, label: Option<&str>) -> Result<&LoopInfo<'ctx>, String> {
        match label {
            None => self
                .loops
                .last()
                .ok_or_else(|| "break/continue outside of a loop".to_string()),
            Some(label) => self
                .loops
                .iter()
                .rev()
                .find(|info| info.label.as_deref() == Some(label))
                .ok_or_else(|| format!("unknown loop label '{label}'")),
        }
    }

    fn gen_return_statement(&mut self, statement: &crate::ast::ReturnStmt) -> Result<(), String> {
        match statement.value {
            Some(value) => {
                let value = self.gen_expr(value)?;
                self.builder
                    .build_return(Some(&value))
                    .map_err(|error| format!("cannot return: {error}"))?;
                Ok(())
            }
            None => {
                self.builder
                    .build_return(None)
                    .map_err(|error| format!("cannot return: {error}"))?;
                Ok(())
            }
        }
    }

    fn gen_match_statement(&mut self, statement: &MatchStmt) -> Result<(), String> {
        let function = self.current_llvm_function()?;
        let end_block = self.context.append_basic_block(function, "match_end");
        let value = self.gen_expr(statement.value)?;
        let value_type = self.type_of(statement.value);
        for case in &statement.cases {
            let case_block = self.context.append_basic_block(function, "match_case");
            let next_block = self.context.append_basic_block(function, "match_next");
            let condition: IntValue<'ctx> = match &case.pattern {
                None | Some(Pattern::Wildcard) => self.context.bool_type().const_int(1, false),
                Some(Pattern::Equal(expression)) => {
                    let pattern = self.gen_expr(*expression)?;
                    self.build_comparison(BinaryOp::Equal, value, pattern, value_type)?
                }
                Some(Pattern::Comparison {
                    operator,
                    value: pattern_value,
                    ..
                }) => {
                    let pattern = self.gen_expr(*pattern_value)?;
                    let op = match operator {
                        crate::lexer::TokenKind::NotEquals => BinaryOp::NotEqual,
                        crate::lexer::TokenKind::Greater => BinaryOp::Greater,
                        crate::lexer::TokenKind::Less => BinaryOp::Less,
                        crate::lexer::TokenKind::GreaterEquals => BinaryOp::GreaterEqual,
                        crate::lexer::TokenKind::LessEquals => BinaryOp::LessEqual,
                        _ => return Err("unsupported match comparison".to_string()),
                    };
                    self.build_comparison(op, value, pattern, value_type)?
                }
                Some(Pattern::Is { .. }) => {
                    return Err("is patterns are not lowered yet".to_string());
                }
            };
            let condition = match &case.guard {
                Some(guard) => {
                    let guard_value = self.gen_expr(*guard)?.into_int_value();
                    let guard_value = self.as_boolean_i1(guard_value);
                    self.builder
                        .build_and(condition, guard_value, "guard")
                        .map_err(|error| format!("cannot combine condition: {error}"))?
                }
                None => condition,
            };
            let condition = self.as_boolean_i1(condition);
            self.builder
                .build_conditional_branch(condition, case_block, next_block)
                .map_err(|error| format!("cannot branch: {error}"))?;
            self.builder.position_at_end(case_block);
            self.gen_block_contents(case.body)?;
            if self
                .builder
                .get_insert_block()
                .is_some_and(|block| block.get_terminator().is_none())
            {
                self.builder
                    .build_unconditional_branch(end_block)
                    .map_err(|error| format!("cannot branch: {error}"))?;
            }
            self.builder.position_at_end(next_block);
        }
        self.builder
            .build_unconditional_branch(end_block)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(end_block);
        Ok(())
    }

    // -- expressions -------------------------------------------------------

    fn gen_expr(&mut self, node: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        match self.ast.node(node).kind.clone() {
            NodeKind::IntegerLiteral(literal_value) => {
                let ty = self.type_of(node);
                let llvm_type = self.llvm_type(ty)?;
                let value = literal::decode_integer(&literal_value.raw)
                    .map_err(|message| format!("invalid integer literal: {message}"))?;
                Ok(llvm_type
                    .into_int_type()
                    .const_int(value as u64, false)
                    .into())
            }
            NodeKind::FloatLiteral(literal_value) => {
                let ty = self.type_of(node);
                let llvm_type = self.llvm_type(ty)?;
                let value = literal::decode_float(&literal_value.raw)
                    .map_err(|message| format!("invalid float literal: {message}"))?;
                Ok(llvm_type.into_float_type().const_float(value).into())
            }
            NodeKind::CharacterLiteral(literal_value) => {
                let ty = self.type_of(node);
                let llvm_type = self.llvm_type(ty)?;
                let decoded = literal::decode_character(&literal_value.raw)
                    .map_err(|message| format!("invalid character literal: {message}"))?;
                Ok(llvm_type
                    .into_int_type()
                    .const_int(u64::from(u32::from(decoded.character)), false)
                    .into())
            }
            NodeKind::StringLiteral(literal_value)
            | NodeKind::VerbatimStringLiteral(literal_value) => {
                self.gen_string_literal(node, &literal_value.raw)
            }
            NodeKind::BooleanLiteral(value) => Ok(self
                .context
                .i8_type()
                .const_int(u64::from(value), false)
                .into()),
            NodeKind::InfiniteLiteral => {
                let llvm_type = self.llvm_type(self.type_of(node))?;
                Ok(llvm_type
                    .into_float_type()
                    .const_float(f64::INFINITY)
                    .into())
            }
            NodeKind::IllegalLiteral => {
                let llvm_type = self.llvm_type(self.type_of(node))?;
                Ok(llvm_type.into_float_type().const_float(f64::NAN).into())
            }
            NodeKind::UninitializedLiteral => {
                let ty = self.type_of(node);
                self.zero_value(ty)
            }
            NodeKind::Name(_) | NodeKind::MemberAccess(_) | NodeKind::PointerMemberAccess(_) => {
                self.gen_value_from_resolution(node)
            }
            NodeKind::This => self.gen_this_value(),
            NodeKind::Index(index) => self.gen_index_expr(node, &index),
            NodeKind::Call(call) => self.gen_call_expr(node, &call),
            NodeKind::Cast(cast) => self.gen_cast_expr(node, &cast),
            NodeKind::Unary(unary) => self.gen_unary_expr(node, &unary),
            NodeKind::Binary(binary) => self.gen_binary_expr(node, &binary),
            NodeKind::Conditional(conditional) => self.gen_conditional_expr(node, &conditional),
            NodeKind::ArrayLiteral(literal_value) => self.gen_array_literal(node, &literal_value),
            NodeKind::CompileTimeExpr(inner) => self.gen_expr(inner),
            _ => Err("unsupported expression in code generation".to_string()),
        }
    }

    fn gen_value_from_resolution(&mut self, node: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let ty = self.type_of(node);
        match self.sema.resolutions[node as usize].clone() {
            Some(Resolution::Local(local)) => {
                let info = self
                    .local_info(local)
                    .ok_or_else(|| "missing local information".to_string())?;
                let slot = *self
                    .local_slots
                    .get(&local)
                    .ok_or_else(|| "missing local slot".to_string())?;
                if info.storage == Storage::Heap {
                    let pointer_type = self.context.ptr_type(AddressSpace::default());
                    let pointer = self
                        .builder
                        .build_load(pointer_type, slot, "heap_pointer")
                        .map_err(|error| format!("cannot load heap pointer: {error}"))?;
                    if matches!(self.sema.types.get(ty), Type::Named(index) if self.sema.type_defs[*index as usize].kind == TypeDefKind::Class)
                    {
                        return Ok(pointer);
                    }
                    let instance_type = self.instance_llvm_type(ty)?;
                    return self
                        .builder
                        .build_load(instance_type, pointer.into_pointer_value(), "heap_value")
                        .map_err(|error| format!("cannot load heap value: {error}"));
                }
                let llvm_type = self.llvm_type(ty)?;
                self.builder
                    .build_load(llvm_type, slot, "local")
                    .map_err(|error| format!("cannot load local: {error}"))
            }
            Some(Resolution::Global(global)) => {
                let global = *self
                    .globals
                    .get(&global)
                    .ok_or_else(|| "missing global".to_string())?;
                let llvm_type = self.llvm_type(ty)?;
                self.builder
                    .build_load(llvm_type, global.as_pointer_value(), "global")
                    .map_err(|error| format!("cannot load global: {error}"))
            }
            Some(Resolution::EnumMember { type_id, index }) => {
                let underlying = self.enum_underlying(type_id)?;
                let value = self.sema.type_defs[type_id.0 as usize].enum_members[index as usize].1;
                let llvm_type = self.llvm_type(underlying)?;
                Ok(llvm_type
                    .into_int_type()
                    .const_int(value as u64, true)
                    .into())
            }
            Some(Resolution::Field { .. }) => self.gen_field_load(node),
            Some(Resolution::This) => self.gen_this_value(),
            Some(Resolution::Construct { .. }) => self.gen_construction(node),
            Some(Resolution::Function(_)) => Err("function values are not lowered yet".to_string()),
            Some(Resolution::Type(_)) | None => Err("name cannot be used as a value".to_string()),
        }
    }

    fn enum_underlying(&self, ty: TypeId) -> Result<TypeId, String> {
        match self.sema.types.get(ty) {
            Type::Named(index) => self.sema.type_defs[*index as usize]
                .underlying
                .ok_or_else(|| "type has no underlying type".to_string()),
            _ => Err("expected an enumeration type".to_string()),
        }
    }

    fn gen_field_load(&mut self, node: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let pointer = self
            .gen_lvalue(node)?
            .ok_or_else(|| "field base is not addressable".to_string())?;
        let llvm_type = self.llvm_type(self.type_of(node))?;
        self.builder
            .build_load(llvm_type, pointer, "field")
            .map_err(|error| format!("cannot load field: {error}"))
    }

    fn gen_index_expr(
        &mut self,
        node: NodeId,
        _index: &IndexExpr,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let pointer = self
            .gen_lvalue(node)?
            .ok_or_else(|| "indexed value is not addressable".to_string())?;
        let llvm_type = self.llvm_type(self.type_of(node))?;
        self.builder
            .build_load(llvm_type, pointer, "element")
            .map_err(|error| format!("cannot load element: {error}"))
    }

    fn gen_lvalue(&mut self, node: NodeId) -> Result<Option<PointerValue<'ctx>>, String> {
        match self.ast.node(node).kind.clone() {
            NodeKind::This => {
                let slot = self
                    .current_this_slot
                    .ok_or_else(|| "no this slot".to_string())?;
                let pointer_type = self.context.ptr_type(AddressSpace::default());
                let pointer = self
                    .builder
                    .build_load(pointer_type, slot, "this")
                    .map_err(|error| format!("cannot load this: {error}"))?;
                Ok(Some(pointer.into_pointer_value()))
            }
            NodeKind::Name(_) => match self.sema.resolutions[node as usize].clone() {
                Some(Resolution::Local(local)) => {
                    let info = self
                        .local_info(local)
                        .ok_or_else(|| "missing local information".to_string())?;
                    let slot = *self
                        .local_slots
                        .get(&local)
                        .ok_or_else(|| "missing local slot".to_string())?;
                    if info.storage == Storage::Heap {
                        let pointer_type = self.context.ptr_type(AddressSpace::default());
                        let pointer = self
                            .builder
                            .build_load(pointer_type, slot, "heap_pointer")
                            .map_err(|error| format!("cannot load heap pointer: {error}"))?;
                        Ok(Some(pointer.into_pointer_value()))
                    } else {
                        Ok(Some(slot))
                    }
                }
                Some(Resolution::Global(global)) => Ok(self
                    .globals
                    .get(&global)
                    .map(|global| global.as_pointer_value())),
                Some(Resolution::This) => {
                    let slot = self
                        .current_this_slot
                        .ok_or_else(|| "no this slot".to_string())?;
                    let pointer_type = self.context.ptr_type(AddressSpace::default());
                    let pointer = self
                        .builder
                        .build_load(pointer_type, slot, "this")
                        .map_err(|error| format!("cannot load this: {error}"))?;
                    Ok(Some(pointer.into_pointer_value()))
                }
                _ => Ok(None),
            },
            NodeKind::Index(index) => {
                let base_type = self.type_of(index.base);
                match self.sema.types.get(base_type).clone() {
                    Type::Array { .. } => {
                        let base_pointer = match self.gen_lvalue(index.base)? {
                            Some(pointer) => pointer,
                            None => {
                                let value = self.gen_expr(index.base)?;
                                let llvm_type = self.llvm_type(base_type)?;
                                let slot = self
                                    .builder
                                    .build_alloca(llvm_type, "array_spill")
                                    .map_err(|error| format!("cannot allocate array: {error}"))?;
                                self.builder
                                    .build_store(slot, value)
                                    .map_err(|error| format!("cannot store array: {error}"))?;
                                slot
                            }
                        };
                        let index_value = self.gen_expr(index.index)?.into_int_value();
                        let element_type = self.type_of(node);
                        let pointer =
                            self.element_address(base_pointer, element_type, index_value)?;
                        Ok(Some(pointer))
                    }
                    Type::Pointer {
                        target: Some(target),
                    } => {
                        let base = self.gen_expr(index.base)?.into_pointer_value();
                        let index_value = self.gen_expr(index.index)?.into_int_value();
                        let pointer = self.element_address(base, target, index_value)?;
                        Ok(Some(pointer))
                    }
                    _ => Err("cannot index this value".to_string()),
                }
            }
            NodeKind::MemberAccess(access) => {
                let Some(Resolution::Field { type_id, index }) =
                    self.sema.resolutions[node as usize].clone()
                else {
                    return Ok(None);
                };
                let base_pointer = match self.gen_lvalue(access.base)? {
                    Some(pointer) => pointer,
                    None => {
                        let value = self.gen_expr(access.base)?;
                        let llvm_type = self.llvm_type(self.type_of(access.base))?;
                        let slot = self
                            .builder
                            .build_alloca(llvm_type, "struct_spill")
                            .map_err(|error| format!("cannot allocate structure: {error}"))?;
                        self.builder
                            .build_store(slot, value)
                            .map_err(|error| format!("cannot store structure: {error}"))?;
                        slot
                    }
                };
                self.struct_field_pointer(base_pointer, type_id, index)
            }
            NodeKind::PointerMemberAccess(access) => {
                let Some(Resolution::Field { type_id, index }) =
                    self.sema.resolutions[node as usize].clone()
                else {
                    return Ok(None);
                };
                let base_pointer = self.gen_expr(access.base)?.into_pointer_value();
                self.struct_field_pointer(base_pointer, type_id, index)
            }
            _ => Ok(None),
        }
    }

    /// Address of element `index` inside the array or pointee starting at `base`.
    fn element_address(
        &self,
        base: PointerValue<'ctx>,
        element_type: TypeId,
        index: IntValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, String> {
        let (element_size, _) = self.sema.size_align(element_type);
        let i64_type = self.context.i64_type();
        let base_integer = self
            .builder
            .build_ptr_to_int(base, i64_type, "base_integer")
            .map_err(|error| format!("cannot convert pointer: {error}"))?;
        let index = self.as_i64(index)?;
        let size = i64_type.const_int(element_size, false);
        let offset = self
            .builder
            .build_int_mul(index, size, "offset")
            .map_err(|error| format!("cannot compute offset: {error}"))?;
        let address = self
            .builder
            .build_int_add(base_integer, offset, "address")
            .map_err(|error| format!("cannot compute address: {error}"))?;
        self.builder
            .build_int_to_ptr(
                address,
                self.context.ptr_type(AddressSpace::default()),
                "element",
            )
            .map_err(|error| format!("cannot convert address: {error}"))
    }

    fn as_i64(&self, value: IntValue<'ctx>) -> Result<IntValue<'ctx>, String> {
        let i64_type = self.context.i64_type();
        let bits = value.get_type().get_bit_width();
        if bits == 64 {
            Ok(value)
        } else if bits < 64 {
            self.builder
                .build_int_z_extend(value, i64_type, "index64")
                .map_err(|error| format!("cannot extend index: {error}"))
        } else {
            self.builder
                .build_int_truncate(value, i64_type, "index64")
                .map_err(|error| format!("cannot truncate index: {error}"))
        }
    }

    fn struct_field_pointer(
        &mut self,
        base_pointer: PointerValue<'ctx>,
        type_id: TypeId,
        field_index: u32,
    ) -> Result<Option<PointerValue<'ctx>>, String> {
        let Type::Named(index) = self.sema.types.get(type_id).clone() else {
            return Err("field access requires a structure".to_string());
        };
        let (llvm_type, llvm_field_index) = {
            let info = self
                .struct_types
                .get(&index)
                .ok_or_else(|| "missing structure type".to_string())?;
            let field_index = *info
                .semantic_to_llvm
                .get(field_index as usize)
                .ok_or_else(|| "missing field index".to_string())?;
            (info.llvm_type, field_index)
        };
        let pointer = self
            .builder
            .build_struct_gep(llvm_type, base_pointer, llvm_field_index, "field_pointer")
            .map_err(|error| format!("cannot address field: {error}"))?;
        Ok(Some(pointer))
    }

    fn gen_string_literal(
        &mut self,
        node: NodeId,
        raw: &str,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let decoded = literal::decode_string(raw)
            .map_err(|message| format!("invalid string literal: {message}"))?;
        let ty = self.type_of(node);
        let llvm_type = self.llvm_type(ty)?;
        let BasicTypeEnum::ArrayType(array_type) = llvm_type else {
            return Err("string literal must have an array type".to_string());
        };
        let element_type = array_type.get_element_type();
        let int_type = element_type.into_int_type();
        if decoded.unit_size == 1 {
            let values: Vec<IntValue<'ctx>> = decoded
                .bytes
                .iter()
                .map(|byte| int_type.const_int(u64::from(*byte), false))
                .collect();
            Ok(int_type.const_array(&values).into())
        } else {
            let mut values = Vec::with_capacity(decoded.bytes.len() / decoded.unit_size as usize);
            for chunk in decoded.bytes.chunks(decoded.unit_size as usize) {
                let mut value: u64 = 0;
                for (index, byte) in chunk.iter().enumerate() {
                    value |= u64::from(*byte) << (8 * index);
                }
                values.push(int_type.const_int(value, false));
            }
            Ok(int_type.const_array(&values).into())
        }
    }

    fn gen_array_literal(
        &mut self,
        node: NodeId,
        literal_value: &crate::ast::ArrayLiteral,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let ty = self.type_of(node);
        let llvm_type = self.llvm_type(ty)?;
        let BasicTypeEnum::ArrayType(array_type) = llvm_type else {
            return Err("array literal must have an array type".to_string());
        };
        let element_type = array_type.get_element_type();
        let slot = self
            .builder
            .build_alloca(llvm_type, "array_literal")
            .map_err(|error| format!("cannot allocate array literal: {error}"))?;
        let element_type_id = match self.sema.types.get(ty).clone() {
            Type::Array { element, .. } => element,
            _ => return Err("array literal type mismatch".to_string()),
        };
        let mut position: u64 = 0;
        for element in &literal_value.elements {
            match element {
                ArrayElement::Value(expression) => {
                    let value = self.gen_expr(*expression)?;
                    let pointer = self.element_address(
                        slot,
                        element_type_id,
                        self.context.i64_type().const_int(position, false),
                    )?;
                    self.builder
                        .build_store(pointer, value)
                        .map_err(|error| format!("cannot store array element: {error}"))?;
                    position += 1;
                }
                ArrayElement::Repeat { value, count } => {
                    let repeated = self.gen_expr(*value)?;
                    let total = self.const_integer(*count)?;
                    for _ in 0..total.max(0) {
                        let pointer = self.element_address(
                            slot,
                            element_type_id,
                            self.context.i64_type().const_int(position, false),
                        )?;
                        self.builder
                            .build_store(pointer, repeated)
                            .map_err(|error| format!("cannot store array element: {error}"))?;
                        position += 1;
                    }
                }
                ArrayElement::Range(range) => {
                    let start = self.const_integer(range.start)?;
                    let end = self.const_integer(range.end)?;
                    let step = match range.step {
                        Some(step) => self.const_integer(step)?,
                        None => {
                            if end >= start {
                                1
                            } else {
                                -1
                            }
                        }
                    };
                    let mut current = start;
                    loop {
                        if step > 0 {
                            if current > end || (!range.inclusive && current == end) {
                                break;
                            }
                        } else if current < end || (!range.inclusive && current == end) {
                            break;
                        }
                        let value = element_type.into_int_type().const_int(current as u64, true);
                        let pointer = self.element_address(
                            slot,
                            element_type_id,
                            self.context.i64_type().const_int(position, false),
                        )?;
                        self.builder
                            .build_store(pointer, value)
                            .map_err(|error| format!("cannot store array element: {error}"))?;
                        position += 1;
                        current += step;
                    }
                }
            }
        }
        self.builder
            .build_load(llvm_type, slot, "array")
            .map_err(|error| format!("cannot load array literal: {error}"))
    }

    fn const_integer(&mut self, node: NodeId) -> Result<i64, String> {
        match self.ast.node(node).kind.clone() {
            NodeKind::IntegerLiteral(literal_value) => literal::decode_integer(&literal_value.raw)
                .map(|value| value as i64)
                .map_err(|message| format!("invalid integer literal: {message}")),
            NodeKind::Unary(unary) if unary.op == UnaryOp::Negate => {
                Ok(-self.const_integer(unary.operand)?)
            }
            NodeKind::Binary(binary) => {
                let left = self.const_integer(binary.left)?;
                let right = self.const_integer(binary.right)?;
                Ok(match binary.op {
                    BinaryOp::Add => left.wrapping_add(right),
                    BinaryOp::Subtract => left.wrapping_sub(right),
                    BinaryOp::Multiply => left.wrapping_mul(right),
                    BinaryOp::Divide => left.wrapping_div(right),
                    BinaryOp::Remainder => left.wrapping_rem(right),
                    BinaryOp::ShiftLeft => left.wrapping_shl(right as u32),
                    BinaryOp::ShiftRight => left.wrapping_shr(right as u32),
                    _ => return Err("unsupported constant expression".to_string()),
                })
            }
            _ => Err("expected a constant integer".to_string()),
        }
    }

    fn gen_call_expr(
        &mut self,
        node: NodeId,
        call: &CallExpr,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        match self.sema.resolutions[node as usize].clone() {
            Some(Resolution::Construct { .. }) => self.gen_construction(node),
            Some(Resolution::Function(index)) => {
                let function = *self
                    .functions
                    .get(&index)
                    .ok_or_else(|| "missing callee".to_string())?;
                let parameter_count = self.sema.functions[index as usize].parameters.len();
                let mut arguments = Vec::with_capacity(parameter_count);
                if self.sema.functions[index as usize].method_of.is_some() {
                    let NodeKind::MemberAccess(access) = self.ast.node(call.callee).kind.clone()
                    else {
                        return Err("method call requires a receiver".to_string());
                    };
                    let receiver = self.gen_instance_pointer(access.base)?;
                    arguments.push(receiver.into());
                }
                for argument in &call.arguments {
                    arguments.push(self.gen_expr(argument.value)?.into());
                }
                let call_site = self
                    .builder
                    .build_call(function, &arguments, "call")
                    .map_err(|error| format!("cannot call function: {error}"))?;
                match self.sema.functions[index as usize].return_type {
                    Some(_) => match call_site.try_as_basic_value() {
                        ValueKind::Basic(value) => Ok(value),
                        ValueKind::Instruction(_) => {
                            Err("call did not produce a value".to_string())
                        }
                    },
                    None => Ok(self.context.i8_type().const_zero().into()),
                }
            }
            _ => Err("call target is not a direct function yet".to_string()),
        }
    }

    /// Address of the instance a call or field access operates on.
    fn gen_instance_pointer(&mut self, node: NodeId) -> Result<PointerValue<'ctx>, String> {
        if matches!(self.ast.node(node).kind, NodeKind::This) {
            let slot = self
                .current_this_slot
                .ok_or_else(|| "no this slot".to_string())?;
            let pointer_type = self.context.ptr_type(AddressSpace::default());
            let pointer = self
                .builder
                .build_load(pointer_type, slot, "this")
                .map_err(|error| format!("cannot load this: {error}"))?;
            return Ok(pointer.into_pointer_value());
        }
        let ty = self.type_of(node);
        if matches!(self.sema.types.get(ty), Type::Named(index) if self.sema.type_defs[*index as usize].kind == TypeDefKind::Class)
        {
            return Ok(self.gen_expr(node)?.into_pointer_value());
        }
        match self.gen_lvalue(node)? {
            Some(pointer) => Ok(pointer),
            None => {
                let value = self.gen_expr(node)?;
                let llvm_type = self.llvm_type(ty)?;
                let slot = self
                    .builder
                    .build_alloca(llvm_type, "receiver")
                    .map_err(|error| format!("cannot allocate receiver: {error}"))?;
                self.builder
                    .build_store(slot, value)
                    .map_err(|error| format!("cannot store receiver: {error}"))?;
                Ok(slot)
            }
        }
    }

    // -- construction ------------------------------------------------------

    fn instance_llvm_type(&mut self, ty: TypeId) -> Result<BasicTypeEnum<'ctx>, String> {
        match self.sema.types.get(ty).clone() {
            Type::Named(index)
                if matches!(
                    self.sema.type_defs[index as usize].kind,
                    TypeDefKind::Structure | TypeDefKind::Class
                ) =>
            {
                Ok(self.struct_type(index)?.into())
            }
            _ => Err("this type has no instance layout".to_string()),
        }
    }

    fn instance_size(&self, ty: TypeId) -> u64 {
        match self.sema.types.get(ty) {
            Type::Named(index) => self.sema.type_defs[*index as usize].instance_size,
            _ => self.sema.size_align(ty).0,
        }
    }

    fn gen_this_value(&mut self) -> Result<BasicValueEnum<'ctx>, String> {
        let slot = self
            .current_this_slot
            .ok_or_else(|| "no this slot".to_string())?;
        let pointer_type = self.context.ptr_type(AddressSpace::default());
        let pointer = self
            .builder
            .build_load(pointer_type, slot, "this")
            .map_err(|error| format!("cannot load this: {error}"))?
            .into_pointer_value();
        let owner = self.sema.functions[self.current_function as usize]
            .method_of
            .ok_or_else(|| "this outside a method".to_string())?;
        match self.sema.types.get(owner).clone() {
            Type::Named(index)
                if self.sema.type_defs[index as usize].kind == TypeDefKind::Class =>
            {
                Ok(pointer.into())
            }
            _ => {
                let instance_type = self.instance_llvm_type(owner)?;
                self.builder
                    .build_load(instance_type, pointer, "this_value")
                    .map_err(|error| format!("cannot load this: {error}"))
            }
        }
    }

    /// Constructs a value of the node's type into `storage` (§4.6).
    fn gen_construct_into(
        &mut self,
        node: NodeId,
        storage: PointerValue<'ctx>,
    ) -> Result<(), String> {
        let Some(Resolution::Construct { type_id, method }) =
            self.sema.resolutions[node as usize].clone()
        else {
            return Err("expected a construction expression".to_string());
        };
        let Type::Named(index) = self.sema.types.get(type_id).clone() else {
            return Err("construction requires a named type".to_string());
        };
        let fields = self.sema.type_defs[index as usize].fields.clone();
        let initializers = self.sema.type_defs[index as usize]
            .field_initializers
            .clone();
        for (position, initializer) in initializers.iter().enumerate() {
            let Some(expression) = initializer else {
                continue;
            };
            let value = self.gen_expr(*expression)?;
            let pointer = self
                .struct_field_pointer(storage, type_id, position as u32)?
                .ok_or_else(|| "missing field pointer".to_string())?;
            self.builder
                .build_store(pointer, value)
                .map_err(|error| format!("cannot initialize field: {error}"))?;
        }
        let _ = fields;
        if let Some(method) = method {
            let function = *self
                .functions
                .get(&method)
                .ok_or_else(|| "missing constructor".to_string())?;
            let mut arguments = Vec::new();
            arguments.push(storage.into());
            if let NodeKind::Call(call) = self.ast.node(node).kind.clone() {
                for argument in &call.arguments {
                    arguments.push(self.gen_expr(argument.value)?.into());
                }
            }
            self.builder
                .build_call(function, &arguments, "construct")
                .map_err(|error| format!("cannot call constructor: {error}"))?;
        }
        Ok(())
    }

    fn gen_construction(&mut self, node: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let Some(Resolution::Construct { type_id, .. }) =
            self.sema.resolutions[node as usize].clone()
        else {
            return Err("expected a construction expression".to_string());
        };
        let Type::Named(index) = self.sema.types.get(type_id).clone() else {
            return Err("construction requires a named type".to_string());
        };
        let is_class = self.sema.type_defs[index as usize].kind == TypeDefKind::Class;
        let instance_type = self.instance_llvm_type(type_id)?;
        let storage = self
            .builder
            .build_alloca(instance_type, "constructed")
            .map_err(|error| format!("cannot allocate instance: {error}"))?;
        self.builder
            .build_store(storage, instance_type.const_zero())
            .map_err(|error| format!("cannot initialize instance: {error}"))?;
        if let Type::Named(class_index) = self.sema.types.get(type_id).clone() {
            if self.sema.type_defs[class_index as usize].kind == TypeDefKind::Class {
                // Initialize the class header with a zero vtable/typeinfo for now.
                let instance_struct = self.struct_type(class_index)?;
                let pointer_type = self.context.ptr_type(AddressSpace::default());
                let zero = pointer_type.const_zero();
                for field_index in 0..2u32 {
                    let pointer = self
                        .builder
                        .build_struct_gep(instance_struct, storage, field_index, "header")
                        .map_err(|error| format!("cannot initialize header: {error}"))?;
                    self.builder
                        .build_store(pointer, zero)
                        .map_err(|error| format!("cannot initialize header: {error}"))?;
                }
            }
        }
        self.gen_construct_into(node, storage)?;
        if is_class {
            Ok(storage.into())
        } else {
            self.builder
                .build_load(instance_type, storage, "constructed_value")
                .map_err(|error| format!("cannot load constructed value: {error}"))
        }
    }

    fn gen_cast_expr(
        &mut self,
        node: NodeId,
        cast: &CastExpr,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let value = self.gen_expr(cast.value)?;
        let source_type = self.type_of(cast.value);
        let target_type = self.type_of(node);
        self.convert(value, source_type, target_type)
    }

    fn convert(
        &mut self,
        value: BasicValueEnum<'ctx>,
        source: TypeId,
        target: TypeId,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        if source == target {
            return Ok(value);
        }
        let source_kind = self.sema.types.get(source).clone();
        let target_kind = self.sema.types.get(target).clone();
        let target_llvm = self.llvm_type(target)?;
        match (&source_kind, &target_kind) {
            (Type::Boolean, Type::Integer { .. }) => {
                let from = value.into_int_value();
                let target_int = target_llvm.into_int_type();
                let from = self
                    .builder
                    .build_int_z_extend(from, target_int, "bool_to_int")
                    .map_err(|error| format!("cannot convert: {error}"))?;
                Ok(from.into())
            }
            (Type::Integer { .. }, Type::Boolean) => {
                let from = value.into_int_value();
                let zero = from.get_type().const_zero();
                let condition = self
                    .builder
                    .build_int_compare(IntPredicate::NE, from, zero, "to_bool")
                    .map_err(|error| format!("cannot convert: {error}"))?;
                Ok(self
                    .builder
                    .build_int_z_extend(condition, self.context.i8_type(), "bool")
                    .map_err(|error| format!("cannot convert: {error}"))?
                    .into())
            }
            (Type::Integer { signed, bytes }, Type::Integer { .. }) => {
                let from = value.into_int_value();
                let target_int = target_llvm.into_int_type();
                let from_bits = u32::from(*bytes) * 8;
                let to_bits = target_int.get_bit_width();
                if to_bits == from_bits {
                    Ok(from.into())
                } else if to_bits > from_bits {
                    let result = if *signed {
                        self.builder.build_int_s_extend(from, target_int, "cast")
                    } else {
                        self.builder.build_int_z_extend(from, target_int, "cast")
                    };
                    Ok(result
                        .map_err(|error| format!("cannot convert: {error}"))?
                        .into())
                } else {
                    Ok(self
                        .builder
                        .build_int_truncate(from, target_int, "cast")
                        .map_err(|error| format!("cannot convert: {error}"))?
                        .into())
                }
            }
            (Type::Float { bytes: from_bytes }, Type::Float { bytes: to_bytes }) => {
                let from = value.into_float_value();
                let target_float = target_llvm.into_float_type();
                if from_bytes == to_bytes {
                    Ok(from.into())
                } else if to_bytes > from_bytes {
                    Ok(self
                        .builder
                        .build_float_ext(from, target_float, "cast")
                        .map_err(|error| format!("cannot convert: {error}"))?
                        .into())
                } else {
                    Ok(self
                        .builder
                        .build_float_trunc(from, target_float, "cast")
                        .map_err(|error| format!("cannot convert: {error}"))?
                        .into())
                }
            }
            (Type::Integer { signed, .. }, Type::Float { .. }) => {
                let from = value.into_int_value();
                let target_float = target_llvm.into_float_type();
                let result = if *signed {
                    self.builder
                        .build_signed_int_to_float(from, target_float, "cast")
                } else {
                    self.builder
                        .build_unsigned_int_to_float(from, target_float, "cast")
                };
                Ok(result
                    .map_err(|error| format!("cannot convert: {error}"))?
                    .into())
            }
            (Type::Float { .. }, Type::Integer { signed, .. }) => {
                let from = value.into_float_value();
                let target_int = target_llvm.into_int_type();
                let result = if *signed {
                    self.builder
                        .build_float_to_signed_int(from, target_int, "cast")
                } else {
                    self.builder
                        .build_float_to_unsigned_int(from, target_int, "cast")
                };
                Ok(result
                    .map_err(|error| format!("cannot convert: {error}"))?
                    .into())
            }
            (Type::Pointer { .. }, Type::Pointer { .. }) => Ok(value),
            (Type::Array { .. }, Type::Pointer { .. }) => {
                // Address of the array value: spill it and take the pointer.
                let array_type = self.llvm_type(source)?;
                let slot = self
                    .builder
                    .build_alloca(array_type, "array_address")
                    .map_err(|error| format!("cannot allocate array: {error}"))?;
                self.builder
                    .build_store(slot, value)
                    .map_err(|error| format!("cannot store array: {error}"))?;
                Ok(slot.into())
            }
            (Type::Pointer { .. }, Type::Array { .. }) => {
                let array_type = self.llvm_type(target)?;
                self.builder
                    .build_load(array_type, value.into_pointer_value(), "array_cast")
                    .map_err(|error| format!("cannot convert pointer: {error}"))
            }
            (Type::Named(_), Type::Integer { .. }) => Ok(value),
            _ => Err(format!(
                "unsupported cast from '{}' to '{}'",
                self.sema.spelling(source),
                self.sema.spelling(target)
            )),
        }
    }

    fn gen_unary_expr(
        &mut self,
        node: NodeId,
        unary: &UnaryExpr,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let operand = self.gen_expr(unary.operand)?;
        let ty = self.type_of(node);
        match unary.op {
            UnaryOp::Negate => {
                let llvm_type = self.llvm_type(ty)?;
                match llvm_type {
                    BasicTypeEnum::IntType(int_type) => Ok(self
                        .builder
                        .build_int_sub(int_type.const_zero(), operand.into_int_value(), "negate")
                        .map_err(|error| format!("cannot negate: {error}"))?
                        .into()),
                    BasicTypeEnum::FloatType(float_type) => Ok(self
                        .builder
                        .build_float_sub(
                            float_type.const_zero(),
                            operand.into_float_value(),
                            "negate",
                        )
                        .map_err(|error| format!("cannot negate: {error}"))?
                        .into()),
                    _ => Err("cannot negate this value".to_string()),
                }
            }
            UnaryOp::LogicalNot => {
                let value = operand.into_int_value();
                let value = self.as_boolean_i1(value);
                let not = self
                    .builder
                    .build_not(value, "not")
                    .map_err(|error| format!("cannot negate boolean: {error}"))?;
                Ok(self
                    .builder
                    .build_int_z_extend(not, self.context.i8_type(), "not_bool")
                    .map_err(|error| format!("cannot negate boolean: {error}"))?
                    .into())
            }
            UnaryOp::BitwiseNot => {
                let value = operand.into_int_value();
                let result = self
                    .builder
                    .build_not(value, "bitnot")
                    .map_err(|error| format!("cannot invert bits: {error}"))?;
                Ok(result.into())
            }
            UnaryOp::Dereference => {
                let target = match self.sema.types.get(self.type_of(unary.operand)).clone() {
                    Type::Pointer {
                        target: Some(target),
                    } => target,
                    _ => return Err("cannot dereference this value".to_string()),
                };
                let llvm_type = self.llvm_type(target)?;
                self.builder
                    .build_load(llvm_type, operand.into_pointer_value(), "deref")
                    .map_err(|error| format!("cannot dereference: {error}"))
            }
            UnaryOp::Reference => Err("reference expressions are not lowered yet".to_string()),
        }
    }

    fn gen_binary_expr(
        &mut self,
        node: NodeId,
        binary: &BinaryExpr,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let operand_type = self.type_of(binary.left);
        match binary.op {
            BinaryOp::LogicalAnd | BinaryOp::LogicalOr => self.gen_logical(binary),
            _ => {
                let left = self.gen_expr(binary.left)?;
                let right = self.gen_expr(binary.right)?;
                let _ = node;
                match binary.op {
                    BinaryOp::Add
                    | BinaryOp::Subtract
                    | BinaryOp::Multiply
                    | BinaryOp::Divide
                    | BinaryOp::Remainder
                    | BinaryOp::BitwiseAnd
                    | BinaryOp::BitwiseOr
                    | BinaryOp::BitwiseXor
                    | BinaryOp::ShiftLeft
                    | BinaryOp::ShiftRight => {
                        self.build_arithmetic(binary.op, left, right, operand_type, binary.left)
                    }
                    _ => Ok(self
                        .build_comparison(binary.op, left, right, operand_type)?
                        .into()),
                }
            }
        }
    }

    fn build_arithmetic(
        &mut self,
        op: BinaryOp,
        left: BasicValueEnum<'ctx>,
        right: BasicValueEnum<'ctx>,
        ty: TypeId,
        node: NodeId,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        match self.sema.types.get(ty).clone() {
            Type::Integer { signed, .. } => {
                let left = left.into_int_value();
                let right = right.into_int_value();
                let int_type = left.get_type();
                let result = match op {
                    BinaryOp::Add => self.builder.build_int_add(left, right, "add"),
                    BinaryOp::Subtract => self.builder.build_int_sub(left, right, "sub"),
                    BinaryOp::Multiply => self.builder.build_int_mul(left, right, "mul"),
                    BinaryOp::Divide | BinaryOp::Remainder => {
                        self.emit_division_by_zero_check(right, node)?;
                        if signed {
                            if op == BinaryOp::Divide {
                                self.builder.build_int_signed_div(left, right, "div")
                            } else {
                                self.builder.build_int_signed_rem(left, right, "rem")
                            }
                        } else if op == BinaryOp::Divide {
                            self.builder.build_int_unsigned_div(left, right, "div")
                        } else {
                            self.builder.build_int_unsigned_rem(left, right, "rem")
                        }
                    }
                    BinaryOp::BitwiseAnd => self.builder.build_and(left, right, "and"),
                    BinaryOp::BitwiseOr => self.builder.build_or(left, right, "or"),
                    BinaryOp::BitwiseXor => self.builder.build_xor(left, right, "xor"),
                    BinaryOp::ShiftLeft | BinaryOp::ShiftRight => {
                        let bits = int_type.get_bit_width();
                        let mask = int_type.const_int(u64::from(bits - 1), false);
                        let masked = self
                            .builder
                            .build_and(right, mask, "shift_count")
                            .map_err(|error| format!("cannot mask shift count: {error}"))?;
                        if op == BinaryOp::ShiftLeft {
                            self.builder.build_left_shift(left, masked, "shl")
                        } else {
                            self.builder
                                .build_right_shift(left, masked, signed, "shift")
                        }
                    }
                    _ => return Err("unsupported integer operator".to_string()),
                };
                Ok(result
                    .map_err(|error| format!("cannot build integer operation: {error}"))?
                    .into())
            }
            Type::Float { .. } => {
                let left = left.into_float_value();
                let right = right.into_float_value();
                let result = match op {
                    BinaryOp::Add => self.builder.build_float_add(left, right, "fadd"),
                    BinaryOp::Subtract => self.builder.build_float_sub(left, right, "fsub"),
                    BinaryOp::Multiply => self.builder.build_float_mul(left, right, "fmul"),
                    BinaryOp::Divide => self.builder.build_float_div(left, right, "fdiv"),
                    _ => return Err("unsupported float operator".to_string()),
                };
                Ok(result
                    .map_err(|error| format!("cannot build float operation: {error}"))?
                    .into())
            }
            _ => Err("operator is not defined for this type".to_string()),
        }
    }

    fn emit_division_by_zero_check(
        &mut self,
        divisor: IntValue<'ctx>,
        _node: NodeId,
    ) -> Result<(), String> {
        let function = self.current_llvm_function()?;
        let zero = divisor.get_type().const_zero();
        let is_zero = self
            .builder
            .build_int_compare(IntPredicate::EQ, divisor, zero, "div_zero")
            .map_err(|error| format!("cannot compare divisor: {error}"))?;
        let trap_block = self.context.append_basic_block(function, "div_zero_trap");
        let continue_block = self.context.append_basic_block(function, "div_continue");
        self.builder
            .build_conditional_branch(is_zero, trap_block, continue_block)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(trap_block);
        self.builder
            .build_unreachable()
            .map_err(|error| format!("cannot trap: {error}"))?;
        self.builder.position_at_end(continue_block);
        Ok(())
    }

    fn gen_logical(&mut self, binary: &BinaryExpr) -> Result<BasicValueEnum<'ctx>, String> {
        let function = self.current_llvm_function()?;
        let left = self.gen_expr(binary.left)?.into_int_value();
        let left = self.as_boolean_i1(left);
        let right_block = self.context.append_basic_block(function, "logic_right");
        let short_block = self.context.append_basic_block(function, "logic_short");
        let end_block = self.context.append_basic_block(function, "logic_end");
        match binary.op {
            BinaryOp::LogicalAnd => {
                self.builder
                    .build_conditional_branch(left, right_block, short_block)
                    .map_err(|error| format!("cannot branch: {error}"))?;
            }
            _ => {
                self.builder
                    .build_conditional_branch(left, short_block, right_block)
                    .map_err(|error| format!("cannot branch: {error}"))?;
            }
        }
        self.builder.position_at_end(short_block);
        let short_value = if binary.op == BinaryOp::LogicalAnd {
            self.context.bool_type().const_zero()
        } else {
            self.context.bool_type().const_int(1, false)
        };
        self.builder
            .build_unconditional_branch(end_block)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(right_block);
        let right = self.gen_expr(binary.right)?.into_int_value();
        let right = self.as_boolean_i1(right);
        let right_end_block = self
            .builder
            .get_insert_block()
            .ok_or_else(|| "no insert block".to_string())?;
        self.builder
            .build_unconditional_branch(end_block)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(end_block);
        let phi = self
            .builder
            .build_phi(self.context.bool_type(), "logic")
            .map_err(|error| format!("cannot build phi: {error}"))?;
        phi.add_incoming(&[(&short_value, short_block), (&right, right_end_block)]);
        Ok(self
            .builder
            .build_int_z_extend(
                phi.as_basic_value().into_int_value(),
                self.context.i8_type(),
                "logic_bool",
            )
            .map_err(|error| format!("cannot convert boolean: {error}"))?
            .into())
    }

    fn build_comparison(
        &mut self,
        op: BinaryOp,
        left: BasicValueEnum<'ctx>,
        right: BasicValueEnum<'ctx>,
        ty: TypeId,
    ) -> Result<IntValue<'ctx>, String> {
        match self.sema.types.get(ty).clone() {
            Type::Integer { signed, .. } => {
                let left = left.into_int_value();
                let right = right.into_int_value();
                let predicate = integer_predicate(op, signed)?;
                let result = self
                    .builder
                    .build_int_compare(predicate, left, right, "compare")
                    .map_err(|error| format!("cannot compare: {error}"))?;
                self.extend_bool(result)
            }
            Type::Boolean => {
                let left = left.into_int_value();
                let right = right.into_int_value();
                let predicate = match op {
                    BinaryOp::Equal => IntPredicate::EQ,
                    BinaryOp::NotEqual => IntPredicate::NE,
                    _ => return Err("unsupported boolean comparison".to_string()),
                };
                let result = self
                    .builder
                    .build_int_compare(predicate, left, right, "compare")
                    .map_err(|error| format!("cannot compare: {error}"))?;
                self.extend_bool(result)
            }
            Type::Float { .. } => {
                let left = left.into_float_value();
                let right = right.into_float_value();
                let predicate = match op {
                    BinaryOp::Equal => FloatPredicate::OEQ,
                    BinaryOp::NotEqual => FloatPredicate::UNE,
                    BinaryOp::Less => FloatPredicate::OLT,
                    BinaryOp::Greater => FloatPredicate::OGT,
                    BinaryOp::LessEqual => FloatPredicate::OLE,
                    BinaryOp::GreaterEqual => FloatPredicate::OGE,
                    _ => return Err("unsupported float comparison".to_string()),
                };
                let result = self
                    .builder
                    .build_float_compare(predicate, left, right, "compare")
                    .map_err(|error| format!("cannot compare: {error}"))?;
                self.extend_bool(result)
            }
            Type::Named(index) => {
                let underlying = self.sema.type_defs[index as usize]
                    .underlying
                    .ok_or_else(|| "unsupported comparison".to_string())?;
                self.build_comparison(op, left, right, underlying)
            }
            Type::Pointer { .. } => {
                let i64_type = self.context.i64_type();
                let left = self
                    .builder
                    .build_ptr_to_int(left.into_pointer_value(), i64_type, "left_ptr")
                    .map_err(|error| format!("cannot compare pointers: {error}"))?;
                let right = self
                    .builder
                    .build_ptr_to_int(right.into_pointer_value(), i64_type, "right_ptr")
                    .map_err(|error| format!("cannot compare pointers: {error}"))?;
                let predicate = match op {
                    BinaryOp::Equal => IntPredicate::EQ,
                    BinaryOp::NotEqual => IntPredicate::NE,
                    _ => return Err("unsupported pointer comparison".to_string()),
                };
                let result = self
                    .builder
                    .build_int_compare(predicate, left, right, "compare")
                    .map_err(|error| format!("cannot compare: {error}"))?;
                self.extend_bool(result)
            }
            _ => Err("comparison is not defined for this type".to_string()),
        }
    }

    fn extend_bool(&self, value: IntValue<'ctx>) -> Result<IntValue<'ctx>, String> {
        if value.get_type().get_bit_width() == 8 {
            Ok(value)
        } else {
            self.builder
                .build_int_z_extend(value, self.context.i8_type(), "bool_result")
                .map_err(|error| format!("cannot convert comparison: {error}"))
        }
    }

    fn as_boolean_i1(&self, value: IntValue<'ctx>) -> IntValue<'ctx> {
        if value.get_type().get_bit_width() == 1 {
            return value;
        }
        let zero = value.get_type().const_zero();
        self.builder
            .build_int_compare(IntPredicate::NE, value, zero, "as_bool")
            .expect("building a comparison cannot fail")
    }

    fn is_signed(&self, ty: TypeId) -> bool {
        matches!(self.sema.types.get(ty), Type::Integer { signed: true, .. })
    }

    fn gen_conditional_expr(
        &mut self,
        node: NodeId,
        conditional: &ConditionalExpr,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let function = self.current_llvm_function()?;
        let llvm_type = self.llvm_type(self.type_of(node))?;
        let condition = self.gen_expr(conditional.condition)?.into_int_value();
        let condition = self.as_boolean_i1(condition);
        let then_block = self.context.append_basic_block(function, "cond_then");
        let else_block = self.context.append_basic_block(function, "cond_else");
        let end_block = self.context.append_basic_block(function, "cond_end");
        self.builder
            .build_conditional_branch(condition, then_block, else_block)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(then_block);
        let then_value = self.gen_expr(conditional.then_value)?;
        let then_end = self
            .builder
            .get_insert_block()
            .ok_or_else(|| "no insert block".to_string())?;
        self.builder
            .build_unconditional_branch(end_block)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(else_block);
        let else_value = self.gen_expr(conditional.else_value)?;
        let else_end = self
            .builder
            .get_insert_block()
            .ok_or_else(|| "no insert block".to_string())?;
        self.builder
            .build_unconditional_branch(end_block)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(end_block);
        let phi = self
            .builder
            .build_phi(llvm_type, "conditional")
            .map_err(|error| format!("cannot build phi: {error}"))?;
        phi.add_incoming(&[(&then_value, then_end), (&else_value, else_end)]);
        Ok(phi.as_basic_value())
    }
}

fn integer_predicate(op: BinaryOp, signed: bool) -> Result<IntPredicate, String> {
    Ok(match op {
        BinaryOp::Equal => IntPredicate::EQ,
        BinaryOp::NotEqual => IntPredicate::NE,
        BinaryOp::Less => {
            if signed {
                IntPredicate::SLT
            } else {
                IntPredicate::ULT
            }
        }
        BinaryOp::Greater => {
            if signed {
                IntPredicate::SGT
            } else {
                IntPredicate::UGT
            }
        }
        BinaryOp::LessEqual => {
            if signed {
                IntPredicate::SLE
            } else {
                IntPredicate::ULE
            }
        }
        BinaryOp::GreaterEqual => {
            if signed {
                IntPredicate::SGE
            } else {
                IntPredicate::UGE
            }
        }
        _ => return Err("unsupported integer comparison".to_string()),
    })
}
