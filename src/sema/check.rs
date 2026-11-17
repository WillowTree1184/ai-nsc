//! Declaration resolution, layout computation and body/expression checking.

use super::types::TypeId;
use super::*;
use crate::ast::*;
use crate::literal;

impl<'a> Sema<'a> {
    // ==================================================================
    // Type resolution
    // ==================================================================

    pub(crate) fn resolve_type(&mut self, node: NodeId) -> Option<TypeId> {
        match self.node_kind(node).clone() {
            NodeKind::BasicType(basic) => self.resolve_basic_type(&basic),
            NodeKind::ArrayType(array) => {
                let mut dimensions = Vec::new();
                for dimension in &array.dimensions {
                    match dimension {
                        ArrayDimension::Size(expression) => {
                            let value = self.eval_const_int(*expression)?;
                            match u64::try_from(value) {
                                Ok(size) => dimensions.push(size),
                                Err(_) => {
                                    self.error(
                                        self.span_of(*expression),
                                        "array size must be a non-negative integer",
                                    );
                                    return None;
                                }
                            }
                        }
                        ArrayDimension::Binding { .. } => {
                            self.unsupported(
                                self.span_of(node),
                                "array dimension bindings (array parameter expansion)",
                            );
                            return None;
                        }
                    }
                }
                let element = self.resolve_type(array.element)?;
                Some(self.result.types.array(dimensions, element))
            }
            NodeKind::PointerType(target) => {
                let target = match target {
                    Some(target) => Some(self.resolve_type(target)?),
                    None => None,
                };
                Some(self.result.types.pointer(target))
            }
            NodeKind::FunctionType(function) => {
                let mut parameters = Vec::new();
                for parameter in &function.parameters {
                    match parameter.ty {
                        Some(ty) => parameters.push(self.resolve_type(ty)?),
                        None => {
                            self.error(parameter.span, "function type parameter requires a type");
                            return None;
                        }
                    }
                }
                let return_type = match function.return_type {
                    Some(ty) => Some(self.resolve_type(ty)?),
                    None => None,
                };
                Some(
                    self.result
                        .types
                        .function(parameters, function.wildcard, None, return_type),
                )
            }
            NodeKind::NamedType(named) => {
                let symbol = self.resolve_name_node(named.name)?;
                match symbol {
                    Symbol::Type(type_id) => {
                        if named.arguments.is_some() {
                            self.unsupported(
                                self.span_of(node),
                                "generic instantiation (monomorphization)",
                            );
                            return None;
                        }
                        Some(type_id)
                    }
                    _ => {
                        self.error(self.span_of(named.name), "expected a type name");
                        None
                    }
                }
            }
            _ => {
                self.error(self.span_of(node), "expected a type");
                None
            }
        }
    }

    fn resolve_basic_type(&mut self, basic: &BasicType) -> Option<TypeId> {
        let span = basic.size_span.unwrap_or_default();
        match basic.kind {
            BasicKind::Boolean => Some(TypeId::BOOLEAN),
            BasicKind::Integer | BasicKind::Cardinal => {
                let bytes = basic.size.unwrap_or(0);
                if !matches!(bytes, 1 | 2 | 4 | 8 | 16) {
                    self.error(
                        span,
                        format!(
                            "invalid {kind} size N = {bytes}; expected 1, 2, 4, 8 or 16",
                            kind = if basic.kind == BasicKind::Integer {
                                "integer"
                            } else {
                                "cardinal"
                            }
                        ),
                    );
                    return None;
                }
                Some(
                    self.result
                        .types
                        .integer(basic.kind == BasicKind::Integer, bytes as u8),
                )
            }
            BasicKind::Float => {
                let bytes = basic.size.unwrap_or(0);
                if !matches!(bytes, 4 | 8) {
                    self.error(
                        span,
                        format!("invalid float size N = {bytes}; expected 4 or 8"),
                    );
                    return None;
                }
                Some(self.result.types.float(bytes as u8))
            }
        }
    }

    /// Resolves a name expression (possibly `module.name` chains) to a symbol.
    fn resolve_name_node(&mut self, node: NodeId) -> Option<Symbol> {
        match self.node_kind(node).clone() {
            NodeKind::Name(name) => self.lookup(&name.name).cloned().or_else(|| {
                self.error(self.span_of(node), format!("unknown name '{}'", name.name));
                None
            }),
            NodeKind::MemberAccess(access) => {
                let base = self.resolve_name_node(access.base)?;
                match base {
                    Symbol::Module(scope) => {
                        self.lookup_in(scope, &access.name).cloned().or_else(|| {
                            self.error(
                                self.span_of(node),
                                format!("'{}' is not a member of the module", access.name),
                            );
                            None
                        })
                    }
                    _ => {
                        self.error(self.span_of(node), "expected a module name");
                        None
                    }
                }
            }
            _ => {
                self.error(self.span_of(node), "expected a name");
                None
            }
        }
    }

    pub(crate) fn spelling(&self, ty: TypeId) -> String {
        match self.result.types.get(ty) {
            Type::Named(index) => self.result.type_defs[*index as usize]
                .qualified_name
                .clone(),
            _ => self.result.types.spelling(ty),
        }
    }

    pub(crate) fn size_align(&self, ty: TypeId) -> (u64, u64) {
        self.result.size_align(ty)
    }

    // ==================================================================
    // Declaration resolution (pass 2)
    // ==================================================================

    pub(crate) fn resolve_structure_like(&mut self, node: NodeId, name: String, kind: TypeDefKind) {
        if kind == TypeDefKind::Class {
            self.unsupported(self.span_of(node), "class declarations");
            return;
        }

        let members = match self.node_kind(node) {
            NodeKind::Structure(declaration) => declaration.members.clone(),
            NodeKind::Class(declaration) => declaration.members.clone(),
            _ => Vec::new(),
        };

        let mut fields = Vec::new();
        let mut offset: u64 = 0;
        let mut max_align: u64 = 1;

        for member in members {
            match self.node_kind(member).clone() {
                NodeKind::Field(field) => {
                    let Some(ty_node) = field.ty else {
                        self.error(self.span_of(member), "field requires a type");
                        continue;
                    };
                    let Some(ty) = self.resolve_type(ty_node) else {
                        continue;
                    };
                    let (size, natural_align) = self.size_align(ty);
                    let alignment = match field.modifiers.aligned {
                        Some(0) => {
                            // `aligned(0)`: compact layout, no padding.
                            offset += size;
                            max_align = max_align.max(1);
                            fields.push(FieldInfo {
                                name: field.name,
                                ty,
                                offset: offset - size,
                            });
                            continue;
                        }
                        Some(requested) => u64::from(requested),
                        None => natural_align,
                    };
                    if alignment != 0 {
                        offset = round_up(offset, alignment);
                        max_align = max_align.max(alignment);
                    }
                    fields.push(FieldInfo {
                        name: field.name,
                        ty,
                        offset,
                    });
                    offset += size;
                }
                NodeKind::Extension(_) => {
                    self.unsupported(self.span_of(member), "extension declarations");
                }
                NodeKind::Accessor(_) => {
                    self.unsupported(self.span_of(member), "accessor declarations");
                }
                NodeKind::Operator(_) => {
                    self.unsupported(self.span_of(member), "operator declarations");
                }
                NodeKind::Module(_) => {}
                NodeKind::Class(_)
                | NodeKind::Structure(_)
                | NodeKind::Interface(_)
                | NodeKind::Enumeration(_)
                | NodeKind::Variant(_)
                | NodeKind::Union(_) => {
                    // Nested types are declared, but their bodies are resolved
                    // by the outer walk; nothing to do here.
                }
                _ => {
                    self.error(
                        self.span_of(member),
                        "only fields and extensions may appear directly in a structure body",
                    );
                }
            }
        }

        let size = round_up(offset, max_align);
        let index = self.type_def_index(node).expect("declared type");
        let definition = &mut self.result.type_defs[index as usize];
        definition.fields = fields;
        definition.size = size;
        definition.align = max_align;
        let _ = name;
    }

    fn type_def_index(&self, node: NodeId) -> Option<TypeDefId> {
        self.result
            .type_defs
            .iter()
            .position(|definition| definition.node == node)
            .map(|index| index as TypeDefId)
    }

