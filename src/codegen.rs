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
    BasicValueEnum, CallSiteValue, FunctionValue, GlobalValue, IntValue, PointerValue, ValueKind,
};
use inkwell::{AddressSpace, FloatPredicate, IntPredicate};

use crate::ast::{
    ArrayElement, Ast, BinaryExpr, BinaryOp, CallExpr, CastExpr, CoalescingAlternative,
    ConditionalExpr, ForStmt, ForTarget, IndexExpr, Initializer, IsExpr, IsTarget, MatchStmt,
    NodeId, NodeKind, Pattern, SetStmt, UnaryExpr, UnaryOp,
};
use crate::literal;
use crate::sema::types::{Type, TypeId};
use crate::sema::{
    CaptureSource, MatchPatternInfo, ParameterInfo, Resolution, SemaResult, Storage, TypeDefKind,
};

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
        typeinfo_globals: HashMap::new(),
        vtable_globals: HashMap::new(),
        interface_tables: HashMap::new(),
        trap_stack: Vec::new(),
        type_key_globals: HashMap::new(),
        closure_globals: HashMap::new(),
        current_env_slot: None,
        builtin_exceptions: HashMap::new(),
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

/// One active lexical trap frame while lowering statements (§5.10.4).
struct TrapFrame<'ctx> {
    /// Storage of the thrown value: `{ typeinfo, payload }`.
    exception_slot: PointerValue<'ctx>,
    /// Block that matches catch clauses.
    dispatch: inkwell::basic_block::BasicBlock<'ctx>,
    /// Whether this frame is a `catch` handler (throws skip it).
    is_handler: bool,
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
    typeinfo_globals: HashMap<u32, GlobalValue<'ctx>>,
    vtable_globals: HashMap<u32, GlobalValue<'ctx>>,
    interface_tables: HashMap<u32, GlobalValue<'ctx>>,
    trap_stack: Vec<TrapFrame<'ctx>>,
    type_key_globals: HashMap<TypeId, GlobalValue<'ctx>>,
    /// Static closure objects created for function values (§2.7.9).
    closure_globals: HashMap<u32, GlobalValue<'ctx>>,
    /// Slot holding the current literal's closure environment pointer.
    current_env_slot: Option<PointerValue<'ctx>>,
    builtin_exceptions: HashMap<u32, GlobalValue<'ctx>>,
}