    pub(crate) fn resolve_enumeration(&mut self, node: NodeId, _name: String) {
        let NodeKind::Enumeration(declaration) = self.node_kind(node).clone() else {
            return;
        };
        let Some(underlying) = self.resolve_type(declaration.underlying) else {
            return;
        };
        let underlying_type = self.result.types.get(underlying).clone();
        let (signed, bytes) = match underlying_type {
            Type::Integer { signed, bytes } => (signed, bytes),
            _ => {
                self.error(
                    self.span_of(declaration.underlying),
                    "enumeration underlying type must be an integer or cardinal type",
                );
                return;
            }
        };

        let mut members = Vec::new();
        let mut next_value: i128 = 0;
        for member in &declaration.members {
            let value = match member.value {
                Some(expression) => match self.eval_const_int(expression) {
                    Some(value) => value,
                    None => continue,
                },
                None => next_value,
            };
            next_value = value.wrapping_add(1);
            if !fits_integer(value, signed, bytes) {
                self.error(
                    member.span,
                    "enumeration member value does not fit the underlying type",
                );
                continue;
            }
            members.push((member.name.clone(), value));
        }

        let size = u64::from(bytes);
        let index = self.type_def_index(node).expect("declared type");
        let definition = &mut self.result.type_defs[index as usize];
        definition.enum_members = members;
        definition.underlying = Some(underlying);
        definition.size = size;
        definition.align = size;
    }

    pub(crate) fn resolve_function_signature(&mut self, node: NodeId, declaration: &FixedDecl) {
        let Some(ty_node) = declaration.ty else {
            // Not a function definition: handled as a global or local constant.
            self.resolve_global_fixed(node, declaration);
            return;
        };
        let NodeKind::FunctionType(function) = self.node_kind(ty_node).clone() else {
            self.resolve_global_fixed(node, declaration);
            return;
        };
        let Some(body) = declaration.init.and_then(|init| match init {
            Initializer::Block(block) => Some(block),
            Initializer::Expression(_) => None,
        }) else {
            self.unsupported(self.span_of(node), "function-valued constants (lambdas)");
            return;
        };

        let Some(function_type) = self.resolve_type(ty_node) else {
            return;
        };
        let mut parameters = Vec::new();
        for (index, parameter) in function.parameters.iter().enumerate() {
            let Some(ty) = parameter.ty else {
                continue;
            };
            let Some(ty) = self.resolve_type(ty) else {
                continue;
            };
            let name = parameter
                .name
                .clone()
                .unwrap_or_else(|| format!("argument{index}"));
            parameters.push((name, ty));
        }
        let return_type = function.return_type.and_then(|ty| self.resolve_type(ty));

        let scope = self.scope_path.clone();
        let parameter_spellings: Vec<String> = parameters
            .iter()
            .map(|(_, ty)| self.spelling(*ty))
            .collect();
        let mangled = mangle_function(&scope, &declaration.name, &parameter_spellings);
        let index = self.result.functions.len() as u32;
        self.result.functions.push(FunctionInfo {
            name: declaration.name.clone(),
            scope,
            node,
            body,
            parameters,
            return_type,
            mangled,
            locals: Vec::new(),
            span: self.span_of(node),
        });
        let _ = function_type;
        self.declare_functions(declaration.name.clone(), index);
    }

    pub(crate) fn resolve_linkable_signature(&mut self, node: NodeId, declaration: &LinkableDecl) {
        let Some(ty_node) = declaration.ty else {
            self.unsupported(self.span_of(node), "linkable declarations without a type");
            return;
        };
        let NodeKind::FunctionType(function) = self.node_kind(ty_node).clone() else {
            self.unsupported(self.span_of(node), "linkable variables");
            return;
        };
        let Some(body) = declaration.init.and_then(|init| match init {
            Initializer::Block(block) => Some(block),
            Initializer::Expression(_) => None,
        }) else {
            self.unsupported(self.span_of(node), "linkable declarations without a body");
            return;
        };
        let mut parameters = Vec::new();
        for (index, parameter) in function.parameters.iter().enumerate() {
            let Some(ty) = parameter.ty else {
                continue;
            };
            let Some(ty) = self.resolve_type(ty) else {
                continue;
            };
            let name = parameter
                .name
                .clone()
                .unwrap_or_else(|| format!("argument{index}"));
            parameters.push((name, ty));
        }
        let return_type = function.return_type.and_then(|ty| self.resolve_type(ty));
        let mangled = declaration
            .symbol
            .clone()
            .unwrap_or_else(|| declaration.name.clone());
        let index = self.result.functions.len() as u32;
        self.result.functions.push(FunctionInfo {
            name: declaration.name.clone(),
            scope: self.scope_path.clone(),
            node,
            body,
            parameters,
            return_type,
            mangled,
            locals: Vec::new(),
            span: self.span_of(node),
        });
        self.declare_functions(declaration.name.clone(), index);
    }

    fn resolve_global_fixed(&mut self, node: NodeId, declaration: &FixedDecl) {
        let Some(ty_node) = declaration.ty else {
            self.error(
                self.span_of(node),
                "a global constant requires a type or a function type",
            );
            return;
        };
        let Some(ty) = self.resolve_type(ty_node) else {
            return;
        };
        let init = declaration.init.and_then(|init| match init {
            Initializer::Expression(expression) => Some(expression),
            Initializer::Block(_) => None,
        });
        let scope = self.scope_path.clone();
        let mangled = mangle_function(&scope, &declaration.name, &[self.spelling(ty)]);
        let index = self.result.globals.len() as u32;
        self.result.globals.push(GlobalInfo {
            name: declaration.name.clone(),
            scope,
            ty,
            init,
            fixed: true,
            mangled,
            node,
            span: self.span_of(node),
        });
        self.declare(
            declaration.name.clone(),
            Symbol::Global(index),
            self.span_of(node),
        );
    }

    pub(crate) fn resolve_global_local(&mut self, node: NodeId) {
        let NodeKind::Local(declaration) = self.node_kind(node).clone() else {
            return;
        };
        for item in &declaration.items {
            let ty = match item.ty {
                Some(ty_node) => self.resolve_type(ty_node),
                None => match item.init {
                    Some(initializer) => {
                        // Inference is performed during initializer checking;
                        // the type is filled in then. Use the error type for now.
                        let _ = initializer;
                        Some(TypeId::ERROR)
                    }
                    None => {
                        self.error(
                            item.span,
                            "local variable requires a type or an initializer",
                        );
                        continue;
                    }
                },
            };
            let Some(ty) = ty else {
                continue;
            };
            let scope = self.scope_path.clone();
            let mangled = mangle_function(&scope, &item.name, &[self.spelling(ty)]);
            let index = self.result.globals.len() as u32;
            self.result.globals.push(GlobalInfo {
                name: item.name.clone(),
                scope,
                ty,
                init: item.init,
                fixed: false,
                mangled,
                node,
                span: item.span,
            });
            self.declare(item.name.clone(), Symbol::Global(index), item.span);
        }
    }

    // ==================================================================
    // Constant evaluation (array sizes, literal typing helpers)
    // ==================================================================

    pub(crate) fn eval_const_int(&mut self, node: NodeId) -> Option<i128> {
        match self.node_kind(node).clone() {
            NodeKind::IntegerLiteral(literal) => match literal::decode_integer(&literal.raw) {
                Ok(value) => {
                    if value > i128::MAX as u128 {
                        self.error(self.span_of(node), "integer literal is too large");
                        return None;
                    }
                    Some(value as i128)
                }
                Err(message) => {
                    self.error(self.span_of(node), message);
                    None
                }
            },
            NodeKind::Unary(unary) if unary.op == UnaryOp::Negate => {
                self.eval_const_int(unary.operand).map(i128::wrapping_neg)
            }
            NodeKind::Binary(binary) => {
                let left = self.eval_const_int(binary.left)?;
                let right = self.eval_const_int(binary.right)?;
                Some(match binary.op {
                    BinaryOp::Add => left.wrapping_add(right),
                    BinaryOp::Subtract => left.wrapping_sub(right),
                    BinaryOp::Multiply => left.wrapping_mul(right),
                    BinaryOp::Divide => {
                        if right == 0 {
                            self.error(
                                self.span_of(node),
                                "division by zero in constant expression",
                            );
                            return None;
                        }
                        left.wrapping_div(right)
                    }
                    BinaryOp::Remainder => {
                        if right == 0 {
                            self.error(
                                self.span_of(node),
                                "remainder by zero in constant expression",
                            );
                            return None;
                        }
                        left.wrapping_rem(right)
                    }
                    BinaryOp::ShiftLeft => left.wrapping_shl(right as u32),
                    BinaryOp::ShiftRight => left.wrapping_shr(right as u32),
                    _ => {
                        self.error(
                            self.span_of(node),
                            "constant expression must be an integer expression",
                        );
                        return None;
                    }
                })
            }
            NodeKind::Name(name) => match self.lookup(&name.name).cloned() {
                Some(Symbol::Local(_)) | Some(Symbol::Functions(_)) => {
                    self.error(
                        self.span_of(node),
                        "constant expression cannot use a local variable",
                    );
                    None
                }
                Some(Symbol::Global(global)) => {
                    let info = &self.result.globals[global as usize];
                    match info.init {
                        Some(initializer) => self.eval_const_int(initializer),
                        None => {
                            self.error(
                                self.span_of(node),
                                "constant expression uses an uninitialized global",
                            );
                            None
                        }
                    }
                }
                _ => {
                    self.error(
                        self.span_of(node),
                        "constant expression must be an integer expression",
                    );
                    None
                }
            },
            _ => {
                self.error(
                    self.span_of(node),
                    "constant expression must be an integer expression",
                );
                None
            }
        }
    }
}

fn round_up(value: u64, alignment: u64) -> u64 {
    if alignment <= 1 {
        return value;
    }
    value.div_ceil(alignment) * alignment
}

pub(crate) fn fits_integer(value: i128, signed: bool, bytes: u8) -> bool {
    if signed {
        let bits = u32::from(bytes) * 8;
        if bits == 128 {
            return true;
        }
        let max = (1i128 << (bits - 1)) - 1;
        let min = -(1i128 << (bits - 1));
        value >= min && value <= max
    } else {
        let bits = u32::from(bytes) * 8;
        if bits >= 128 {
            return value >= 0;
        }
        value >= 0 && (value as u128) < (1u128 << bits)
    }
}

// ======================================================================
// Pass 3: bodies, statements, expressions
// ======================================================================

impl<'a> Sema<'a> {
    pub(crate) fn check_function_bodies(&mut self) {
        for index in 0..self.result.functions.len() {
            let (body, parameters, span) = {
                let function = &self.result.functions[index];
                (function.body, function.parameters.clone(), function.span)
            };
            self.current_function = Some(index as u32);
            self.loop_labels.clear();
            self.push_scope();
            for (parameter_index, (name, ty)) in parameters.iter().enumerate() {
                self.add_local(name.clone(), *ty, false, Some(parameter_index as u32), span);
            }
            self.check_block(body);
            self.pop_scope();
            self.current_function = None;
        }
    }

    pub(crate) fn check_global_initializers(&mut self) {
        for index in 0..self.result.globals.len() {
            let (init, ty, span) = {
                let global = &self.result.globals[index];
                (global.init, global.ty, global.span)
            };
            if let Some(initializer) = init {
                if ty == TypeId::ERROR {
                    continue;
                }
                let inferred = self.check_expr(initializer, Some(ty));
                if let Some(inferred) = inferred {
                    if !self.is_assignable(inferred, ty) {
                        self.error(
                            span,
                            format!(
                                "initializer of type '{}' is not assignable to '{}'",
                                self.spelling(inferred),
                                self.spelling(ty)
                            ),
                        );
                    }
                }
            }
        }
    }

    // -- helpers -----------------------------------------------------------

    fn current_function_index(&self) -> u32 {
        self.current_function
            .expect("only called while checking a function")
    }

    fn add_local(
        &mut self,
        name: String,
        ty: TypeId,
        fixed: bool,
        parameter: Option<u32>,
        span: Span,
    ) -> LocalId {
        let function_index = self.current_function_index();
        let locals = &mut self.result.functions[function_index as usize].locals;
        let id = locals.len() as LocalId;
        locals.push(LocalInfo {
            name: name.clone(),
            ty,
            fixed,
            parameter,
            span,
        });
        self.declare(name, Symbol::Local(id), span);
        id
    }

    fn local_info(&self, local: LocalId) -> Option<&LocalInfo> {
        let function = self.current_function?;
        self.result.functions[function as usize]
            .locals
            .get(local as usize)
    }

    fn type_of(&self, ty: TypeId) -> Type {
        self.result.types.get(ty).clone()
    }

    fn is_integer(&self, ty: TypeId) -> bool {
        matches!(self.result.types.get(ty), Type::Integer { .. })
    }

    fn is_numeric(&self, ty: TypeId) -> bool {
        matches!(
            self.result.types.get(ty),
            Type::Integer { .. } | Type::Float { .. }
        )
    }

    fn is_boolean(&self, ty: TypeId) -> bool {
        matches!(self.result.types.get(ty), Type::Boolean)
    }

    fn is_assignable(&self, from: TypeId, to: TypeId) -> bool {
        if from == to {
            return true;
        }
        // Enumeration values convert implicitly to their underlying type (§3.7.4).
        if let (Type::Named(index), _) = (self.type_of(from), self.type_of(to)) {
            if let Some(underlying) = self.result.type_defs[index as usize].underlying {
                return underlying == to;
            }
        }
        // Values convert implicitly to `boolean`? No: §3.2 forbids basic
        // implicit conversions.
        // Class/interface subtyping would go here; classes are not implemented yet.
        false
    }

    fn expect_assignable(&mut self, span: Span, from: TypeId, to: TypeId) {
        if from == TypeId::ERROR || to == TypeId::ERROR {
            return;
        }
        if !self.is_assignable(from, to) {
            self.error(
                span,
                format!(
                    "expected '{}', found '{}'",
                    self.spelling(to),
                    self.spelling(from)
                ),
            );
        }
    }

    // -- statements --------------------------------------------------------

    pub(crate) fn check_block(&mut self, node: NodeId) {
        let statements = match self.node_kind(node) {
            NodeKind::Block(statements) => statements,
            NodeKind::VerbatimBlock(block) => {
                self.check_block(block);
                return;
            }
            _ => {
                self.error(self.span_of(node), "expected a block");
                return;
            }
        };
        self.push_scope();
        for statement in statements {
            self.check_statement(statement);
        }
        self.pop_scope();
    }