impl<'ctx, 'a> Generator<'ctx, 'a> {
    fn run(&mut self) -> Result<(), String> {
        self.ensure_libc()?;
        self.prepare_struct_types()?;
        self.declare_functions()?;
        self.build_class_metadata()?;
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
                TypeDefKind::Class | TypeDefKind::Interface => {
                    self.context.ptr_type(AddressSpace::default()).into()
                }
                TypeDefKind::Variant | TypeDefKind::Union => self.instance_llvm_type(ty)?,
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
                definition.generic_parameters.is_empty()
                    && matches!(definition.kind, TypeDefKind::Structure | TypeDefKind::Class)
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
        let environment = self.sema.functions[function_index as usize].environment;
        let info = &self.sema.functions[function_index as usize];
        let parameter_types = info.parameters.clone();
        let return_type = info.return_type;
        let mut parameter_metadata = Vec::with_capacity(parameter_types.len() + 1);
        if environment {
            parameter_metadata.push(self.context.ptr_type(AddressSpace::default()).into());
        }
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
            // Generic function templates are only lowered through their
            // instantiations (§10.1).
            if !self.sema.functions[index as usize]
                .generic_parameters
                .is_empty()
            {
                continue;
            }
            let signature = self.function_type(index)?;
            let mangled = self.sema.functions[index as usize].mangled.clone();
            let function = self.module.add_function(&mangled, signature, None);
            self.functions.insert(index, function);
        }
        Ok(())
    }

    /// Creates the per-class type information and virtual dispatch tables
    /// (§13.1): typeinfo = `{ name, base }`, vtable = array of method pointers.
    fn build_class_metadata(&mut self) -> Result<(), String> {
        let pointer_type = self.context.ptr_type(AddressSpace::default());
        let classes: Vec<u32> = self
            .sema
            .type_defs
            .iter()
            .enumerate()
            .filter(|(_, definition)| {
                // Generic declarations are templates and get no metadata of
                // their own; their instances do (§10.1).
                definition.generic_parameters.is_empty()
                    && matches!(definition.kind, TypeDefKind::Class | TypeDefKind::Interface)
            })
            .map(|(index, _)| index as u32)
            .collect();

        // Type information: [name, base typeinfo, interface table, null].
        let typeinfo_type = pointer_type.array_type(4);
        for &index in &classes {
            let name = self.sema.type_defs[index as usize].qualified_name.clone();
            let string_value = self.context.const_string(name.as_bytes(), false);
            let name_global = self.module.add_global(
                string_value.get_type(),
                None,
                &format!("nsc.class_name.{index}"),
            );
            name_global.set_initializer(&string_value);
            name_global.set_constant(true);
            let typeinfo_global =
                self.module
                    .add_global(typeinfo_type, None, &format!("nsc.typeinfo.{index}"));
            let initializer = pointer_type.const_array(&[
                name_global.as_pointer_value(),
                pointer_type.const_zero(),
                pointer_type.const_zero(),
                pointer_type.const_zero(),
            ]);
            typeinfo_global.set_initializer(&initializer);
            typeinfo_global.set_constant(true);
            self.typeinfo_globals.insert(index, typeinfo_global);
        }
        // Interface dispatch tables (§3.7.3): each class gets a terminated array
        // of { interface typeinfo, interface vtable } pairs.
        for &index in &classes {
            let mut entries: Vec<(TypeId, Vec<u32>)> = Vec::new();
            let mut chain = Vec::new();
            let mut current = self.sema.type_defs[index as usize].base;
            while let Some(base) = current {
                chain.push(base);
                current = match self.sema.types.get(base) {
                    Type::Named(base_index) => self.sema.type_defs[*base_index as usize].base,
                    _ => None,
                };
            }
            chain.reverse();
            for base in chain {
                if let Type::Named(base_index) = self.sema.types.get(base) {
                    for entry in self.sema.type_defs[*base_index as usize]
                        .interface_dispatch
                        .clone()
                    {
                        if let Some(existing) =
                            entries.iter_mut().find(|(iface, _)| *iface == entry.0)
                        {
                            *existing = entry;
                        } else {
                            entries.push(entry);
                        }
                    }
                }
            }
            for entry in self.sema.type_defs[index as usize]
                .interface_dispatch
                .clone()
            {
                if let Some(existing) = entries.iter_mut().find(|(iface, _)| *iface == entry.0) {
                    *existing = entry;
                } else {
                    entries.push(entry);
                }
            }
            if entries.is_empty() {
                continue;
            }
            let mut table_values: Vec<PointerValue<'ctx>> = Vec::new();
            for (iface_type, impl_methods) in entries {
                let Type::Named(iface_index) = self.sema.types.get(iface_type) else {
                    continue;
                };
                let Some(iface_typeinfo) = self.typeinfo_globals.get(iface_index).copied() else {
                    continue;
                };
                let vtable_type = pointer_type.array_type(impl_methods.len() as u32);
                let vtable_global = self.module.add_global(
                    vtable_type,
                    None,
                    &format!("nsc.iface_vtable.{index}.{iface_index}"),
                );
                let implementations: Vec<FunctionValue<'ctx>> = impl_methods
                    .iter()
                    .filter_map(|method| self.functions.get(method).copied())
                    .collect();
                let values: Vec<PointerValue<'ctx>> = implementations
                    .iter()
                    .map(|function| function.as_global_value().as_pointer_value())
                    .collect();
                vtable_global.set_initializer(&pointer_type.const_array(&values));
                vtable_global.set_constant(true);
                table_values.push(iface_typeinfo.as_pointer_value());
                table_values.push(vtable_global.as_pointer_value());
            }
            table_values.push(pointer_type.const_zero());
            table_values.push(pointer_type.const_zero());
            let table_type = pointer_type.array_type(table_values.len() as u32);
            let table_global =
                self.module
                    .add_global(table_type, None, &format!("nsc.iface_table.{index}"));
            table_global.set_initializer(&pointer_type.const_array(&table_values));
            table_global.set_constant(true);
            self.interface_tables.insert(index, table_global);
        }

        // Static instances for the Built-In Library exception classes so that
        // runtime exception sources can throw them (§15.2).
        const BUILTIN_EXCEPTIONS: [&str; 6] = [
            "Exception",
            "NotImplementedException",
            "DivideByZeroException",
            "OutOfRangeException",
            "AllocateFailedException",
            "DeallocateFailedException",
        ];
        for (index, definition) in self.sema.type_defs.iter().enumerate() {
            if !BUILTIN_EXCEPTIONS.contains(&definition.qualified_name.as_str()) {
                continue;
            }
            let instance_type = self.struct_type(index as u32)?;
            let global =
                self.module
                    .add_global(instance_type, None, &format!("nsc.exception.{index}"));
            // InnerException defaults to Optional.Null (tag 1) rather than the
            // zero-initialized Optional.Value(null) (§15.2).
            let pointer_type = self.context.ptr_type(AddressSpace::default());
            let initializer = match definition.fields.first() {
                Some(field) => {
                    let field_type = self.llvm_type(field.ty)?;
                    let BasicTypeEnum::ArrayType(array_type) = field_type else {
                        return Err("exception field must be a variant".to_string());
                    };
                    let mut bytes: Vec<inkwell::values::IntValue<'ctx>> =
                        Vec::with_capacity(array_type.len() as usize);
                    for position in 0..array_type.len() {
                        let value = if position == 0 {
                            self.context.i8_type().const_int(1, false)
                        } else {
                            self.context.i8_type().const_zero()
                        };
                        bytes.push(value);
                    }
                    let field_value = self.context.i8_type().const_array(&bytes);
                    instance_type.const_named_struct(&[
                        pointer_type.const_null().into(),
                        pointer_type.const_null().into(),
                        field_value.into(),
                    ])
                }
                None => instance_type.const_zero(),
            };
            global.set_initializer(&initializer);
            self.builtin_exceptions.insert(index as u32, global);
        }

        for &index in &classes {
            let base_global = match self.sema.type_defs[index as usize].base {
                Some(base_type) => match self.sema.types.get(base_type) {
                    Type::Named(base_index) => self.typeinfo_globals.get(base_index).copied(),
                    _ => None,
                },
                None => None,
            };
            let name_pointer = {
                let name = self.sema.type_defs[index as usize].qualified_name.clone();
                let string_value = self.context.const_string(name.as_bytes(), false);
                // Reuse the existing name global by name lookup.
                self.module
                    .get_global(&format!("nsc.class_name.{index}"))
                    .unwrap_or_else(|| {
                        let global = self.module.add_global(
                            string_value.get_type(),
                            None,
                            &format!("nsc.class_name.{index}"),
                        );
                        global.set_initializer(&string_value);
                        global.set_constant(true);
                        global
                    })
            };
            let base_pointer = base_global.map_or_else(
                || pointer_type.const_zero(),
                |global| global.as_pointer_value(),
            );
            let interface_table = self.interface_tables.get(&index).map_or_else(
                || pointer_type.const_zero(),
                |global| global.as_pointer_value(),
            );
            let initializer = pointer_type.const_array(&[
                name_pointer.as_pointer_value(),
                base_pointer,
                interface_table,
                pointer_type.const_zero(),
            ]);
            if let Some(typeinfo_global) = self.typeinfo_globals.get(&index) {
                typeinfo_global.set_initializer(&initializer);
            }
        }

        // Virtual dispatch tables reference the declared functions.
        for &index in &classes {
            let slots = self.sema.type_defs[index as usize].vtable.clone();
            let vtable_type = pointer_type.array_type(slots.len() as u32);
            let vtable_global =
                self.module
                    .add_global(vtable_type, None, &format!("nsc.vtable.{index}"));
            let values: Vec<PointerValue<'ctx>> = slots
                .iter()
                .filter_map(|method| {
                    self.functions
                        .get(method)
                        .map(|function| function.as_global_value().as_pointer_value())
                })
                .collect();
            let initializer = pointer_type.const_array(&values);
            vtable_global.set_initializer(&initializer);
            vtable_global.set_constant(true);
            self.vtable_globals.insert(index, vtable_global);
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
            if !self.sema.functions[index as usize]
                .generic_parameters
                .is_empty()
            {
                continue;
            }
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
            let Some(body) = body else {
                continue;
            };
            self.current_function = index;
            self.current_return = return_type;
            self.local_slots.clear();
            self.current_this_slot = None;
            self.current_env_slot = None;
            self.loops.clear();

            let entry = self.context.append_basic_block(function, "entry");
            self.builder.position_at_end(entry);
            if self.sema.functions[index as usize].environment {
                let pointer_type = self.context.ptr_type(AddressSpace::default());
                let env_slot = self
                    .builder
                    .build_alloca(pointer_type, "environment")
                    .map_err(|error| format!("cannot allocate environment: {error}"))?;
                let env_value = function
                    .get_nth_param(0)
                    .ok_or_else(|| "literal has no environment parameter".to_string())?;
                self.builder
                    .build_store(env_slot, env_value)
                    .map_err(|error| format!("cannot store environment: {error}"))?;
                self.current_env_slot = Some(env_slot);
            }
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
                if matches!(local.storage, Storage::Capture(_)) {
                    continue;
                }
                let llvm_type = if matches!(local.storage, Storage::Heap | Storage::Reference) {
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
            match body {
                crate::sema::FunctionBody::Block(block) => self.gen_block_contents(block)?,
                crate::sema::FunctionBody::Expression(expression) => {
                    let value = self.gen_expr(expression)?;
                    match return_type {
                        Some(_) => {
                            self.builder
                                .build_return(Some(&value))
                                .map_err(|error| format!("cannot return: {error}"))?;
                        }
                        None => {
                            self.builder
                                .build_return(None)
                                .map_err(|error| format!("cannot return: {error}"))?;
                        }
                    }
                }
                crate::sema::FunctionBody::BackingGet { owner, field_index } => {
                    let this_pointer = self
                        .current_this_slot
                        .ok_or_else(|| "accessor without this".to_string())?;
                    let pointer_type = self.context.ptr_type(AddressSpace::default());
                    let this = self
                        .builder
                        .build_load(pointer_type, this_pointer, "this")
                        .map_err(|error| format!("cannot load this: {error}"))?
                        .into_pointer_value();
                    let field_pointer = self
                        .struct_field_pointer(this, owner, field_index)?
                        .ok_or_else(|| "missing backing field".to_string())?;
                    let field_type = self.llvm_type(
                        return_type
                            .ok_or_else(|| "accessor getter has no return type".to_string())?,
                    )?;
                    let value = self
                        .builder
                        .build_load(field_type, field_pointer, "backing")
                        .map_err(|error| format!("cannot load backing: {error}"))?;
                    self.builder
                        .build_return(Some(&value))
                        .map_err(|error| format!("cannot return: {error}"))?;
                }
                crate::sema::FunctionBody::BackingSet { owner, field_index } => {
                    let this_pointer = self
                        .current_this_slot
                        .ok_or_else(|| "accessor without this".to_string())?;
                    let pointer_type = self.context.ptr_type(AddressSpace::default());
                    let this = self
                        .builder
                        .build_load(pointer_type, this_pointer, "this")
                        .map_err(|error| format!("cannot load this: {error}"))?
                        .into_pointer_value();
                    let field_pointer = self
                        .struct_field_pointer(this, owner, field_index)?
                        .ok_or_else(|| "missing backing field".to_string())?;
                    let value = function
                        .get_nth_param(1)
                        .ok_or_else(|| "setter requires a value".to_string())?;
                    self.builder
                        .build_store(field_pointer, value)
                        .map_err(|error| format!("cannot store backing: {error}"))?;
                    self.builder
                        .build_return(None)
                        .map_err(|error| format!("cannot return: {error}"))?;
                }
            }
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
        // Initialize the static exception instances' class headers.
        let builtin_exceptions: Vec<(u32, GlobalValue<'ctx>)> = self
            .builtin_exceptions
            .iter()
            .map(|(index, global)| (*index, *global))
            .collect();
        for (index, global) in builtin_exceptions {
            let instance_struct = self.struct_type(index)?;
            let storage = global.as_pointer_value();
            let vtable = self.vtable_globals.get(&index).map_or_else(
                || self.context.ptr_type(AddressSpace::default()).const_zero(),
                |vtable| vtable.as_pointer_value(),
            );
            let typeinfo = self.typeinfo_globals.get(&index).map_or_else(
                || self.context.ptr_type(AddressSpace::default()).const_zero(),
                |typeinfo| typeinfo.as_pointer_value(),
            );
            for (field_index, header_value) in [vtable, typeinfo].into_iter().enumerate() {
                let pointer = self
                    .builder
                    .build_struct_gep(
                        instance_struct,
                        storage,
                        field_index as u32,
                        "exception_header",
                    )
                    .map_err(|error| format!("cannot initialize exception: {error}"))?;
                self.builder
                    .build_store(pointer, header_value)
                    .map_err(|error| format!("cannot initialize exception: {error}"))?;
            }
        }

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
            NodeKind::Trap(statement) => self.gen_trap_statement(node, &statement),
            NodeKind::Throw(statement) => self.gen_throw_statement(&statement),
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
            NodeKind::Match(statement) => self.gen_match_statement(node, &statement),
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
                let taken = self
                    .sema
                    .compile_time_conditions
                    .get(&node)
                    .copied()
                    .unwrap_or(false);
                if taken {
                    self.gen_block_contents(statement.then_block)
                } else if let Some(else_block) = statement.else_block {
                    self.gen_block_contents(else_block)
                } else {
                    Ok(())
                }
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

    fn builder_terminated(&self) -> bool {
        self.builder
            .get_insert_block()
            .is_some_and(|block| block.get_terminator().is_some())
    }

    fn type_key(&mut self, ty: TypeId) -> Result<PointerValue<'ctx>, String> {
        if let Some(global) = self.type_key_globals.get(&ty) {
            return Ok(global.as_pointer_value());
        }
        let index = self.type_key_globals.len();
        // Keys are laid out like type information so that catch matching can walk
        // them safely: every slot is null, so a class or interface walk stops
        // immediately instead of reading garbage (§5.10.4).
        let pointer_type = self.context.ptr_type(AddressSpace::default());
        let key_type = pointer_type.array_type(4);
        let global = self
            .module
            .add_global(key_type, None, &format!("nsc.typekey.{index}"));
        global.set_initializer(&key_type.const_zero());
        global.set_constant(true);
        self.type_key_globals.insert(ty, global);
        Ok(global.as_pointer_value())
    }

    /// Checks whether a thrown value's typeinfo matches a catch target type.
    fn emit_typeinfo_match(
        &mut self,
        typeinfo: PointerValue<'ctx>,
        target: TypeId,
    ) -> Result<IntValue<'ctx>, String> {
        let Type::Named(index) = self.sema.types.get(target).clone() else {
            let key = self.type_key(target)?;
            return self
                .builder
                .build_int_compare(IntPredicate::EQ, typeinfo, key, "catch_type")
                .map_err(|error| format!("cannot compare catch type: {error}"));
        };
        match self.sema.type_defs[index as usize].kind {
            TypeDefKind::Class => self.emit_class_from_typeinfo(typeinfo, index),
            TypeDefKind::Interface => self.emit_interface_from_typeinfo(typeinfo, index),
            _ => {
                let key = self.type_key(target)?;
                self.builder
                    .build_int_compare(IntPredicate::EQ, typeinfo, key, "catch_type")
                    .map_err(|error| format!("cannot compare catch type: {error}"))
            }
        }
    }

    /// Walks a typeinfo base chain looking for a class.
    fn emit_class_from_typeinfo(
        &mut self,
        typeinfo: PointerValue<'ctx>,
        target_index: u32,
    ) -> Result<IntValue<'ctx>, String> {
        let pointer_type = self.context.ptr_type(AddressSpace::default());
        let target = self
            .typeinfo_globals
            .get(&target_index)
            .ok_or_else(|| "missing target type information".to_string())?
            .as_pointer_value();
        let current_slot = self
            .builder
            .build_alloca(pointer_type, "catch_current")
            .map_err(|error| format!("cannot allocate catch state: {error}"))?;
        self.builder
            .build_store(current_slot, typeinfo)
            .map_err(|error| format!("cannot store catch state: {error}"))?;
        let result_slot = self
            .builder
            .build_alloca(self.context.bool_type(), "catch_result")
            .map_err(|error| format!("cannot allocate catch result: {error}"))?;
        self.builder
            .build_store(result_slot, self.context.bool_type().const_zero())
            .map_err(|error| format!("cannot store catch result: {error}"))?;
        let function = self.current_llvm_function()?;
        let loop_block = self.context.append_basic_block(function, "catch_loop");
        let true_block = self.context.append_basic_block(function, "catch_true");
        let end_block = self.context.append_basic_block(function, "catch_end");
        self.builder
            .build_unconditional_branch(loop_block)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(loop_block);
        let current = self
            .builder
            .build_load(pointer_type, current_slot, "current")
            .map_err(|error| format!("cannot load catch state: {error}"))?
            .into_pointer_value();
        let is_null = self
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                current,
                pointer_type.const_zero(),
                "catch_null",
            )
            .map_err(|error| format!("cannot compare catch state: {error}"))?;
        let check_block = self.context.append_basic_block(function, "catch_check");
        self.builder
            .build_conditional_branch(is_null, end_block, check_block)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(check_block);
        let matches = self
            .builder
            .build_int_compare(IntPredicate::EQ, current, target, "catch_match")
            .map_err(|error| format!("cannot compare catch type: {error}"))?;
        let next_block = self.context.append_basic_block(function, "catch_next");
        self.builder
            .build_conditional_branch(matches, true_block, next_block)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(next_block);
        let base = self.load_pointer_at(current, 8)?;
        self.builder
            .build_store(current_slot, base)
            .map_err(|error| format!("cannot store catch state: {error}"))?;
        self.builder
            .build_unconditional_branch(loop_block)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(true_block);
        self.builder
            .build_store(result_slot, self.context.bool_type().const_int(1, false))
            .map_err(|error| format!("cannot store catch result: {error}"))?;
        self.builder
            .build_unconditional_branch(end_block)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(end_block);
        Ok(self
            .builder
            .build_load(self.context.bool_type(), result_slot, "catch_result")
            .map_err(|error| format!("cannot load catch result: {error}"))?
            .into_int_value())
    }

    fn emit_abort(&mut self) -> Result<(), String> {
        let abort = self.module.get_function("abort").unwrap_or_else(|| {
            let signature = self.context.void_type().fn_type(&[], false);
            self.module.add_function("abort", signature, None)
        });
        self.builder
            .build_call(abort, &[], "")
            .map_err(|error| format!("cannot call abort: {error}"))?;
        self.builder
            .build_unreachable()
            .map_err(|error| format!("cannot terminate: {error}"))?;
        Ok(())
    }

    /// Propagates the exception in `exception_slot` to the next outer body trap,
    /// or terminates the program (§5.10.1-7).
    fn emit_outer_dispatch(&mut self, exception_slot: PointerValue<'ctx>) -> Result<(), String> {
        let outer = self
            .trap_stack
            .iter()
            .rev()
            .find(|frame| !frame.is_handler)
            .map(|frame| (frame.exception_slot, frame.dispatch));
        let Some((outer_slot, outer_dispatch)) = outer else {
            return self.emit_abort();
        };
        let typeinfo = self.load_pointer_at(exception_slot, 0)?;
        let payload = self.load_pointer_at(exception_slot, 8)?;
        let typeinfo_slot =
            self.address_with_size(outer_slot, 1, self.context.i64_type().const_zero())?;
        let payload_slot =
            self.address_with_size(outer_slot, 1, self.context.i64_type().const_int(8, false))?;
        self.builder
            .build_store(typeinfo_slot, typeinfo)
            .map_err(|error| format!("cannot propagate exception: {error}"))?;
        self.builder
            .build_store(payload_slot, payload)
            .map_err(|error| format!("cannot propagate exception: {error}"))?;
        self.builder
            .build_unconditional_branch(outer_dispatch)
            .map_err(|error| format!("cannot propagate exception: {error}"))?;
        Ok(())
    }

    fn gen_trap_statement(
        &mut self,
        node: NodeId,
        statement: &crate::ast::TrapStmt,
    ) -> Result<(), String> {
        let function = self.current_llvm_function()?;
        let pointer_type = self.context.ptr_type(AddressSpace::default());
        let exception_type = self
            .context
            .struct_type(&[pointer_type.into(), pointer_type.into()], false);
        let exception_slot = self
            .builder
            .build_alloca(exception_type, "exception")
            .map_err(|error| format!("cannot allocate exception: {error}"))?;
        let dispatch = self.context.append_basic_block(function, "trap_dispatch");
        let after_body = self.context.append_basic_block(function, "trap_after_body");
        let end = self.context.append_basic_block(function, "trap_end");
        let finally_blocks = statement.finally.map(|finally| {
            (
                finally,
                self.context.append_basic_block(function, "trap_finally"),
                self.context
                    .append_basic_block(function, "trap_finally_propagate"),
            )
        });
        self.trap_stack.push(TrapFrame {
            exception_slot,
            dispatch,
            is_handler: false,
        });
        self.gen_block_contents(statement.body)?;
        self.trap_stack.pop();
        if !self.builder_terminated() {
            self.builder
                .build_unconditional_branch(after_body)
                .map_err(|error| format!("cannot branch: {error}"))?;
        }
        self.builder.position_at_end(after_body);
        if let Some((finally, normal, _)) = finally_blocks {
            self.builder
                .build_unconditional_branch(normal)
                .map_err(|error| format!("cannot branch: {error}"))?;
            self.builder.position_at_end(normal);
            self.gen_block_contents(finally)?;
            if !self.builder_terminated() {
                self.builder
                    .build_unconditional_branch(end)
                    .map_err(|error| format!("cannot branch: {error}"))?;
            }
        } else {
            self.builder
                .build_unconditional_branch(end)
                .map_err(|error| format!("cannot branch: {error}"))?;
        }
        let catches = self
            .sema
            .trap_catches
            .get(&node)
            .cloned()
            .unwrap_or_default();
        let mut check_block = dispatch;
        for catch in &catches {
            let catch_block = self.context.append_basic_block(function, "trap_catch");
            let next_block = self.context.append_basic_block(function, "trap_check");
            self.builder.position_at_end(check_block);
            if catch.wildcard {
                self.builder
                    .build_unconditional_branch(catch_block)
                    .map_err(|error| format!("cannot branch: {error}"))?;
            } else if let Some(target) = catch.ty {
                let typeinfo = self
                    .load_pointer_at(exception_slot, 0)?
                    .into_pointer_value();
                let condition = self.emit_typeinfo_match(typeinfo, target)?;
                self.builder
                    .build_conditional_branch(condition, catch_block, next_block)
                    .map_err(|error| format!("cannot branch: {error}"))?;
            } else {
                self.builder
                    .build_unconditional_branch(next_block)
                    .map_err(|error| format!("cannot branch: {error}"))?;
            }
            self.builder.position_at_end(catch_block);
            if let Some(binding) = catch.binding {
                let slot = *self
                    .local_slots
                    .get(&binding)
                    .ok_or_else(|| "missing catch binding slot".to_string())?;
                let payload = self
                    .load_pointer_at(exception_slot, 8)?
                    .into_pointer_value();
                let target = catch.ty.ok_or_else(|| "catch type missing".to_string())?;
                let is_reference = matches!(self.sema.types.get(target), Type::Named(index) if matches!(self.sema.type_defs[*index as usize].kind, TypeDefKind::Class | TypeDefKind::Interface));
                if is_reference {
                    self.builder
                        .build_store(slot, payload)
                        .map_err(|error| format!("cannot bind exception: {error}"))?;
                } else {
                    let llvm_type = self.llvm_type(target)?;
                    let value = self
                        .builder
                        .build_load(llvm_type, payload, "catch_value")
                        .map_err(|error| format!("cannot load exception: {error}"))?;
                    self.builder
                        .build_store(slot, value)
                        .map_err(|error| format!("cannot bind exception: {error}"))?;
                }
            }
            self.trap_stack.push(TrapFrame {
                exception_slot,
                dispatch,
                is_handler: true,
            });
            self.gen_block_contents(catch.body)?;
            self.trap_stack.pop();
            if !self.builder_terminated() {
                match finally_blocks {
                    Some((_, normal, _)) => self
                        .builder
                        .build_unconditional_branch(normal)
                        .map_err(|error| format!("cannot branch: {error}"))?,
                    None => self
                        .builder
                        .build_unconditional_branch(end)
                        .map_err(|error| format!("cannot branch: {error}"))?,
                };
            }
            check_block = next_block;
        }
        self.builder.position_at_end(check_block);
        match finally_blocks {
            Some((finally, _, propagate)) => {
                self.builder
                    .build_unconditional_branch(propagate)
                    .map_err(|error| format!("cannot branch: {error}"))?;
                self.builder.position_at_end(propagate);
                self.gen_block_contents(finally)?;
                if !self.builder_terminated() {
                    self.emit_outer_dispatch(exception_slot)?;
                }
            }
            None => self.emit_outer_dispatch(exception_slot)?,
        }
        self.builder.position_at_end(end);
        Ok(())
    }

    fn gen_throw_statement(&mut self, statement: &crate::ast::ThrowStmt) -> Result<(), String> {
        let value = self.gen_expr(statement.value)?;
        let ty = self.type_of(statement.value);
        self.emit_throw_value(value, ty)
    }

    /// Emits a throw of `value` (of static type `ty`) to the nearest enclosing
    /// non-handler trap frame (§5.10.3).
    fn emit_throw_value(&mut self, value: BasicValueEnum<'ctx>, ty: TypeId) -> Result<(), String> {
        let frame_index = self.trap_stack.iter().rposition(|frame| !frame.is_handler);
        let Some(frame_index) = frame_index else {
            return Err("'throw' outside a trap".to_string());
        };
        let (exception_slot, dispatch) = {
            let frame = &self.trap_stack[frame_index];
            (frame.exception_slot, frame.dispatch)
        };
        let (typeinfo, payload) = match self.sema.types.get(ty).clone() {
            Type::Named(index)
                if matches!(
                    self.sema.type_defs[index as usize].kind,
                    TypeDefKind::Class | TypeDefKind::Interface
                ) =>
            {
                let instance = value.into_pointer_value();
                let typeinfo = self.load_pointer_at(instance, 8)?.into_pointer_value();
                (typeinfo, instance)
            }
            _ => {
                let llvm_type = self.llvm_type(ty)?;
                let spill = self
                    .builder
                    .build_alloca(llvm_type, "throw_value")
                    .map_err(|error| format!("cannot allocate thrown value: {error}"))?;
                self.builder
                    .build_store(spill, value)
                    .map_err(|error| format!("cannot store thrown value: {error}"))?;
                (self.type_key(ty)?, spill)
            }
        };
        let typeinfo_slot =
            self.address_with_size(exception_slot, 1, self.context.i64_type().const_zero())?;
        let payload_slot = self.address_with_size(
            exception_slot,
            1,
            self.context.i64_type().const_int(8, false),
        )?;
        self.builder
            .build_store(typeinfo_slot, typeinfo)
            .map_err(|error| format!("cannot store exception: {error}"))?;
        self.builder
            .build_store(payload_slot, payload)
            .map_err(|error| format!("cannot store exception: {error}"))?;
        self.builder
            .build_unconditional_branch(dispatch)
            .map_err(|error| format!("cannot transfer exception: {error}"))?;
        let dead = self
            .context
            .append_basic_block(self.current_llvm_function()?, "throw_dead");
        self.builder.position_at_end(dead);
        Ok(())
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
                    | Some(Resolution::ConstructVariant { .. })
                    | Some(Resolution::ConstructUnion { .. })
            );
            if is_construction {
                match self.sema.resolutions[initializer as usize].clone() {
                    Some(Resolution::Construct { .. }) => {
                        self.gen_construct_into(initializer, storage)?;
                    }
                    _ => self.gen_member_construction_into(initializer, storage)?,
                }
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

    fn gen_coalesce_expr(
        &mut self,
        node: NodeId,
        coalesce: &crate::ast::CoalesceExpr,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let info = *self
            .sema
            .coalesce
            .get(&node)
            .ok_or_else(|| "missing coalescing information".to_string())?;
        let Type::Named(index) = self.sema.types.get(info.decision_type).clone() else {
            return Err("coalescing requires a variant decision".to_string());
        };
        let (tag_type, payload_offset) = {
            let definition = &self.sema.type_defs[index as usize];
            (
                definition
                    .tag_type
                    .ok_or_else(|| "decision variant has no tag type".to_string())?,
                definition.payload_offset,
            )
        };
        let decision_llvm = self.instance_llvm_type(info.decision_type)?;
        // A user-defined `operator ??` produces the decision value; otherwise the
        // operand itself is the decision.
        let value = match info.operator {
            Some(method) => {
                let operand = self.gen_expr(coalesce.value)?;
                self.gen_operator_call(method, vec![operand])?
            }
            None => self.gen_expr(coalesce.value)?,
        };
        let decision_slot = self
            .builder
            .build_alloca(decision_llvm, "decision")
            .map_err(|error| format!("cannot allocate decision: {error}"))?;
        self.builder
            .build_store(decision_slot, value)
            .map_err(|error| format!("cannot store decision: {error}"))?;
        let result_type = info.result_type;
        let result_llvm = if result_type == TypeId::VOID {
            self.context.i8_type().into()
        } else {
            self.llvm_type(result_type)?
        };
        let result_slot = self
            .builder
            .build_alloca(result_llvm, "coalesced")
            .map_err(|error| format!("cannot allocate coalesced value: {error}"))?;
        let tag_llvm = self.llvm_type(tag_type)?.into_int_type();
        let tag_address =
            self.address_with_size(decision_slot, 1, self.context.i64_type().const_zero())?;
        let tag = self
            .builder
            .build_load(tag_llvm, tag_address, "decision_tag")
            .map_err(|error| format!("cannot read decision: {error}"))?
            .into_int_value();
        let is_value = self
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                tag,
                tag_llvm.const_int(u64::from(info.value_state), false),
                "is_value",
            )
            .map_err(|error| format!("cannot compare decision: {error}"))?;
        let function = self.current_llvm_function()?;
        let value_block = self.context.append_basic_block(function, "coalesce_value");
        let alternative_block = self
            .context
            .append_basic_block(function, "coalesce_alternative");
        let end_block = self.context.append_basic_block(function, "coalesce_end");
        self.builder
            .build_conditional_branch(is_value, value_block, alternative_block)
            .map_err(|error| format!("cannot branch: {error}"))?;

        self.builder.position_at_end(value_block);
        if result_type == TypeId::VOID {
            self.builder
                .build_store(result_slot, self.context.i8_type().const_zero())
                .map_err(|error| format!("cannot store coalesced value: {error}"))?;
        } else {
            let payload_address = self.address_with_size(
                decision_slot,
                1,
                self.context.i64_type().const_int(payload_offset, false),
            )?;
            let payload = self
                .builder
                .build_load(result_llvm, payload_address, "coalesce_payload")
                .map_err(|error| format!("cannot load payload: {error}"))?;
            self.builder
                .build_store(result_slot, payload)
                .map_err(|error| format!("cannot store coalesced value: {error}"))?;
        }
        self.builder
            .build_unconditional_branch(end_block)
            .map_err(|error| format!("cannot branch: {error}"))?;

        self.builder.position_at_end(alternative_block);
        let mut terminated = false;
        match &coalesce.alternative {
            CoalescingAlternative::Expression(expression) => {
                let alternative = self.gen_expr(*expression)?;
                if result_type != TypeId::VOID {
                    self.builder
                        .build_store(result_slot, alternative)
                        .map_err(|error| format!("cannot store coalesced value: {error}"))?;
                }
            }
            CoalescingAlternative::Throw(exception) => {
                match exception {
                    Some(expression) => {
                        let value = self.gen_expr(*expression)?;
                        let ty = self.type_of(*expression);
                        self.emit_throw_value(value, ty)?;
                    }
                    None => {
                        let payload_address = self.address_with_size(
                            decision_slot,
                            1,
                            self.context.i64_type().const_int(payload_offset, false),
                        )?;
                        let alternative_llvm = self.llvm_type(info.alternative_type)?;
                        let payload = self
                            .builder
                            .build_load(alternative_llvm, payload_address, "alternative")
                            .map_err(|error| format!("cannot load alternative: {error}"))?;
                        self.emit_throw_value(payload, info.alternative_type)?;
                    }
                }
                // The throw branch never reaches the merge block; terminate the
                // dead block so the module stays well formed.
                self.builder
                    .build_unreachable()
                    .map_err(|error| format!("cannot terminate throw: {error}"))?;
                terminated = true;
            }
            CoalescingAlternative::Return(value) => {
                match value {
                    Some(expression) => {
                        let value = self.gen_expr(*expression)?;
                        self.builder
                            .build_return(Some(&value))
                            .map_err(|error| format!("cannot return: {error}"))?;
                    }
                    None => {
                        let decision = self
                            .builder
                            .build_load(decision_llvm, decision_slot, "decision_value")
                            .map_err(|error| format!("cannot load decision: {error}"))?;
                        self.builder
                            .build_return(Some(&decision))
                            .map_err(|error| format!("cannot return: {error}"))?;
                    }
                }
                terminated = true;
            }
        }
        if !terminated {
            self.builder
                .build_unconditional_branch(end_block)
                .map_err(|error| format!("cannot branch: {error}"))?;
        }
        self.builder.position_at_end(end_block);
        if result_type == TypeId::VOID {
            Ok(self.context.i8_type().const_zero().into())
        } else {
            self.builder
                .build_load(result_llvm, result_slot, "coalesce_result")
                .map_err(|error| format!("cannot load coalesced value: {error}"))
        }
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
        // Heap storage holds the instance pointer; value storage (scope-lifetime
        // locals, §5.8) is the instance itself.
        let is_heap = info.storage == Storage::Heap;
        if info.storage != Storage::Heap && info.storage != Storage::Value {
            return Err("dispose requires a class or structure variable".to_string());
        }
        let storage = if is_heap {
            self.builder
                .build_load(pointer_type, slot, "heap_pointer")
                .map_err(|error| format!("cannot load heap pointer: {error}"))?
                .into_pointer_value()
        } else {
            slot
        };
        if let Some(destructor) = self.find_method(info.ty, "Destruct") {
            let function = *self
                .functions
                .get(&destructor)
                .ok_or_else(|| "missing destructor".to_string())?;
            self.builder
                .build_call(function, &[storage.into()], "destruct")
                .map_err(|error| format!("cannot call destructor: {error}"))?;
        }
        if is_heap {
            let free = self
                .free_fn
                .ok_or_else(|| "free is not declared".to_string())?;
            self.builder
                .build_call(free, &[storage.into()], "deallocate")
                .map_err(|error| format!("cannot deallocate: {error}"))?;
        }
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

    /// Generates `node` and applies its implicit conversion operator, if one was
    /// recorded by semantic analysis (§9.7).
    /// Allocates `size` bytes with `malloc`.
    fn malloc_bytes(&mut self, size: u64) -> Result<PointerValue<'ctx>, String> {
        let malloc = self
            .malloc_fn
            .ok_or_else(|| "malloc is not declared".to_string())?;
        let call = self
            .builder
            .build_call(
                malloc,
                &[self.context.i64_type().const_int(size, false).into()],
                "allocate",
            )
            .map_err(|error| format!("cannot allocate memory: {error}"))?;
        match call.try_as_basic_value() {
            ValueKind::Basic(value) => {
                let pointer = value.into_pointer_value();
                if size > 0 {
                    let null = self.context.ptr_type(AddressSpace::default()).const_null();
                    let failed = self
                        .builder
                        .build_int_compare(
                            IntPredicate::EQ,
                            self.builder
                                .build_ptr_to_int(
                                    pointer,
                                    self.context.i64_type(),
                                    "allocation_bits",
                                )
                                .map_err(|error| format!("cannot check allocation: {error}"))?,
                            self.context.i64_type().const_zero(),
                            "allocation_failed",
                        )
                        .map_err(|error| format!("cannot check allocation: {error}"))?;
                    let _ = (null, failed);
                }
                Ok(pointer)
            }
            ValueKind::Instruction(_) => Err("malloc produced no value".to_string()),
        }
    }

    /// Creates the closure object for a function literal (§2.7.9): a static
    /// `{ code, null }` pair when nothing is captured, or a heap-allocated
    /// environment record plus closure when variables are captured.
    fn literal_closure(&mut self, index: u32) -> Result<PointerValue<'ctx>, String> {
        let pointer_type = self.context.ptr_type(AddressSpace::default());
        let captures = self.sema.functions[index as usize].captures.clone();
        let code = *self
            .functions
            .get(&index)
            .ok_or_else(|| "missing literal function".to_string())?;
        let code_pointer = code.as_global_value().as_pointer_value();
        if captures.is_empty() {
            if let Some(global) = self.closure_globals.get(&index) {
                return Ok(global.as_pointer_value());
            }
            let closure_type = pointer_type.array_type(2);
            let global =
                self.module
                    .add_global(closure_type, None, &format!("nsc.literal.{index}"));
            let initializer = pointer_type.const_array(&[code_pointer, pointer_type.const_null()]);
            global.set_initializer(&initializer);
            global.set_constant(true);
            self.closure_globals.insert(index, global);
            return Ok(global.as_pointer_value());
        }
        // Environment record.
        let mut offset: u64 = 0;
        let mut alignment: u64 = 1;
        for capture in &captures {
            let (size, align) = self.sema.size_align(capture.ty);
            let align = align.max(1);
            offset = round_up_u64(offset, align);
            alignment = alignment.max(align);
            offset += size;
        }
        let size = round_up_u64(offset, alignment);
        let environment = self.malloc_bytes(size)?;
        for capture in &captures {
            let value = match capture.source {
                CaptureSource::Local(local) => {
                    let slot = *self
                        .local_slots
                        .get(&local)
                        .ok_or_else(|| "missing captured local slot".to_string())?;
                    let storage = self
                        .local_info(local)
                        .map(|info| info.storage)
                        .unwrap_or(Storage::Value);
                    match storage {
                        Storage::Reference => {
                            let pointer = self
                                .builder
                                .build_load(pointer_type, slot, "capture_reference")
                                .map_err(|error| format!("cannot load reference: {error}"))?
                                .into_pointer_value();
                            let llvm_type = self.llvm_type(capture.ty)?;
                            self.builder
                                .build_load(llvm_type, pointer, "captured_value")
                                .map_err(|error| format!("cannot load captured value: {error}"))?
                        }
                        _ => {
                            let llvm_type = self.llvm_type(capture.ty)?;
                            self.builder
                                .build_load(llvm_type, slot, "captured_value")
                                .map_err(|error| format!("cannot load captured value: {error}"))?
                        }
                    }
                }
                CaptureSource::This => {
                    let slot = self
                        .current_this_slot
                        .ok_or_else(|| "captured 'this' outside a method".to_string())?;
                    self.builder
                        .build_load(pointer_type, slot, "captured_this")
                        .map_err(|error| format!("cannot load this: {error}"))?
                }
            };
            let address = self.address_with_size(
                environment,
                1,
                self.context.i64_type().const_int(capture.offset, false),
            )?;
            self.builder
                .build_store(address, value)
                .map_err(|error| format!("cannot store captured value: {error}"))?;
        }
        let closure = self.malloc_bytes(16)?;
        let code_slot = self.address_with_size(closure, 1, self.context.i64_type().const_zero())?;
        let environment_slot =
            self.address_with_size(closure, 1, self.context.i64_type().const_int(8, false))?;
        self.builder
            .build_store(code_slot, code_pointer)
            .map_err(|error| format!("cannot store closure code: {error}"))?;
        self.builder
            .build_store(environment_slot, environment)
            .map_err(|error| format!("cannot store closure environment: {error}"))?;
        Ok(closure)
    }

    /// Returns a pointer to the closure object for `index`: a static
    /// `{ code, environment }` pair whose code is a trampoline that prepends the
    /// (unused) environment parameter (§2.7.9).
    fn function_value_closure(&mut self, index: u32) -> Result<PointerValue<'ctx>, String> {
        let key = index;
        if let Some(global) = self.closure_globals.get(&key) {
            return Ok(global.as_pointer_value());
        }
        let pointer_type = self.context.ptr_type(AddressSpace::default());
        let info = self.sema.functions[index as usize].parameters.clone();
        let trampoline_name = format!("nsc.valuefn.{index}");
        let trampoline = match self.module.get_function(&trampoline_name) {
            Some(function) => function,
            None => {
                let mut parameter_types: Vec<inkwell::types::BasicMetadataTypeEnum<'ctx>> =
                    Vec::with_capacity(info.len() + 1);
                parameter_types.push(pointer_type.into());
                for parameter in &info {
                    parameter_types.push(self.llvm_parameter_type(parameter)?.into());
                }
                let return_type = self.sema.functions[index as usize].return_type;
                let function_type = match return_type {
                    Some(ty) => self.llvm_type(ty)?.fn_type(&parameter_types, false),
                    None => self.context.void_type().fn_type(&parameter_types, false),
                };
                let function = self
                    .module
                    .add_function(&trampoline_name, function_type, None);
                let entry = self.context.append_basic_block(function, "entry");
                let previous_block = self.builder.get_insert_block();
                let previous_function = self.current_function;
                self.builder.position_at_end(entry);
                let target = *self
                    .functions
                    .get(&index)
                    .ok_or_else(|| "missing function for closure".to_string())?;
                let mut arguments: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> =
                    Vec::with_capacity(info.len());
                for position in 0..info.len() {
                    let value = function
                        .get_nth_param((position + 1) as u32)
                        .ok_or_else(|| "missing trampoline parameter".to_string())?;
                    arguments.push(value.into());
                }
                let call_site = self
                    .builder
                    .build_call(target, &arguments, "closure_call")
                    .map_err(|error| format!("cannot build closure trampoline: {error}"))?;
                match return_type {
                    Some(_) => {
                        let value = match call_site.try_as_basic_value() {
                            ValueKind::Basic(value) => value,
                            ValueKind::Instruction(_) => {
                                return Err("closure trampoline lost its value".to_string())
                            }
                        };
                        self.builder
                            .build_return(Some(&value))
                            .map_err(|error| format!("cannot return: {error}"))?;
                    }
                    None => {
                        self.builder
                            .build_return(None)
                            .map_err(|error| format!("cannot return: {error}"))?;
                    }
                }
                self.current_function = previous_function;
                if let Some(block) = previous_block {
                    self.builder.position_at_end(block);
                }
                function
            }
        };
        let closure_type = pointer_type.array_type(2);
        let global = self
            .module
            .add_global(closure_type, None, &format!("nsc.closure.{index}"));
        let initializer = pointer_type.const_array(&[
            trampoline.as_global_value().as_pointer_value(),
            pointer_type.const_null(),
        ]);
        global.set_initializer(&initializer);
        global.set_constant(true);
        self.closure_globals.insert(key, global);
        Ok(global.as_pointer_value())
    }

    /// Loads the code and environment pointers of a closure value.
    fn load_closure(
        &mut self,
        closure: PointerValue<'ctx>,
    ) -> Result<(PointerValue<'ctx>, PointerValue<'ctx>), String> {
        let pointer_type = self.context.ptr_type(AddressSpace::default());
        let code_slot = self.address_with_size(closure, 1, self.context.i64_type().const_zero())?;
        let environment_slot =
            self.address_with_size(closure, 1, self.context.i64_type().const_int(8, false))?;
        let code = self
            .builder
            .build_load(pointer_type, code_slot, "closure_code")
            .map_err(|error| format!("cannot load closure code: {error}"))?
            .into_pointer_value();
        let environment = self
            .builder
            .build_load(pointer_type, environment_slot, "closure_env")
            .map_err(|error| format!("cannot load closure environment: {error}"))?
            .into_pointer_value();
        Ok((code, environment))
    }

    fn gen_coerced_expr(&mut self, node: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let value = self.gen_expr(node)?;
        match self.sema.implicit_conversions.get(&node).copied() {
            Some(method) => self.gen_operator_call(method, vec![value]),
            None => Ok(value),
        }
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
            let value = self.gen_coerced_expr(*initializer)?;
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
        if let Some(Resolution::Indexer { setter, .. }) =
            self.sema.resolutions[statement.target as usize].clone()
        {
            let setter = setter.ok_or_else(|| "indexer is read-only".to_string())?;
            let NodeKind::Index(index) = self.ast.node(statement.target).kind.clone() else {
                return Err("indexer write requires an index expression".to_string());
            };
            let receiver = self.gen_instance_pointer(index.base)?;
            let index_value = self.gen_expr(index.index)?;
            let value = self.gen_expr(statement.value)?;
            let function = *self
                .functions
                .get(&setter)
                .ok_or_else(|| "missing indexer setter".to_string())?;
            let arguments: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> =
                vec![receiver.into(), index_value.into(), value.into()];
            self.builder
                .build_call(function, &arguments, "indexer_set")
                .map_err(|error| format!("cannot call indexer: {error}"))?;
            return Ok(());
        }
        if let Some(Resolution::Accessor { setter, .. }) =
            self.sema.resolutions[statement.target as usize].clone()
        {
            let setter = setter.ok_or_else(|| "accessor is read-only".to_string())?;
            let NodeKind::MemberAccess(access) = self.ast.node(statement.target).kind.clone()
            else {
                return Err("accessor write requires a member access".to_string());
            };
            let receiver = self.gen_instance_pointer(access.base)?;
            let receiver_type = self.type_of(access.base);
            let value = self.gen_expr(statement.value)?;
            self.gen_method_dispatch(setter, receiver_type, receiver, vec![value.into()])?;
            return Ok(());
        }
        let pointer = self
            .gen_lvalue(statement.target)?
            .ok_or_else(|| "assignment target is not addressable".to_string())?;
        let value = self.gen_coerced_expr(statement.value)?;
        self.builder
            .build_store(pointer, value)
            .map_err(|error| format!("cannot assign: {error}"))?;
        Ok(())
    }

    fn gen_compound_set_statement(
        &mut self,
        statement: &crate::ast::CompoundSetStmt,
    ) -> Result<(), String> {
        if let Some(Resolution::Accessor { getter, setter }) =
            self.sema.resolutions[statement.target as usize].clone()
        {
            let getter = getter.ok_or_else(|| "accessor is write-only".to_string())?;
            let setter = setter.ok_or_else(|| "accessor is read-only".to_string())?;
            let NodeKind::MemberAccess(access) = self.ast.node(statement.target).kind.clone()
            else {
                return Err("accessor write requires a member access".to_string());
            };
            let receiver = self.gen_instance_pointer(access.base)?;
            let receiver_type = self.type_of(access.base);
            let current_call =
                self.gen_method_dispatch(getter, receiver_type, receiver, Vec::new())?;
            let current = match current_call.try_as_basic_value() {
                ValueKind::Basic(value) => value,
                ValueKind::Instruction(_) => {
                    return Err("accessor getter produced no value".to_string());
                }
            };
            let target_type = self.type_of(statement.target);
            let value = self.gen_expr(statement.value)?;
            let result =
                self.build_arithmetic(statement.op, current, value, target_type, statement.target)?;
            self.gen_method_dispatch(setter, receiver_type, receiver, vec![result.into()])?;
            return Ok(());
        }
        let pointer = self
            .gen_lvalue(statement.target)?
            .ok_or_else(|| "assignment target is not addressable".to_string())?;
        let target_type = self.type_of(statement.target);
        let llvm_type = self.llvm_type(target_type)?;
        // The RHS is evaluated before the current value is read (§5.7).
        let value = self.gen_coerced_expr(statement.value)?;
        let current = self
            .builder
            .build_load(llvm_type, pointer, "compound_current")
            .map_err(|error| format!("cannot load target: {error}"))?;
        // Pointer compound arithmetic (§3.6.2): `p += i` / `p -= i`.
        let result = if matches!(self.sema.types.get(target_type), Type::Pointer { .. })
            && matches!(statement.op, BinaryOp::Add | BinaryOp::Subtract)
        {
            let base = current.into_pointer_value();
            let offset = self.pointer_offset(value.into_int_value())?;
            let offset = if statement.op == BinaryOp::Subtract {
                self.builder
                    .build_int_neg(offset, "negate_offset")
                    .map_err(|error| format!("cannot negate offset: {error}"))?
            } else {
                offset
            };
            let unit = match self.sema.types.get(target_type) {
                Type::Pointer {
                    target: Some(target),
                } => self.sema.size_align(*target).0.max(1),
                _ => 1,
            };
            self.address_with_size(base, unit, offset)?.into()
        } else {
            match statement.op {
                BinaryOp::Add
                | BinaryOp::Subtract
                | BinaryOp::Multiply
                | BinaryOp::Divide
                | BinaryOp::Remainder
                | BinaryOp::BitwiseAnd
                | BinaryOp::BitwiseOr
                | BinaryOp::BitwiseXor
                | BinaryOp::ShiftLeft
                | BinaryOp::ShiftRight => self.build_arithmetic(
                    statement.op,
                    current,
                    value,
                    target_type,
                    statement.target,
                )?,
                _ => return Err("unsupported compound assignment operator".to_string()),
            }
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
                if statement.by_ref {
                    // The binding aliases the element, so assignments inside the
                    // body write the array (§5.9.5).
                    self.builder
                        .build_store(binding_slot, element_pointer)
                        .map_err(|error| format!("cannot store element reference: {error}"))?;
                } else {
                    let value = self
                        .builder
                        .build_load(element_llvm, element_pointer, "element")
                        .map_err(|error| format!("cannot load element: {error}"))?;
                    self.builder
                        .build_store(binding_slot, value)
                        .map_err(|error| format!("cannot store element: {error}"))?;
                }
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
                let value = self.gen_coerced_expr(value)?;
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

    fn gen_match_statement(&mut self, node: NodeId, statement: &MatchStmt) -> Result<(), String> {
        let function = self.current_llvm_function()?;
        let end_block = self.context.append_basic_block(function, "match_end");
        let value_type = self.type_of(statement.value);
        let value = self.gen_expr(statement.value)?;
        let llvm_type = self.llvm_type(value_type)?;
        let value_slot = self
            .builder
            .build_alloca(llvm_type, "match_value")
            .map_err(|error| format!("cannot allocate match value: {error}"))?;
        self.builder
            .build_store(value_slot, value)
            .map_err(|error| format!("cannot store match value: {error}"))?;

        for (case_index, case) in statement.cases.iter().enumerate() {
            let case_block = self.context.append_basic_block(function, "match_case");
            let next_block = self.context.append_basic_block(function, "match_next");
            let pattern_info = self
                .sema
                .match_patterns
                .get(&(node, case_index as u32))
                .cloned();
            let (condition, binding): (IntValue<'ctx>, Option<MatchPatternInfo>) =
                match pattern_info.clone() {
                    Some(MatchPatternInfo::IsVariant { type_id, state, .. }) => {
                        let Type::Named(index) = self.sema.types.get(type_id).clone() else {
                            return Err("variant pattern requires a variant".to_string());
                        };
                        let tag_type = self.sema.type_defs[index as usize]
                            .tag_type
                            .ok_or_else(|| "variant has no tag type".to_string())?;
                        let tag_llvm = self.llvm_type(tag_type)?.into_int_type();
                        let tag_address = self.address_with_size(
                            value_slot,
                            1,
                            self.context.i64_type().const_zero(),
                        )?;
                        let tag = self
                            .builder
                            .build_load(tag_llvm, tag_address, "variant_tag")
                            .map_err(|error| format!("cannot load variant tag: {error}"))?
                            .into_int_value();
                        let condition = self
                            .builder
                            .build_int_compare(
                                IntPredicate::EQ,
                                tag,
                                tag_llvm.const_int(u64::from(state), false),
                                "is_state",
                            )
                            .map_err(|error| format!("cannot compare state: {error}"))?;
                        (condition, pattern_info)
                    }
                    Some(MatchPatternInfo::Operator { method })
                    | Some(MatchPatternInfo::DerivedNotEqual { method }) => {
                        let expression =
                            match &case.pattern {
                                Some(Pattern::Equal(expression)) => *expression,
                                Some(Pattern::Comparison { value, .. }) => *value,
                                _ => return Err(
                                    "operator pattern requires an equality or comparison pattern"
                                        .to_string(),
                                ),
                            };
                        let value = self
                            .builder
                            .build_load(llvm_type, value_slot, "match_load")
                            .map_err(|error| format!("cannot load match value: {error}"))?;
                        let pattern = self.gen_expr(expression)?;
                        let result = self.gen_operator_call(method, vec![value, pattern])?;
                        let negated =
                            matches!(pattern_info, Some(MatchPatternInfo::DerivedNotEqual { .. }));
                        let result = if negated {
                            self.negate_boolean(result)?
                        } else {
                            result
                        };
                        (result.into_int_value(), None)
                    }
                    Some(MatchPatternInfo::DerivedComparison { ordering, equality }) => {
                        let expression = match &case.pattern {
                            Some(Pattern::Comparison { value, .. }) => *value,
                            _ => {
                                return Err(
                                    "derived comparison pattern requires a comparison pattern"
                                        .to_string(),
                                )
                            }
                        };
                        let value = self
                            .builder
                            .build_load(llvm_type, value_slot, "match_load")
                            .map_err(|error| format!("cannot load match value: {error}"))?;
                        let pattern = self.gen_expr(expression)?;
                        let ordering_result =
                            self.gen_operator_call(ordering, vec![value, pattern])?;
                        let equality_result =
                            self.gen_operator_call(equality, vec![value, pattern])?;
                        let combined = self.or_booleans(ordering_result, equality_result)?;
                        (combined.into_int_value(), None)
                    }
                    Some(MatchPatternInfo::IsClass { type_id, .. }) => {
                        let pointer_type = self.context.ptr_type(AddressSpace::default());
                        let instance = self
                            .builder
                            .build_load(pointer_type, value_slot, "class_value")
                            .map_err(|error| format!("cannot load class value: {error}"))?
                            .into_pointer_value();
                        let condition = self.emit_type_check(instance, type_id)?;
                        (condition, pattern_info)
                    }
                    None => {
                        let value = self
                            .builder
                            .build_load(llvm_type, value_slot, "match_load")
                            .map_err(|error| format!("cannot load match value: {error}"))?;
                        let condition: IntValue<'ctx> = match &case.pattern {
                            None | Some(Pattern::Wildcard) => {
                                self.context.bool_type().const_int(1, false)
                            }
                            Some(Pattern::Equal(expression)) => {
                                let pattern = self.gen_expr(*expression)?;
                                self.gen_pattern_comparison(
                                    BinaryOp::Equal,
                                    value,
                                    pattern,
                                    value_type,
                                    *expression,
                                )?
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
                                    crate::lexer::TokenKind::GreaterEquals => {
                                        BinaryOp::GreaterEqual
                                    }
                                    crate::lexer::TokenKind::LessEquals => BinaryOp::LessEqual,
                                    _ => {
                                        return Err("unsupported match comparison".to_string());
                                    }
                                };
                                self.gen_pattern_comparison(
                                    op,
                                    value,
                                    pattern,
                                    value_type,
                                    *pattern_value,
                                )?
                            }
                            Some(Pattern::Is { .. }) => {
                                return Err("unresolved is pattern".to_string());
                            }
                        };
                        (condition, None)
                    }
                };

            let condition = self.as_boolean_i1(condition);
            let bind_block = self.context.append_basic_block(function, "match_bind");
            self.builder
                .build_conditional_branch(condition, bind_block, next_block)
                .map_err(|error| format!("cannot branch: {error}"))?;

            // Pattern bindings are visible to the guard (§5.9.6).
            self.builder.position_at_end(bind_block);
            if let Some(binding) = binding {
                self.emit_pattern_binding(&binding, value_slot)?;
            }
            let mut branch_condition = None;
            if let Some(guard) = case.guard {
                let guard_value = self.gen_expr(guard)?.into_int_value();
                branch_condition = Some(self.as_boolean_i1(guard_value));
            }
            match branch_condition {
                Some(condition) => self
                    .builder
                    .build_conditional_branch(condition, case_block, next_block)
                    .map_err(|error| format!("cannot branch: {error}"))?,
                None => self
                    .builder
                    .build_unconditional_branch(case_block)
                    .map_err(|error| format!("cannot branch: {error}"))?,
            };

            self.builder.position_at_end(case_block);
            // A `match` is a logical control flow statement (§5.9.1): `break`
            // leaves the match and `continue` skips the rest of the case and
            // retries the following cases (§5.9.6).
            self.loops.push(LoopInfo {
                label: statement.label.clone(),
                break_block: end_block,
                continue_block: next_block,
            });
            self.gen_block_contents(case.body)?;
            self.loops.pop();
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

    fn emit_pattern_binding(
        &mut self,
        binding: &MatchPatternInfo,
        value_slot: PointerValue<'ctx>,
    ) -> Result<(), String> {
        match binding {
            MatchPatternInfo::Operator { .. }
            | MatchPatternInfo::DerivedNotEqual { .. }
            | MatchPatternInfo::DerivedComparison { .. } => Ok(()),
            MatchPatternInfo::IsVariant {
                type_id,
                state,
                binding,
            } => {
                let Some(binding) = binding else {
                    return Ok(());
                };
                let slot = *self
                    .local_slots
                    .get(binding)
                    .ok_or_else(|| "missing binding slot".to_string())?;
                let Type::Named(index) = self.sema.types.get(*type_id).clone() else {
                    return Err("variant pattern requires a variant".to_string());
                };
                let (payload_type, payload_offset) = {
                    let state = self.sema.type_defs[index as usize]
                        .variant_states
                        .get(*state as usize)
                        .ok_or_else(|| "missing variant state".to_string())?;
                    (
                        state.payload,
                        self.sema.type_defs[index as usize].payload_offset,
                    )
                };
                let Some(payload_type) = payload_type else {
                    return Ok(());
                };
                let payload_llvm = self.llvm_type(payload_type)?;
                let payload_address = self.address_with_size(
                    value_slot,
                    1,
                    self.context.i64_type().const_int(payload_offset, false),
                )?;
                let payload = self
                    .builder
                    .build_load(payload_llvm, payload_address, "payload")
                    .map_err(|error| format!("cannot load payload: {error}"))?;
                self.builder
                    .build_store(slot, payload)
                    .map_err(|error| format!("cannot bind payload: {error}"))?;
                Ok(())
            }
            MatchPatternInfo::IsClass { binding, .. } => {
                let Some(binding) = binding else {
                    return Ok(());
                };
                let slot = *self
                    .local_slots
                    .get(binding)
                    .ok_or_else(|| "missing binding slot".to_string())?;
                let pointer_type = self.context.ptr_type(AddressSpace::default());
                let instance = self
                    .builder
                    .build_load(pointer_type, value_slot, "class_value")
                    .map_err(|error| format!("cannot load class value: {error}"))?;
                self.builder
                    .build_store(slot, instance)
                    .map_err(|error| format!("cannot bind class value: {error}"))?;
                Ok(())
            }
        }
    }

    /// Emits an RTTI membership test for a class value, returning an i1.
    fn gen_interface_call(
        &mut self,
        receiver: PointerValue<'ctx>,
        interface_index: u32,
        method: u32,
        arguments: &[inkwell::values::BasicMetadataValueEnum<'ctx>],
    ) -> Result<CallSiteValue<'ctx>, String> {
        let method_list = self.sema.type_defs[interface_index as usize]
            .method_list
            .clone();
        let slot = method_list
            .iter()
            .position(|entry| *entry == method)
            .ok_or_else(|| "interface method is not part of the interface".to_string())?;
        let pointer_type = self.context.ptr_type(AddressSpace::default());
        let typeinfo = self.load_pointer_at(receiver, 8)?.into_pointer_value();
        let table = self.load_pointer_at(typeinfo, 16)?.into_pointer_value();
        let target = self
            .typeinfo_globals
            .get(&interface_index)
            .ok_or_else(|| "missing interface type information".to_string())?
            .as_pointer_value();
        let function = self.current_llvm_function()?;
        let loop_block = self.context.append_basic_block(function, "iface_loop");
        let compare_block = self.context.append_basic_block(function, "iface_compare");
        let advance_block = self.context.append_basic_block(function, "iface_advance");
        let found_block = self.context.append_basic_block(function, "iface_found");
        let fail_block = self.context.append_basic_block(function, "iface_fail");
        let end_block = self.context.append_basic_block(function, "iface_end");
        let current_slot = self
            .builder
            .build_alloca(pointer_type, "iface_table")
            .map_err(|error| format!("cannot allocate interface state: {error}"))?;
        self.builder
            .build_store(current_slot, table)
            .map_err(|error| format!("cannot store interface state: {error}"))?;
        let callee_slot = self
            .builder
            .build_alloca(pointer_type, "iface_callee")
            .map_err(|error| format!("cannot allocate interface callee: {error}"))?;
        self.builder
            .build_store(callee_slot, pointer_type.const_zero())
            .map_err(|error| format!("cannot store interface callee: {error}"))?;
        self.builder
            .build_unconditional_branch(loop_block)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(loop_block);
        let current = self
            .builder
            .build_load(pointer_type, current_slot, "table")
            .map_err(|error| format!("cannot load interface table: {error}"))?
            .into_pointer_value();
        let entry = self.load_pointer_at(current, 0)?.into_pointer_value();
        let is_null = self
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                entry,
                pointer_type.const_zero(),
                "iface_null",
            )
            .map_err(|error| format!("cannot compare interface entry: {error}"))?;
        self.builder
            .build_conditional_branch(is_null, fail_block, compare_block)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(compare_block);
        let matches = self
            .builder
            .build_int_compare(IntPredicate::EQ, entry, target, "iface_match")
            .map_err(|error| format!("cannot compare interface type: {error}"))?;
        self.builder
            .build_conditional_branch(matches, found_block, advance_block)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(advance_block);
        let next =
            self.address_with_size(current, 1, self.context.i64_type().const_int(16, false))?;
        self.builder
            .build_store(current_slot, next)
            .map_err(|error| format!("cannot store interface state: {error}"))?;
        self.builder
            .build_unconditional_branch(loop_block)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(found_block);
        let vtable = self.load_pointer_at(current, 8)?.into_pointer_value();
        let slot_address = self.address_with_size(
            vtable,
            8,
            self.context.i64_type().const_int(slot as u64, false),
        )?;
        let callee = self
            .builder
            .build_load(pointer_type, slot_address, "iface_callee_load")
            .map_err(|error| format!("cannot load interface callee: {error}"))?;
        self.builder
            .build_store(callee_slot, callee)
            .map_err(|error| format!("cannot store interface callee: {error}"))?;
        self.builder
            .build_unconditional_branch(end_block)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(fail_block);
        let abort = self.module.get_function("abort").unwrap_or_else(|| {
            let signature = self.context.void_type().fn_type(&[], false);
            self.module.add_function("abort", signature, None)
        });
        self.builder
            .build_call(abort, &[], "")
            .map_err(|error| format!("cannot call abort: {error}"))?;
        self.builder
            .build_unreachable()
            .map_err(|error| format!("cannot terminate: {error}"))?;
        self.builder.position_at_end(end_block);
        let callee = self
            .builder
            .build_load(pointer_type, callee_slot, "iface_callee")
            .map_err(|error| format!("cannot load interface callee: {error}"))?
            .into_pointer_value();
        let signature = self.function_type(method)?;
        self.builder
            .build_indirect_call(signature, callee, arguments, "interface_call")
            .map_err(|error| format!("cannot dispatch interface call: {error}"))
    }

    /// Emits an RTTI search through a class's interface tables.
    fn emit_interface_check(
        &mut self,
        instance: PointerValue<'ctx>,
        interface_index: u32,
    ) -> Result<IntValue<'ctx>, String> {
        let typeinfo = self.load_pointer_at(instance, 8)?.into_pointer_value();
        self.emit_interface_from_typeinfo(typeinfo, interface_index)
    }

    /// Emits an interface-table search starting from a typeinfo pointer.
    fn emit_interface_from_typeinfo(
        &mut self,
        typeinfo: PointerValue<'ctx>,
        interface_index: u32,
    ) -> Result<IntValue<'ctx>, String> {
        let pointer_type = self.context.ptr_type(AddressSpace::default());
        let target = self
            .typeinfo_globals
            .get(&interface_index)
            .ok_or_else(|| "missing interface type information".to_string())?
            .as_pointer_value();
        let result_slot = self
            .builder
            .build_alloca(self.context.bool_type(), "iface_result")
            .map_err(|error| format!("cannot allocate result: {error}"))?;
        self.builder
            .build_store(result_slot, self.context.bool_type().const_zero())
            .map_err(|error| format!("cannot store result: {error}"))?;
        let typeinfo_slot = self
            .builder
            .build_alloca(pointer_type, "iface_typeinfo")
            .map_err(|error| format!("cannot allocate interface state: {error}"))?;
        self.builder
            .build_store(typeinfo_slot, typeinfo)
            .map_err(|error| format!("cannot store interface state: {error}"))?;
        let function = self.current_llvm_function()?;
        let outer = self.context.append_basic_block(function, "iface_outer");
        let outer_next = self.context.append_basic_block(function, "iface_next_type");
        let table_loop = self
            .context
            .append_basic_block(function, "iface_table_loop");
        let table_advance = self
            .context
            .append_basic_block(function, "iface_table_next");
        let table_next_type = self
            .context
            .append_basic_block(function, "iface_next_table");
        let found = self.context.append_basic_block(function, "iface_found");
        let end = self.context.append_basic_block(function, "iface_end");
        let table_slot = self
            .builder
            .build_alloca(pointer_type, "iface_table_ptr")
            .map_err(|error| format!("cannot allocate interface state: {error}"))?;
        let current_type_slot = self
            .builder
            .build_alloca(pointer_type, "iface_current_type")
            .map_err(|error| format!("cannot allocate interface state: {error}"))?;
        self.builder
            .build_unconditional_branch(outer)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(outer);
        let current_type = self
            .builder
            .build_load(pointer_type, typeinfo_slot, "current_type")
            .map_err(|error| format!("cannot load typeinfo: {error}"))?
            .into_pointer_value();
        self.builder
            .build_store(current_type_slot, current_type)
            .map_err(|error| format!("cannot store typeinfo: {error}"))?;
        let table = self.load_pointer_at(current_type, 16)?;
        self.builder
            .build_store(table_slot, table)
            .map_err(|error| format!("cannot store interface table: {error}"))?;
        self.builder
            .build_unconditional_branch(table_loop)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(table_loop);
        let table = self
            .builder
            .build_load(pointer_type, table_slot, "table")
            .map_err(|error| format!("cannot load interface table: {error}"))?
            .into_pointer_value();
        let table_is_null = self
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                table,
                pointer_type.const_zero(),
                "iface_table_null",
            )
            .map_err(|error| format!("cannot compare interface table: {error}"))?;
        let table_present = self
            .context
            .append_basic_block(function, "iface_table_present");
        self.builder
            .build_conditional_branch(table_is_null, table_next_type, table_present)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(table_present);
        let entry = self.load_pointer_at(table, 0)?.into_pointer_value();
        let is_null = self
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                entry,
                pointer_type.const_zero(),
                "iface_null",
            )
            .map_err(|error| format!("cannot compare interface entry: {error}"))?;
        self.builder
            .build_conditional_branch(is_null, table_next_type, table_advance)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(table_advance);
        let matches = self
            .builder
            .build_int_compare(IntPredicate::EQ, entry, target, "iface_match")
            .map_err(|error| format!("cannot compare interface type: {error}"))?;
        let table_advance_next = self.context.append_basic_block(function, "iface_advance");
        self.builder
            .build_conditional_branch(matches, found, table_advance_next)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(table_advance_next);
        let next =
            self.address_with_size(table, 1, self.context.i64_type().const_int(16, false))?;
        self.builder
            .build_store(table_slot, next)
            .map_err(|error| format!("cannot store interface table: {error}"))?;
        self.builder
            .build_unconditional_branch(table_loop)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(table_next_type);
        let current_type = self
            .builder
            .build_load(pointer_type, current_type_slot, "current_type")
            .map_err(|error| format!("cannot load typeinfo: {error}"))?;
        let base_typeinfo = self.load_pointer_at(current_type.into_pointer_value(), 8)?;
        let is_null = self
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                base_typeinfo.into_pointer_value(),
                pointer_type.const_zero(),
                "base_null",
            )
            .map_err(|error| format!("cannot compare base typeinfo: {error}"))?;
        self.builder
            .build_conditional_branch(is_null, end, outer_next)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(outer_next);
        self.builder
            .build_store(typeinfo_slot, base_typeinfo)
            .map_err(|error| format!("cannot store typeinfo: {error}"))?;
        self.builder
            .build_unconditional_branch(outer)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(found);
        self.builder
            .build_store(result_slot, self.context.bool_type().const_int(1, false))
            .map_err(|error| format!("cannot store result: {error}"))?;
        self.builder
            .build_unconditional_branch(end)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(end);
        Ok(self
            .builder
            .build_load(self.context.bool_type(), result_slot, "iface_result")
            .map_err(|error| format!("cannot load result: {error}"))?
            .into_int_value())
    }

    fn emit_type_check(
        &mut self,
        instance: PointerValue<'ctx>,
        target_type: TypeId,
    ) -> Result<IntValue<'ctx>, String> {
        let Type::Named(target_index) = self.sema.types.get(target_type).clone() else {
            return Err("is target must be a class".to_string());
        };
        if self.sema.type_defs[target_index as usize].kind == TypeDefKind::Interface {
            return self.emit_interface_check(instance, target_index);
        }
        let target_global = *self
            .typeinfo_globals
            .get(&target_index)
            .ok_or_else(|| "missing target type information".to_string())?;
        let pointer_type = self.context.ptr_type(AddressSpace::default());
        let current_typeinfo = self.load_pointer_at(instance, 8)?.into_pointer_value();
        let current_slot = self
            .builder
            .build_alloca(pointer_type, "is_current")
            .map_err(|error| format!("cannot allocate is state: {error}"))?;
        self.builder
            .build_store(current_slot, current_typeinfo)
            .map_err(|error| format!("cannot store is state: {error}"))?;
        let result_slot = self
            .builder
            .build_alloca(self.context.bool_type(), "is_result")
            .map_err(|error| format!("cannot allocate is result: {error}"))?;
        self.builder
            .build_store(result_slot, self.context.bool_type().const_zero())
            .map_err(|error| format!("cannot store is result: {error}"))?;
        let function = self.current_llvm_function()?;
        let loop_block = self.context.append_basic_block(function, "is_loop");
        let advance_block = self.context.append_basic_block(function, "is_base");
        let true_block = self.context.append_basic_block(function, "is_true");
        let end_block = self.context.append_basic_block(function, "is_end");
        self.builder
            .build_unconditional_branch(loop_block)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(loop_block);
        let current = self
            .builder
            .build_load(pointer_type, current_slot, "current")
            .map_err(|error| format!("cannot load is state: {error}"))?
            .into_pointer_value();
        let matches = self
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                current,
                target_global.as_pointer_value(),
                "is_match",
            )
            .map_err(|error| format!("cannot compare types: {error}"))?;
        self.builder
            .build_conditional_branch(matches, true_block, advance_block)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(advance_block);
        let is_null = self
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                current,
                pointer_type.const_zero(),
                "is_null",
            )
            .map_err(|error| format!("cannot compare types: {error}"))?;
        let next_block = self.context.append_basic_block(function, "is_next");
        self.builder
            .build_conditional_branch(is_null, end_block, next_block)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(next_block);
        let base_typeinfo = self.load_pointer_at(current, 8)?;
        self.builder
            .build_store(current_slot, base_typeinfo)
            .map_err(|error| format!("cannot store is state: {error}"))?;
        self.builder
            .build_unconditional_branch(loop_block)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(true_block);
        self.builder
            .build_store(result_slot, self.context.bool_type().const_int(1, false))
            .map_err(|error| format!("cannot store is result: {error}"))?;
        self.builder
            .build_unconditional_branch(end_block)
            .map_err(|error| format!("cannot branch: {error}"))?;
        self.builder.position_at_end(end_block);
        Ok(self
            .builder
            .build_load(self.context.bool_type(), result_slot, "is_result")
            .map_err(|error| format!("cannot load is result: {error}"))?
            .into_int_value())
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
            NodeKind::This | NodeKind::Base => self.gen_this_value(),
            NodeKind::Index(index) => self.gen_index_expr(node, &index),
            NodeKind::Call(call) => self.gen_call_expr(node, &call),
            NodeKind::Cast(cast) => self.gen_cast_expr(node, &cast),
            NodeKind::Unary(unary) => self.gen_unary_expr(node, &unary),
            NodeKind::Binary(binary) => self.gen_binary_expr(node, &binary),
            NodeKind::Is(is_expression) => self.gen_is_expr(node, &is_expression),
            NodeKind::Conditional(conditional) => self.gen_conditional_expr(node, &conditional),
            NodeKind::Coalesce(coalesce) => self.gen_coalesce_expr(node, &coalesce),
            NodeKind::FunctionLiteral(_) => {
                let index = *self
                    .sema
                    .function_literals
                    .get(&node)
                    .ok_or_else(|| "missing function literal".to_string())?;
                Ok(self.literal_closure(index)?.into())
            }
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
                if let Storage::Capture(capture_index) = info.storage {
                    let capture = self.sema.functions[self.current_function as usize]
                        .captures
                        .get(capture_index as usize)
                        .cloned()
                        .ok_or_else(|| "missing capture information".to_string())?;
                    let env_slot = self
                        .current_env_slot
                        .ok_or_else(|| "missing environment".to_string())?;
                    let pointer_type = self.context.ptr_type(AddressSpace::default());
                    let environment = self
                        .builder
                        .build_load(pointer_type, env_slot, "environment")
                        .map_err(|error| format!("cannot load environment: {error}"))?
                        .into_pointer_value();
                    let address = self.address_with_size(
                        environment,
                        1,
                        self.context.i64_type().const_int(capture.offset, false),
                    )?;
                    let llvm_type = self.llvm_type(capture.ty)?;
                    return self
                        .builder
                        .build_load(llvm_type, address, "captured")
                        .map_err(|error| format!("cannot load captured value: {error}"));
                }
                let slot = *self
                    .local_slots
                    .get(&local)
                    .ok_or_else(|| "missing local slot".to_string())?;
                if info.storage == Storage::Reference {
                    let pointer_type = self.context.ptr_type(AddressSpace::default());
                    let pointer = self
                        .builder
                        .build_load(pointer_type, slot, "reference")
                        .map_err(|error| format!("cannot load reference: {error}"))?;
                    let llvm_type = self.llvm_type(ty)?;
                    return self
                        .builder
                        .build_load(llvm_type, pointer.into_pointer_value(), "referenced")
                        .map_err(|error| format!("cannot load referenced value: {error}"));
                }
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
                let Type::Named(type_index) = self.sema.types.get(type_id) else {
                    return Err("enumeration member requires an enumeration type".to_string());
                };
                let value =
                    self.sema.type_defs[*type_index as usize].enum_members[index as usize].1;
                let llvm_type = self.llvm_type(underlying)?;
                Ok(llvm_type
                    .into_int_type()
                    .const_int(value as u64, true)
                    .into())
            }
            Some(Resolution::Field { .. }) => self.gen_field_load(node),
            Some(Resolution::Operator { .. })
            | Some(Resolution::DerivedNotEqual { .. })
            | Some(Resolution::DerivedComparison { .. })
            | Some(Resolution::Indexer { .. })
            | Some(Resolution::Conversion { .. }) => {
                Err("operator resolution is lowered by its enclosing expression".to_string())
            }
            Some(Resolution::This) | Some(Resolution::Base) => self.gen_this_value(),
            Some(Resolution::Construct { .. })
            | Some(Resolution::ConstructVariant { .. })
            | Some(Resolution::ConstructUnion { .. }) => self.gen_construction(node),
            Some(Resolution::Accessor { getter, .. }) => {
                let getter = getter.ok_or_else(|| "accessor is write-only".to_string())?;
                let NodeKind::MemberAccess(access) = self.ast.node(node).kind.clone() else {
                    return Err("accessor access requires a member access".to_string());
                };
                let receiver = self.gen_instance_pointer(access.base)?;
                let receiver_type = self.type_of(access.base);
                let call = self.gen_method_dispatch(getter, receiver_type, receiver, Vec::new())?;
                match call.try_as_basic_value() {
                    ValueKind::Basic(value) => Ok(value),
                    ValueKind::Instruction(_) => {
                        Err("accessor getter produced no value".to_string())
                    }
                }
            }
            Some(Resolution::VariantState { type_id, state }) => {
                self.gen_variant_state_value(type_id, state)
            }
            Some(Resolution::UnionMember { .. }) => {
                let pointer = self
                    .gen_lvalue(node)?
                    .ok_or_else(|| "union member is not addressable".to_string())?;
                let llvm_type = self.llvm_type(self.type_of(node))?;
                self.builder
                    .build_load(llvm_type, pointer, "union_member")
                    .map_err(|error| format!("cannot load union member: {error}"))
            }
            Some(Resolution::CompileTimeNoOp) => Ok(self.context.i8_type().const_zero().into()),
            Some(Resolution::Constant { ty, value }) => {
                let llvm_type = self.llvm_type(ty)?.into_int_type();
                Ok(llvm_type.const_int(value as u64, false).into())
            }
            Some(Resolution::SizeOf { ty }) => {
                let size = self.sema.size_align(ty).0;
                let llvm_type = self.llvm_type(self.type_of(node))?.into_int_type();
                Ok(llvm_type.const_int(size, false).into())
            }
            Some(Resolution::Function(index)) => Ok(self.function_value_closure(index)?.into()),
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

    fn negate_boolean(
        &mut self,
        value: BasicValueEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let value = value.into_int_value();
        let width = value.get_type().get_bit_width().max(8);
        let all_ones = value.get_type().const_all_ones();
        if width == 8 {
            let negated = self
                .builder
                .build_xor(value, all_ones, "not")
                .map_err(|error| format!("cannot negate: {error}"))?;
            return Ok(negated.into());
        }
        let extended = self
            .builder
            .build_int_z_extend(value, self.context.i8_type(), "bool")
            .map_err(|error| format!("cannot extend boolean: {error}"))?;
        self.builder
            .build_xor(extended, self.context.i8_type().const_all_ones(), "not")
            .map(Into::into)
            .map_err(|error| format!("cannot negate: {error}"))
    }

    fn or_booleans(
        &mut self,
        left: BasicValueEnum<'ctx>,
        right: BasicValueEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let left = self.as_boolean_i1(left.into_int_value());
        let right = self.as_boolean_i1(right.into_int_value());
        self.builder
            .build_or(left, right, "or")
            .map(Into::into)
            .map_err(|error| format!("cannot combine booleans: {error}"))
    }

    /// Calls an operator implementation with the given argument values.
    fn gen_operator_call(
        &mut self,
        method: u32,
        arguments: Vec<BasicValueEnum<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let function = *self
            .functions
            .get(&method)
            .ok_or_else(|| "missing operator implementation".to_string())?;
        let arguments: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> =
            arguments.into_iter().map(Into::into).collect();
        let call = self
            .builder
            .build_call(function, &arguments, "operator")
            .map_err(|error| format!("cannot call operator: {error}"))?;
        match call.try_as_basic_value() {
            ValueKind::Basic(value) => Ok(value),
            ValueKind::Instruction(_) => Err("operator produced no value".to_string()),
        }
    }

    fn gen_index_expr(
        &mut self,
        node: NodeId,
        index: &IndexExpr,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        if let Some(Resolution::Indexer { getter, .. }) =
            self.sema.resolutions[node as usize].clone()
        {
            let getter = getter.ok_or_else(|| "indexer is write-only".to_string())?;
            let receiver = self.gen_instance_pointer(index.base)?;
            let index_value = self.gen_expr(index.index)?;
            let function = *self
                .functions
                .get(&getter)
                .ok_or_else(|| "missing indexer getter".to_string())?;
            let arguments: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> =
                vec![receiver.into(), index_value.into()];
            let call = self
                .builder
                .build_call(function, &arguments, "indexer")
                .map_err(|error| format!("cannot call indexer: {error}"))?;
            return match call.try_as_basic_value() {
                ValueKind::Basic(value) => Ok(value),
                ValueKind::Instruction(_) => Err("indexer produced no value".to_string()),
            };
        }
        self.gen_index_expr_impl(node, index)
    }

    fn gen_index_expr_impl(
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

    /// Address of the storage of a resolved local, global, `this` or `base`
    /// expression (`owner.name` reuses this too, §6.4).
    fn gen_lvalue_resolution(
        &mut self,
        node: NodeId,
    ) -> Result<Option<PointerValue<'ctx>>, String> {
        match self.sema.resolutions[node as usize].clone() {
            Some(Resolution::Local(local)) => {
                let info = self
                    .local_info(local)
                    .ok_or_else(|| "missing local information".to_string())?;
                if let Storage::Capture(capture_index) = info.storage {
                    let capture = self.sema.functions[self.current_function as usize]
                        .captures
                        .get(capture_index as usize)
                        .cloned()
                        .ok_or_else(|| "missing capture information".to_string())?;
                    let env_slot = self
                        .current_env_slot
                        .ok_or_else(|| "missing environment".to_string())?;
                    let pointer_type = self.context.ptr_type(AddressSpace::default());
                    let environment = self
                        .builder
                        .build_load(pointer_type, env_slot, "environment")
                        .map_err(|error| format!("cannot load environment: {error}"))?
                        .into_pointer_value();
                    let address = self.address_with_size(
                        environment,
                        1,
                        self.context.i64_type().const_int(capture.offset, false),
                    )?;
                    return Ok(Some(address));
                }
                let slot = *self
                    .local_slots
                    .get(&local)
                    .ok_or_else(|| "missing local slot".to_string())?;
                if info.storage == Storage::Reference {
                    let pointer_type = self.context.ptr_type(AddressSpace::default());
                    let pointer = self
                        .builder
                        .build_load(pointer_type, slot, "reference")
                        .map_err(|error| format!("cannot load reference: {error}"))?;
                    return Ok(Some(pointer.into_pointer_value()));
                }
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
            Some(Resolution::This) | Some(Resolution::Base) => {
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
        }
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
            NodeKind::Name(_) => self.gen_lvalue_resolution(node),
            NodeKind::Index(index) => {
                let base_type = self.type_of(index.base);
                match self.sema.types.get(base_type).clone() {
                    Type::Array {
                        dimensions,
                        element: _,
                    } => {
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
                        let index_integer = self.as_i64(index_value)?;
                        let length = i64::try_from(dimensions.first().copied().unwrap_or(0))
                            .map_err(|_| "array is too large".to_string())?;
                        let in_bounds = self
                            .builder
                            .build_int_compare(
                                IntPredicate::ULT,
                                index_integer,
                                self.context.i64_type().const_int(length as u64, false),
                                "in_bounds",
                            )
                            .map_err(|error| format!("cannot check bounds: {error}"))?;
                        let function = self.current_llvm_function()?;
                        let ok_block = self.context.append_basic_block(function, "index_ok");
                        let fail_block = self.context.append_basic_block(function, "index_fail");
                        self.builder
                            .build_conditional_branch(in_bounds, ok_block, fail_block)
                            .map_err(|error| format!("cannot check bounds: {error}"))?;
                        self.builder.position_at_end(fail_block);
                        self.gen_runtime_throw("OutOfRangeException")?;
                        if !self.builder_terminated() {
                            self.builder
                                .build_unreachable()
                                .map_err(|error| format!("cannot trap: {error}"))?;
                        }
                        self.builder.position_at_end(ok_block);
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
                let resolution = self.sema.resolutions[node as usize].clone();
                // `owner.name` resolves to a local, global, `this` or `base`
                // storage rather than to a field (§6.4).
                if matches!(
                    resolution,
                    Some(Resolution::Local(_))
                        | Some(Resolution::Global(_))
                        | Some(Resolution::This)
                        | Some(Resolution::Base)
                ) {
                    return self.gen_lvalue_resolution(node);
                }
                let base_type = self.type_of(access.base);
                if let Some(Resolution::UnionMember { type_id, member }) = resolution {
                    let Type::Named(index) = self.sema.types.get(type_id).clone() else {
                        return Err("union member requires a union".to_string());
                    };
                    let offset = self.sema.type_defs[index as usize]
                        .union_members
                        .get(member as usize)
                        .map_or(0, |member| member.offset);
                    let base_pointer = self.struct_or_union_address(access.base, type_id)?;
                    let address = self.address_with_size(
                        base_pointer,
                        1,
                        self.context.i64_type().const_int(offset, false),
                    )?;
                    return Ok(Some(address));
                }
                let Some(Resolution::Field { type_id, index }) = resolution else {
                    return Ok(None);
                };
                let base_is_class = matches!(
                    self.sema.types.get(base_type),
                    Type::Named(index)
                        if self.sema.type_defs[*index as usize].kind == TypeDefKind::Class
                );
                let base_pointer = if base_is_class {
                    self.gen_expr(access.base)?.into_pointer_value()
                } else {
                    self.struct_or_union_address(access.base, self.type_of(access.base))?
                };
                self.struct_field_pointer(base_pointer, type_id, index)
            }
            // An array cast to a pointer refers to the array's own storage and a
            // pointer cast to an array refers to the pointee (§3.6.1), so both
            // stay addressable instead of producing a copy.
            NodeKind::Cast(cast) => {
                let source = self.type_of(cast.value);
                let target = self.type_of(node);
                match (
                    self.sema.types.get(source).clone(),
                    self.sema.types.get(target).clone(),
                ) {
                    (Type::Array { .. }, Type::Pointer { .. }) => self.gen_lvalue(cast.value),
                    (Type::Pointer { .. }, Type::Array { .. }) => {
                        let pointer = self.gen_expr(cast.value)?.into_pointer_value();
                        Ok(Some(pointer))
                    }
                    _ => Ok(None),
                }
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
        self.address_with_size(base, element_size, index)
    }

    /// Address `base + index * element_size` (all sizes in bytes).
    fn address_with_size(
        &self,
        base: PointerValue<'ctx>,
        element_size: u64,
        index: IntValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, String> {
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

    /// Loads the pointer stored `offset` bytes into `base`.
    fn load_pointer_at(
        &mut self,
        base: PointerValue<'ctx>,
        offset: u64,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let address =
            self.address_with_size(base, 1, self.context.i64_type().const_int(offset, false))?;
        let pointer_type = self.context.ptr_type(AddressSpace::default());
        self.builder
            .build_load(pointer_type, address, "loaded_pointer")
            .map_err(|error| format!("cannot load pointer: {error}"))
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

    /// Address of a structure/class/union value: its lvalue, or a spill.
    fn struct_or_union_address(
        &mut self,
        node: NodeId,
        value_type: TypeId,
    ) -> Result<PointerValue<'ctx>, String> {
        if let Some(pointer) = self.gen_lvalue(node)? {
            return Ok(pointer);
        }
        let value = self.gen_expr(node)?;
        let llvm_type = self.instance_llvm_type(value_type)?;
        let slot = self
            .builder
            .build_alloca(llvm_type, "value_spill")
            .map_err(|error| format!("cannot allocate value: {error}"))?;
        self.builder
            .build_store(slot, value)
            .map_err(|error| format!("cannot store value: {error}"))?;
        Ok(slot)
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
            Some(Resolution::Construct { .. })
            | Some(Resolution::ConstructVariant { .. })
            | Some(Resolution::ConstructUnion { .. }) => self.gen_construction(node),
            Some(Resolution::SizeOf { ty }) => {
                let size = self.sema.size_align(ty).0;
                let llvm_type = self.llvm_type(self.type_of(node))?.into_int_type();
                Ok(llvm_type.const_int(size, false).into())
            }
            Some(Resolution::Function(index)) => {
                let function = *self
                    .functions
                    .get(&index)
                    .ok_or_else(|| "missing callee".to_string())?;
                let is_method = self.sema.functions[index as usize].method_of.is_some();
                let mut arguments: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> =
                    Vec::with_capacity(call.arguments.len() + usize::from(is_method));
                let mut receiver = None;
                let mut base_call = false;
                if is_method {
                    let NodeKind::MemberAccess(access) = self.ast.node(call.callee).kind.clone()
                    else {
                        return Err("method call requires a receiver".to_string());
                    };
                    base_call = matches!(self.ast.node(access.base).kind, NodeKind::Base);
                    let receiver_pointer = self.gen_instance_pointer(access.base)?;
                    receiver = Some(receiver_pointer);
                    arguments.push(receiver_pointer.into());
                }
                let parameter_offset = usize::from(is_method);
                let parameters = self.sema.functions[index as usize].parameters.clone();
                let mapped = self.sema.call_arguments.get(&node).cloned();
                let count = mapped
                    .as_ref()
                    .map_or(call.arguments.len(), |mapped| mapped.len());
                for position in 0..count {
                    let value_node = match &mapped {
                        Some(mapped) => mapped[position],
                        None => call.arguments[position].value,
                    };
                    let by_ref = parameters
                        .get(parameter_offset + position)
                        .is_some_and(|parameter| parameter.by_ref);
                    if by_ref {
                        let address = self.gen_lvalue(value_node)?.ok_or_else(|| {
                            "by-reference argument is not addressable".to_string()
                        })?;
                        arguments.push(address.into());
                    } else {
                        arguments.push(self.gen_coerced_expr(value_node)?.into());
                    }
                }
                let call_site = if let Some(receiver_pointer) = receiver {
                    let receiver_type = if is_method {
                        match self.ast.node(call.callee).kind.clone() {
                            NodeKind::MemberAccess(access) => self.type_of(access.base),
                            _ => TypeId::ERROR,
                        }
                    } else {
                        TypeId::ERROR
                    };
                    let interface_index = if base_call {
                        None
                    } else {
                        match self.sema.types.get(receiver_type) {
                            Type::Named(receiver_index)
                                if self.sema.type_defs[*receiver_index as usize].kind
                                    == TypeDefKind::Interface =>
                            {
                                Some(*receiver_index)
                            }
                            _ => None,
                        }
                    };
                    match interface_index {
                        Some(interface_index) => self.gen_interface_call(
                            receiver_pointer,
                            interface_index,
                            index,
                            &arguments,
                        )?,
                        None => {
                            let slot = if base_call {
                                None
                            } else {
                                self.vtable_slot(receiver_type, index)
                            };
                            match slot {
                                Some(slot) => {
                                    // Virtual dispatch: load the slot from the vtable in
                                    // the instance header (§3.7.1).
                                    let pointer_type =
                                        self.context.ptr_type(AddressSpace::default());
                                    let vtable = self
                                        .builder
                                        .build_load(pointer_type, receiver_pointer, "vtable")
                                        .map_err(|error| format!("cannot load vtable: {error}"))?
                                        .into_pointer_value();
                                    let slot_address = self.address_with_size(
                                        vtable,
                                        8,
                                        self.context.i64_type().const_int(slot as u64, false),
                                    )?;
                                    let callee_pointer = self
                                        .builder
                                        .build_load(pointer_type, slot_address, "vslot")
                                        .map_err(|error| {
                                            format!("cannot load vtable slot: {error}")
                                        })?
                                        .into_pointer_value();
                                    let signature = self.function_type(index)?;
                                    self.builder
                                        .build_indirect_call(
                                            signature,
                                            callee_pointer,
                                            &arguments,
                                            "virtual_call",
                                        )
                                        .map_err(|error| format!("cannot dispatch call: {error}"))?
                                }
                                None => self
                                    .builder
                                    .build_call(function, &arguments, "call")
                                    .map_err(|error| format!("cannot call function: {error}"))?,
                            }
                        }
                    }
                } else {
                    self.builder
                        .build_call(function, &arguments, "call")
                        .map_err(|error| format!("cannot call function: {error}"))?
                };
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
            Some(Resolution::CompileTimeNoOp) => Ok(self.context.i8_type().const_zero().into()),
            Some(Resolution::Constant { ty, value }) => {
                let llvm_type = self.llvm_type(ty)?.into_int_type();
                Ok(llvm_type.const_int(value as u64, false).into())
            }
            _ => {
                // Indirect call through a function value (§3.6.3, §4.5).
                let callee_type = self.type_of(call.callee);
                let Type::Function {
                    parameters,
                    wildcard,
                    return_type,
                    ..
                } = self.sema.types.get(callee_type).clone()
                else {
                    return Err("call target is not callable".to_string());
                };
                let closure = self.gen_expr(call.callee)?.into_pointer_value();
                let (code, environment) = self.load_closure(closure)?;
                let pointer_type = self.context.ptr_type(AddressSpace::default());
                let mut llvm_parameters = Vec::with_capacity(parameters.len() + 1);
                llvm_parameters.push(pointer_type.into());
                for parameter in &parameters {
                    llvm_parameters.push(self.llvm_type(*parameter)?.into());
                }
                let function_type = match return_type {
                    Some(ty) => self.llvm_type(ty)?.fn_type(&llvm_parameters, wildcard),
                    None => self.context.void_type().fn_type(&llvm_parameters, wildcard),
                };
                let mut arguments = Vec::with_capacity(call.arguments.len() + 1);
                arguments.push(environment.into());
                let mapped = self.sema.call_arguments.get(&node).cloned();
                let count = mapped
                    .as_ref()
                    .map_or(call.arguments.len(), |mapped| mapped.len());
                for position in 0..count {
                    let value_node = match &mapped {
                        Some(mapped) => mapped[position],
                        None => call.arguments[position].value,
                    };
                    arguments.push(self.gen_coerced_expr(value_node)?.into());
                }
                let call_site = self
                    .builder
                    .build_indirect_call(function_type, code, &arguments, "indirect")
                    .map_err(|error| format!("cannot call function value: {error}"))?;
                match return_type {
                    Some(_) => match call_site.try_as_basic_value() {
                        ValueKind::Basic(value) => Ok(value),
                        ValueKind::Instruction(_) => {
                            Err("function value did not produce a value".to_string())
                        }
                    },
                    None => Ok(self.context.i8_type().const_zero().into()),
                }
            }
        }
    }

    /// Invokes a method or accessor on a receiver, choosing interface, virtual
    /// or direct dispatch based on the static receiver type.
    fn gen_method_dispatch(
        &mut self,
        method: u32,
        receiver_type: TypeId,
        receiver: PointerValue<'ctx>,
        extra_arguments: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>>,
    ) -> Result<CallSiteValue<'ctx>, String> {
        let mut arguments: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> =
            Vec::with_capacity(extra_arguments.len() + 1);
        arguments.push(receiver.into());
        arguments.extend(extra_arguments);
        if let Type::Named(index) = self.sema.types.get(receiver_type).clone() {
            if self.sema.type_defs[index as usize].kind == TypeDefKind::Interface {
                return self.gen_interface_call(receiver, index, method, &arguments);
            }
            if self.sema.type_defs[index as usize].kind == TypeDefKind::Class {
                if let Some(slot) = self.vtable_slot(receiver_type, method) {
                    let pointer_type = self.context.ptr_type(AddressSpace::default());
                    let vtable = self
                        .builder
                        .build_load(pointer_type, receiver, "vtable")
                        .map_err(|error| format!("cannot load vtable: {error}"))?
                        .into_pointer_value();
                    let slot_address = self.address_with_size(
                        vtable,
                        8,
                        self.context.i64_type().const_int(slot as u64, false),
                    )?;
                    let callee = self
                        .builder
                        .build_load(pointer_type, slot_address, "vslot")
                        .map_err(|error| format!("cannot load vtable slot: {error}"))?
                        .into_pointer_value();
                    let signature = self.function_type(method)?;
                    return self
                        .builder
                        .build_indirect_call(signature, callee, &arguments, "virtual_call")
                        .map_err(|error| format!("cannot dispatch call: {error}"));
                }
            }
        }
        let function = *self
            .functions
            .get(&method)
            .ok_or_else(|| "missing method".to_string())?;
        self.builder
            .build_call(function, &arguments, "method_call")
            .map_err(|error| format!("cannot call method: {error}"))
    }

    /// Address of the instance a call or field access operates on.
    fn gen_instance_pointer(&mut self, node: NodeId) -> Result<PointerValue<'ctx>, String> {
        if matches!(self.ast.node(node).kind, NodeKind::This | NodeKind::Base) {
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
        if matches!(
            self.sema.types.get(ty),
            Type::Named(index) if matches!(
                self.sema.type_defs[*index as usize].kind,
                TypeDefKind::Class | TypeDefKind::Interface
            )
        ) {
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
            Type::Named(index) => {
                let definition = &self.sema.type_defs[index as usize];
                match definition.kind {
                    TypeDefKind::Structure | TypeDefKind::Class => {
                        let struct_type = self.struct_type(index)?;
                        Ok(struct_type.into())
                    }
                    TypeDefKind::Variant | TypeDefKind::Union => {
                        let size = u32::try_from(definition.instance_size)
                            .map_err(|_| "instance is too large".to_string())?;
                        Ok(self.context.i8_type().array_type(size.max(1)).into())
                    }
                    _ => Err("this type has no instance layout".to_string()),
                }
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
        if method.is_none() {
            // Built-in exceptions default InnerException to Optional.Null and
            // accept an explicit inner exception at construction (§15.2).
            if crate::sema::BUILTIN_EXCEPTION_NAMES
                .contains(&self.sema.type_defs[index as usize].name.as_str())
                && !self.sema.type_defs[index as usize].fields.is_empty()
            {
                let pointer = self
                    .struct_field_pointer(storage, type_id, 0)?
                    .ok_or_else(|| "missing InnerException field".to_string())?;
                match self.sema.call_arguments.get(&node).cloned() {
                    Some(mapped) if !mapped.is_empty() => {
                        let value = self.gen_coerced_expr(mapped[0])?;
                        self.builder.build_store(pointer, value).map_err(|error| {
                            format!("cannot initialize InnerException: {error}")
                        })?;
                    }
                    _ => {
                        // Optional.Null tag byte.
                        let tag =
                            self.builder
                                .build_load(self.context.i8_type(), pointer, "inner_tag");
                        let _ = tag;
                        self.builder
                            .build_store(pointer, self.context.i8_type().const_int(1, false))
                            .map_err(|error| {
                                format!("cannot initialize InnerException: {error}")
                            })?;
                    }
                }
            }
        }
        if let Some(method) = method {
            let function = *self
                .functions
                .get(&method)
                .ok_or_else(|| "missing constructor".to_string())?;
            let mut arguments = Vec::new();
            arguments.push(storage.into());
            let mapped = self.sema.call_arguments.get(&node).cloned();
            match mapped {
                Some(mapped) => {
                    for value_node in mapped {
                        arguments.push(self.gen_coerced_expr(value_node)?.into());
                    }
                }
                None => {
                    if let NodeKind::Call(call) = self.ast.node(node).kind.clone() {
                        for argument in &call.arguments {
                            arguments.push(self.gen_expr(argument.value)?.into());
                        }
                    }
                }
            }
            self.builder
                .build_call(function, &arguments, "construct")
                .map_err(|error| format!("cannot call constructor: {error}"))?;
        }
        Ok(())
    }

    fn gen_variant_state_value(
        &mut self,
        type_id: TypeId,
        state: u32,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let instance_type = self.instance_llvm_type(type_id)?;
        let storage = self
            .builder
            .build_alloca(instance_type, "variant_state")
            .map_err(|error| format!("cannot allocate variant: {error}"))?;
        self.builder
            .build_store(storage, instance_type.const_zero())
            .map_err(|error| format!("cannot initialize variant: {error}"))?;
        self.gen_variant_into(storage, type_id, state, None)?;
        self.builder
            .build_load(instance_type, storage, "variant_value")
            .map_err(|error| format!("cannot load variant: {error}"))
    }

    fn gen_variant_into(
        &mut self,
        storage: PointerValue<'ctx>,
        type_id: TypeId,
        state: u32,
        payload: Option<BasicValueEnum<'ctx>>,
    ) -> Result<(), String> {
        let Type::Named(index) = self.sema.types.get(type_id).clone() else {
            return Err("variant construction requires a variant".to_string());
        };
        let (tag_type, payload_offset) = {
            let definition = &self.sema.type_defs[index as usize];
            (
                definition
                    .tag_type
                    .ok_or_else(|| "variant has no tag type".to_string())?,
                definition.payload_offset,
            )
        };
        let tag_llvm = self.llvm_type(tag_type)?.into_int_type();
        let tag_address =
            self.address_with_size(storage, 1, self.context.i64_type().const_zero())?;
        self.builder
            .build_store(tag_address, tag_llvm.const_int(u64::from(state), false))
            .map_err(|error| format!("cannot store variant tag: {error}"))?;
        if let Some(payload) = payload {
            let payload_address = self.address_with_size(
                storage,
                1,
                self.context.i64_type().const_int(payload_offset, false),
            )?;
            self.builder
                .build_store(payload_address, payload)
                .map_err(|error| format!("cannot store variant payload: {error}"))?;
        }
        Ok(())
    }

    /// Lowers a member construction (`Variant.State(value)` / `Union.Member(value)`).
    fn gen_member_construction_into(
        &mut self,
        node: NodeId,
        storage: PointerValue<'ctx>,
    ) -> Result<(), String> {
        let argument = match self.ast.node(node).kind.clone() {
            NodeKind::Call(call) => call
                .arguments
                .first()
                .map(|argument| argument.value)
                .ok_or_else(|| "member construction requires a value".to_string())?,
            _ => return Err("expected a member construction".to_string()),
        };
        match self.sema.resolutions[node as usize].clone() {
            Some(Resolution::ConstructVariant { type_id, state }) => {
                let Type::Named(index) = self.sema.types.get(type_id).clone() else {
                    return Err("variant construction requires a variant".to_string());
                };
                let payload_type = self.sema.type_defs[index as usize]
                    .variant_states
                    .get(state as usize)
                    .and_then(|state| state.payload);
                let payload = match payload_type {
                    Some(_) => Some(self.gen_expr(argument)?),
                    None => None,
                };
                self.gen_variant_into(storage, type_id, state, payload)
            }
            Some(Resolution::ConstructUnion { type_id, member }) => {
                let Type::Named(index) = self.sema.types.get(type_id).clone() else {
                    return Err("union construction requires a union".to_string());
                };
                let (offset, member_type) = {
                    let entry = self.sema.type_defs[index as usize]
                        .union_members
                        .get(member as usize)
                        .ok_or_else(|| "missing union member".to_string())?;
                    (entry.offset, entry.ty)
                };
                let value = self.gen_expr(argument)?;
                let address = self.address_with_size(
                    storage,
                    1,
                    self.context.i64_type().const_int(offset, false),
                )?;
                let _ = member_type;
                self.builder
                    .build_store(address, value)
                    .map_err(|error| format!("cannot store union member: {error}"))?;
                Ok(())
            }
            _ => Err("expected a member construction".to_string()),
        }
    }

    fn gen_construction(&mut self, node: NodeId) -> Result<BasicValueEnum<'ctx>, String> {
        let resolution = self.sema.resolutions[node as usize].clone();
        let (type_id, kind) = match resolution.clone() {
            Some(Resolution::Construct { type_id, .. }) => {
                let Type::Named(index) = self.sema.types.get(type_id).clone() else {
                    return Err("construction requires a named type".to_string());
                };
                (type_id, self.sema.type_defs[index as usize].kind)
            }
            Some(Resolution::ConstructVariant { type_id, .. })
            | Some(Resolution::ConstructUnion { type_id, .. }) => {
                let Type::Named(index) = self.sema.types.get(type_id).clone() else {
                    return Err("construction requires a named type".to_string());
                };
                (type_id, self.sema.type_defs[index as usize].kind)
            }
            _ => return Err("expected a construction expression".to_string()),
        };
        let Type::Named(_index) = self.sema.types.get(type_id).clone() else {
            return Err("construction requires a named type".to_string());
        };
        if matches!(kind, TypeDefKind::Variant | TypeDefKind::Union) {
            let instance_type = self.instance_llvm_type(type_id)?;
            let storage = self
                .builder
                .build_alloca(instance_type, "member_construction")
                .map_err(|error| format!("cannot allocate value: {error}"))?;
            self.builder
                .build_store(storage, instance_type.const_zero())
                .map_err(|error| format!("cannot initialize value: {error}"))?;
            self.gen_member_construction_into(node, storage)?;
            return self
                .builder
                .build_load(instance_type, storage, "member_value")
                .map_err(|error| format!("cannot load value: {error}"));
        }
        let is_class = kind == TypeDefKind::Class;
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
                let instance_struct = self.struct_type(class_index)?;
                let vtable_pointer = self
                    .vtable_globals
                    .get(&class_index)
                    .map(|global| global.as_pointer_value())
                    .unwrap_or_else(|| self.context.ptr_type(AddressSpace::default()).const_zero());
                let typeinfo_pointer = self
                    .typeinfo_globals
                    .get(&class_index)
                    .map(|global| global.as_pointer_value())
                    .unwrap_or_else(|| self.context.ptr_type(AddressSpace::default()).const_zero());
                for (field_index, header_value) in
                    [vtable_pointer, typeinfo_pointer].into_iter().enumerate()
                {
                    let pointer = self
                        .builder
                        .build_struct_gep(instance_struct, storage, field_index as u32, "header")
                        .map_err(|error| format!("cannot initialize header: {error}"))?;
                    self.builder
                        .build_store(pointer, header_value)
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
        if let Some(Resolution::Conversion { method }) =
            self.sema.resolutions[node as usize].clone()
        {
            let value = self.gen_expr(cast.value)?;
            return self.gen_operator_call(method, vec![value]);
        }
        self.gen_cast_expr_impl(node, cast)
    }

    fn gen_cast_expr_impl(
        &mut self,
        node: NodeId,
        cast: &CastExpr,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let source_type = self.type_of(cast.value);
        let target_type = self.type_of(node);
        // An array converts to a pointer to its first element without copying
        // (§3.6.1), so the address of the array itself is the conversion result.
        if matches!(self.sema.types.get(source_type), Type::Array { .. })
            && matches!(self.sema.types.get(target_type), Type::Pointer { .. })
        {
            if let Some(address) = self.gen_lvalue(cast.value)? {
                return Ok(address.into());
            }
        }
        let value = self.gen_expr(cast.value)?;
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
            (Type::Float { .. }, Type::Integer { signed, bytes }) => {
                // Out-of-range values must wrap (§3.2), so reduce the value
                // modulo 2^bits before the (undefined-behaviour) float-to-int
                // conversion.
                let from = value.into_float_value();
                let f64_type = self.context.f64_type();
                let wide = if from.get_type() == f64_type {
                    from
                } else {
                    self.builder
                        .build_float_ext(from, f64_type, "cast_f64")
                        .map_err(|error| format!("cannot convert: {error}"))?
                };
                let bits = u32::from(*bytes) * 8;
                let modulus = f64_type.const_float((2.0f64).powi(bits as i32));
                let reduced = self
                    .builder
                    .build_float_rem(wide, modulus, "cast_mod")
                    .map_err(|error| format!("cannot convert: {error}"))?;
                // Move negative remainders into [0, 2^bits).
                let is_negative = self
                    .builder
                    .build_float_compare(
                        FloatPredicate::OLT,
                        reduced,
                        f64_type.const_zero(),
                        "cast_neg",
                    )
                    .map_err(|error| format!("cannot convert: {error}"))?;
                let shifted = self
                    .builder
                    .build_float_add(reduced, modulus, "cast_shift")
                    .map_err(|error| format!("cannot convert: {error}"))?;
                let wrapped = self
                    .builder
                    .build_select(is_negative, shifted, reduced, "cast_wrap")
                    .map_err(|error| format!("cannot convert: {error}"))?
                    .into_float_value();
                let target_int = target_llvm.into_int_type();
                let result = if *signed {
                    // Reinterpret the unsigned remainder as a two's-complement
                    // value of the target width.
                    let half = f64_type.const_float((2.0f64).powi(bits.saturating_sub(1) as i32));
                    let above_half = self
                        .builder
                        .build_float_compare(FloatPredicate::OGE, wrapped, half, "cast_signed")
                        .map_err(|error| format!("cannot convert: {error}"))?;
                    let signed_value = self
                        .builder
                        .build_float_sub(wrapped, modulus, "cast_twos")
                        .map_err(|error| format!("cannot convert: {error}"))?;
                    let chosen = self
                        .builder
                        .build_select(above_half, signed_value, wrapped, "cast_signed_value")
                        .map_err(|error| format!("cannot convert: {error}"))?
                        .into_float_value();
                    self.builder
                        .build_float_to_signed_int(chosen, target_int, "cast")
                } else {
                    self.builder
                        .build_float_to_unsigned_int(wrapped, target_int, "cast")
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
            // Reinterpret the same bytes with a different element shape
            // (§3.6.1); sema only allows equal sizes here.
            (Type::Array { .. }, Type::Array { .. }) => {
                let source_type = self.llvm_type(source)?;
                let target_type = self.llvm_type(target)?;
                let slot = self
                    .builder
                    .build_alloca(source_type, "array_shape")
                    .map_err(|error| format!("cannot allocate array: {error}"))?;
                self.builder
                    .build_store(slot, value)
                    .map_err(|error| format!("cannot store array: {error}"))?;
                self.builder
                    .build_load(target_type, slot, "array_shape_value")
                    .map_err(|error| format!("cannot convert array: {error}"))
            }
            // Enumerations convert explicitly to and from integer types through
            // their underlying type (§3.7.4).
            (Type::Named(index), Type::Integer { .. }) => {
                let underlying = self.sema.type_defs[*index as usize]
                    .underlying
                    .ok_or_else(|| "only enumerations can be cast to integers".to_string())?;
                self.convert(value, underlying, target)
            }
            (Type::Integer { .. }, Type::Named(index)) => {
                let underlying = self.sema.type_defs[*index as usize]
                    .underlying
                    .ok_or_else(|| "only enumerations can be cast from integers".to_string())?;
                self.convert(value, source, underlying)
            }
            // Class and interface values are references (§3.4): converting
            // between them reuses the reference, which sema has already checked.
            (Type::Named(left), Type::Named(right)) => {
                let left_kind = self.sema.type_defs[*left as usize].kind;
                let right_kind = self.sema.type_defs[*right as usize].kind;
                if matches!(left_kind, TypeDefKind::Class | TypeDefKind::Interface)
                    && matches!(right_kind, TypeDefKind::Class | TypeDefKind::Interface)
                {
                    Ok(value)
                } else {
                    Err(format!(
                        "unsupported cast from '{}' to '{}'",
                        self.sema.spelling(source),
                        self.sema.spelling(target)
                    ))
                }
            }
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
        if let Some(Resolution::Operator { method }) = self.sema.resolutions[node as usize].clone()
        {
            let operand = self.gen_expr(unary.operand)?;
            return self.gen_operator_call(method, vec![operand]);
        }
        self.gen_unary_expr_impl(node, unary)
    }

    fn gen_unary_expr_impl(
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
        match self.sema.resolutions[node as usize].clone() {
            Some(Resolution::Operator { method }) => {
                let left = self.gen_expr(binary.left)?;
                let right = self.gen_expr(binary.right)?;
                return self.gen_operator_call(method, vec![left, right]);
            }
            Some(Resolution::DerivedNotEqual { method }) => {
                let left = self.gen_expr(binary.left)?;
                let right = self.gen_expr(binary.right)?;
                let result = self.gen_operator_call(method, vec![left, right])?;
                return self.negate_boolean(result);
            }
            Some(Resolution::DerivedComparison { ordering, equality }) => {
                let left = self.gen_expr(binary.left)?;
                let right = self.gen_expr(binary.right)?;
                let ordering_result = self.gen_operator_call(ordering, vec![left, right])?;
                let left = self.gen_expr(binary.left)?;
                let right = self.gen_expr(binary.right)?;
                let equality_result = self.gen_operator_call(equality, vec![left, right])?;
                return self.or_booleans(ordering_result, equality_result);
            }
            _ => {}
        }
        self.gen_binary_expr_impl(node, binary)
    }

    fn gen_binary_expr_impl(
        &mut self,
        node: NodeId,
        binary: &BinaryExpr,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        let operand_type = self.type_of(binary.left);
        if let Some(value) = self.gen_pointer_arithmetic(node, binary)? {
            return Ok(value);
        }
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

    /// Widens a pointer offset to `i64` preserving its sign, so negative offsets
    /// address lower memory instead of wrapping to a huge positive index.
    fn pointer_offset(&mut self, value: IntValue<'ctx>) -> Result<IntValue<'ctx>, String> {
        let i64_type = self.context.i64_type();
        let bits = value.get_type().get_bit_width();
        if bits == 64 {
            return Ok(value);
        }
        if bits > 64 {
            return self
                .builder
                .build_int_truncate(value, i64_type, "offset64")
                .map_err(|error| format!("cannot convert offset: {error}"));
        }
        self.builder
            .build_int_s_extend(value, i64_type, "offset64")
            .map_err(|error| format!("cannot convert offset: {error}"))
    }

    /// Lowers built-in pointer arithmetic (§3.6.2). The unit is the size of the
    /// pointee, or one byte for a target-less pointer.
    fn gen_pointer_arithmetic(
        &mut self,
        node: NodeId,
        binary: &BinaryExpr,
    ) -> Result<Option<BasicValueEnum<'ctx>>, String> {
        let left_type = self.type_of(binary.left);
        let right_type = self.type_of(binary.right);
        let left_pointer = matches!(self.sema.types.get(left_type), Type::Pointer { .. });
        let right_pointer = matches!(self.sema.types.get(right_type), Type::Pointer { .. });
        let unit = |pointer: TypeId, sema: &SemaResult| match sema.types.get(pointer) {
            Type::Pointer {
                target: Some(target),
            } => sema.size_align(*target).0.max(1),
            _ => 1,
        };
        match binary.op {
            BinaryOp::Add if left_pointer && !right_pointer => {
                let base = self.gen_expr(binary.left)?.into_pointer_value();
                let index = self.gen_expr(binary.right)?.into_int_value();
                let index = self.pointer_offset(index)?;
                let element = unit(left_type, self.sema);
                let address = self.address_with_size(base, element, index)?;
                Ok(Some(address.into()))
            }
            BinaryOp::Add if right_pointer && !left_pointer => {
                let base = self.gen_expr(binary.right)?.into_pointer_value();
                let index = self.gen_expr(binary.left)?.into_int_value();
                let index = self.pointer_offset(index)?;
                let element = unit(right_type, self.sema);
                let address = self.address_with_size(base, element, index)?;
                Ok(Some(address.into()))
            }
            BinaryOp::Subtract if left_pointer && !right_pointer => {
                let base = self.gen_expr(binary.left)?.into_pointer_value();
                let offset = self.gen_expr(binary.right)?.into_int_value();
                let offset = self.pointer_offset(offset)?;
                let negated = self
                    .builder
                    .build_int_neg(offset, "negate_offset")
                    .map_err(|error| format!("cannot negate offset: {error}"))?;
                let element = unit(left_type, self.sema);
                let address = self.address_with_size(base, element, negated)?;
                Ok(Some(address.into()))
            }
            BinaryOp::Subtract if left_pointer && right_pointer => {
                let left = self.gen_expr(binary.left)?.into_pointer_value();
                let right = self.gen_expr(binary.right)?.into_pointer_value();
                let i64_type = self.context.i64_type();
                let left_integer = self
                    .builder
                    .build_ptr_to_int(left, i64_type, "left_address")
                    .map_err(|error| format!("cannot convert pointer: {error}"))?;
                let right_integer = self
                    .builder
                    .build_ptr_to_int(right, i64_type, "right_address")
                    .map_err(|error| format!("cannot convert pointer: {error}"))?;
                let difference = self
                    .builder
                    .build_int_sub(left_integer, right_integer, "difference")
                    .map_err(|error| format!("cannot subtract pointers: {error}"))?;
                let element = i64_type.const_int(unit(left_type, self.sema), false);
                let count = self
                    .builder
                    .build_int_signed_div(difference, element, "element_count")
                    .map_err(|error| format!("cannot divide addresses: {error}"))?;
                let _ = node;
                Ok(Some(count.into()))
            }
            _ => Ok(None),
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
                            // INT_MIN / -1 overflows; the spec's arithmetic wraps
                            // (§3.2), so select the wrapped result instead of
                            // leaving the operation's poison value in place.
                            let minus_one = int_type.const_all_ones();
                            let is_minus_one = self
                                .builder
                                .build_int_compare(
                                    IntPredicate::EQ,
                                    right,
                                    minus_one,
                                    "is_minus_one",
                                )
                                .map_err(|error| format!("cannot compare divisor: {error}"))?;
                            // Avoid executing the hardware division for the
                            // overflowing case: branch instead of selecting.
                            let function = self.current_llvm_function()?;
                            let special_block =
                                self.context.append_basic_block(function, "div_overflow");
                            let normal_block =
                                self.context.append_basic_block(function, "div_normal");
                            let merge_block =
                                self.context.append_basic_block(function, "div_merge");
                            self.builder
                                .build_conditional_branch(is_minus_one, special_block, normal_block)
                                .map_err(|error| format!("cannot branch: {error}"))?;
                            self.builder.position_at_end(normal_block);
                            let regular = if op == BinaryOp::Divide {
                                self.builder.build_int_signed_div(left, right, "div")
                            } else {
                                self.builder.build_int_signed_rem(left, right, "rem")
                            }
                            .map_err(|error| format!("cannot compute signed division: {error}"))?;
                            self.builder
                                .build_unconditional_branch(merge_block)
                                .map_err(|error| format!("cannot branch: {error}"))?;
                            self.builder.position_at_end(special_block);
                            let wrapped = if op == BinaryOp::Divide {
                                self.builder
                                    .build_int_sub(int_type.const_zero(), left, "wrapped_div")
                                    .map_err(|error| format!("cannot wrap division: {error}"))?
                            } else {
                                int_type.const_zero()
                            };
                            self.builder
                                .build_unconditional_branch(merge_block)
                                .map_err(|error| format!("cannot branch: {error}"))?;
                            self.builder.position_at_end(merge_block);
                            let phi = self
                                .builder
                                .build_phi(int_type, "signed_div")
                                .map_err(|error| format!("cannot build phi: {error}"))?;
                            phi.add_incoming(&[
                                (&regular, normal_block),
                                (&wrapped, special_block),
                            ]);
                            Ok(phi.as_basic_value().into_int_value())
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

    /// Throws a Built-In Library runtime exception (§5.10.1-6/9): the innermost
    /// body trap handles it, otherwise the program terminates.
    fn gen_runtime_throw(&mut self, class_name: &str) -> Result<(), String> {
        let index = self
            .sema
            .type_defs
            .iter()
            .position(|definition| definition.qualified_name == class_name)
            .ok_or_else(|| format!("missing built-in exception '{class_name}'"))?
            as u32;
        let instance = self
            .builtin_exceptions
            .get(&index)
            .ok_or_else(|| format!("missing built-in exception instance '{class_name}'"))?
            .as_pointer_value();
        let typeinfo = self
            .typeinfo_globals
            .get(&index)
            .ok_or_else(|| format!("missing built-in exception type information '{class_name}'"))?
            .as_pointer_value();
        let innermost = self
            .trap_stack
            .last()
            .map(|frame| (frame.exception_slot, frame.dispatch, frame.is_handler));
        match innermost {
            Some((slot, dispatch, false)) => {
                let typeinfo_slot =
                    self.address_with_size(slot, 1, self.context.i64_type().const_zero())?;
                let payload_slot =
                    self.address_with_size(slot, 1, self.context.i64_type().const_int(8, false))?;
                self.builder
                    .build_store(typeinfo_slot, typeinfo)
                    .map_err(|error| format!("cannot raise exception: {error}"))?;
                self.builder
                    .build_store(payload_slot, instance)
                    .map_err(|error| format!("cannot raise exception: {error}"))?;
                self.builder
                    .build_unconditional_branch(dispatch)
                    .map_err(|error| format!("cannot raise exception: {error}"))?;
                let dead = self
                    .context
                    .append_basic_block(self.current_llvm_function()?, "exception_dead");
                self.builder.position_at_end(dead);
                Ok(())
            }
            _ => self.emit_abort(),
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
        self.gen_runtime_throw("DivideByZeroException")?;
        if !self.builder_terminated() {
            self.builder
                .build_unreachable()
                .map_err(|error| format!("cannot trap: {error}"))?;
        }
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

    /// Lowers one value-equality or comparison match pattern (§5.9.6): a
    /// user-defined operator when sema resolved one, built-in comparison
    /// otherwise.
    fn gen_pattern_comparison(
        &mut self,
        op: BinaryOp,
        left: BasicValueEnum<'ctx>,
        right: BasicValueEnum<'ctx>,
        value_type: TypeId,
        pattern_node: NodeId,
    ) -> Result<IntValue<'ctx>, String> {
        match self.sema.resolutions[pattern_node as usize].clone() {
            Some(Resolution::Operator { method }) => {
                let result = self.gen_operator_call(method, vec![left, right])?;
                Ok(result.into_int_value())
            }
            Some(Resolution::DerivedNotEqual { method }) => {
                let result = self.gen_operator_call(method, vec![left, right])?;
                let negated = self.negate_boolean(result)?;
                Ok(negated.into_int_value())
            }
            Some(Resolution::DerivedComparison { ordering, equality }) => {
                let ordering_result = self.gen_operator_call(ordering, vec![left, right])?;
                let equality_result = self.gen_operator_call(equality, vec![left, right])?;
                let combined = self.or_booleans(ordering_result, equality_result)?;
                Ok(combined.into_int_value())
            }
            _ => self.build_comparison(op, left, right, value_type),
        }
    }

    /// Variant equality (§3.7.5): equal states and equal payloads.
    fn build_variant_comparison(
        &mut self,
        op: BinaryOp,
        left: BasicValueEnum<'ctx>,
        right: BasicValueEnum<'ctx>,
        ty: TypeId,
        index: u32,
    ) -> Result<IntValue<'ctx>, String> {
        let negate = match op {
            BinaryOp::Equal => false,
            BinaryOp::NotEqual => true,
            _ => return Err("unsupported variant comparison".to_string()),
        };
        let (tag_type, payload_offset, states) = {
            let definition = &self.sema.type_defs[index as usize];
            (
                definition
                    .tag_type
                    .ok_or_else(|| "variant has no tag type".to_string())?,
                definition.payload_offset,
                definition.variant_states.clone(),
            )
        };
        let instance_type = self.instance_llvm_type(ty)?;
        let left_slot = self
            .builder
            .build_alloca(instance_type, "variant_left")
            .map_err(|error| format!("cannot allocate variant: {error}"))?;
        let right_slot = self
            .builder
            .build_alloca(instance_type, "variant_right")
            .map_err(|error| format!("cannot allocate variant: {error}"))?;
        self.builder
            .build_store(left_slot, left)
            .map_err(|error| format!("cannot compare variants: {error}"))?;
        self.builder
            .build_store(right_slot, right)
            .map_err(|error| format!("cannot compare variants: {error}"))?;
        let tag_llvm = self.llvm_type(tag_type)?.into_int_type();
        let zero = self.context.i64_type().const_zero();
        let left_tag_address = self.address_with_size(left_slot, 1, zero)?;
        let right_tag_address = self.address_with_size(right_slot, 1, zero)?;
        let left_tag = self
            .builder
            .build_load(tag_llvm, left_tag_address, "left_tag")
            .map_err(|error| format!("cannot compare variants: {error}"))?
            .into_int_value();
        let right_tag = self
            .builder
            .build_load(tag_llvm, right_tag_address, "right_tag")
            .map_err(|error| format!("cannot compare variants: {error}"))?
            .into_int_value();
        let tags_equal = self
            .builder
            .build_int_compare(IntPredicate::EQ, left_tag, right_tag, "tags_equal")
            .map_err(|error| format!("cannot compare variants: {error}"))?;
        let mut equal = tags_equal;
        for (state_index, state) in states.iter().enumerate() {
            let Some(payload_type) = state.payload else {
                continue;
            };
            let offset = self.context.i64_type().const_int(payload_offset, false);
            let left_address = self.address_with_size(left_slot, 1, offset)?;
            let right_address = self.address_with_size(right_slot, 1, offset)?;
            let payload_llvm = self.llvm_type(payload_type)?;
            let left_payload = self
                .builder
                .build_load(payload_llvm, left_address, "left_payload")
                .map_err(|error| format!("cannot compare variants: {error}"))?;
            let right_payload = self
                .builder
                .build_load(payload_llvm, right_address, "right_payload")
                .map_err(|error| format!("cannot compare variants: {error}"))?;
            let payload_equal =
                self.build_comparison(BinaryOp::Equal, left_payload, right_payload, payload_type)?;
            let payload_equal = self.as_boolean_i1(payload_equal);
            let same_state = self
                .builder
                .build_int_compare(
                    IntPredicate::EQ,
                    left_tag,
                    tag_llvm.const_int(state_index as u64, false),
                    "same_state",
                )
                .map_err(|error| format!("cannot compare variants: {error}"))?;
            let state_equal = self
                .builder
                .build_and(tags_equal, payload_equal, "state_equal")
                .map_err(|error| format!("cannot compare variants: {error}"))?;
            equal = self
                .builder
                .build_select(same_state, state_equal, equal, "variant_equal")
                .map_err(|error| format!("cannot compare variants: {error}"))?
                .into_int_value();
        }
        let result = if negate {
            self.builder
                .build_not(equal, "variant_not_equal")
                .map_err(|error| format!("cannot compare variants: {error}"))?
        } else {
            equal
        };
        self.extend_bool(result)
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
                let kind = self.sema.type_defs[index as usize].kind;
                if kind == TypeDefKind::Variant {
                    return self.build_variant_comparison(op, left, right, ty, index);
                }
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

    fn vtable_slot(&self, receiver_type: TypeId, method: u32) -> Option<usize> {
        let Type::Named(index) = self.sema.types.get(receiver_type) else {
            return None;
        };
        let definition = &self.sema.type_defs[*index as usize];
        if definition.kind != TypeDefKind::Class {
            return None;
        }
        definition.vtable.iter().position(|entry| *entry == method)
    }

    fn gen_is_expr(
        &mut self,
        node: NodeId,
        is_expression: &IsExpr,
    ) -> Result<BasicValueEnum<'ctx>, String> {
        match &is_expression.target {
            IsTarget::Identifier { .. } => match self.sema.resolutions[node as usize].clone() {
                Some(Resolution::VariantState { type_id, state }) => {
                    let value = self.gen_expr(is_expression.value)?;
                    let value_llvm = self.llvm_type(self.type_of(is_expression.value))?;
                    let slot = self
                        .builder
                        .build_alloca(value_llvm, "is_variant")
                        .map_err(|error| format!("cannot allocate variant: {error}"))?;
                    self.builder
                        .build_store(slot, value)
                        .map_err(|error| format!("cannot store variant: {error}"))?;
                    let Type::Named(index) = self.sema.types.get(type_id).clone() else {
                        return Err("variant is-target requires a variant".to_string());
                    };
                    let tag_type = self.sema.type_defs[index as usize]
                        .tag_type
                        .ok_or_else(|| "variant has no tag type".to_string())?;
                    let tag_llvm = self.llvm_type(tag_type)?.into_int_type();
                    let tag_address =
                        self.address_with_size(slot, 1, self.context.i64_type().const_zero())?;
                    let tag = self
                        .builder
                        .build_load(tag_llvm, tag_address, "variant_tag")
                        .map_err(|error| format!("cannot load variant tag: {error}"))?
                        .into_int_value();
                    let condition = self
                        .builder
                        .build_int_compare(
                            IntPredicate::EQ,
                            tag,
                            tag_llvm.const_int(u64::from(state), false),
                            "is_state",
                        )
                        .map_err(|error| format!("cannot compare state: {error}"))?;
                    self.boolean_from_i1(condition)
                }
                Some(Resolution::Type(target_type)) => {
                    let Type::Named(target_index) = self.sema.types.get(target_type).clone() else {
                        return Err("is target must be a class".to_string());
                    };
                    let value = self.gen_expr(is_expression.value)?;
                    let instance = value.into_pointer_value();
                    let condition = self.emit_type_check(instance, target_type)?;
                    let _ = target_index;
                    self.boolean_from_i1(condition)
                }
                Some(Resolution::Constant { ty, value }) => {
                    let llvm_type = self.llvm_type(ty)?.into_int_type();
                    Ok(llvm_type.const_int(value as u64, false).into())
                }
                _ => Err("missing is target type".to_string()),
            },
            IsTarget::Illegal => {
                let value = self.gen_expr(is_expression.value)?.into_float_value();
                let is_nan = self
                    .builder
                    .build_float_compare(FloatPredicate::UNO, value, value, "is_illegal")
                    .map_err(|error| format!("cannot compare: {error}"))?;
                self.boolean_from_i1(is_nan)
            }
            IsTarget::Infinite { negative } => {
                let value = self.gen_expr(is_expression.value)?.into_float_value();
                let infinity = value.get_type().const_float(f64::INFINITY);
                let infinity = if *negative {
                    let zero = value.get_type().const_zero();
                    self.builder
                        .build_float_sub(zero, infinity, "negative_infinity")
                        .map_err(|error| format!("cannot build negative infinity: {error}"))?
                } else {
                    infinity
                };
                let is_infinite = self
                    .builder
                    .build_float_compare(FloatPredicate::OEQ, value, infinity, "is_infinite")
                    .map_err(|error| format!("cannot compare: {error}"))?;
                self.boolean_from_i1(is_infinite)
            }
        }
    }

    fn boolean_from_i1(&self, value: IntValue<'ctx>) -> Result<BasicValueEnum<'ctx>, String> {
        self.builder
            .build_int_z_extend(value, self.context.i8_type(), "is_bool")
            .map(Into::into)
            .map_err(|error| format!("cannot convert boolean: {error}"))
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

/// Rounds `value` up to a multiple of `alignment`.
fn round_up_u64(value: u64, alignment: u64) -> u64 {
    if alignment <= 1 {
        return value;
    }
    value.div_ceil(alignment) * alignment
}