    fn check_statement(&mut self, node: NodeId) {
        match self.node_kind(node).clone() {
            NodeKind::Block(_) | NodeKind::VerbatimBlock(_) => self.check_block(node),
            NodeKind::Local(declaration) => self.check_local_statement(node, &declaration),
            NodeKind::Fixed(declaration) => self.check_fixed_statement(node, &declaration),
            NodeKind::Set(statement) => {
                let target_type = self.check_expr(statement.target, None);
                self.ensure_assignable(statement.target);
                if let Some(target_type) = target_type {
                    let value_type = self.check_expr(statement.value, Some(target_type));
                    if let Some(value_type) = value_type {
                        self.expect_assignable(
                            self.span_of(statement.value),
                            value_type,
                            target_type,
                        );
                    }
                }
            }
            NodeKind::CompoundSet(statement) => {
                let target_type = self.check_expr(statement.target, None);
                self.ensure_assignable(statement.target);
                let Some(target_type) = target_type else {
                    return;
                };
                let value_type = self.check_expr(statement.value, Some(target_type));
                if let Some(value_type) = value_type {
                    self.expect_assignable(self.span_of(statement.value), value_type, target_type);
                }
                if !self.compound_operator_allowed(statement.op, target_type) {
                    self.error(
                        self.span_of(node),
                        format!(
                            "compound assignment is not defined for '{}'",
                            self.spelling(target_type)
                        ),
                    );
                }
            }
            NodeKind::If(statement) => {
                for branch in &statement.branches {
                    let condition = self.check_expr(branch.condition, Some(TypeId::BOOLEAN));
                    if let Some(condition) = condition {
                        self.expect_assignable(branch.span, condition, TypeId::BOOLEAN);
                    }
                    self.check_block(branch.body);
                }
                if let Some(else_block) = statement.else_block {
                    self.check_block(else_block);
                }
            }
            NodeKind::While(loop_statement) => {
                self.check_condition(loop_statement.condition);
                self.loop_labels.push(loop_statement.label.clone());
                self.check_block(loop_statement.body);
                self.loop_labels.pop();
            }
            NodeKind::DoWhile(loop_statement) => {
                self.loop_labels.push(loop_statement.label.clone());
                self.check_block(loop_statement.body);
                self.loop_labels.pop();
                self.check_condition(loop_statement.condition);
            }
            NodeKind::For(for_statement) => self.check_for_statement(node, &for_statement),
            NodeKind::Match(match_statement) => self.check_match_statement(&match_statement),
            NodeKind::Break(statement) => {
                if self.loop_labels.is_empty() {
                    self.error(self.span_of(node), "'break' is only allowed inside a loop");
                } else if let Some(label) = &statement.label {
                    if !self.loop_labels.iter().flatten().any(|name| name == label) {
                        self.error(self.span_of(node), format!("unknown loop label '{label}'"));
                    }
                }
            }
            NodeKind::Continue(statement) => {
                if self.loop_labels.is_empty() {
                    self.error(
                        self.span_of(node),
                        "'continue' is only allowed inside a loop",
                    );
                } else if let Some(label) = &statement.label {
                    if !self.loop_labels.iter().flatten().any(|name| name == label) {
                        self.error(self.span_of(node), format!("unknown loop label '{label}'"));
                    }
                }
            }
            NodeKind::Return(statement) => {
                let return_type = self
                    .current_function
                    .map(|index| self.result.functions[index as usize].return_type);
                match (statement.value, return_type.flatten()) {
                    (Some(value), Some(expected)) => {
                        let value_type = self.check_expr(value, Some(expected));
                        if let Some(value_type) = value_type {
                            self.expect_assignable(self.span_of(value), value_type, expected);
                        }
                    }
                    (Some(value), None) => {
                        self.check_expr(value, None);
                        self.error(
                            self.span_of(node),
                            "a function with the empty return type cannot return a value",
                        );
                    }
                    (None, Some(expected)) => {
                        self.error(
                            self.span_of(node),
                            format!(
                                "function must return a value of type '{}'",
                                self.spelling(expected)
                            ),
                        );
                    }
                    (None, None) => {}
                }
            }
            NodeKind::ExpressionStmt(expression) => {
                self.check_expr(expression, None);
                if !matches!(self.node_kind(expression), NodeKind::Call(_)) {
                    self.error(
                        self.span_of(node),
                        "only a function call may be used as a statement",
                    );
                }
            }
            NodeKind::Use(statement) => {
                for target in &statement.targets {
                    self.check_expr(*target, None);
                }
            }
            NodeKind::CompileTimeBlock(block) => self.check_block(block),
            NodeKind::CompileTimeIf(statement) => {
                self.check_expr(statement.condition, None);
                self.check_block(statement.then_block);
                if let Some(else_block) = statement.else_block {
                    self.check_block(else_block);
                }
            }
            NodeKind::Throw(_) => self.unsupported(self.span_of(node), "throw statements"),
            NodeKind::Trap(_) => self.unsupported(self.span_of(node), "trap statements"),
            NodeKind::Dispose(_) => self.unsupported(self.span_of(node), "dispose statements"),
            NodeKind::Heap(_) => self.unsupported(self.span_of(node), "heap definitions"),
            NodeKind::Class(_)
            | NodeKind::Structure(_)
            | NodeKind::Interface(_)
            | NodeKind::Enumeration(_)
            | NodeKind::Variant(_)
            | NodeKind::Union(_) => {
                self.unsupported(self.span_of(node), "nested type declarations")
            }
            NodeKind::Extension(_) => {
                self.unsupported(self.span_of(node), "extension declarations")
            }
            NodeKind::Accessor(_) => self.unsupported(self.span_of(node), "accessor declarations"),
            NodeKind::Operator(_) => self.unsupported(self.span_of(node), "operator declarations"),
            NodeKind::Field(_) => self.error(
                self.span_of(node),
                "a field may only be declared in a type body",
            ),
            NodeKind::Module(_) => self.unsupported(self.span_of(node), "nested modules"),
            NodeKind::Linkable(_) => self.unsupported(
                self.span_of(node),
                "linkable declarations in a function body",
            ),
            _ => self.error(self.span_of(node), "unsupported statement"),
        }
    }

    fn compound_operator_allowed(&self, op: BinaryOp, ty: TypeId) -> bool {
        match op {
            BinaryOp::Add
            | BinaryOp::Subtract
            | BinaryOp::Multiply
            | BinaryOp::Divide
            | BinaryOp::Remainder => self.is_numeric(ty),
            BinaryOp::ShiftLeft | BinaryOp::ShiftRight => self.is_integer(ty),
            BinaryOp::BitwiseAnd | BinaryOp::BitwiseOr | BinaryOp::BitwiseXor => {
                self.is_integer(ty)
            }
            _ => false,
        }
    }

    fn check_condition(&mut self, condition: NodeId) {
        let ty = self.check_expr(condition, Some(TypeId::BOOLEAN));
        if let Some(ty) = ty {
            self.expect_assignable(self.span_of(condition), ty, TypeId::BOOLEAN);
        }
    }

    fn check_local_statement(&mut self, node: NodeId, declaration: &LocalDecl) {
        let mut locals = Vec::new();
        for item in &declaration.items {
            let declared_type = item.ty.and_then(|ty| self.resolve_type(ty));
            let ty = match (declared_type, item.init) {
                (Some(ty), Some(initializer)) => {
                    let inferred = self.check_expr(initializer, Some(ty));
                    if let Some(inferred) = inferred {
                        self.expect_assignable(item.span, inferred, ty);
                    }
                    ty
                }
                (Some(ty), None) => ty,
                (None, Some(initializer)) => match self.check_expr(initializer, None) {
                    Some(ty) => ty,
                    None => TypeId::ERROR,
                },
                (None, None) => {
                    self.error(
                        item.span,
                        "local variable requires a type or an initializer",
                    );
                    continue;
                }
            };
            let local = self.add_local(item.name.clone(), ty, false, None, item.span);
            locals.push(local);
        }
        self.result.local_declarations.insert(node, locals);
    }

    fn check_fixed_statement(&mut self, node: NodeId, declaration: &FixedDecl) {
        let declared_type = declaration.ty.and_then(|ty| self.resolve_type(ty));
        if let Some(ty) = declared_type {
            if matches!(self.type_of(ty), Type::Function { .. }) {
                self.unsupported(self.span_of(node), "nested function definitions (closures)");
                return;
            }
            if let Some(initializer) = declaration.init {
                match initializer {
                    Initializer::Expression(expression) => {
                        let inferred = self.check_expr(expression, Some(ty));
                        if let Some(inferred) = inferred {
                            self.expect_assignable(self.span_of(node), inferred, ty);
                        }
                    }
                    Initializer::Block(_) => {
                        self.error(
                            self.span_of(node),
                            "constant requires an expression initializer",
                        );
                    }
                }
            }
            let local =
                self.add_local(declaration.name.clone(), ty, true, None, self.span_of(node));
            self.result.local_declarations.insert(node, vec![local]);
        } else if let Some(Initializer::Expression(expression)) = declaration.init {
            let ty = self.check_expr(expression, None).unwrap_or(TypeId::ERROR);
            let local =
                self.add_local(declaration.name.clone(), ty, true, None, self.span_of(node));
            self.result.local_declarations.insert(node, vec![local]);
        } else {
            self.error(
                self.span_of(node),
                "constant requires a type or an initializer",
            );
        }
    }

    fn ensure_assignable(&mut self, node: NodeId) {
        match self.node_kind(node).clone() {
            NodeKind::Name(name) => match self.lookup(&name.name).cloned() {
                Some(Symbol::Local(local)) => {
                    if self.local_info(local).is_some_and(|info| info.fixed) {
                        self.error(self.span_of(node), "cannot assign to a constant");
                    }
                }
                Some(Symbol::Global(global)) => {
                    if self.result.globals[global as usize].fixed {
                        self.error(self.span_of(node), "cannot assign to a constant");
                    }
                }
                _ => self.error(self.span_of(node), "expression is not assignable"),
            },
            NodeKind::Index(_) | NodeKind::MemberAccess(_) | NodeKind::PointerMemberAccess(_) => {}
            _ => self.error(self.span_of(node), "expression is not assignable"),
        }
    }

    fn check_for_statement(&mut self, node: NodeId, statement: &ForStmt) {
        let binding_type = self.resolve_type(statement.ty).unwrap_or(TypeId::ERROR);
        if statement.by_ref {
            self.unsupported(self.span_of(node), "by-reference for bindings");
            return;
        }
        match &statement.target {
            ForTarget::Range(range) => {
                if !(self.is_integer(binding_type) || self.is_numeric(binding_type)) {
                    self.error(
                        self.span_of(node),
                        "for range requires an integer or cardinal binding type",
                    );
                }
                for expression in [Some(range.start), range.step, Some(range.end)]
                    .into_iter()
                    .flatten()
                {
                    let ty = self.check_expr(expression, Some(binding_type));
                    if let Some(ty) = ty {
                        self.expect_assignable(self.span_of(expression), ty, binding_type);
                    }
                }
            }
            ForTarget::Expression(target) => {
                let target_type = self.check_expr(*target, None);
                if let Some(target_type) = target_type {
                    match self.type_of(target_type) {
                        Type::Array {
                            dimensions,
                            element,
                        } => {
                            let item_type = if dimensions.len() > 1 {
                                let mut rest = dimensions.clone();
                                rest.remove(0);
                                self.result.types.array(rest, element)
                            } else {
                                element
                            };
                            self.expect_assignable(self.span_of(node), item_type, binding_type);
                        }
                        _ => {
                            self.unsupported(self.span_of(node), "iteration over non-array values");
                        }
                    }
                }
            }
        }

        self.push_scope();
        let binding_local = self.add_local(
            statement.name.clone(),
            binding_type,
            false,
            None,
            self.span_of(node),
        );
        self.result.loop_bindings.insert(node, binding_local);
        self.result.node_types[node as usize] = Some(binding_type);
        self.loop_labels.push(statement.label.clone());
        self.check_block(statement.body);
        self.loop_labels.pop();
        self.pop_scope();
    }

    fn check_match_statement(&mut self, statement: &MatchStmt) {
        let value_type = self.check_expr(statement.value, None);
        self.loop_labels.push(statement.label.clone());
        for case in &statement.cases {
            if let Some(pattern) = &case.pattern {
                match pattern {
                    Pattern::Equal(expression) => {
                        if let Some(value_type) = value_type {
                            let ty = self.check_expr(*expression, Some(value_type));
                            if let Some(ty) = ty {
                                self.expect_assignable(self.span_of(*expression), ty, value_type);
                            }
                        } else {
                            self.check_expr(*expression, None);
                        }
                    }
                    Pattern::Comparison { value, .. } => {
                        let ty = self.check_expr(*value, value_type);
                        if let Some(value_type) = value_type {
                            if !self.is_numeric(value_type) {
                                self.error(
                                    self.span_of(*value),
                                    "comparison patterns require a numeric value type",
                                );
                            }
                        }
                        if let (Some(ty), Some(value_type)) = (ty, value_type) {
                            self.expect_assignable(self.span_of(*value), ty, value_type);
                        }
                    }
                    Pattern::Is { .. } => {
                        self.unsupported(self.span_of(statement.value), "is patterns")
                    }
                    Pattern::Wildcard => {}
                }
            }
            if let Some(guard) = case.guard {
                self.check_condition(guard);
            }
            self.check_block(case.body);
        }
        self.loop_labels.pop();
    }
}

// ======================================================================
// Expressions
// ======================================================================

impl<'a> Sema<'a> {
    pub(crate) fn check_expr(&mut self, node: NodeId, expected: Option<TypeId>) -> Option<TypeId> {
        let ty = self.check_expr_inner(node, expected);
        if let Some(ty) = ty {
            self.result.node_types[node as usize] = Some(ty);
        }
        ty
    }

    fn check_expr_inner(&mut self, node: NodeId, expected: Option<TypeId>) -> Option<TypeId> {
        match self.node_kind(node).clone() {
            NodeKind::IntegerLiteral(literal) => {
                self.type_integer_literal(node, &literal.raw, expected)
            }
            NodeKind::FloatLiteral(literal) => {
                self.type_float_literal(node, &literal.raw, expected)
            }
            NodeKind::CharacterLiteral(literal) => {
                self.type_character_literal(node, &literal.raw, expected)
            }
            NodeKind::StringLiteral(literal) | NodeKind::VerbatimStringLiteral(literal) => {
                self.type_string_literal(node, &literal.raw)
            }
            NodeKind::BooleanLiteral(_) => Some(TypeId::BOOLEAN),
            NodeKind::InfiniteLiteral | NodeKind::IllegalLiteral => match expected {
                Some(ty) if matches!(self.type_of(ty), Type::Float { .. }) => Some(ty),
                _ => {
                    self.error(
                        self.span_of(node),
                        "this literal requires a floating-point type from context",
                    );
                    None
                }
            },
            NodeKind::UninitializedLiteral => match expected {
                Some(ty) => Some(ty),
                None => {
                    self.error(
                        self.span_of(node),
                        "uninitialized requires a type from context",
                    );
                    None
                }
            },
            NodeKind::Name(name) => self.check_name_expr(node, &name),
            NodeKind::MemberAccess(access) => self.check_member_access(node, &access, false),
            NodeKind::PointerMemberAccess(access) => self.check_member_access(node, &access, true),
            NodeKind::Index(index) => self.check_index_expr(node, &index),
            NodeKind::Call(call) => self.check_call(node, &call),
            NodeKind::Cast(cast) => self.check_cast(node, &cast),
            NodeKind::Unary(unary) => self.check_unary(node, &unary, expected),
            NodeKind::Binary(binary) => self.check_binary(node, &binary, expected),
            NodeKind::Conditional(conditional) => {
                self.check_condition(conditional.condition);
                let then_type = self.check_expr(conditional.then_value, expected);
                if let (Some(then_type), Some(expected)) = (then_type, expected) {
                    self.expect_assignable(
                        self.span_of(conditional.then_value),
                        then_type,
                        expected,
                    );
                }
                let else_type = self.check_expr(conditional.else_value, then_type.or(expected));
                if let (Some(then_type), Some(else_type)) = (then_type, else_type) {
                    self.expect_assignable(
                        self.span_of(conditional.else_value),
                        else_type,
                        then_type,
                    );
                }
                then_type
            }
            NodeKind::ArrayLiteral(literal) => self.check_array_literal(node, &literal, expected),
            NodeKind::CompileTimeExpr(inner) => self.check_expr(inner, expected),
            NodeKind::Is(_) => {
                self.unsupported(self.span_of(node), "is expressions");
                None
            }
            NodeKind::Coalesce(_) => {
                self.unsupported(self.span_of(node), "null coalescing expressions");
                None
            }
            NodeKind::FunctionLiteral(_) => {
                self.unsupported(self.span_of(node), "function literals (lambdas)");
                None
            }
            NodeKind::Monomorphization(_) => {
                self.unsupported(self.span_of(node), "monomorphization expressions");
                None
            }
            NodeKind::This | NodeKind::Owner | NodeKind::Base => {
                self.unsupported(self.span_of(node), "this/owner/base expressions");
                None
            }
            NodeKind::Construct(_) | NodeKind::MemberConstruct(_) => {
                self.unsupported(self.span_of(node), "construction expressions");
                None
            }
            _ => {
                self.error(self.span_of(node), "expected an expression");
                None
            }
        }
    }

    // -- literal typing ----------------------------------------------------

    fn type_integer_literal(
        &mut self,
        node: NodeId,
        raw: &str,
        expected: Option<TypeId>,
    ) -> Option<TypeId> {
        let value = match literal::decode_integer(raw) {
            Ok(value) => value,
            Err(message) => {
                self.error(self.span_of(node), message);
                return None;
            }
        };
        if let Some(expected) = expected {
            match self.type_of(expected) {
                Type::Integer { signed, bytes } => {
                    let fits = if signed {
                        value <= i128::MAX as u128 && fits_integer(value as i128, true, bytes)
                    } else {
                        fits_unsigned(value, bytes)
                    };
                    if !fits {
                        self.error(
                            self.span_of(node),
                            format!("integer literal does not fit '{}'", self.spelling(expected)),
                        );
                        return None;
                    }
                    return Some(expected);
                }
                Type::Float { bytes } => {
                    let float = value as f64;
                    if bytes == 4 && !fits_f32(float) {
                        self.error(
                            self.span_of(node),
                            format!(
                                "integer literal cannot be represented by '{}'",
                                self.spelling(expected)
                            ),
                        );
                        return None;
                    }
                    return Some(expected);
                }
                _ => {
                    self.error(
                        self.span_of(node),
                        format!(
                            "integer literal is not compatible with '{}'",
                            self.spelling(expected)
                        ),
                    );
                    return None;
                }
            }
        }
        // Default: the smallest signed integer type that can represent it.
        for bytes in [1u8, 2, 4, 8, 16] {
            if value <= i128::MAX as u128 && fits_integer(value as i128, true, bytes) {
                return Some(self.result.types.integer(true, bytes));
            }
        }
        self.error(
            self.span_of(node),
            "integer literal is too large for any integer type",
        );
        None
    }

    fn type_float_literal(
        &mut self,
        node: NodeId,
        raw: &str,
        expected: Option<TypeId>,
    ) -> Option<TypeId> {
        let value = match literal::decode_float(raw) {
            Ok(value) => value,
            Err(message) => {
                self.error(self.span_of(node), message);
                return None;
            }
        };
        if let Some(expected) = expected {
            if let Type::Float { bytes } = self.type_of(expected) {
                if bytes == 4 && !fits_f32(value) {
                    self.error(
                        self.span_of(node),
                        format!(
                            "float literal cannot be represented by '{}'",
                            self.spelling(expected)
                        ),
                    );
                    return None;
                }
                return Some(expected);
            }
            self.error(
                self.span_of(node),
                format!(
                    "float literal is not compatible with '{}'",
                    self.spelling(expected)
                ),
            );
            return None;
        }
        if fits_f32(value) {
            Some(self.result.types.float(4))
        } else {
            Some(self.result.types.float(8))
        }
    }

    fn type_character_literal(
        &mut self,
        node: NodeId,
        raw: &str,
        expected: Option<TypeId>,
    ) -> Option<TypeId> {
        let decoded = match literal::decode_character(raw) {
            Ok(decoded) => decoded,
            Err(message) => {
                self.error(self.span_of(node), message);
                return None;
            }
        };
        let value = u32::from(decoded.character);
        if let Some(expected) = expected {
            if let Type::Integer {
                signed: false,
                bytes,
            } = self.type_of(expected)
            {
                let max = match bytes {
                    1 => 0x7F,
                    2 => 0xFFFF,
                    _ => u32::MAX,
                };
                if decoded.encoding == Some(literal::Encoding::Utf16)
                    && decoded.unit_size == 2
                    && value > 0xFFFF
                {
                    self.error(self.span_of(node), "character does not fit UTF-16");
                    return None;
                }
                if value > max {
                    self.error(
                        self.span_of(node),
                        format!(
                            "character literal does not fit '{}'",
                            self.spelling(expected)
                        ),
                    );
                    return None;
                }
                return Some(expected);
            }
            self.error(
                self.span_of(node),
                format!(
                    "character literal is not compatible with '{}'",
                    self.spelling(expected)
                ),
            );
            return None;
        }
        Some(self.result.types.integer(false, decoded.unit_size as u8))
    }

    fn type_string_literal(&mut self, node: NodeId, raw: &str) -> Option<TypeId> {
        let decoded = match literal::decode_string(raw) {
            Ok(decoded) => decoded,
            Err(message) => {
                self.error(self.span_of(node), message);
                return None;
            }
        };
        let unit = decoded.unit_size;
        let count = decoded.bytes.len() as u64 / u64::from(unit);
        let element = self.result.types.integer(false, unit as u8);
        Some(self.result.types.array(vec![count], element))
    }

    // -- names -------------------------------------------------------------

    fn check_name_expr(&mut self, node: NodeId, name: &NameExpr) -> Option<TypeId> {
        let Some(symbol) = self.lookup(&name.name).cloned() else {
            self.error(self.span_of(node), format!("unknown name '{}'", name.name));
            return None;
        };
        match symbol {
            Symbol::Local(local) => {
                self.result.resolutions[node as usize] = Some(Resolution::Local(local));
                self.local_info(local).map(|info| info.ty)
            }
            Symbol::Global(global) => {
                self.result.resolutions[node as usize] = Some(Resolution::Global(global));
                Some(self.result.globals[global as usize].ty)
            }
            Symbol::Functions(overloads) => {
                if overloads.len() != 1 {
                    self.error(
                        self.span_of(node),
                        "overloaded function used as a value; a call is required",
                    );
                    return None;
                }
                let index = overloads[0];
                self.result.resolutions[node as usize] = Some(Resolution::Function(index));
                let parameters = self.result.functions[index as usize]
                    .parameters
                    .iter()
                    .map(|(_, ty)| *ty)
                    .collect::<Vec<_>>();
                let return_type = self.result.functions[index as usize].return_type;
                Some(
                    self.result
                        .types
                        .function(parameters, false, None, return_type),
                )
            }
            Symbol::Type(type_id) => {
                self.result.resolutions[node as usize] = Some(Resolution::Type(type_id));
                self.error(
                    self.span_of(node),
                    "a type name cannot be used as a value here",
                );
                None
            }
            Symbol::Module(_) => {
                self.error(
                    self.span_of(node),
                    "a module name cannot be used as a value",
                );
                None
            }
        }
    }

    fn check_member_access(
        &mut self,
        node: NodeId,
        access: &MemberAccess,
        pointer: bool,
    ) -> Option<TypeId> {
        // Module-qualified name: `Module.member`.
        if let Some(Symbol::Module(scope)) = self.try_resolve_name(access.base) {
            let member = self.lookup_in(scope, &access.name).cloned();
            match member {
                Some(Symbol::Functions(overloads)) if overloads.len() == 1 => {
                    let index = overloads[0];
                    self.result.resolutions[node as usize] = Some(Resolution::Function(index));
                    let parameters = self.result.functions[index as usize]
                        .parameters
                        .iter()
                        .map(|(_, ty)| *ty)
                        .collect::<Vec<_>>();
                    let return_type = self.result.functions[index as usize].return_type;
                    return Some(
                        self.result
                            .types
                            .function(parameters, false, None, return_type),
                    );
                }
                Some(Symbol::Global(global)) => {
                    self.result.resolutions[node as usize] = Some(Resolution::Global(global));
                    return Some(self.result.globals[global as usize].ty);
                }
                Some(Symbol::Type(_)) => {
                    self.error(self.span_of(node), "a type name cannot be used as a value");
                    return None;
                }
                _ => {
                    self.error(
                        self.span_of(node),
                        format!("'{}' is not a member of the module", access.name),
                    );
                    return None;
                }
            }
        }

        // Enumeration member: `Status.Running` (§3.7.4).
        if let Some(Symbol::Type(type_id)) = self.try_resolve_name(access.base) {
            let Type::Named(index) = self.type_of(type_id) else {
                self.error(self.span_of(node), "type is not a namespace");
                return None;
            };
            let definition = &self.result.type_defs[index as usize];
            if definition.kind != TypeDefKind::Enumeration {
                self.error(
                    self.span_of(node),
                    "only enumeration members can be accessed through a type name",
                );
                return None;
            }
            if let Some(member_index) = definition
                .enum_members
                .iter()
                .position(|(name, _)| *name == access.name)
            {
                self.result.resolutions[node as usize] = Some(Resolution::EnumMember {
                    type_id,
                    index: member_index as u32,
                });
                return Some(type_id);
            }
            self.error(
                self.span_of(node),
                format!(
                    "'{}' is not a member of '{}'",
                    access.name, definition.qualified_name
                ),
            );
            return None;
        }

        // Field access on a value.
        let base_type = self.check_expr(access.base, None)?;
        let base_type = if pointer {
            match self.type_of(base_type) {
                Type::Pointer {
                    target: Some(target),
                } => target,
                Type::Pointer { target: None } => {
                    self.error(
                        self.span_of(access.base),
                        "cannot access members through a target-less pointer",
                    );
                    return None;
                }
                _ => {
                    self.error(
                        self.span_of(access.base),
                        "pointer member access requires a pointer",
                    );
                    return None;
                }
            }
        } else {
            base_type
        };

        match self.type_of(base_type) {
            Type::Named(index) => {
                let definition = &self.result.type_defs[index as usize];
                if definition.kind != TypeDefKind::Structure
                    && definition.kind != TypeDefKind::Class
                {
                    self.error(
                        self.span_of(node),
                        format!("'{}' has no fields", definition.qualified_name),
                    );
                    return None;
                }
                if let Some(field_index) = definition
                    .fields
                    .iter()
                    .position(|field| field.name == access.name)
                {
                    self.result.resolutions[node as usize] = Some(Resolution::Field {
                        type_id: self.result.types.intern(Type::Named(index)),
                        index: field_index as u32,
                    });
                    return Some(definition.fields[field_index].ty);
                }
                self.error(
                    self.span_of(node),
                    format!(
                        "'{}' is not a member of '{}'",
                        access.name, definition.qualified_name
                    ),
                );
                None
            }
            _ => {
                self.error(
                    self.span_of(node),
                    "member access requires a structure or class value",
                );
                None
            }
        }
    }

    /// Resolves a name chain to a symbol without reporting an error.
    fn try_resolve_name(&mut self, node: NodeId) -> Option<Symbol> {
        match self.node_kind(node).clone() {
            NodeKind::Name(name) => self.lookup(&name.name).cloned(),
            NodeKind::MemberAccess(access) => {
                let base = self.try_resolve_name(access.base)?;
                match base {
                    Symbol::Module(scope) => self.lookup_in(scope, &access.name).cloned(),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn check_index_expr(&mut self, node: NodeId, index: &IndexExpr) -> Option<TypeId> {
        let base_type = self.check_expr(index.base, None)?;
        let index_type = self.check_expr(index.index, None);
        if let Some(index_type) = index_type {
            if !self.is_integer(index_type) {
                self.error(
                    self.span_of(index.index),
                    "array index must be an integer or cardinal value",
                );
            }
        }
        match self.type_of(base_type) {
            Type::Array {
                mut dimensions,
                element,
            } => {
                if dimensions.len() > 1 {
                    dimensions.remove(0);
                    Some(self.result.types.array(dimensions, element))
                } else {
                    Some(element)
                }
            }
            Type::Pointer {
                target: Some(target),
            } => Some(target),
            Type::Pointer { target: None } => {
                self.error(self.span_of(node), "cannot index a target-less pointer");
                None
            }
            _ => {
                self.error(self.span_of(node), "indexing requires an array or pointer");
                None
            }
        }
    }

    fn check_cast(&mut self, node: NodeId, cast: &CastExpr) -> Option<TypeId> {
        let target = self.resolve_type(cast.type_)?;
        let source = self.check_expr(cast.value, None)?;
        if !self.can_explicit_cast(source, target) {
            self.error(
                self.span_of(node),
                format!(
                    "cannot convert '{}' to '{}'",
                    self.spelling(source),
                    self.spelling(target)
                ),
            );
            return None;
        }
        Some(target)
    }

    fn can_explicit_cast(&self, from: TypeId, to: TypeId) -> bool {
        let from_type = self.type_of(from);
        let to_type = self.type_of(to);
        match (&from_type, &to_type) {
            // Numeric and boolean conversions (§3.2).
            (
                Type::Integer { .. } | Type::Float { .. } | Type::Boolean,
                Type::Integer { .. } | Type::Float { .. } | Type::Boolean,
            ) => true,
            // Pointer conversions.
            (Type::Pointer { .. }, Type::Pointer { .. }) => true,
            // Array <-> pointer conversions (§3.6.2).
            (Type::Array { .. }, Type::Pointer { .. }) => true,
            (Type::Pointer { .. }, Type::Array { .. }) => true,
            // Enumeration <-> underlying numeric type.
            (Type::Named(index), Type::Integer { .. } | Type::Float { .. }) => {
                self.result.type_defs[*index as usize].underlying.is_some()
            }
            (Type::Integer { .. } | Type::Float { .. }, Type::Named(index)) => {
                self.result.type_defs[*index as usize].underlying.is_some()
            }
            _ => false,
        }
    }

    fn check_unary(
        &mut self,
        node: NodeId,
        unary: &UnaryExpr,
        expected: Option<TypeId>,
    ) -> Option<TypeId> {
        match unary.op {
            UnaryOp::Negate => {
                let ty = self.check_expr(unary.operand, expected)?;
                if !self.is_numeric(ty) {
                    self.error(self.span_of(node), "unary '-' requires a numeric operand");
                    return None;
                }
                Some(ty)
            }
            UnaryOp::LogicalNot => {
                self.check_condition(unary.operand);
                Some(TypeId::BOOLEAN)
            }
            UnaryOp::BitwiseNot => {
                let ty = self.check_expr(unary.operand, expected)?;
                if !self.is_integer(ty) {
                    self.error(
                        self.span_of(node),
                        "unary '^' requires an integer or cardinal operand",
                    );
                    return None;
                }
                Some(ty)
            }
            UnaryOp::Reference => {
                self.unsupported(self.span_of(node), "reference expressions");
                None
            }
            UnaryOp::Dereference => {
                let ty = self.check_expr(unary.operand, None)?;
                match self.type_of(ty) {
                    Type::Pointer {
                        target: Some(target),
                    } => Some(target),
                    Type::Pointer { target: None } => {
                        self.error(
                            self.span_of(node),
                            "cannot dereference a target-less pointer",
                        );
                        None
                    }
                    _ => {
                        self.error(self.span_of(node), "unary '*' requires a pointer operand");
                        None
                    }
                }
            }
        }
    }

    fn check_binary(
        &mut self,
        node: NodeId,
        binary: &BinaryExpr,
        expected: Option<TypeId>,
    ) -> Option<TypeId> {
        // Literals take their type from the surrounding context; if the context
        // does not provide one, the other operand does (§2.7.1, §2.7.2).
        let left_is_literal = matches!(
            self.node_kind(binary.left),
            NodeKind::IntegerLiteral(_) | NodeKind::FloatLiteral(_) | NodeKind::CharacterLiteral(_)
        );
        let right_is_literal = matches!(
            self.node_kind(binary.right),
            NodeKind::IntegerLiteral(_) | NodeKind::FloatLiteral(_) | NodeKind::CharacterLiteral(_)
        );

        let mut left_type =
            self.check_expr(binary.left, if left_is_literal { expected } else { None });
        let mut right_type = match left_type {
            Some(left_type) => self.check_expr(binary.right, Some(left_type)),
            None => self.check_expr(binary.right, expected),
        };
        if left_type != right_type {
            if left_is_literal {
                if let Some(right_type) = right_type {
                    left_type = self.check_expr(binary.left, Some(right_type));
                }
            } else if right_is_literal {
                if let Some(left_type) = left_type {
                    right_type = self.check_expr(binary.right, Some(left_type));
                }
            }
        }
        // If both operands are literals and the context asks for a numeric type,
        // that type wins.
        if left_is_literal && right_is_literal {
            if let (Some(expected_type), Some(left_seen), Some(right_seen)) =
                (expected, left_type, right_type)
            {
                if (expected_type != left_seen || expected_type != right_seen)
                    && (self.is_numeric(expected_type) || self.is_boolean(expected_type))
                {
                    left_type = self.check_expr(binary.left, Some(expected_type));
                    right_type = self.check_expr(binary.right, Some(expected_type));
                }
            }
        }

        let (Some(left_type), Some(right_type)) = (left_type, right_type) else {
            return None;
        };
        if left_type != right_type {
            self.error(
                self.span_of(node),
                format!(
                    "operator '{}' requires both operands to have the same type, found '{}' and '{}'",
                    binary_op_spelling(binary.op),
                    self.spelling(left_type),
                    self.spelling(right_type)
                ),
            );
            return None;
        }

        match binary.op {
            BinaryOp::Add
            | BinaryOp::Subtract
            | BinaryOp::Multiply
            | BinaryOp::Divide
            | BinaryOp::Remainder => {
                if !self.is_numeric(left_type) {
                    self.error(
                        self.span_of(node),
                        "arithmetic operator requires numeric operands",
                    );
                    return None;
                }
                Some(left_type)
            }
            BinaryOp::ShiftLeft | BinaryOp::ShiftRight => {
                if !self.is_integer(left_type) {
                    self.error(
                        self.span_of(node),
                        "shift operator requires integer or cardinal operands",
                    );
                    return None;
                }
                Some(left_type)
            }
            BinaryOp::Less | BinaryOp::Greater | BinaryOp::LessEqual | BinaryOp::GreaterEqual => {
                if !self.is_numeric(left_type) {
                    self.error(
                        self.span_of(node),
                        "comparison operator requires numeric operands",
                    );
                    return None;
                }
                Some(TypeId::BOOLEAN)
            }
            BinaryOp::Equal | BinaryOp::NotEqual => {
                let comparable = self.is_numeric(left_type)
                    || self.is_boolean(left_type)
                    || matches!(self.type_of(left_type), Type::Named(index) if self.result.type_defs[index as usize].underlying.is_some())
                    || matches!(self.type_of(left_type), Type::Pointer { .. });
                if !comparable {
                    self.error(
                        self.span_of(node),
                        "equality operator requires numeric, boolean, enumeration or pointer operands",
                    );
                    return None;
                }
                Some(TypeId::BOOLEAN)
            }
            BinaryOp::BitwiseAnd | BinaryOp::BitwiseOr | BinaryOp::BitwiseXor => {
                if !self.is_integer(left_type) {
                    self.error(
                        self.span_of(node),
                        "bitwise operator requires integer or cardinal operands",
                    );
                    return None;
                }
                Some(left_type)
            }
            BinaryOp::LogicalAnd | BinaryOp::LogicalOr => {
                if !self.is_boolean(left_type) {
                    self.error(
                        self.span_of(node),
                        "logical operator requires boolean operands",
                    );
                    return None;
                }
                Some(TypeId::BOOLEAN)
            }
        }
    }

    fn check_call(&mut self, node: NodeId, call: &CallExpr) -> Option<TypeId> {
        if let Some(Symbol::Functions(overloads)) = self.try_callee_symbol(call.callee) {
            return self.resolve_overload(node, &overloads, &call.arguments);
        }
        if let Some(Symbol::Type(_)) = self.try_callee_symbol(call.callee) {
            self.unsupported(self.span_of(node), "construction expressions");
            return None;
        }

        let callee_type = self.check_expr(call.callee, None)?;
        match self.type_of(callee_type) {
            Type::Function {
                parameters,
                wildcard,
                return_type,
                ..
            } => {
                if !wildcard && parameters.len() != call.arguments.len() {
                    self.error(
                        self.span_of(node),
                        format!(
                            "function expects {} argument(s), found {}",
                            parameters.len(),
                            call.arguments.len()
                        ),
                    );
                    return None;
                }
                for (argument, parameter) in call.arguments.iter().zip(parameters.iter()) {
                    let ty = self.check_expr(argument.value, Some(*parameter));
                    if let Some(ty) = ty {
                        self.expect_assignable(argument.span, ty, *parameter);
                    }
                }
                return_type
            }
            _ => {
                self.error(self.span_of(node), "expression is not callable");
                None
            }
        }
    }

    fn try_callee_symbol(&mut self, callee: NodeId) -> Option<Symbol> {
        if let NodeKind::MemberAccess(access) = self.node_kind(callee).clone() {
            if let Some(Symbol::Module(scope)) = self.try_resolve_name(access.base) {
                return self.lookup_in(scope, &access.name).cloned();
            }
            return None;
        }
        self.try_resolve_name(callee)
    }

    fn resolve_overload(
        &mut self,
        node: NodeId,
        overloads: &[u32],
        arguments: &[Argument],
    ) -> Option<TypeId> {
        let mut exact = Vec::new();
        for &candidate in overloads {
            let parameters = self.result.functions[candidate as usize].parameters.clone();
            if parameters.len() != arguments.len() {
                continue;
            }
            self.suppress_errors += 1;
            let mut matches = true;
            for (argument, (_, parameter_type)) in arguments.iter().zip(parameters.iter()) {
                let ty = self.check_expr(argument.value, Some(*parameter_type));
                if ty != Some(*parameter_type) {
                    matches = false;
                    break;
                }
            }
            self.suppress_errors -= 1;
            if matches {
                exact.push(candidate);
            }
        }

        let selected = match exact.len() {
            1 => exact[0],
            0 => {
                // Second phase: implicit conversions (enumeration -> underlying).
                let mut convertible = Vec::new();
                for &candidate in overloads {
                    let parameters = self.result.functions[candidate as usize].parameters.clone();
                    if parameters.len() != arguments.len() {
                        continue;
                    }
                    self.suppress_errors += 1;
                    let mut matches = true;
                    for (argument, (_, parameter_type)) in arguments.iter().zip(parameters.iter()) {
                        let ty = self.check_expr(argument.value, Some(*parameter_type));
                        match ty {
                            Some(ty) if self.is_assignable(ty, *parameter_type) => {}
                            _ => {
                                matches = false;
                                break;
                            }
                        }
                    }
                    self.suppress_errors -= 1;
                    if matches {
                        convertible.push(candidate);
                    }
                }
                if convertible.len() == 1 {
                    convertible[0]
                } else if convertible.is_empty() {
                    self.error(self.span_of(node), "no overload matches the argument types");
                    return None;
                } else {
                    self.error(
                        self.span_of(node),
                        "ambiguous call: several overloads match the argument types",
                    );
                    return None;
                }
            }
            _ => {
                self.error(
                    self.span_of(node),
                    "ambiguous call: several overloads match the argument types",
                );
                return None;
            }
        };

        let parameters = self.result.functions[selected as usize].parameters.clone();
        for (argument, (_, parameter_type)) in arguments.iter().zip(parameters.iter()) {
            let ty = self.check_expr(argument.value, Some(*parameter_type));
            if let Some(ty) = ty {
                self.expect_assignable(argument.span, ty, *parameter_type);
            }
        }
        self.result.resolutions[node as usize] = Some(Resolution::Function(selected));
        self.result.functions[selected as usize].return_type
    }

    fn check_array_literal(
        &mut self,
        node: NodeId,
        literal: &ArrayLiteral,
        expected: Option<TypeId>,
    ) -> Option<TypeId> {
        let expected_element = expected.and_then(|expected| match self.type_of(expected) {
            Type::Array {
                dimensions,
                element,
            } => {
                if dimensions.len() > 1 {
                    let mut rest = dimensions.clone();
                    rest.remove(0);
                    Some(self.result.types.array(rest, element))
                } else {
                    Some(element)
                }
            }
            _ => None,
        });

        let mut element_type = expected_element;
        let mut count: u64 = 0;

        for element in &literal.elements {
            match element {
                ArrayElement::Value(expression) => {
                    let ty = match element_type {
                        Some(element_type) => self.check_expr(*expression, Some(element_type)),
                        None => self.check_expr(*expression, None),
                    };
                    if let Some(ty) = ty {
                        if let Some(element_type) = element_type {
                            self.expect_assignable(self.span_of(*expression), ty, element_type);
                        } else {
                            element_type = Some(ty);
                        }
                    }
                    count += 1;
                }
                ArrayElement::Repeat {
                    value,
                    count: repeat_count,
                } => {
                    let Some(element_type) = element_type else {
                        self.error(
                            self.span_of(*value),
                            "cannot infer the element type of a repeat element",
                        );
                        return None;
                    };
                    let ty = self.check_expr(*value, Some(element_type));
                    if let Some(ty) = ty {
                        self.expect_assignable(self.span_of(*value), ty, element_type);
                    }
                    let repeat = self.eval_const_int(*repeat_count)?;
                    if repeat < 0 {
                        self.error(
                            self.span_of(*repeat_count),
                            "repeat count must be non-negative",
                        );
                        return None;
                    }
                    count += repeat as u64;
                }
                ArrayElement::Range(range) => {
                    let Some(element_type) = element_type else {
                        self.error(
                            self.span_of(node),
                            "cannot infer the element type of a range element",
                        );
                        return None;
                    };
                    if !self.is_integer(element_type) {
                        self.error(
                            self.span_of(node),
                            "range elements require an integer element type",
                        );
                        return None;
                    }
                    let start = self.eval_const_int(range.start)?;
                    let end = self.eval_const_int(range.end)?;
                    let step = match range.step {
                        Some(step) => self.eval_const_int(step)?,
                        None => {
                            if end >= start {
                                1
                            } else {
                                -1
                            }
                        }
                    };
                    if step == 0 {
                        self.error(self.span_of(node), "range step must not be zero");
                        return None;
                    }
                    count += range_count(start, end, step, range.inclusive);
                }
            }
        }

        let element_type = element_type.unwrap_or(TypeId::ERROR);
        Some(self.result.types.array(vec![count], element_type))
    }
}

fn fits_f32(value: f64) -> bool {
    let narrowed = value as f32;
    narrowed.is_finite() || !value.is_finite()
}

fn fits_unsigned(value: u128, bytes: u8) -> bool {
    let bits = u32::from(bytes) * 8;
    if bits >= 128 {
        true
    } else {
        value < (1u128 << bits)
    }
}

fn range_count(start: i128, end: i128, step: i128, inclusive: bool) -> u64 {
    let distance = if step > 0 {
        if end < start {
            return 0;
        }
        end - start
    } else {
        if end > start {
            return 0;
        }
        start - end
    };
    let magnitude = step.unsigned_abs();
    let distance = u128::try_from(distance).unwrap_or(u128::MAX);
    let mut count = distance / magnitude;
    let remainder = distance % magnitude;
    if inclusive {
        count += 1;
    } else if remainder == 0 {
        // Exclusive end exactly on a step boundary.
    }
    u64::try_from(count).unwrap_or(u64::MAX)
}

fn binary_op_spelling(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Subtract => "-",
        BinaryOp::Multiply => "*",
        BinaryOp::Divide => "/",
        BinaryOp::Remainder => "%",
        BinaryOp::ShiftLeft => "<<",
        BinaryOp::ShiftRight => ">>",
        BinaryOp::Less => "<",
        BinaryOp::Greater => ">",
        BinaryOp::LessEqual => "<=",
        BinaryOp::GreaterEqual => ">=",
        BinaryOp::Equal => "==",
        BinaryOp::NotEqual => "!=",
        BinaryOp::BitwiseAnd => "&",
        BinaryOp::BitwiseOr => "|",
        BinaryOp::BitwiseXor => "^",
        BinaryOp::LogicalAnd => "&&",
        BinaryOp::LogicalOr => "||",
    }
}
