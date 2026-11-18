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
                let mut runtime: Option<NodeId> = None;
                let mut position = 0usize;
                for dimension in &array.dimensions {
                    position += 1;
                    match dimension {
                        ArrayDimension::Size(expression) => {
                            // A dimension may reference a bound dimension by name
                            // inside an array extension (§3.8.3).
                            if let NodeKind::Name(name) = self.node_kind(*expression) {
                                let bound = self
                                    .array_binding_lengths
                                    .iter()
                                    .rev()
                                    .find_map(|lengths| lengths.get(&name.name).copied())
                                    .or_else(|| {
                                        self.constant_env.iter().rev().find_map(|environment| {
                                            environment
                                                .get(&name.name)
                                                .and_then(|value| u64::try_from(*value).ok())
                                        })
                                    });
                                if let Some(length) = bound {
                                    dimensions.push(length);
                                    continue;
                                }
                            }
                            if !self.is_compile_time_constant(*expression) {
                                // Runtime dimensions are only allowed inside a
                                // function body (§3.6.1), and only as the leading
                                // dimension (the VLA value carries one length).
                                if self.in_function_body && runtime.is_none() && position == 1 {
                                    runtime = Some(*expression);
                                    continue;
                                }
                                if self.in_function_body && runtime.is_none() {
                                    self.unsupported(
                                        self.span_of(*expression),
                                        "non-leading variable-length array dimensions",
                                    );
                                    return None;
                                }
                                self.error(
                                    self.span_of(*expression),
                                    "array size must be compile-time constant here",
                                );
                                return None;
                            }
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
                        ArrayDimension::Binding { name, .. } => {
                            let bound = self
                                .array_binding_lengths
                                .iter()
                                .rev()
                                .find_map(|lengths| lengths.get(name).copied());
                            match bound {
                                Some(length) => dimensions.push(length),
                                None => {
                                    self.unsupported(
                                        self.span_of(node),
                                        "array dimension bindings (array parameter expansion)",
                                    );
                                    return None;
                                }
                            }
                        }
                    }
                }
                let element = self.resolve_type(array.element)?;
                if let Some(length_expression) = runtime {
                    // Remaining (compile-time) dimensions form the element type,
                    // e.g. `array(rows, 3) T` is a VLA of `array(3) T`.
                    let element = if dimensions.is_empty() {
                        element
                    } else {
                        self.result.types.array(dimensions, element)
                    };
                    let length_type = self.check_expr(length_expression, None)?;
                    if !self.is_integer(length_type) {
                        self.error(
                            self.span_of(length_expression),
                            "array length must be an integer or cardinal",
                        );
                        return None;
                    }
                    return Some(self.result.types.intern(Type::Vla {
                        element,
                        length: length_type,
                    }));
                }
                // Nested fixed arrays are one array with combined dimensions
                // (`array(3) array(4) T` == `array(3,4) T`, §3.6.1).
                match self.result.types.get(element).clone() {
                    Type::Array {
                        dimensions: inner,
                        element: inner_element,
                    } => {
                        let mut combined = dimensions;
                        combined.extend(inner);
                        Some(self.result.types.array(combined, inner_element))
                    }
                    _ => Some(self.result.types.array(dimensions, element)),
                }
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
                let calling_convention =
                    self.resolve_calling_convention(function.calling_convention);
                Some(self.result.types.function(
                    parameters,
                    function.wildcard,
                    calling_convention,
                    return_type,
                ))
            }
            NodeKind::NamedType(named) => {
                // A generic parameter is substituted by the active instantiation
                // environment and shadows outer names (§10.1).
                if named.arguments.is_none() {
                    if let NodeKind::Name(name) = self.node_kind(named.name) {
                        if let Some(substituted) = self.lookup_generic_parameter(&name.name) {
                            return Some(substituted);
                        }
                    }
                }
                let symbol = self.resolve_name_node(named.name)?;
                match symbol {
                    Symbol::Type(type_id) => {
                        let type_index = match self.result.types.get(type_id) {
                            Type::Named(index) => Some(*index),
                            _ => None,
                        };
                        let generic_parameters = type_index
                            .map(|index| {
                                self.result.type_defs[index as usize]
                                    .generic_parameters
                                    .clone()
                            })
                            .unwrap_or_default();
                        match named.arguments {
                            Some(arguments) => {
                                if generic_parameters.is_empty() {
                                    self.error(
                                        self.span_of(node),
                                        format!(
                                            "'{}' does not declare generic parameters",
                                            self.spelling(type_id)
                                        ),
                                    );
                                    return None;
                                }
                                let template_node = type_index
                                    .map(|index| self.result.type_defs[index as usize].node)?;
                                let arguments = self.resolve_type_arguments(&arguments)?;
                                self.instantiate_type(template_node, arguments, self.span_of(node))
                            }
                            None => {
                                if !generic_parameters.is_empty() {
                                    self.error(
                                        self.span_of(node),
                                        format!(
                                            "generic type '{}' requires type arguments",
                                            self.spelling(type_id)
                                        ),
                                    );
                                    return None;
                                }
                                Some(type_id)
                            }
                        }
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
        self.result.spelling(ty)
    }

    pub(crate) fn size_align(&self, ty: TypeId) -> (u64, u64) {
        self.result.size_align(ty)
    }

    // ==================================================================
    // Declaration resolution (pass 2)
    // ==================================================================

    /// Merges the active generic substitution environments (§10.1), innermost
    /// layer last. The result is stored on functions so that bodies can be
    /// checked after the environment stack has been unwound.
    pub(crate) fn captured_generic_env(&self) -> std::collections::HashMap<String, TypeId> {
        let mut merged = std::collections::HashMap::new();
        for layer in &self.generic_env {
            for (name, ty) in layer {
                merged.insert(name.clone(), *ty);
            }
        }
        merged
    }

    pub(crate) fn resolve_structure_like(
        &mut self,
        node: NodeId,
        _name: String,
        kind: TypeDefKind,
    ) {
        let (members, bases) = match self.node_kind(node) {
            NodeKind::Structure(declaration) => (declaration.members.clone(), Vec::new()),
            NodeKind::Class(declaration) => {
                (declaration.members.clone(), declaration.bases.clone())
            }
            _ => return,
        };
        let is_class = kind == TypeDefKind::Class;
        let Some(type_id) = self.type_id_for_node(node) else {
            return;
        };
        let type_index = match self.result.types.get(type_id) {
            Type::Named(index) => *index,
            _ => return,
        };

        // Base class (§3.7.1): at most one class, plus interface contracts.
        let mut base_type = None;
        let mut interfaces: Vec<TypeId> = Vec::new();
        if is_class {
            for (position, base) in bases.iter().enumerate() {
                let Some(base_type_id) = self.resolve_type(*base) else {
                    continue;
                };
                let base_kind = match self.result.types.get(base_type_id) {
                    Type::Named(index) => self.result.type_defs[*index as usize].kind,
                    _ => {
                        self.error(self.span_of(*base), "base type must be a named type");
                        continue;
                    }
                };
                if base_kind == TypeDefKind::Class {
                    if position != 0 || base_type.is_some() {
                        self.error(self.span_of(*base), "a class may inherit at most one class");
                    } else {
                        base_type = Some(base_type_id);
                    }
                } else if base_kind == TypeDefKind::Interface {
                    interfaces.push(base_type_id);
                } else {
                    self.error(
                        self.span_of(*base),
                        "base class list entries must be classes or interfaces",
                    );
                }
            }
        }

        // Every class derives from the built-in Object root unless it names a
        // base class (§15.2).
        if is_class && base_type.is_none() {
            if let Some(Symbol::Type(object)) = self.lookup("Object").cloned() {
                let class_name = match self.node_kind(node) {
                    NodeKind::Class(declaration) => declaration.name.clone(),
                    _ => String::new(),
                };
                if class_name != "Object" {
                    base_type = Some(object);
                }
            }
        }
        let sealed_class = match self.node_kind(node) {
            NodeKind::Class(declaration) => declaration.modifiers.sealed,
            NodeKind::Structure(declaration) => declaration.modifiers.sealed,
            _ => false,
        };
        let static_class = match self.node_kind(node) {
            NodeKind::Class(declaration) => declaration.modifiers.static_,
            NodeKind::Structure(declaration) => declaration.modifiers.static_,
            _ => false,
        };
        // Register the base and sealed flags before processing members, so that
        // extension methods can validate `override` against the base chain.
        {
            let abstract_class = match self.node_kind(node) {
                NodeKind::Class(declaration) => declaration.modifiers.abstract_,
                _ => false,
            };
            let definition = &mut self.result.type_defs[type_index as usize];
            definition.base = base_type;
            definition.sealed = sealed_class;
            definition.is_abstract = abstract_class;
            definition.is_static = static_class;
            definition.interfaces = interfaces.clone();
        }

        let struct_alignment = match self.node_kind(node) {
            NodeKind::Structure(declaration) => declaration.modifiers.aligned,
            NodeKind::Class(declaration) => {
                if declaration.modifiers.aligned.is_some() {
                    self.error(
                        self.span_of(node),
                        "'aligned' is only allowed on structure declarations",
                    );
                }
                None
            }
            _ => None,
        };

        // The instance starts with the class header, then inherited fields, then
        // fields declared here (declaration order, §3.7.2).
        let mut fields: Vec<FieldInfo> = Vec::new();
        let mut field_initializers: Vec<Option<NodeId>> = Vec::new();
        let mut offset: u64 = if is_class { 16 } else { 0 };
        let mut max_align: u64 = if is_class { 8 } else { 1 };

        if let Some(base_type) = base_type {
            if let Type::Named(base_index) = self.types_of(base_type) {
                let base_definition = &self.result.type_defs[base_index as usize];
                fields.extend(base_definition.fields.clone());
                field_initializers.extend(base_definition.field_initializers.clone());
                offset = base_definition.instance_size;
                max_align = max_align.max(base_definition.instance_align);
            }
        }

        for member in &members {
            match self.node_kind(*member) {
                NodeKind::Field(field) => {
                    let Some(ty_node) = field.ty else {
                        self.error(self.span_of(*member), "field requires a type");
                        continue;
                    };
                    let Some(ty) = self.resolve_type(ty_node) else {
                        continue;
                    };
                    // A `static` field is class-level storage, not instance
                    // layout (§9.15): it becomes a global initialized by the
                    // launcher.
                    if field.modifiers.static_ {
                        let class_name = match self.result.types.get(type_id) {
                            Type::Named(index) => self.result.type_defs[*index as usize]
                                .qualified_name
                                .clone(),
                            _ => String::new(),
                        };
                        let mut scope: Vec<String> =
                            class_name.split('.').map(str::to_string).collect();
                        scope.pop();
                        let spelling = self.spelling(ty);
                        let mangled = mangle_function(&scope, &field.name, &[spelling]);
                        let init = match field.init {
                            Some(Initializer::Expression(expression)) => Some(expression),
                            Some(Initializer::Block(_)) => {
                                self.error(
                                    self.span_of(*member),
                                    "static field initializer must be an expression",
                                );
                                None
                            }
                            None => None,
                        };
                        let static_scoped = self.resolve_scoped_targets(
                            &field.modifiers,
                            Some(type_id),
                            self.span_of(*member),
                        );
                        let index = self.result.globals.len() as u32;
                        self.result.globals.push(GlobalInfo {
                            accessibility: field.modifiers.accessibility,
                            scoped: static_scoped,
                            verbatim: field.modifiers.verbatim,
                            name: field.name.clone(),
                            scope,
                            ty,
                            init,
                            fixed: false,
                            mangled,
                            node: *member,
                            span: self.span_of(*member),
                        });
                        if let Type::Named(type_index) = self.result.types.get(type_id) {
                            self.result.type_defs[*type_index as usize]
                                .static_fields
                                .insert(field.name.clone(), index);
                        }
                        continue;
                    }
                    if static_class {
                        self.error(
                            self.span_of(*member),
                            "a static class must not declare instance fields",
                        );
                        continue;
                    }
                    if is_class && field.modifiers.aligned.is_some() {
                        self.error(
                            self.span_of(*member),
                            "'aligned' is only allowed on structure fields",
                        );
                    }
                    let (size, natural_align) = self.size_align(ty);
                    let field_alignment = if is_class {
                        None
                    } else {
                        field.modifiers.aligned.or(struct_alignment)
                    };
                    let alignment = match field_alignment {
                        Some(0) => 0,
                        Some(requested) => u64::from(requested),
                        None => natural_align,
                    };
                    if alignment != 0 {
                        offset = round_up(offset, alignment);
                        max_align = max_align.max(alignment);
                    }
                    if fields.iter().any(|existing| existing.name == field.name) {
                        self.error(
                            self.span_of(*member),
                            format!("duplicate field '{}'", field.name),
                        );
                    }
                    let field_scoped = self.resolve_scoped_targets(
                        &field.modifiers,
                        Some(type_id),
                        self.span_of(*member),
                    );
                    fields.push(FieldInfo {
                        name: field.name.clone(),
                        ty,
                        offset,
                        accessibility: field.modifiers.accessibility,
                        readonly: field.modifiers.readonly,
                        scoped: field_scoped,
                    });
                    field_initializers.push(match field.init {
                        Some(Initializer::Expression(expression)) => Some(expression),
                        Some(Initializer::Block(_)) => {
                            self.error(
                                self.span_of(*member),
                                "field initializer must be an expression",
                            );
                            None
                        }
                        None => None,
                    });
                    offset += size;
                }
                NodeKind::Extension(_) => {
                    self.resolve_extension_declaration(*member, Some(type_id));
                }
                NodeKind::Module(_) => {}
                NodeKind::Accessor(_) => {
                    let is_interface = matches!(
                        self.types_of(type_id),
                        Type::Named(index) if self.result.type_defs[index as usize].kind == TypeDefKind::Interface
                    );
                    if is_interface {
                        self.resolve_accessor_declaration(*member, Some(type_id));
                    } else {
                        self.error(
                            self.span_of(*member),
                            "accessors must be declared in an extension block",
                        );
                    }
                }
                NodeKind::Operator(_) => {
                    let is_interface = matches!(
                        self.types_of(type_id),
                        Type::Named(index) if self.result.type_defs[index as usize].kind == TypeDefKind::Interface
                    );
                    if is_interface {
                        self.resolve_operator_declaration(*member, Some(type_id));
                    } else {
                        self.error(
                            self.span_of(*member),
                            "operators must be declared in an extension block",
                        );
                    }
                }
                NodeKind::Class(_)
                | NodeKind::Structure(_)
                | NodeKind::Interface(_)
                | NodeKind::Enumeration(_)
                | NodeKind::Variant(_)
                | NodeKind::Union(_) => {}
                _ => {
                    self.error(
                        self.span_of(*member),
                        "only fields and extension blocks may appear directly in a type body",
                    );
                }
            }
        }

        let sealed = sealed_class;
        if let Some(base_type) = base_type {
            if let Type::Named(base_index) = self.types_of(base_type) {
                if self.result.type_defs[base_index as usize].sealed {
                    self.error(self.span_of(node), "cannot inherit from a sealed class");
                }
            }
        }
        let instance_size = round_up(offset, max_align);
        {
            let definition = &mut self.result.type_defs[type_index as usize];
            definition.sealed = sealed;
            definition.fields = fields;
            definition.field_initializers = field_initializers;
            definition.base = base_type;
            definition.instance_size = instance_size;
            definition.instance_align = max_align;
            if is_class {
                // The value representation of a class is a reference (§3.4).
                definition.size = 8;
                definition.align = 8;
            } else {
                definition.size = instance_size;
                definition.align = max_align;
            }
        }
        self.resolve_pending_accessors();
        if is_class {
            self.verify_interface_implementations(type_id);
        }
    }

    pub(crate) fn types_of(&self, ty: TypeId) -> Type {
        self.result.types.get(ty).clone()
    }

    fn type_id_for_node(&mut self, node: NodeId) -> Option<TypeId> {
        let index = self
            .result
            .type_defs
            .iter()
            .position(|definition| definition.node == node)? as u32;
        Some(self.result.types.intern(Type::Named(index)))
    }

    /// Validates that a method declared `override` really overrides a virtual
    /// base method (§9.13).
    fn validate_override(&mut self, method: u32, owner: TypeId) {
        let base = match self.result.types.get(owner) {
            Type::Named(index) => self.result.type_defs[*index as usize].base,
            _ => None,
        };
        let Some(base) = base else {
            let span = self.result.functions[method as usize].span;
            self.error(span, "'override' requires a base class method");
            return;
        };
        let name = self.result.functions[method as usize].name.clone();
        let candidates = self.collect_methods(base, &name);
        let parameters = self.result.functions[method as usize].parameters.clone();
        for candidate in candidates {
            let base_parameters = self.result.functions[candidate as usize].parameters.clone();
            if base_parameters.len() != parameters.len() {
                continue;
            }
            let same = base_parameters
                .iter()
                .zip(parameters.iter())
                .skip(1)
                .all(|(base, own)| base.ty == own.ty);
            if !same {
                continue;
            }
            let base_info = &self.result.functions[candidate as usize];
            if !base_info.is_virtual {
                let span = self.result.functions[method as usize].span;
                self.error(span, "cannot override a non-virtual method");
                return;
            }
            if base_info.is_sealed {
                let span = self.result.functions[method as usize].span;
                self.error(span, "cannot override a sealed method");
                return;
            }
            return;
        }
        let span = self.result.functions[method as usize].span;
        self.error(span, "'override' does not match any base class method");
    }

    fn resolve_extension_target(&mut self, node: NodeId) -> Option<TypeId> {
        match self.node_kind(node) {
            NodeKind::ExtensionTarget(target) => match target {
                ExtensionTarget::Type(type_node) => self.resolve_type(type_node),
                ExtensionTarget::Generic { .. } => {
                    self.unsupported(self.span_of(node), "generic extension targets");
                    None
                }
                ExtensionTarget::Array { .. } => {
                    self.unsupported(self.span_of(node), "array extension targets");
                    None
                }
            },
            _ => self.resolve_type(node),
        }
    }

    pub(crate) fn resolve_extension_declaration(&mut self, node: NodeId, owner: Option<TypeId>) {
        let NodeKind::Extension(declaration) = self.node_kind(node) else {
            return;
        };
        // An `extension for Generic<T>` records a template that is instantiated
        // together with the targeted type (§3.8.3).
        if owner.is_none() {
            if let Some(target) = declaration.target {
                if matches!(
                    self.node_kind(target),
                    NodeKind::ExtensionTarget(ExtensionTarget::Generic { .. })
                ) {
                    self.register_generic_extension(node, target);
                    return;
                }
                if let NodeKind::ExtensionTarget(ExtensionTarget::Array {
                    bindings,
                    parameter,
                }) = self.node_kind(target).clone()
                {
                    // `extension for array(length: cardinal(4)) T`: a template
                    // specialised for every matching array type (§3.8.3).
                    let mut resolved_bindings = Vec::new();
                    for (name, ty_node) in bindings {
                        let Some(ty) = self.resolve_type(ty_node) else {
                            return;
                        };
                        resolved_bindings.push((name, ty));
                    }
                    self.result
                        .array_extension_templates
                        .push(ArrayExtensionTemplate {
                            extension: node,
                            bindings: resolved_bindings,
                            parameter: GenericParameterInfo::from_ast(&parameter),
                        });
                    return;
                }
            }
        }
        let target = match (owner, declaration.target) {
            (Some(owner), _) => Some(owner),
            (None, Some(target)) => self.resolve_extension_target(target),
            (None, None) => None,
        };
        let Some(target) = target else {
            self.error(self.span_of(node), "extension requires a target type");
            return;
        };
        let target_kind = match self.result.types.get(target) {
            Type::Named(index) => self.result.type_defs[*index as usize].kind,
            _ => {
                self.error(self.span_of(node), "extension target must be a named type");
                return;
            }
        };
        if !matches!(
            target_kind,
            TypeDefKind::Structure
                | TypeDefKind::Class
                | TypeDefKind::Interface
                | TypeDefKind::Variant
                | TypeDefKind::Union
        ) {
            self.error(
                self.span_of(node),
                "extensions may only target structures, classes and interfaces",
            );
            return;
        }

        let members = declaration.members.clone();
        for member in members {
            match self.node_kind(member) {
                NodeKind::Fixed(declaration) => {
                    if declaration.modifiers.static_ {
                        let is_function = declaration.ty.is_some_and(|ty| {
                            matches!(self.node_kind(ty), NodeKind::FunctionType(_))
                        });
                        if is_function {
                            self.create_static_method(member, &declaration, target);
                        } else {
                            self.create_static_constant(member, &declaration, target);
                        }
                    } else {
                        self.create_method(member, &declaration, target);
                    }
                }
                NodeKind::Accessor(declaration) => {
                    if declaration.modifiers.static_ {
                        self.create_static_accessor(member, &declaration, target);
                        continue;
                    }
                    self.pending_accessors.push(PendingAccessor {
                        node: member,
                        owner: target,
                        generic_env: self.captured_generic_env(),
                    });
                }
                NodeKind::Operator(_) => {
                    self.resolve_operator_declaration(member, Some(target));
                }
                NodeKind::Field(_) => {
                    self.error(self.span_of(member), "an extension must not declare fields");
                }
                NodeKind::Class(_)
                | NodeKind::Structure(_)
                | NodeKind::Interface(_)
                | NodeKind::Enumeration(_)
                | NodeKind::Variant(_)
                | NodeKind::Union(_) => {
                    self.unsupported(self.span_of(member), "nested type declarations");
                }
                NodeKind::Module(_) => {}
                _ => {
                    self.error(
                        self.span_of(member),
                        "extension members must be function definitions",
                    );
                }
            }
        }
    }

    /// Registers a generic method as a template (§10.1): its body and signature
    /// are resolved only when an instantiation is created for a call.
    fn create_generic_method_template(
        &mut self,
        node: NodeId,
        declaration: &FixedDecl,
        owner: TypeId,
    ) {
        let Some(Initializer::Block(_)) = declaration.init else {
            self.error(self.span_of(node), "a generic method requires a body");
            return;
        };
        let Type::Named(owner_index) = self.types_of(owner) else {
            return;
        };
        let owner_name = self.result.type_defs[owner_index as usize]
            .qualified_name
            .clone();
        let scope: Vec<String> = owner_name.split('.').map(str::to_string).collect();
        let generic_parameters: Vec<GenericParameterInfo> = declaration
            .generics
            .iter()
            .map(GenericParameterInfo::from_ast)
            .collect();
        let index = self.result.functions.len() as u32;
        self.result.functions.push(FunctionInfo {
            name: declaration.name.clone(),
            scope,
            node,
            body: None,
            parameters: Vec::new(),
            method_of: Some(owner),
            static_owner: None,
            is_virtual: false,
            is_override: false,
            is_sealed: false,
            accessibility: declaration.modifiers.accessibility,
            calling_convention: None,
            scoped: Vec::new(),
            is_abstract: false,
            return_type: None,
            mangled: String::new(),
            exposed_symbol: None,
            generic_parameters,
            generic_arguments: Vec::new(),
            template: None,
            decl_scope: self.current_scope(),
            generic_env: self.captured_generic_env(),
            environment: false,
            captures: Vec::new(),
            constant_values: HashMap::new(),
            locals: Vec::new(),
            span: self.span_of(node),
        });
        let definition = &mut self.result.type_defs[owner_index as usize];
        definition
            .methods
            .entry(declaration.name.clone())
            .or_default()
            .push(index);
        definition.method_list.push(index);
    }

    /// Registers a method declared in an extension block (§8.4).
    fn create_method(
        &mut self,
        node: NodeId,
        declaration: &FixedDecl,
        owner: TypeId,
    ) -> Option<u32> {
        let ty_node = declaration.ty?;
        let NodeKind::FunctionType(function) = self.node_kind(ty_node) else {
            self.unsupported(
                self.span_of(node),
                "function-valued constants in extensions",
            );
            return None;
        };
        if !declaration.generics.is_empty() {
            // Generic methods are templates instantiated by Monomorphization
            // expressions at call sites (§10.1, §10.2).
            self.create_generic_method_template(node, declaration, owner);
            return None;
        }
        let (body, is_abstract) = match declaration.init {
            Some(Initializer::Block(block)) => (Some(FunctionBody::Block(block)), false),
            None => (None, true),
            Some(Initializer::Expression(_)) => {
                self.unsupported(self.span_of(node), "function values in extensions");
                return None;
            }
        };
        let owner_kind = match self.result.types.get(owner) {
            Type::Named(index) => self.result.type_defs[*index as usize].kind,
            // Array extension targets are value types (§3.8.3).
            _ => TypeDefKind::Structure,
        };
        if is_abstract && owner_kind != TypeDefKind::Interface {
            self.error(
                self.span_of(node),
                "only interface methods may omit their body",
            );
            return None;
        }
        // A function in an extension block is a method only when it declares a
        // `this` parameter of the extension target type (§8.4); otherwise it is
        // an extension-scoped function constant.
        let this_position = function
            .parameters
            .iter()
            .position(|parameter| parameter.name.as_deref() == Some("this"));
        let mut is_method = false;
        if let Some(position) = this_position {
            if let Some(this_type) = function.parameters[position].ty {
                self.suppress_errors += 1;
                let resolved = self.resolve_type(this_type);
                self.suppress_errors -= 1;
                is_method = resolved == Some(owner);
            }
        }
        if !is_method {
            if is_abstract {
                self.error(
                    self.span_of(node),
                    "an extension-scoped function requires a body",
                );
                return None;
            }
            let parameters = self.resolve_parameters(&function)?;
            let return_type = function.return_type.and_then(|ty| self.resolve_type(ty));
            let calling_convention = self.resolve_calling_convention(function.calling_convention);
            if let Some(Initializer::Block(body_block)) = declaration.init {
                self.register_function(FunctionDeclaration {
                    node,
                    name: declaration.name.clone(),
                    body: body_block,
                    parameters,
                    return_type,
                    method_of: None,
                    exposed_symbol: None,
                    calling_convention,
                });
            }
            return None;
        }
        let this_parameter = &function.parameters[this_position.unwrap_or(0)];
        let value_type_target = matches!(
            owner_kind,
            TypeDefKind::Structure | TypeDefKind::Union | TypeDefKind::Variant
        );
        if value_type_target && !this_parameter.by_ref {
            self.error(
                this_parameter.span,
                "the 'this' parameter of a value-type method must be by-reference ('&')",
            );
            return None;
        }
        let method_convention = self.resolve_calling_convention(function.calling_convention);

        let mut parameters = Vec::new();
        for (index, parameter) in function.parameters.iter().enumerate() {
            if Some(index) == this_position {
                continue;
            }
            if let Some(expanded) = self.expand_array_parameter(parameter) {
                parameters.extend(expanded);
                continue;
            }
            let Some(parameter_type_node) = parameter.ty else {
                self.error(parameter.span, "method parameter requires a type");
                return None;
            };
            let parameter_type = self.resolve_type(parameter_type_node)?;
            let name = parameter
                .name
                .clone()
                .unwrap_or_else(|| format!("argument{index}"));
            parameters.push(ParameterInfo {
                name,
                ty: parameter_type,
                by_ref: parameter.by_ref,
                default: parameter.default,
                compile_time: parameter.compile_time,
            });
        }
        let return_type = function.return_type.and_then(|ty| self.resolve_type(ty));
        parameters.insert(
            0,
            ParameterInfo {
                name: "this".to_string(),
                ty: owner,
                by_ref: this_parameter.by_ref,
                default: None,
                compile_time: false,
            },
        );
        let owner_name = match self.result.types.get(owner) {
            Type::Named(index) => self.result.type_defs[*index as usize]
                .qualified_name
                .clone(),
            _ => self.spelling(owner),
        };
        let scope: Vec<String> = owner_name.split('.').map(str::to_string).collect();
        let spellings: Vec<String> = parameters
            .iter()
            .map(|parameter| self.spelling(parameter.ty))
            .collect();
        let mangled = mangle_function(&scope, &declaration.name, &spellings);
        let index = self.result.functions.len() as u32;
        let generic_env = self.captured_generic_env();
        let decl_scope = self.current_scope();
        // Methods with '#' parameters are templates (§11.5).
        let body = if parameters.iter().any(|parameter| parameter.compile_time) {
            None
        } else {
            body
        };
        self.result.functions.push(FunctionInfo {
            name: declaration.name.clone(),
            scope,
            node,
            body,
            parameters,
            method_of: Some(owner),
            static_owner: None,
            is_virtual: declaration.modifiers.virtual_ || declaration.modifiers.override_,
            is_override: declaration.modifiers.override_,
            is_sealed: declaration.modifiers.sealed,
            accessibility: crate::ast::Accessibility::Public,
            calling_convention: None,
            scoped: Vec::new(),
            is_abstract,
            return_type,
            mangled,
            exposed_symbol: None,
            generic_parameters: Vec::new(),
            generic_arguments: Vec::new(),
            template: None,
            decl_scope,
            generic_env,
            environment: false,
            captures: Vec::new(),
            constant_values: HashMap::new(),
            locals: Vec::new(),
            span: self.span_of(node),
        });
        if let Some(lengths) = self.array_binding_lengths.last() {
            for (name, value) in lengths {
                self.result.functions[index as usize]
                    .constant_values
                    .insert(name.clone(), *value as i128);
            }
        }
        self.result.functions[index as usize].accessibility = declaration.modifiers.accessibility;
        self.result.functions[index as usize].calling_convention = method_convention;
        self.result.functions[index as usize].scoped =
            self.resolve_scoped_targets(&declaration.modifiers, Some(owner), self.span_of(node));
        if declaration.modifiers.override_ {
            self.validate_override(index, owner);
        }
        if let Type::Named(owner_index) = self.result.types.get(owner).clone() {
            let definition = &mut self.result.type_defs[owner_index as usize];
            definition
                .methods
                .entry(declaration.name.clone())
                .or_default()
                .push(index);
            definition.method_list.push(index);
        } else {
            self.result
                .owner_methods
                .entry(owner)
                .or_default()
                .push(index);
        }
        Some(index)
    }

    /// Registers a `static fixed` constant (§9.15, §5.3): class-level immutable
    /// storage initialized by the launcher.
    fn create_static_constant(&mut self, node: NodeId, declaration: &FixedDecl, owner: TypeId) {
        let Some(ty_node) = declaration.ty else {
            self.error(self.span_of(node), "a static constant requires a type");
            return;
        };
        let Some(ty) = self.resolve_type(ty_node) else {
            return;
        };
        let init = match declaration.init {
            Some(Initializer::Expression(expression)) => Some(expression),
            Some(Initializer::Block(_)) => {
                self.error(
                    self.span_of(node),
                    "a static constant requires an expression initializer",
                );
                None
            }
            None => {
                self.error(self.span_of(node), "a constant must be initialized (§5.3)");
                None
            }
        };
        let Type::Named(owner_index) = self.types_of(owner) else {
            return;
        };
        let owner_name = self.result.type_defs[owner_index as usize]
            .qualified_name
            .clone();
        let mut scope: Vec<String> = owner_name.split('.').map(str::to_string).collect();
        scope.pop();
        let spelling = self.spelling(ty);
        let mangled = mangle_function(&scope, &declaration.name, &[spelling]);
        let index = self.result.globals.len() as u32;
        self.result.globals.push(GlobalInfo {
            accessibility: crate::ast::Accessibility::Public,
            scoped: Vec::new(),
            verbatim: declaration.modifiers.verbatim,
            name: declaration.name.clone(),
            scope,
            ty,
            init,
            fixed: true,
            mangled,
            node,
            span: self.span_of(node),
        });
        self.result.type_defs[owner_index as usize]
            .static_fields
            .insert(declaration.name.clone(), index);
    }

    /// Registers a `static accessor`: a class-level property backed by a static
    /// field, exposed as `Class.Name` (§3.8.2, §9.15).
    fn create_static_accessor(&mut self, node: NodeId, declaration: &AccessorDecl, owner: TypeId) {
        if !declaration.members.is_empty() {
            self.unsupported(
                self.span_of(node),
                "static accessors with explicit accessor members",
            );
            return;
        }
        let Some(backing) = declaration.backing else {
            self.error(
                self.span_of(node),
                "a static accessor requires a backing static field",
            );
            return;
        };
        let NodeKind::Name(name) = self.node_kind(backing) else {
            self.error(
                self.span_of(backing),
                "an accessor backing must be a field name",
            );
            return;
        };
        let Type::Named(owner_index) = self.types_of(owner) else {
            return;
        };
        let backing_global = self.result.type_defs[owner_index as usize]
            .static_fields
            .get(&name.name)
            .copied();
        let Some(backing_global) = backing_global else {
            self.error(
                self.span_of(backing),
                format!("unknown static backing field '{}'", name.name),
            );
            return;
        };
        let Some(ty) = self.resolve_type(declaration.ty) else {
            return;
        };
        let global_ty = self.result.globals[backing_global as usize].ty;
        if global_ty != ty {
            self.error(
                self.span_of(node),
                "the accessor type must match its backing static field",
            );
            return;
        }
        self.result.type_defs[owner_index as usize]
            .static_fields
            .insert(declaration.name.clone(), backing_global);
    }

    /// Registers a `static` method: no `this`, class-level storage (§9.15).
    fn create_static_method(
        &mut self,
        node: NodeId,
        declaration: &FixedDecl,
        owner: TypeId,
    ) -> Option<u32> {
        let Some(ty_node) = declaration.ty else {
            self.error(
                self.span_of(node),
                "a static method requires a function type",
            );
            return None;
        };
        let NodeKind::FunctionType(function) = self.node_kind(ty_node).clone() else {
            self.unsupported(self.span_of(node), "static function-valued constants");
            return None;
        };
        let Some(Initializer::Block(body)) = declaration.init else {
            self.error(self.span_of(node), "a static method requires a body");
            return None;
        };
        let parameters = self.resolve_parameters(&function)?;
        let return_type = function.return_type.and_then(|ty| self.resolve_type(ty));
        let method_convention = self.resolve_calling_convention(function.calling_convention);
        let Type::Named(owner_index) = self.types_of(owner) else {
            return None;
        };
        let _ = method_convention;
        let owner_name = self.result.type_defs[owner_index as usize]
            .qualified_name
            .clone();
        let scope: Vec<String> = owner_name.split('.').map(str::to_string).collect();
        let spellings: Vec<String> = parameters
            .iter()
            .map(|parameter| self.spelling(parameter.ty))
            .collect();
        let mangled = mangle_function(&scope, &declaration.name, &spellings);
        let generic_env = self.captured_generic_env();
        let decl_scope = self.current_scope();
        let index = self.result.functions.len() as u32;
        self.result.functions.push(FunctionInfo {
            name: declaration.name.clone(),
            scope,
            node,
            body: Some(FunctionBody::Block(body)),
            parameters,
            method_of: None,
            static_owner: Some(owner),
            is_virtual: false,
            is_override: false,
            is_sealed: false,
            accessibility: crate::ast::Accessibility::Public,
            calling_convention: None,
            scoped: Vec::new(),
            is_abstract: false,
            return_type,
            mangled,
            exposed_symbol: None,
            generic_parameters: Vec::new(),
            generic_arguments: Vec::new(),
            template: None,
            decl_scope,
            generic_env,
            environment: false,
            captures: Vec::new(),
            constant_values: HashMap::new(),
            locals: Vec::new(),
            span: self.span_of(node),
        });
        self.result.functions[index as usize].accessibility = declaration.modifiers.accessibility;
        self.result.functions[index as usize].calling_convention = method_convention;
        self.result.functions[index as usize].scoped =
            self.resolve_scoped_targets(&declaration.modifiers, Some(owner), self.span_of(node));
        self.result.type_defs[owner_index as usize]
            .static_methods
            .entry(declaration.name.clone())
            .or_default()
            .push(index);
        Some(index)
    }

    /// Checks that every interface method has a matching implementation in the
    /// class and records the per-interface dispatch table (§3.7.3).
    fn verify_interface_implementations(&mut self, class_type: TypeId) {
        let Type::Named(class_index) = self.types_of(class_type) else {
            return;
        };
        let interfaces = self.result.type_defs[class_index as usize]
            .interfaces
            .clone();
        let mut dispatch = Vec::new();
        for interface_type in interfaces {
            let Type::Named(interface_index) = self.types_of(interface_type) else {
                continue;
            };
            let method_list = self.result.type_defs[interface_index as usize]
                .method_list
                .clone();
            let interface_name = self.result.type_defs[interface_index as usize]
                .qualified_name
                .clone();
            let mut implementations = Vec::new();
            let mut ok = true;
            for method in method_list {
                let (name, parameters, span) = {
                    let info = &self.result.functions[method as usize];
                    (info.name.clone(), info.parameters.clone(), info.span)
                };
                let candidates = self.collect_methods(class_type, &name);
                let implementation = candidates.into_iter().find(|candidate| {
                    let info = &self.result.functions[*candidate as usize];
                    info.parameters.len() == parameters.len()
                        && info
                            .parameters
                            .iter()
                            .zip(parameters.iter())
                            .skip(1)
                            .all(|(candidate, interface)| candidate.ty == interface.ty)
                });
                match implementation {
                    Some(implementation) => implementations.push(implementation),
                    None => {
                        self.error(
                            span,
                            format!(
                                "class does not implement interface member '{name}' of '{interface_name}'"
                            ),
                        );
                        ok = false;
                    }
                }
            }
            if ok {
                dispatch.push((interface_type, implementations));
            }
        }
        self.result.type_defs[class_index as usize].interface_dispatch = dispatch;
    }

    pub(crate) fn resolve_interface(&mut self, node: NodeId) {
        let NodeKind::Interface(declaration) = self.node_kind(node) else {
            return;
        };
        let Some(type_id) = self.type_id_for_node(node) else {
            return;
        };
        for base in &declaration.bases {
            if let Some(base_type) = self.resolve_type(*base) {
                if matches!(self.types_of(base_type), Type::Named(index) if self.result.type_defs[index as usize].kind == TypeDefKind::Interface)
                {
                    self.unsupported(self.span_of(*base), "interface inheritance");
                } else {
                    self.error(
                        self.span_of(*base),
                        "an interface may only extend interfaces",
                    );
                }
            }
        }
        let members = declaration.members.clone();
        for member in members {
            match self.node_kind(member) {
                NodeKind::Extension(_) => {
                    self.resolve_extension_declaration(member, Some(type_id));
                }
                NodeKind::Accessor(_) => {
                    self.resolve_accessor_declaration(member, Some(type_id));
                }
                NodeKind::Operator(_) => {
                    self.resolve_operator_declaration(member, Some(type_id));
                }
                NodeKind::Module(_) => {}
                _ => {
                    self.error(
                        self.span_of(member),
                        "an interface body may only contain extension blocks and accessors",
                    );
                }
            }
        }
        let Type::Named(index) = self.types_of(type_id) else {
            return;
        };
        let definition = &mut self.result.type_defs[index as usize];
        // An interface is a reference type with no instance layout of its own.
        definition.size = 8;
        definition.align = 8;
        definition.instance_size = 0;
        definition.instance_align = 1;
        self.resolve_pending_accessors();
    }

    /// Resolves accessor declarations into getter/setter methods (§3.8.2).
    /// Resolves an operator definition into implementation functions (§4.9).
    pub(crate) fn resolve_operator_declaration(&mut self, node: NodeId, owner: Option<TypeId>) {
        let NodeKind::Operator(declaration) = self.node_kind(node) else {
            return;
        };
        let Some(owner) = owner else {
            self.unsupported(self.span_of(node), "operators outside type bodies");
            return;
        };
        let owner_index = match self.types_of(owner) {
            Type::Named(index) => Some(index),
            _ => None,
        };
        let is_interface = owner_index
            .map(|index| self.result.type_defs[index as usize].kind == TypeDefKind::Interface)
            .unwrap_or(false);
        if !matches!(
            declaration.modifiers.accessibility,
            Accessibility::Unspecified
        ) {
            self.error(
                self.span_of(node),
                "operator definitions must not carry accessibility modifiers",
            );
        }
        match &declaration.head {
            OperatorHead::Indexer {
                parameters,
                return_type,
            } => {
                let Some(return_ty) = return_type.and_then(|ty| self.resolve_type(ty)) else {
                    self.error(
                        self.span_of(node),
                        "an indexer operator requires a return type",
                    );
                    return;
                };
                let Some(indices) = self.resolve_operator_parameters(parameters) else {
                    return;
                };
                let members = match &declaration.body {
                    OperatorBody::AccessorMembers(members) => members.clone(),
                    OperatorBody::None if is_interface => Vec::new(),
                    _ => {
                        self.error(
                            self.span_of(node),
                            "an indexer operator body must be an accessor member list",
                        );
                        return;
                    }
                };
                let mut getter = None;
                let mut setter = None;
                for member in members {
                    let implementation = match (member.expression, member.body) {
                        (Some(expression), None) => Some(FunctionBody::Expression(expression)),
                        (None, Some(block)) => Some(FunctionBody::Block(block)),
                        (None, None) => None,
                        (Some(_), Some(_)) => {
                            self.error(
                                member.span,
                                "an accessor member takes either an expression or a block",
                            );
                            None
                        }
                    };
                    let body = if is_interface { None } else { implementation };
                    if !is_interface && body.is_none() {
                        self.error(
                            member.span,
                            "indexer accessor members require an implementation",
                        );
                        continue;
                    }
                    if member.is_getter {
                        getter = self.create_operator_method(
                            owner,
                            "operator[index]@get",
                            &indices,
                            Some(return_ty),
                            body,
                            member.span,
                            true,
                        );
                    } else {
                        let mut value_parameters = indices.clone();
                        value_parameters.push(ParameterInfo {
                            name: "value".to_string(),
                            ty: return_ty,
                            by_ref: false,
                            default: None,
                            compile_time: false,
                        });
                        setter = self.create_operator_method(
                            owner,
                            "operator[index]@set",
                            &value_parameters,
                            None,
                            body,
                            member.span,
                            true,
                        );
                    }
                }
                let operator = OperatorInfo {
                    kind: OperatorKind::Index,
                    method: u32::MAX,
                    getter,
                    setter,
                    implicitable: false,
                };
                self.register_operator_info(owner, owner_index, operator);
            }
            OperatorHead::Shorthand {
                symbol,
                parameters: _,
                return_type,
            }
            | OperatorHead::Notation {
                symbol,
                return_type,
                ..
            } => {
                let parameters = match &declaration.head {
                    OperatorHead::Shorthand { parameters, .. } => parameters.clone(),
                    _ => Vec::new(),
                };
                let Some(kind) = self.operator_kind(symbol, parameters.len()) else {
                    return;
                };
                let Some(arguments) = self.resolve_operator_parameters(&parameters) else {
                    return;
                };
                let (Some(return_ty), true) =
                    (return_type.and_then(|ty| self.resolve_type(ty)), true)
                else {
                    self.error(
                        self.span_of(node),
                        "an operator requires an explicit return type",
                    );
                    return;
                };
                let body = self.operator_body(&declaration.body, is_interface);
                let method_name = format!("operator{symbol}");
                let Some(method) = self.create_operator_method(
                    owner,
                    &method_name,
                    &arguments,
                    Some(return_ty),
                    body,
                    self.span_of(node),
                    false,
                ) else {
                    return;
                };
                let operator = OperatorInfo {
                    kind,
                    method,
                    getter: None,
                    setter: None,
                    implicitable: declaration.modifiers.implicitable,
                };
                self.register_operator_info(owner, owner_index, operator);
            }
            OperatorHead::Conversion {
                type_,
                parameters,
                return_type,
            } => {
                let Some(target) = self.resolve_type(*type_) else {
                    return;
                };
                // The function-type notation (`operator T: function(from: S) T`)
                // carries the parameters and return type in the type itself.
                let declared = return_type.and_then(|ty| self.resolve_type(ty));
                let function_signature =
                    declared.and_then(|declared| match self.result.types.get(declared).clone() {
                        Type::Function {
                            parameters: function_parameters,
                            return_type: function_return,
                            ..
                        } => Some((function_parameters, function_return)),
                        _ => None,
                    });
                let (arguments, declared_target) = match function_signature {
                    Some((function_parameters, function_return)) => (
                        function_parameters
                            .iter()
                            .enumerate()
                            .map(|(index, parameter)| ParameterInfo {
                                name: format!("argument{index}"),
                                ty: *parameter,
                                by_ref: false,
                                default: None,
                                compile_time: false,
                            })
                            .collect::<Vec<_>>(),
                        function_return,
                    ),
                    None => {
                        let Some(arguments) = self.resolve_operator_parameters(parameters) else {
                            return;
                        };
                        (arguments, Some(target))
                    }
                };
                if let Some(declared_target) = declared_target {
                    if declared_target != target {
                        self.error(
                            self.span_of(node),
                            "a conversion operator's return type must equal its target type",
                        );
                        return;
                    }
                }
                if arguments.iter().any(|parameter| parameter.by_ref) {
                    self.error(
                        self.span_of(node),
                        "a conversion operator must not take by-reference parameters",
                    );
                    return;
                }
                let body = self.operator_body(&declaration.body, is_interface);
                let method_name = format!("operator:{}", self.spelling(target));
                let Some(method) = self.create_operator_method(
                    owner,
                    &method_name,
                    &arguments,
                    Some(target),
                    body,
                    self.span_of(node),
                    false,
                ) else {
                    return;
                };
                let operator = OperatorInfo {
                    kind: OperatorKind::Conversion,
                    method,
                    getter: None,
                    setter: None,
                    implicitable: declaration.modifiers.implicitable,
                };
                self.register_operator_info(owner, owner_index, operator);
            }
        }
    }

    /// Records an operator on a named type or a non-named owner (array
    /// extensions, §3.8.3).
    fn register_operator_info(
        &mut self,
        owner: TypeId,
        owner_index: Option<u32>,
        operator: OperatorInfo,
    ) {
        if let Some(index) = owner_index {
            self.result.type_defs[index as usize]
                .operators
                .push(operator);
        } else {
            self.result
                .owner_operators
                .entry(owner)
                .or_default()
                .push(operator);
        }
    }

    fn operator_kind(&mut self, symbol: &str, parameter_count: usize) -> Option<OperatorKind> {
        let is_unary = parameter_count == 1;
        let kind = match symbol {
            "+" => OperatorKind::Add,
            "-" => {
                if is_unary {
                    OperatorKind::Negate
                } else {
                    OperatorKind::Subtract
                }
            }
            "*" => OperatorKind::Multiply,
            "/" => OperatorKind::Divide,
            "%" => OperatorKind::Remainder,
            "==" => OperatorKind::Equal,
            ">" => OperatorKind::Greater,
            "<" => OperatorKind::Less,
            "??" => OperatorKind::Coalesce,
            "!=" | "<=" | ">=" => {
                self.error(
                    self.span_of(NodeId::default()),
                    "derived operators must not be defined",
                );
                return None;
            }
            _ => {
                self.error(
                    self.span_of(NodeId::default()),
                    format!("operator '{symbol}' is not overloadable"),
                );
                return None;
            }
        };
        if kind == OperatorKind::Add && is_unary {
            self.error(
                self.span_of(NodeId::default()),
                "unary '+' is not overloadable",
            );
            return None;
        }
        if !matches!(
            kind,
            OperatorKind::Negate | OperatorKind::Coalesce | OperatorKind::Conversion
        ) && parameter_count != 2
        {
            self.error(
                self.span_of(NodeId::default()),
                "binary operators take exactly two parameters",
            );
            return None;
        }
        if kind == OperatorKind::Negate && parameter_count != 1 {
            self.error(
                self.span_of(NodeId::default()),
                "unary '-' takes exactly one parameter",
            );
            return None;
        }
        if kind == OperatorKind::Coalesce && parameter_count != 1 {
            self.error(
                self.span_of(NodeId::default()),
                "'??' takes exactly one parameter",
            );
            return None;
        }
        Some(kind)
    }

    fn operator_body(&mut self, body: &OperatorBody, is_interface: bool) -> Option<FunctionBody> {
        match body {
            OperatorBody::Expression(expression) => Some(FunctionBody::Expression(*expression)),
            OperatorBody::Block(block) => Some(FunctionBody::Block(*block)),
            OperatorBody::None => {
                if !is_interface {
                    self.error(
                        self.span_of(NodeId::default()),
                        "an operator definition requires a body",
                    );
                }
                None
            }
            OperatorBody::AccessorMembers(_) => {
                self.error(
                    self.span_of(NodeId::default()),
                    "accessor members are only allowed for indexer operators",
                );
                None
            }
        }
    }

    fn resolve_operator_parameters(
        &mut self,
        parameters: &[Parameter],
    ) -> Option<Vec<ParameterInfo>> {
        let mut resolved = Vec::new();
        for (index, parameter) in parameters.iter().enumerate() {
            let Some(parameter_type_node) = parameter.ty else {
                self.error(parameter.span, "operator parameters require a type");
                return None;
            };
            if parameter.by_ref {
                self.error(
                    parameter.span,
                    "operator parameters must not be by-reference",
                );
                return None;
            }
            let ty = self.resolve_type(parameter_type_node)?;
            let name = parameter
                .name
                .clone()
                .unwrap_or_else(|| format!("argument{index}"));
            if name == "this" {
                self.error(parameter.span, "operators must not take a 'this' parameter");
                return None;
            }
            resolved.push(ParameterInfo {
                name,
                ty,
                by_ref: false,
                default: None,
                compile_time: false,
            });
        }
        Some(resolved)
    }

    #[allow(clippy::too_many_arguments)]
    fn create_operator_method(
        &mut self,
        owner: TypeId,
        name: &str,
        parameters: &[ParameterInfo],
        return_type: Option<TypeId>,
        body: Option<FunctionBody>,
        span: Span,
        with_this: bool,
    ) -> Option<u32> {
        let mut all_parameters = Vec::new();
        if with_this {
            all_parameters.push(ParameterInfo {
                name: "this".to_string(),
                ty: owner,
                by_ref: true,
                default: None,
                compile_time: false,
            });
        }
        all_parameters.extend(parameters.iter().cloned());
        let owner_name = match self.result.types.get(owner) {
            Type::Named(index) => self.result.type_defs[*index as usize]
                .qualified_name
                .clone(),
            _ => self.spelling(owner),
        };
        let scope: Vec<String> = owner_name.split('.').map(str::to_string).collect();
        let spellings: Vec<String> = all_parameters
            .iter()
            .map(|parameter| self.spelling(parameter.ty))
            .collect();
        let mangled = mangle_function(&scope, name, &spellings);
        let index = self.result.functions.len() as u32;
        let generic_env = self.captured_generic_env();
        let decl_scope = self.current_scope();
        // Operators with '#' parameters are templates (§11.5).
        let body = if all_parameters
            .iter()
            .any(|parameter| parameter.compile_time)
        {
            None
        } else {
            body
        };
        self.result.functions.push(FunctionInfo {
            name: name.to_string(),
            scope,
            node: NodeId::default(),
            body,
            parameters: all_parameters,
            method_of: with_this.then_some(owner),
            static_owner: None,
            is_virtual: false,
            is_override: false,
            is_sealed: false,
            accessibility: crate::ast::Accessibility::Public,
            calling_convention: None,
            scoped: Vec::new(),
            is_abstract: body.is_none(),
            return_type,
            mangled,
            exposed_symbol: None,
            generic_parameters: Vec::new(),
            generic_arguments: Vec::new(),
            template: None,
            decl_scope,
            generic_env,
            environment: false,
            captures: Vec::new(),
            constant_values: HashMap::new(),
            locals: Vec::new(),
            span,
        });
        if let Some(lengths) = self.array_binding_lengths.last() {
            for (name, value) in lengths {
                self.result.functions[index as usize]
                    .constant_values
                    .insert(name.clone(), *value as i128);
            }
        }
        if with_this {
            if let Type::Named(owner_index) = self.result.types.get(owner).clone() {
                let definition = &mut self.result.type_defs[owner_index as usize];
                definition
                    .methods
                    .entry(name.to_string())
                    .or_default()
                    .push(index);
                definition.method_list.push(index);
            }
        }
        Some(index)
    }

    /// Resolves accessors that were waiting for their owner's field layout.
    pub(crate) fn resolve_pending_accessors(&mut self) {
        let pending: Vec<PendingAccessor> = std::mem::take(&mut self.pending_accessors);
        for entry in pending {
            self.generic_env.push(entry.generic_env);
            self.resolve_accessor_declaration(entry.node, Some(entry.owner));
            self.generic_env.pop();
        }
    }

    pub(crate) fn resolve_accessor_declaration(&mut self, node: NodeId, owner: Option<TypeId>) {
        let NodeKind::Accessor(declaration) = self.node_kind(node) else {
            return;
        };
        let Some(owner) = owner else {
            self.unsupported(self.span_of(node), "accessors outside type bodies");
            return;
        };
        let Type::Named(owner_index) = self.types_of(owner) else {
            return;
        };
        let owner_kind = self.result.type_defs[owner_index as usize].kind;
        if owner_kind == TypeDefKind::Enumeration {
            self.error(self.span_of(node), "enumerations cannot declare accessors");
            return;
        }
        let is_interface = owner_kind == TypeDefKind::Interface;
        if is_interface && declaration.backing.is_some() {
            self.error(
                self.span_of(node),
                "interface accessors must not have a backing field",
            );
            return;
        }
        let Some(ty) = self.resolve_type(declaration.ty) else {
            return;
        };
        let mut default_get = None;
        let mut default_set = None;
        if let Some(backing) = declaration.backing {
            match self.node_kind(backing) {
                NodeKind::Name(name) => match self.find_field(owner, &name.name) {
                    Some((field_owner, field_index)) => {
                        default_get = Some(FunctionBody::BackingGet {
                            owner: field_owner,
                            field_index,
                        });
                        default_set = Some(FunctionBody::BackingSet {
                            owner: field_owner,
                            field_index,
                        });
                    }
                    None => {
                        self.error(
                            self.span_of(backing),
                            format!("unknown backing field '{}'", name.name),
                        );
                    }
                },
                _ => self.error(
                    self.span_of(backing),
                    "an accessor backing must be a field name",
                ),
            }
        }
        let accessor_scoped =
            self.resolve_scoped_targets(&declaration.modifiers, Some(owner), self.span_of(node));
        let mut accessor = AccessorInfo {
            accessibility: declaration.modifiers.accessibility,
            scoped: accessor_scoped,
            name: declaration.name.clone(),
            ty,
            getter: None,
            setter: None,
        };
        for member in &declaration.members {
            if let Some(annotation) = member.ty {
                if let Some(annotated) = self.resolve_type(annotation) {
                    if annotated != ty {
                        self.error(
                            member.span,
                            "accessor member type must match the accessor type",
                        );
                    }
                }
            }
            let implementation = match (member.expression, member.body) {
                (Some(expression), None) => Some(FunctionBody::Expression(expression)),
                (None, Some(block)) => Some(FunctionBody::Block(block)),
                (Some(_), Some(_)) => {
                    self.error(
                        member.span,
                        "an accessor member takes either an expression or a block",
                    );
                    None
                }
                (None, None) => {
                    if member.is_getter {
                        default_get
                    } else {
                        default_set
                    }
                }
            };
            let body = if is_interface { None } else { implementation };
            if !is_interface && body.is_none() {
                self.error(
                    member.span,
                    "accessor member requires an implementation or a backing field",
                );
                continue;
            }
            let method = self.create_accessor_method(
                owner,
                &declaration.name,
                member.is_getter,
                ty,
                body,
                member.span,
            );
            if member.is_getter {
                accessor.getter = method;
            } else {
                accessor.setter = method;
            }
        }
        let definition = &mut self.result.type_defs[owner_index as usize];
        if definition
            .accessors
            .iter()
            .any(|existing| existing.name == accessor.name)
        {
            self.error(
                self.span_of(node),
                format!("accessor '{}' is declared twice", accessor.name),
            );
            return;
        }
        definition.accessors.push(accessor);
    }

    fn create_accessor_method(
        &mut self,
        owner: TypeId,
        name: &str,
        is_getter: bool,
        ty: TypeId,
        body: Option<FunctionBody>,
        span: Span,
    ) -> Option<u32> {
        let suffix = if is_getter { "get" } else { "set" };
        let method_name = format!("{name}@{suffix}");
        let mut parameters = vec![ParameterInfo {
            name: "this".to_string(),
            ty: owner,
            by_ref: true,
            default: None,
            compile_time: false,
        }];
        if !is_getter {
            parameters.push(ParameterInfo {
                name: "value".to_string(),
                ty,
                by_ref: false,
                default: None,
                compile_time: false,
            });
        }
        let return_type = if is_getter { Some(ty) } else { None };
        let owner_name = match self.result.types.get(owner) {
            Type::Named(index) => self.result.type_defs[*index as usize]
                .qualified_name
                .clone(),
            _ => self.spelling(owner),
        };
        let scope: Vec<String> = owner_name.split('.').map(str::to_string).collect();
        let spellings: Vec<String> = parameters
            .iter()
            .map(|parameter| self.spelling(parameter.ty))
            .collect();
        let mangled = mangle_function(&scope, &method_name, &spellings);
        let index = self.result.functions.len() as u32;
        let generic_env = self.captured_generic_env();
        let decl_scope = self.current_scope();
        self.result.functions.push(FunctionInfo {
            name: method_name.clone(),
            scope,
            node: NodeId::default(),
            body,
            parameters,
            method_of: Some(owner),
            static_owner: None,
            is_virtual: false,
            is_override: false,
            is_sealed: false,
            accessibility: crate::ast::Accessibility::Public,
            calling_convention: None,
            scoped: Vec::new(),
            is_abstract: body.is_none(),
            return_type,
            mangled,
            exposed_symbol: None,
            generic_parameters: Vec::new(),
            generic_arguments: Vec::new(),
            template: None,
            decl_scope,
            generic_env,
            environment: false,
            captures: Vec::new(),
            constant_values: HashMap::new(),
            locals: Vec::new(),
            span,
        });
        if let Some(lengths) = self.array_binding_lengths.last() {
            for (name, value) in lengths {
                self.result.functions[index as usize]
                    .constant_values
                    .insert(name.clone(), *value as i128);
            }
        }
        if let Type::Named(owner_index) = self.result.types.get(owner).clone() {
            let definition = &mut self.result.type_defs[owner_index as usize];
            definition
                .methods
                .entry(method_name)
                .or_default()
                .push(index);
            definition.method_list.push(index);
        } else {
            self.result
                .owner_methods
                .entry(owner)
                .or_default()
                .push(index);
        }
        Some(index)
    }

    /// Finds an accessor by name along the inheritance chain.
    fn find_accessor(&self, owner: TypeId, name: &str) -> Option<(TypeId, AccessorInfo)> {
        if let Some(accessor) = self
            .result
            .owner_accessors
            .get(&owner)
            .and_then(|accessors| accessors.iter().find(|accessor| accessor.name == name))
        {
            return Some((owner, accessor.clone()));
        }
        let mut current = Some(owner);
        let mut depth = 0;
        while let Some(ty) = current {
            depth += 1;
            if depth > 32 {
                break;
            }
            let Type::Named(index) = self.types_of(ty) else {
                break;
            };
            let definition = &self.result.type_defs[index as usize];
            if let Some(accessor) = definition
                .accessors
                .iter()
                .find(|accessor| accessor.name == name)
            {
                return Some((ty, accessor.clone()));
            }
            current = definition.base;
        }
        None
    }

    /// Finds a field by name along the inheritance chain.
    /// Whether a member with the given base accessibility may be used from the
    /// scope currently being checked (§9.1, §9.2, §9.3, §9.4).
    fn member_accessible(
        &self,
        accessibility: crate::ast::Accessibility,
        declaring: TypeId,
    ) -> bool {
        match accessibility {
            crate::ast::Accessibility::Public => true,
            crate::ast::Accessibility::Protected => {
                let Some(owner) = self.current_owner else {
                    return false;
                };
                let mut current = Some(owner);
                let mut depth = 0;
                while let Some(ty) = current {
                    depth += 1;
                    if depth > 32 {
                        break;
                    }
                    if ty == declaring {
                        return true;
                    }
                    let Type::Named(index) = self.types_of(ty) else {
                        break;
                    };
                    current = self.result.type_defs[index as usize].base;
                }
                false
            }
            // `Unspecified` defaults to private for members (§9.1).
            crate::ast::Accessibility::Private | crate::ast::Accessibility::Unspecified => {
                self.current_owner == Some(declaring)
            }
        }
    }

    /// Resolves the `scoped(...)` targets of a declaration (§9.5).
    fn resolve_scoped_targets(
        &mut self,
        modifiers: &crate::ast::Modifiers,
        owner: Option<TypeId>,
        span: Span,
    ) -> Vec<ScopedInfo> {
        let mut result = Vec::new();
        for target in &modifiers.scoped {
            match target {
                crate::ast::ScopedTarget::Operator { name } => result.push(ScopedInfo {
                    kind: match name {
                        Some(name) => ScopedKind::OperatorSymbol(name.clone()),
                        None => ScopedKind::Operator,
                    },
                    sealed: false,
                }),
                crate::ast::ScopedTarget::Named { sealed, name } => {
                    match self.lookup(name).cloned() {
                        Some(Symbol::Type(ty)) => result.push(ScopedInfo {
                            kind: ScopedKind::Type(ty),
                            sealed: *sealed,
                        }),
                        _ => {
                            let member = owner.and_then(|owner| {
                                self.collect_methods(owner, name).first().copied()
                            });
                            match member {
                                Some(method) => result.push(ScopedInfo {
                                    kind: ScopedKind::Member(method),
                                    sealed: *sealed,
                                }),
                                None => {
                                    if owner.is_some() {
                                        // The member may be declared later in the
                                        // same type; resolve after declarations.
                                        result.push(ScopedInfo {
                                            kind: ScopedKind::Pending(name.clone()),
                                            sealed: *sealed,
                                        });
                                    } else {
                                        self.error(span, format!("unknown scoped target '{name}'"));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        result
    }

    /// Whether `owner` is `target` or inherits from it.
    fn owner_derives_from(&self, owner: TypeId, target: TypeId) -> bool {
        let mut current = Some(owner);
        let mut depth = 0;
        while let Some(ty) = current {
            depth += 1;
            if depth > 32 {
                return false;
            }
            if ty == target {
                return true;
            }
            let Type::Named(index) = self.types_of(ty) else {
                return false;
            };
            current = self.result.type_defs[index as usize].base;
        }
        false
    }

    /// Whether the current function is the scoped member target or one of its
    /// overrides (§9.5).
    fn member_scope_allows(&self, target: u32, sealed: bool) -> bool {
        let Some(current) = self.current_function else {
            return false;
        };
        if current == target {
            return true;
        }
        if sealed {
            return false;
        }
        let base = &self.result.functions[target as usize];
        let current_function = &self.result.functions[current as usize];
        if !current_function.is_override || current_function.name != base.name {
            return false;
        }
        match (current_function.method_of, base.method_of) {
            (Some(current_owner), Some(base_owner)) => {
                self.owner_derives_from(current_owner, base_owner)
            }
            _ => false,
        }
    }

    /// Resolves `scoped` member-name targets that were declared before their
    /// member (§9.5).
    pub(crate) fn resolve_pending_scoped(&mut self) {
        for index in 0..self.result.type_defs.len() {
            let owner_type = self.result.types.intern(Type::Named(index as u32));
            let method_list = self.result.type_defs[index].method_list.clone();
            for method in method_list {
                let scoped = self.result.functions[method as usize].scoped.clone();
                let resolved = self.resolve_pending_scoped_list(owner_type, scoped);
                self.result.functions[method as usize].scoped = resolved;
            }
            let field_count = self.result.type_defs[index].fields.len();
            for field in 0..field_count {
                let scoped = self.result.type_defs[index].fields[field].scoped.clone();
                let resolved = self.resolve_pending_scoped_list(owner_type, scoped);
                self.result.type_defs[index].fields[field].scoped = resolved;
            }
            let accessor_count = self.result.type_defs[index].accessors.len();
            for accessor in 0..accessor_count {
                let scoped = self.result.type_defs[index].accessors[accessor]
                    .scoped
                    .clone();
                let resolved = self.resolve_pending_scoped_list(owner_type, scoped);
                self.result.type_defs[index].accessors[accessor].scoped = resolved;
            }
        }
    }

    fn resolve_pending_scoped_list(
        &mut self,
        owner: TypeId,
        scoped: Vec<ScopedInfo>,
    ) -> Vec<ScopedInfo> {
        scoped
            .into_iter()
            .map(|entry| match entry.kind {
                ScopedKind::Pending(name) => {
                    let member = self
                        .collect_methods(owner, &name)
                        .first()
                        .copied()
                        .map(ScopedKind::Member);
                    match member {
                        Some(kind) => ScopedInfo {
                            kind,
                            sealed: entry.sealed,
                        },
                        None => {
                            self.error(Span::default(), format!("unknown scoped target '{name}'"));
                            ScopedInfo {
                                kind: ScopedKind::Pending(name),
                                sealed: entry.sealed,
                            }
                        }
                    }
                }
                other => ScopedInfo {
                    kind: other,
                    sealed: entry.sealed,
                },
            })
            .collect()
    }

    /// Whether the function being checked is an operator whose parameter list
    /// contains `ty` (an operator of that type, §9.5).
    fn current_is_operator_of(&self, ty: TypeId) -> bool {
        let Some(index) = self.current_function else {
            return false;
        };
        let function = &self.result.functions[index as usize];
        function.name.starts_with("operator")
            && function
                .parameters
                .iter()
                .any(|parameter| parameter.ty == ty)
    }

    /// Whether the current function is an operator declared by `owner`, optionally
    /// for a specific symbol or conversion target (§9.5).
    fn current_operator_matches(&self, owner: TypeId, symbol: Option<&str>) -> bool {
        let Some(index) = self.current_function else {
            return false;
        };
        let function = &self.result.functions[index as usize];
        if !function.name.starts_with("operator") {
            return false;
        }
        let declared_by_owner = function.method_of == Some(owner) || {
            match self.types_of(owner) {
                Type::Named(index) => {
                    let qualified = &self.result.type_defs[index as usize].qualified_name;
                    let scope = function.scope.join(".");
                    &scope == qualified
                }
                _ => false,
            }
        };
        if !declared_by_owner {
            return false;
        }
        let Some(symbol) = symbol else {
            return true;
        };
        if function.name == format!("operator{symbol}") {
            return true;
        }
        if let Some(target) = function.name.strip_prefix("operator:") {
            return target == symbol;
        }
        false
    }

    /// Whether the innermost checked member is inside the accessible set of the
    /// declaration's scoped targets (§9.5).
    fn scope_allows(&self, declaring: TypeId, scoped: &[ScopedInfo]) -> bool {
        if scoped.is_empty() {
            return true;
        }
        scoped.iter().any(|target| match &target.kind {
            ScopedKind::Type(ty) => {
                if self.current_owner == Some(*ty) || self.current_is_operator_of(*ty) {
                    return true;
                }
                if target.sealed {
                    return false;
                }
                let Some(mut current) = self.current_owner else {
                    return false;
                };
                let mut depth = 0;
                loop {
                    depth += 1;
                    if depth > 32 {
                        return false;
                    }
                    if current == *ty {
                        return true;
                    }
                    let Type::Named(index) = self.types_of(current) else {
                        return false;
                    };
                    let Some(base) = self.result.type_defs[index as usize].base else {
                        return false;
                    };
                    current = base;
                }
            }
            ScopedKind::Operator => {
                self.current_is_operator_of(declaring)
                    || self.current_operator_matches(declaring, None)
            }
            ScopedKind::OperatorSymbol(symbol) => {
                self.current_operator_matches(declaring, Some(symbol))
            }
            ScopedKind::Member(member) => self.member_scope_allows(*member, target.sealed),
            ScopedKind::Pending(_) => false,
        })
    }

    fn find_field(&self, owner: TypeId, name: &str) -> Option<(TypeId, u32)> {
        let mut current = Some(owner);
        let mut depth = 0;
        while let Some(ty) = current {
            depth += 1;
            if depth > 32 {
                break;
            }
            let Type::Named(index) = self.types_of(ty) else {
                break;
            };
            let definition = &self.result.type_defs[index as usize];
            if let Some(position) = definition
                .fields
                .iter()
                .position(|field| field.name == name)
            {
                return Some((ty, position as u32));
            }
            current = definition.base;
        }
        None
    }

    pub(crate) fn resolve_variant(&mut self, node: NodeId) {
        let NodeKind::Variant(declaration) = self.node_kind(node) else {
            return;
        };
        let Some(type_id) = self.type_id_for_node(node) else {
            return;
        };
        let mut states: Vec<VariantStateInfo> = Vec::new();
        for member in &declaration.members {
            let payload = match member.payload {
                Some(payload_node) => self.resolve_type(payload_node),
                None => None,
            };
            if states.iter().any(|state| state.name == member.name) {
                self.error(
                    member.span,
                    format!("duplicate variant state '{}'", member.name),
                );
                continue;
            }
            states.push(VariantStateInfo {
                name: member.name.clone(),
                payload,
            });
        }
        if let Type::Named(index) = self.result.types.get(type_id).clone() {
            self.result.type_defs[index as usize].variant_states = states;
        }
        for extension in &declaration.extensions {
            self.resolve_extension_declaration(*extension, Some(type_id));
        }
    }

    pub(crate) fn resolve_union(&mut self, node: NodeId) {
        let NodeKind::Union(declaration) = self.node_kind(node) else {
            return;
        };
        let Some(type_id) = self.type_id_for_node(node) else {
            return;
        };
        let mut members: Vec<UnionMemberInfo> = Vec::new();
        for member in &declaration.members {
            let Some(ty) = self.resolve_type(member.ty) else {
                continue;
            };
            if members.iter().any(|existing| existing.name == member.name) {
                self.error(
                    self.span_of(member.ty),
                    format!("duplicate union member '{}'", member.name),
                );
                continue;
            }
            members.push(UnionMemberInfo {
                name: member.name.clone(),
                ty,
                offset: 0,
                aligned: member.modifiers.aligned,
            });
        }
        if let Type::Named(index) = self.result.types.get(type_id).clone() {
            self.result.type_defs[index as usize].union_members = members;
        }
        for extension in &declaration.extensions {
            self.resolve_extension_declaration(*extension, Some(type_id));
        }
    }

    /// Computes variant and union layouts once every member type is resolved.
    pub(crate) fn finalize_variant_union_layouts(&mut self) {
        let count = self.result.type_defs.len();
        for index in 0..count {
            match self.result.type_defs[index].kind {
                TypeDefKind::Variant => self.finalize_variant_layout(index),
                TypeDefKind::Union => self.finalize_union_layout(index),
                _ => {}
            }
        }
    }

    pub(crate) fn finalize_variant_layout(&mut self, index: usize) {
        let states = self.result.type_defs[index].variant_states.clone();
        // State space: the smallest unsigned integer type that can hold the
        // state count (§3.7.5).
        let state_count = states.len().max(1) as u64;
        let tag_bytes: u8 = if state_count <= 256 {
            1
        } else if state_count <= 65_536 {
            2
        } else if state_count <= u32::MAX as u64 {
            4
        } else {
            8
        };
        let tag_type = self.result.types.integer(false, tag_bytes);
        let mut payload_size = 0u64;
        let mut payload_align = 1u64;
        for state in &states {
            if let Some(payload) = state.payload {
                let (size, align) = self.size_align(payload);
                payload_size = payload_size.max(size);
                payload_align = payload_align.max(align);
            }
        }
        let tag_size = u64::from(tag_bytes);
        let payload_offset = round_up(tag_size, payload_align);
        let total = payload_offset + payload_size;
        let alignment = tag_size.max(payload_align);
        let definition = &mut self.result.type_defs[index];
        definition.tag_type = Some(tag_type);
        definition.payload_offset = payload_offset;
        definition.payload_size = payload_size;
        definition.instance_size = round_up(total, alignment);
        definition.instance_align = alignment;
        definition.size = definition.instance_size;
        definition.align = alignment;
    }

    fn finalize_union_layout(&mut self, index: usize) {
        let members = self.result.type_defs[index].union_members.clone();
        let mut offset = 0u64;
        let mut alignment = 1u64;
        let mut layout = Vec::with_capacity(members.len());
        for member in &members {
            let (size, member_align) = self.size_align(member.ty);
            // `aligned(N)` on a union member is a byte offset from the union's
            // base, not an alignment request (§9.14): it must not be adjusted.
            let member_offset = match member.aligned {
                Some(offset) => u64::from(offset),
                None => 0,
            };
            let effective_align = member_align;
            offset = offset.max(member_offset + size);
            alignment = alignment.max(effective_align);
            layout.push(UnionMemberInfo {
                name: member.name.clone(),
                ty: member.ty,
                offset: member_offset,
                aligned: member.aligned,
            });
        }
        let definition = &mut self.result.type_defs[index];
        definition.union_members = layout;
        definition.instance_size = round_up(offset, alignment);
        definition.instance_align = alignment;
        definition.size = definition.instance_size;
        definition.align = alignment;
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

        let mut members: Vec<(String, i128)> = Vec::new();
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
            if members.iter().any(|(name, _)| *name == member.name) {
                self.error(
                    member.span,
                    format!("duplicate enumeration member '{}'", member.name),
                );
                continue;
            }
            members.push((member.name.clone(), value));
        }

        let size = u64::from(bytes);
        let Some(type_id) = self.type_id_for_node(node) else {
            return;
        };
        let Type::Named(index) = self.result.types.get(type_id).clone() else {
            return;
        };
        let definition = &mut self.result.type_defs[index as usize];
        definition.enum_members = members;
        definition.underlying = Some(underlying);
        definition.size = size;
        definition.align = size;
    }

    /// Resolves a `; CallingConvention` expression to its numeric value (§3.6.3)
    /// and rejects conventions the current target cannot represent.
    fn resolve_calling_convention(&mut self, node: Option<NodeId>) -> Option<i32> {
        let node = node?;
        let ty = self.check_expr(node, None)?;
        let Type::Named(index) = self.types_of(ty) else {
            self.error(self.span_of(node), "expected a CallingConvention value");
            return None;
        };
        let (name, members) = {
            let definition = &self.result.type_defs[index as usize];
            (
                definition.qualified_name.clone(),
                definition.enum_members.clone(),
            )
        };
        if name != "CallingConvention" {
            self.error(self.span_of(node), "expected a CallingConvention value");
            return None;
        }
        let Some(Resolution::EnumMember { index: member, .. }) =
            self.result.resolutions[node as usize].clone()
        else {
            self.error(self.span_of(node), "expected a CallingConvention value");
            return None;
        };
        let Some((_, value)) = members.get(member as usize) else {
            self.error(self.span_of(node), "expected a CallingConvention value");
            return None;
        };
        let value = *value as i32;
        // LLVM's x86-64 backend supports the default, C, System V AMD64 and
        // Microsoft x64 conventions; the remainder are not representable here.
        if !matches!(value, 0 | 1 | 4 | 7) {
            self.error(
                self.span_of(node),
                "this calling convention is not supported on the current target",
            );
            return None;
        }
        Some(value)
    }

    pub(crate) fn resolve_function_signature(&mut self, node: NodeId, declaration: &FixedDecl) {
        let Some(ty_node) = declaration.ty else {
            // Not a function definition: handled as a global or local constant.
            self.resolve_global_fixed(node, declaration);
            return;
        };
        let NodeKind::FunctionType(function) = self.node_kind(ty_node) else {
            self.resolve_global_fixed(node, declaration);
            return;
        };
        let body = match declaration.init {
            Some(Initializer::Block(block)) => block,
            _ => {
                self.unsupported(self.span_of(node), "function-valued constants (lambdas)");
                return;
            }
        };
        let Some(parameters) = self.resolve_parameters(&function) else {
            return;
        };
        let return_type = function.return_type.and_then(|ty| self.resolve_type(ty));
        let calling_convention = self.resolve_calling_convention(function.calling_convention);
        self.register_function(FunctionDeclaration {
            node,
            name: declaration.name.clone(),
            body,
            parameters,
            return_type,
            method_of: None,
            exposed_symbol: None,
            calling_convention,
        });
    }

    /// Expands the array parameter sugar
    /// `message[length: cardinal(4)]: cardinal(1)` into two parameters, a pointer
    /// to the element type and the bound dimension length (§2.5, §3.6.1).
    fn expand_array_parameter(&mut self, parameter: &Parameter) -> Option<Vec<ParameterInfo>> {
        let ty_node = parameter.ty?;
        let NodeKind::ArrayType(array) = self.node_kind(ty_node) else {
            return None;
        };
        if array.dimensions.is_empty() {
            return None;
        }
        let mut bindings = Vec::new();
        for dimension in &array.dimensions {
            match dimension {
                ArrayDimension::Binding { name, ty } => bindings.push((name.clone(), *ty)),
                ArrayDimension::Size(_) => return None,
            }
        }
        if bindings.is_empty() {
            return None;
        }
        let element = self.resolve_type(array.element)?;
        let pointer = self.result.types.pointer(Some(element));
        let name = parameter
            .name
            .clone()
            .unwrap_or_else(|| "message".to_string());
        let mut expanded = vec![ParameterInfo {
            name,
            ty: pointer,
            by_ref: false,
            default: None,
            compile_time: false,
        }];
        for (binding_name, binding_type_node) in bindings {
            let binding_type = self.resolve_type(binding_type_node)?;
            expanded.push(ParameterInfo {
                name: binding_name,
                ty: binding_type,
                by_ref: false,
                default: None,
                compile_time: false,
            });
        }
        Some(expanded)
    }

    pub(crate) fn resolve_parameters(
        &mut self,
        function: &crate::ast::FunctionType,
    ) -> Option<Vec<ParameterInfo>> {
        let mut parameters = Vec::new();
        for (index, parameter) in function.parameters.iter().enumerate() {
            if let Some(expanded) = self.expand_array_parameter(parameter) {
                parameters.extend(expanded);
                continue;
            }
            let Some(parameter_type_node) = parameter.ty else {
                self.error(parameter.span, "parameter requires a type");
                return None;
            };
            let parameter_type = self.resolve_type(parameter_type_node)?;
            let name = parameter
                .name
                .clone()
                .unwrap_or_else(|| format!("argument{index}"));
            parameters.push(ParameterInfo {
                name,
                ty: parameter_type,
                by_ref: parameter.by_ref,
                default: parameter.default,
                compile_time: parameter.compile_time,
            });
        }
        Some(parameters)
    }

    fn register_function(&mut self, declaration: FunctionDeclaration) -> u32 {
        let FunctionDeclaration {
            node,
            name,
            body,
            parameters,
            return_type,
            method_of,
            exposed_symbol,
            calling_convention,
        } = declaration;
        let scope = self.scope_path.clone();
        let spellings: Vec<String> = parameters
            .iter()
            .map(|parameter| self.spelling(parameter.ty))
            .collect();
        let mangled = exposed_symbol
            .clone()
            .unwrap_or_else(|| mangle_function(&scope, &name, &spellings));
        let index = self.result.functions.len() as u32;
        let generic_env = self.captured_generic_env();
        let decl_scope = self.current_scope();
        // Functions with `#` parameters are templates: only their
        // specializations are checked and lowered (§11.5).
        let is_constant_template = parameters.iter().any(|parameter| parameter.compile_time);
        self.result.functions.push(FunctionInfo {
            name: name.clone(),
            scope,
            node,
            body: if is_constant_template {
                None
            } else {
                Some(FunctionBody::Block(body))
            },
            parameters,
            method_of,
            static_owner: None,
            is_virtual: false,
            is_override: false,
            is_sealed: false,
            accessibility: crate::ast::Accessibility::Public,
            calling_convention,
            scoped: Vec::new(),
            is_abstract: false,
            return_type,
            mangled,
            exposed_symbol: None,
            generic_parameters: Vec::new(),
            generic_arguments: Vec::new(),
            template: None,
            decl_scope,
            generic_env,
            environment: false,
            captures: Vec::new(),
            constant_values: HashMap::new(),
            locals: Vec::new(),
            span: self.span_of(node),
        });
        self.declare_functions(name, index);
        index
    }

    pub(crate) fn resolve_linkable_signature(&mut self, node: NodeId, declaration: &LinkableDecl) {
        let Some(ty_node) = declaration.ty else {
            self.unsupported(self.span_of(node), "linkable declarations without a type");
            return;
        };
        let NodeKind::FunctionType(function) = self.node_kind(ty_node) else {
            self.unsupported(self.span_of(node), "linkable variables");
            return;
        };
        let body = match declaration.init {
            Some(Initializer::Block(block)) => block,
            _ => {
                self.unsupported(self.span_of(node), "linkable declarations without a body");
                return;
            }
        };
        let Some(parameters) = self.resolve_parameters(&function) else {
            return;
        };
        let return_type = function.return_type.and_then(|ty| self.resolve_type(ty));
        let exposed = declaration
            .symbol
            .clone()
            .unwrap_or_else(|| declaration.name.clone());
        self.register_function(FunctionDeclaration {
            node,
            name: declaration.name.clone(),
            body,
            parameters,
            return_type,
            method_of: None,
            exposed_symbol: Some(exposed),
            calling_convention: None,
        });
    }

    /// Builds the virtual dispatch tables after all declarations are resolved.
    pub(crate) fn build_vtables(&mut self) {
        for index in 0..self.result.type_defs.len() as u32 {
            // Generic declarations are templates; only their instances get
            // layouts and virtual dispatch tables (§10.1).
            if !self.result.type_defs[index as usize]
                .generic_parameters
                .is_empty()
            {
                continue;
            }
            if self.result.type_defs[index as usize].kind == TypeDefKind::Class {
                self.build_vtable(index);
            }
        }
    }

    fn build_vtable(&mut self, index: u32) -> Vec<u32> {
        if self.result.type_defs[index as usize].vtable_ready {
            return self.result.type_defs[index as usize].vtable.clone();
        }
        let base = self.result.type_defs[index as usize].base;
        let mut slots = match base {
            Some(base) => match self.result.types.get(base).clone() {
                Type::Named(base_index) => self.build_vtable(base_index),
                _ => Vec::new(),
            },
            None => Vec::new(),
        };
        let method_list = self.result.type_defs[index as usize].method_list.clone();
        let class_name = self.result.type_defs[index as usize].qualified_name.clone();
        for method in method_list {
            let info = &self.result.functions[method as usize];
            if !info.is_virtual {
                continue;
            }
            if info.is_override {
                let name = info.name.clone();
                let parameters = info.parameters.clone();
                let slot = slots.iter().position(|candidate| {
                    let base_info = &self.result.functions[*candidate as usize];
                    base_info.name == name
                        && base_info.parameters.len() == parameters.len()
                        && base_info
                            .parameters
                            .iter()
                            .zip(parameters.iter())
                            .skip(1)
                            .all(|(base, own)| base.ty == own.ty)
                });
                match slot {
                    Some(slot) => slots[slot] = method,
                    None => {
                        let span = self.result.functions[method as usize].span;
                        self.error(
                            span,
                            format!("'override' does not match a virtual method of '{class_name}'"),
                        );
                    }
                }
            } else {
                slots.push(method);
            }
        }
        let definition = &mut self.result.type_defs[index as usize];
        definition.vtable = slots.clone();
        definition.vtable_ready = true;
        slots
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
        if init.is_none() {
            self.error(self.span_of(node), "a constant must be initialized (§5.3)");
        }
        let scope = self.scope_path.clone();
        let mangled = mangle_function(&scope, &declaration.name, &[self.spelling(ty)]);
        let index = self.result.globals.len() as u32;
        self.result.globals.push(GlobalInfo {
            accessibility: crate::ast::Accessibility::Public,
            scoped: Vec::new(),
            verbatim: declaration.modifiers.verbatim,
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
                accessibility: crate::ast::Accessibility::Public,
                scoped: Vec::new(),
                verbatim: declaration.modifiers.verbatim,
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

    /// Evaluates a compile-time constant boolean used by `#if` (§11.6).
    pub(crate) fn eval_compile_time_bool(&mut self, node: NodeId, depth: u32) -> Option<bool> {
        if depth > 16 {
            return None;
        }
        match self.node_kind(node).clone() {
            NodeKind::BooleanLiteral(value) => Some(value),
            NodeKind::Unary(unary) if unary.op == UnaryOp::LogicalNot => {
                Some(!self.eval_compile_time_bool(unary.operand, depth + 1)?)
            }
            NodeKind::Binary(binary) => match binary.op {
                BinaryOp::LogicalAnd => {
                    let left = self.eval_compile_time_bool(binary.left, depth + 1)?;
                    if !left {
                        Some(false)
                    } else {
                        self.eval_compile_time_bool(binary.right, depth + 1)
                    }
                }
                BinaryOp::LogicalOr => {
                    let left = self.eval_compile_time_bool(binary.left, depth + 1)?;
                    if left {
                        Some(true)
                    } else {
                        self.eval_compile_time_bool(binary.right, depth + 1)
                    }
                }
                BinaryOp::Equal
                | BinaryOp::NotEqual
                | BinaryOp::Greater
                | BinaryOp::Less
                | BinaryOp::GreaterEqual
                | BinaryOp::LessEqual => {
                    let left = self.eval_const_int(binary.left)?;
                    let right = self.eval_const_int(binary.right)?;
                    Some(match binary.op {
                        BinaryOp::Equal => left == right,
                        BinaryOp::NotEqual => left != right,
                        BinaryOp::Greater => left > right,
                        BinaryOp::Less => left < right,
                        BinaryOp::GreaterEqual => left >= right,
                        BinaryOp::LessEqual => left <= right,
                        _ => return None,
                    })
                }
                _ => None,
            },
            NodeKind::Call(call) => {
                // Platform.Is is a compile-time constant (§15.3).
                let name = builtin_namespace_call(self.ast, call.callee, "Platform")?;
                if name != "Is" || call.arguments.len() != 1 {
                    return None;
                }
                let argument = &call.arguments[0];
                match self.node_kind(argument.value) {
                    NodeKind::StringLiteral(literal) => {
                        let decoded = literal::decode_string(&literal.raw).ok()?;
                        let name = String::from_utf8_lossy(&decoded.bytes).into_owned();
                        Some(
                            Self::platform_kind_from_name(&name)
                                .is_some_and(|kind| kind == Self::host_platform_kind()),
                        )
                    }
                    NodeKind::MemberAccess(access) => {
                        let Some(Symbol::Type(type_id)) = self.try_resolve_name(access.base) else {
                            return None;
                        };
                        let Type::Named(index) = self.types_of(type_id) else {
                            return None;
                        };
                        let definition = &self.result.type_defs[index as usize];
                        let (_, value) = definition
                            .enum_members
                            .iter()
                            .find(|(member, _)| *member == access.name)?;
                        Some(*value as u32 == Self::host_platform_kind())
                    }
                    _ => None,
                }
            }
            NodeKind::Name(name) => match self
                .top_level_constants
                .get(&name.name)
                .map(|(value, _)| *value)
                .or_else(|| {
                    self.constant_env
                        .iter()
                        .rev()
                        .find_map(|environment| environment.get(&name.name).copied())
                }) {
                Some(value) => Some(value != 0),
                None => match self.lookup(&name.name).cloned() {
                    Some(Symbol::Global(global)) => {
                        let (fixed, init) = {
                            let info = &self.result.globals[global as usize];
                            (info.fixed, info.init)
                        };
                        if !fixed {
                            return None;
                        }
                        let init = init?;
                        match self.node_kind(init).clone() {
                            NodeKind::BooleanLiteral(_)
                            | NodeKind::Unary(_)
                            | NodeKind::Binary(_) => self.eval_compile_time_bool(init, depth + 1),
                            _ => None,
                        }
                    }
                    Some(Symbol::Local(_)) => None,
                    _ => None,
                },
            },
            _ => None,
        }
    }

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
            NodeKind::CompileTimeExpr(inner) => self.eval_const_int(inner),
            // The compiler API call `Type.SizeOf<T>()` is a compile-time constant
            // (§11.8).
            NodeKind::Call(call) => {
                let argument = builtin_size_of_type_argument(self.ast, call.callee)?;
                let ty = self.resolve_type(argument)?;
                Some(self.size_align(ty).0 as i128)
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
            NodeKind::Name(name) => {
                // Top-level compile-time variables are compile-time bindings
                // (§11.2).
                if let Some((value, _)) = self.top_level_constants.get(&name.name) {
                    return Some(*value);
                }
                // Compile-time constant parameters are compile-time bindings
                // (§11.5).
                for environment in self.constant_env.iter().rev() {
                    if let Some(value) = environment.get(&name.name) {
                        return Some(*value);
                    }
                }
                // Compile-time block locals act as constants (§11.4).
                for scope in self.compile_time_constants.iter().rev() {
                    if let Some(value) = scope.get(&name.name) {
                        return Some(*value);
                    }
                }
                match self.lookup(&name.name).cloned() {
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
                }
            }
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

/// Metadata needed to register a function or method.
struct FunctionDeclaration {
    node: NodeId,
    name: String,
    body: NodeId,
    parameters: Vec<ParameterInfo>,
    return_type: Option<TypeId>,
    method_of: Option<TypeId>,
    exposed_symbol: Option<String>,
    calling_convention: Option<i32>,
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
        // Instantiating a generic type or function can add functions while
        // bodies are being checked (§10.2), so the loop follows the growing
        // function list instead of a snapshot.
        let mut index = self.bodies_checked;
        while index < self.result.functions.len() {
            let current = index;
            index += 1;
            let (body, parameters, method_of, span, is_abstract) = {
                let function = &self.result.functions[current];
                (
                    function.body,
                    function.parameters.clone(),
                    function.method_of,
                    function.span,
                    function.is_abstract,
                )
            };
            if is_abstract || body.is_none() {
                continue;
            }
            let body = body.expect("body checked above");
            let generic_env = self.result.functions[current].generic_env.clone();
            let static_owner = self.result.functions[current].static_owner;
            let environment = self.result.functions[current].environment;
            let captures = self.result.functions[current].captures.clone();
            let constant_values = self.result.functions[current].constant_values.clone();
            self.current_function = Some(current as u32);
            self.current_owner = method_of.or(static_owner);
            self.loop_labels.clear();
            self.heap_units.clear();
            self.heap_unit_locals.clear();
            self.early_dispose_warned.clear();
            self.push_scope();
            if environment {
                // Captured names are visible inside the literal (§2.7.9).
                self.push_scope();
                for capture in &captures {
                    self.declare(capture.name.clone(), Symbol::Local(capture.local), span);
                }
            }
            if !constant_values.is_empty() {
                self.constant_env.push(constant_values.clone());
            }
            self.generic_env.push(generic_env);
            // Static fields of the owning class are in scope in a static method
            // body (§9.15).
            if let Some(owner) = static_owner {
                if let Type::Named(index) = self.types_of(owner) {
                    let fields: Vec<(String, u32)> = self.result.type_defs[index as usize]
                        .static_fields
                        .iter()
                        .map(|(name, global)| (name.clone(), *global))
                        .collect();
                    for (name, global) in fields {
                        self.declare(name, Symbol::Global(global), span);
                    }
                }
            }
            let mut llvm_parameter_index = u32::from(environment);
            if let Some(owner) = method_of {
                self.declare("this".to_string(), Symbol::This { owner }, span);
                llvm_parameter_index = 1;
            }
            for (position, parameter) in parameters.iter().enumerate() {
                if method_of.is_some() && position == 0 {
                    continue;
                }
                self.add_local(
                    parameter.name.clone(),
                    parameter.ty,
                    false,
                    Some(llvm_parameter_index),
                    if parameter.by_ref {
                        Storage::Reference
                    } else {
                        Storage::Value
                    },
                    span,
                );
                llvm_parameter_index += 1;
            }
            let previous_in_body = self.in_function_body;
            self.in_function_body = true;
            match body {
                FunctionBody::Block(block) => self.check_body_block(block),
                FunctionBody::Expression(expression) => {
                    self.check_expr(expression, None);
                }
                FunctionBody::BackingGet { .. } | FunctionBody::BackingSet { .. } => {}
                FunctionBody::BuiltinAllocate | FunctionBody::BuiltinDeallocate => {}
            }
            self.in_function_body = previous_in_body;
            // NOLAA never-disposed warning (§12.2): conservative — units that
            // escaped the function (returned, passed on or stored) are exempt.
            let pending: Vec<(Span, String)> = self
                .heap_units
                .iter()
                .filter(|unit| {
                    !unit.manual
                        && !unit.disposed
                        && !unit.abandoned
                        && !unit.escaped
                        && !self
                            .result
                            .jump_dispose
                            .values()
                            .any(|locals| locals.contains(&unit.local))
                })
                .map(|unit| {
                    (
                        unit.span,
                        format!("heap allocation '{}' is never disposed", unit.name),
                    )
                })
                .collect();
            for (span, message) in pending {
                self.warn(span, message);
            }
            if !constant_values.is_empty() {
                self.constant_env.pop();
            }
            if environment {
                self.pop_scope();
            }
            self.generic_env.pop();
            self.pop_scope();
            self.current_function = None;
            self.current_owner = None;
        }
        self.bodies_checked = self.result.functions.len();
        self.check_return_paths();
    }

    /// Diagnoses functions with a non-empty return type that can fall off the end
    /// of their body (§5.10.2). The judgement is deliberately conservative: a
    /// construct is only accepted as returning when every path through it returns.
    fn check_return_paths(&mut self) {
        for index in 0..self.result.functions.len() {
            let (body, return_type, span, is_template, is_abstract) = {
                let function = &self.result.functions[index];
                (
                    function.body,
                    function.return_type,
                    function.span,
                    function.is_template(),
                    function.is_abstract,
                )
            };
            if is_template || is_abstract {
                continue;
            }
            let Some(return_type) = return_type else {
                continue;
            };
            if return_type == TypeId::VOID {
                continue;
            }
            let Some(FunctionBody::Block(block)) = body else {
                continue;
            };
            if self.block_always_returns(block) {
                continue;
            }
            self.error(span, "not all code paths of this function return a value");
        }
    }

    fn block_always_returns(&self, node: NodeId) -> bool {
        match self.node_kind(node) {
            NodeKind::Block(statements) => statements
                .last()
                .is_some_and(|statement| self.statement_always_returns(*statement)),
            NodeKind::VerbatimBlock(inner) => self.block_always_returns(inner),
            _ => false,
        }
    }

    fn statement_always_returns(&self, node: NodeId) -> bool {
        match self.node_kind(node) {
            NodeKind::Return(_) => true,
            // A bare block used as the last statement of a body/block.
            NodeKind::Block(_) | NodeKind::VerbatimBlock(_) => self.block_always_returns(node),
            NodeKind::If(statement) => {
                let Some(else_block) = statement.else_block else {
                    return false;
                };
                statement
                    .branches
                    .iter()
                    .all(|branch| self.block_always_returns(branch.body))
                    && self.block_always_returns(else_block)
            }
            NodeKind::Match(statement) => {
                let has_wildcard = statement.cases.iter().any(|case| {
                    matches!(case.pattern, Some(Pattern::Wildcard)) && case.guard.is_none()
                });
                has_wildcard
                    && statement
                        .cases
                        .iter()
                        .all(|case| case.guard.is_none() && self.block_always_returns(case.body))
            }
            NodeKind::Trap(statement) => {
                self.block_always_returns(statement.body)
                    && statement
                        .catches
                        .iter()
                        .all(|catch| self.block_always_returns(catch.body))
            }
            NodeKind::DoWhile(statement) => self.block_always_returns(statement.body),
            NodeKind::CompileTimeBlock(block) => self.block_always_returns(block),
            _ => false,
        }
    }

    pub(crate) fn check_global_initializers(&mut self) {
        // Instance field initializers are expressions too: check their types and
        // the function-value access rule (§3.8.1, §9.1).
        let field_owners: Vec<u32> = (0..self.result.type_defs.len())
            .filter(|index| {
                let definition = &self.result.type_defs[*index];
                definition.generic_parameters.is_empty()
                    && definition.template.is_none()
                    && !definition.field_initializers.is_empty()
            })
            .map(|index| index as u32)
            .collect();
        for owner in field_owners {
            let fields = self.result.type_defs[owner as usize].fields.clone();
            let initializers = self.result.type_defs[owner as usize]
                .field_initializers
                .clone();
            // Field initializers are resolved in the declaring type's scope: its
            // methods and static members are visible (§3.8.1, §9.1).
            self.push_scope();
            let owner_type = self.result.types.intern(Type::Named(owner));
            let saved_owner = self.current_owner;
            self.current_owner = Some(owner_type);
            let methods: Vec<(String, Vec<u32>)> = self.result.type_defs[owner as usize]
                .methods
                .iter()
                .map(|(name, overloads)| (name.clone(), overloads.clone()))
                .collect();
            for (name, overloads) in methods {
                self.declare(name, Symbol::Functions(overloads), Span::default());
            }
            let statics: Vec<(String, u32)> = self.result.type_defs[owner as usize]
                .static_fields
                .iter()
                .map(|(name, global)| (name.clone(), *global))
                .collect();
            for (name, global) in statics {
                self.declare(name, Symbol::Global(global), Span::default());
            }
            for (field, initializer) in fields.iter().zip(initializers.iter()) {
                let Some(initializer) = initializer else {
                    continue;
                };
                self.initializer_depth += 1;
                let inferred = self.check_expr(*initializer, Some(field.ty));
                self.initializer_depth -= 1;
                if let Some(inferred) = inferred {
                    self.expect_assignable(self.span_of(*initializer), inferred, field.ty);
                }
                self.check_function_value_access(
                    field.accessibility,
                    *initializer,
                    self.span_of(*initializer),
                );
            }
            self.current_owner = saved_owner;
            self.pop_scope();
        }
        for index in 0..self.result.globals.len() {
            let (init, ty, span) = {
                let global = &self.result.globals[index];
                (global.init, global.ty, global.span)
            };
            if let Some(initializer) = init {
                if ty == TypeId::ERROR {
                    continue;
                }
                self.initializer_depth += 1;
                let inferred = self.check_expr(initializer, Some(ty));
                self.initializer_depth -= 1;
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
                let accessibility = self.result.globals[index].accessibility;
                self.check_function_value_access(accessibility, initializer, span);
            }
        }
    }

    /// Checks the function-value access rule (§9.1): a function value must not be
    /// stored in a location with a wider accessible range.
    fn check_function_value_access(
        &mut self,
        carrier: crate::ast::Accessibility,
        value: NodeId,
        span: Span,
    ) {
        let value_access = match self.result.resolutions[value as usize].clone() {
            Some(Resolution::Function(index)) => {
                self.result.functions[index as usize].accessibility
            }
            _ => return,
        };
        let rank = |accessibility: crate::ast::Accessibility| match accessibility {
            crate::ast::Accessibility::Public => 2,
            crate::ast::Accessibility::Protected => 1,
            crate::ast::Accessibility::Private | crate::ast::Accessibility::Unspecified => 0,
        };
        if rank(carrier) > rank(value_access) {
            self.error(
                span,
                "a function value with a narrower access right cannot be stored in a wider location",
            );
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
        storage: Storage,
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
            storage,
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

    /// Whether `==` is defined for `ty` without a user-defined operator.
    fn has_builtin_equality(&self, ty: TypeId) -> bool {
        self.is_numeric(ty)
            || self.is_boolean(ty)
            || matches!(self.type_of(ty), Type::Pointer { .. })
            || matches!(self.type_of(ty), Type::Named(index) if self.result.type_defs[index as usize].underlying.is_some())
            || self.variant_has_builtin_equality(ty)
    }

    /// Whether `==` is defined for a variant: same state and equal payload, with
    /// every payload itself comparable (§3.7.5).
    fn variant_has_builtin_equality(&self, ty: TypeId) -> bool {
        let Type::Named(index) = self.type_of(ty) else {
            return false;
        };
        let definition = &self.result.type_defs[index as usize];
        if definition.kind != TypeDefKind::Variant {
            return false;
        }
        definition
            .variant_states
            .iter()
            .all(|state| match state.payload {
                None => true,
                Some(payload) => self.has_builtin_equality(payload),
            })
    }

    fn is_boolean(&self, ty: TypeId) -> bool {
        matches!(self.result.types.get(ty), Type::Boolean)
    }

    pub(crate) fn is_assignable(&self, from: TypeId, to: TypeId) -> bool {
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
        // A class converts implicitly to the interfaces it implements, including
        // interfaces implemented by a base class (§3.7.3).
        if matches!(self.type_of(to), Type::Named(index) if self.result.type_defs[index as usize].kind == TypeDefKind::Interface)
        {
            let mut current = Some(from);
            let mut depth = 0;
            while let Some(ty) = current {
                depth += 1;
                if depth > 32 {
                    break;
                }
                if ty == to {
                    return true;
                }
                let Type::Named(index) = self.type_of(ty) else {
                    return false;
                };
                let definition = &self.result.type_defs[index as usize];
                if definition.interfaces.contains(&to) {
                    return true;
                }
                current = definition.base;
            }
        }
        // Derived classes convert implicitly to their base classes (§3.7.1).
        if matches!(self.type_of(from), Type::Named(_))
            && matches!(self.type_of(to), Type::Named(_))
        {
            let mut current = Some(from);
            let mut depth = 0;
            while let Some(ty) = current {
                depth += 1;
                if depth > 32 {
                    break;
                }
                if ty == to {
                    return true;
                }
                let Type::Named(index) = self.type_of(ty) else {
                    return false;
                };
                current = self.result.type_defs[index as usize].base;
            }
        }
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
        // Disposal tracking is restored at block boundaries so that disjoint
        // branches do not look like a double dispose (§5.8).
        let disposed_before = self.disposed_locals.clone();
        let block_before = self.current_block;
        self.current_block = node;
        self.block_stack.push(node);
        for statement in statements {
            self.check_statement(statement);
        }
        self.block_stack.pop();
        self.current_block = block_before;
        self.plan_auto_dispose(node);
        self.disposed_locals = disposed_before;
        self.pop_scope();
    }

    /// Schedules NOLAA auto-dispose before `break`/`continue` leaves the loop
    /// body scope (§12.2): units declared inside the innermost loop body are
    /// still alive and safe to dispose.
    fn plan_loop_jump_dispose(&mut self, node: NodeId) {
        let Some(body) = self.loop_blocks.last().copied() else {
            return;
        };
        let Some(position) = self.block_stack.iter().rposition(|block| *block == body) else {
            return;
        };
        let inside: Vec<NodeId> = self.block_stack[position..].to_vec();
        let mut locals = Vec::new();
        let candidates: Vec<LocalId> = self
            .heap_units
            .iter()
            .filter(|unit| {
                !unit.manual
                    && !unit.abandoned
                    && !unit.escaped
                    && !unit.disposed
                    && inside.contains(&unit.block)
            })
            .map(|unit| unit.local)
            .collect();
        for local in candidates {
            if !self.heap_group_disposable(local) {
                continue;
            }
            for peer in self.heap_group_peers(local) {
                if !locals.contains(&peer) {
                    locals.push(peer);
                }
            }
        }
        if !locals.is_empty() {
            self.result.jump_dispose.insert(node, locals);
        }
    }

    /// Local ids of all heap units bound to `local` through `NOLAA.Bind`
    /// (§12.2: same analysis unit, same safe point).
    fn heap_group_peers(&self, local: LocalId) -> Vec<LocalId> {
        for group in &self.heap_groups {
            if group.contains(&local) {
                return group.clone();
            }
        }
        vec![local]
    }

    /// Whether every member of `local`'s analysis group may be auto-disposed
    /// now (non-manual, not abandoned, not escaped, not yet disposed).
    fn heap_group_disposable(&self, local: LocalId) -> bool {
        for peer in self.heap_group_peers(local) {
            let Some(index) = self.heap_unit_locals.get(&peer).copied() else {
                return false;
            };
            let unit = &self.heap_units[index];
            if unit.manual || unit.abandoned || unit.escaped || unit.disposed {
                return false;
            }
        }
        true
    }

    /// Schedules NOLAA auto-dispose before a `return` leaves the function
    /// scope (§12.2). A return is always a safe point for units that are still
    /// alive; already handled units (manual, abandoned, escaped or disposed at
    /// an enclosing block end) are skipped.
    fn plan_return_dispose(&mut self, node: NodeId) {
        let mut locals = Vec::new();
        let candidates: Vec<LocalId> = self
            .heap_units
            .iter()
            .filter(|unit| !unit.manual && !unit.abandoned && !unit.escaped && !unit.disposed)
            .map(|unit| unit.local)
            .collect();
        for local in candidates {
            if !self.heap_group_disposable(local) {
                continue;
            }
            for peer in self.heap_group_peers(local) {
                if !locals.contains(&peer) {
                    locals.push(peer);
                }
            }
        }
        if !locals.is_empty() {
            self.result.jump_dispose.insert(node, locals);
        }
    }

    /// Schedules NOLAA auto-dispose for eligible heap units declared in `block`
    /// (§12.2): non-manual, not explicitly disposed, not abandoned and not
    /// escaping.
    fn plan_auto_dispose(&mut self, block: NodeId) {
        // Without a fall-through safe point NOLAA must not insert a dispose
        // (§12.2), so keep the never-disposed warning for such units.
        if self.block_always_returns(block) {
            return;
        }
        let mut locals = Vec::new();
        let candidates: Vec<(usize, LocalId)> = self
            .heap_units
            .iter()
            .enumerate()
            .filter(|(_, unit)| {
                unit.block == block
                    && !unit.manual
                    && !unit.disposed
                    && !unit.abandoned
                    && !unit.escaped
            })
            .map(|(index, unit)| (index, unit.local))
            .collect();
        for (_, local) in candidates {
            if !self.heap_group_disposable(local) {
                continue;
            }
            for peer in self.heap_group_peers(local) {
                if let Some(index) = self.heap_unit_locals.get(&peer).copied() {
                    self.heap_units[index].disposed = true;
                }
                if !locals.contains(&peer) {
                    locals.push(peer);
                }
            }
        }
        if !locals.is_empty() {
            self.result
                .auto_dispose
                .entry(block)
                .or_default()
                .extend(locals);
        }
    }

    /// Checks a function body block in the scope that already holds `this` and
    /// the parameters: the body block *is* the function's scope (§6.1), so
    /// `owner` from inside it reaches the enclosing declaration scope.
    fn check_body_block(&mut self, block: NodeId) {
        let statements = match self.node_kind(block) {
            NodeKind::Block(statements) => statements,
            NodeKind::VerbatimBlock(inner) => {
                self.check_body_block(inner);
                return;
            }
            _ => {
                self.error(self.span_of(block), "expected a block");
                return;
            }
        };
        let disposed_before = self.disposed_locals.clone();
        let block_before = self.current_block;
        self.current_block = block;
        self.block_stack.push(block);
        for statement in statements {
            self.check_statement(statement);
        }
        self.block_stack.pop();
        self.current_block = block_before;
        self.plan_auto_dispose(block);
        self.disposed_locals = disposed_before;
    }

    fn check_statement(&mut self, node: NodeId) {
        match self.node_kind(node).clone() {
            NodeKind::Block(_) => self.check_block(node),
            NodeKind::VerbatimBlock(inner) => {
                let previous = self.in_verbatim;
                self.in_verbatim = true;
                self.check_block(inner);
                self.in_verbatim = previous;
            }
            NodeKind::Local(declaration) => self.check_local_statement(node, &declaration),
            NodeKind::Fixed(declaration) => self.check_fixed_statement(node, &declaration),
            NodeKind::Set(statement) => {
                let target_type = self.check_expr(statement.target, None);
                self.ensure_assignable(statement.target);
                if let Some(Resolution::Accessor { setter, .. }) =
                    self.result.resolutions[statement.target as usize].clone()
                {
                    if setter.is_none() {
                        self.error(self.span_of(statement.target), "accessor is read-only");
                    }
                }
                if let Some(target_type) = target_type {
                    let value_type = self.check_expr(statement.value, Some(target_type));
                    if let Some(value_type) = value_type {
                        self.expect_assignable(
                            self.span_of(statement.value),
                            value_type,
                            target_type,
                        );
                    }
                    let carrier = match self.result.resolutions[statement.target as usize].clone() {
                        Some(Resolution::Field { type_id, index }) => {
                            if let Type::Named(owner) = self.types_of(type_id) {
                                self.result.type_defs[owner as usize]
                                    .fields
                                    .get(index as usize)
                                    .map(|field| field.accessibility)
                            } else {
                                None
                            }
                        }
                        Some(Resolution::Global(global)) => {
                            Some(self.result.globals[global as usize].accessibility)
                        }
                        _ => None,
                    };
                    if let Some(carrier) = carrier {
                        self.check_function_value_access(
                            carrier,
                            statement.value,
                            self.span_of(statement.value),
                        );
                    }
                }
            }
            NodeKind::CompoundSet(statement) => {
                let target_type = self.check_expr(statement.target, None);
                self.ensure_assignable(statement.target);
                if let Some(Resolution::Accessor { setter, .. }) =
                    self.result.resolutions[statement.target as usize].clone()
                {
                    if setter.is_none() {
                        self.error(self.span_of(statement.target), "accessor is read-only");
                    }
                }
                let Some(target_type) = target_type else {
                    return;
                };
                // A pointer target accepts any integer offset (§3.6.2), so the
                // literal must not be typed from the pointer.
                let pointer_target = matches!(self.type_of(target_type), Type::Pointer { .. });
                let value_type = self.check_expr(
                    statement.value,
                    if pointer_target {
                        None
                    } else {
                        Some(target_type)
                    },
                );
                if pointer_target {
                    if let Some(value_type) = value_type {
                        if !self.is_integer(value_type) {
                            self.error(
                                self.span_of(statement.value),
                                "pointer arithmetic requires an integer offset",
                            );
                        }
                    }
                } else if let Some(value_type) = value_type {
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
                self.loop_blocks.push(loop_statement.body);
                self.check_block(loop_statement.body);
                self.loop_blocks.pop();
                self.loop_labels.pop();
            }
            NodeKind::DoWhile(loop_statement) => {
                self.loop_labels.push(loop_statement.label.clone());
                self.loop_blocks.push(loop_statement.body);
                self.check_block(loop_statement.body);
                self.loop_blocks.pop();
                self.loop_labels.pop();
                self.check_condition(loop_statement.condition);
            }
            NodeKind::For(for_statement) => self.check_for_statement(node, &for_statement),
            NodeKind::Match(match_statement) => self.check_match_statement(node, &match_statement),
            NodeKind::Break(statement) => {
                self.plan_loop_jump_dispose(node);
                if matches!(self.trap_contexts.last(), Some(TrapContextKind::Finally)) {
                    self.error(
                        self.span_of(node),
                        "'break' must not appear in a 'finally' block",
                    );
                }
                if self.loop_labels.is_empty() {
                    self.error(self.span_of(node), "'break' is only allowed inside a loop");
                } else if let Some(label) = &statement.label {
                    if !self.loop_labels.iter().flatten().any(|name| name == label) {
                        self.error(self.span_of(node), format!("unknown loop label '{label}'"));
                    }
                }
            }
            NodeKind::Continue(statement) => {
                self.plan_loop_jump_dispose(node);
                if matches!(self.trap_contexts.last(), Some(TrapContextKind::Finally)) {
                    self.error(
                        self.span_of(node),
                        "'continue' must not appear in a 'finally' block",
                    );
                }
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
                self.plan_return_dispose(node);
                if let Some(value) = statement.value {
                    self.note_heap_escape(value);
                }
                if matches!(self.trap_contexts.last(), Some(TrapContextKind::Finally)) {
                    self.error(
                        self.span_of(node),
                        "'return' must not appear in a 'finally' block",
                    );
                }
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
                self.resolve_use_statement(&statement);
            }
            NodeKind::CompileTimeBlock(block) => {
                if self.in_verbatim {
                    self.error(
                        self.span_of(node),
                        "verbatim forbids compile-time evaluation (§9.8)",
                    );
                    return;
                }
                // Locals declared here are compile-time constants (§11.4).
                self.compile_time_constants
                    .push(std::collections::HashMap::new());
                self.check_block(block);
                self.compile_time_constants.pop();
            }
            NodeKind::CompileTimeIf(statement) => {
                if self.in_verbatim {
                    self.error(
                        self.span_of(node),
                        "verbatim forbids compile-time evaluation (§9.8)",
                    );
                    return;
                }
                self.check_expr(statement.condition, Some(TypeId::BOOLEAN));
                let taken = self
                    .eval_compile_time_bool(statement.condition, 0)
                    .unwrap_or_else(|| {
                        self.error(
                            self.span_of(statement.condition),
                            "the '#if' condition must be a compile-time constant",
                        );
                        false
                    });
                self.result.compile_time_conditions.insert(node, taken);
                if taken {
                    self.check_block(statement.then_block);
                } else if let Some(else_block) = statement.else_block {
                    self.check_block(else_block);
                }
            }
            NodeKind::Throw(statement) => self.check_throw_statement(&statement),
            NodeKind::Trap(statement) => self.check_trap_statement(node, &statement),
            NodeKind::Dispose(statement) => self.check_dispose_statement(&statement),
            NodeKind::Heap(declaration) => self.check_heap_statement(node, &declaration),
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
            // Pointers support `+=`/`-=` with an integer offset (§3.6.2).
            BinaryOp::Add | BinaryOp::Subtract => {
                self.is_numeric(ty) || matches!(self.type_of(ty), Type::Pointer { .. })
            }
            BinaryOp::Multiply | BinaryOp::Divide | BinaryOp::Remainder => self.is_numeric(ty),
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
                    self.initializer_depth += 1;
                    let inferred = self.check_expr(initializer, Some(ty));
                    self.initializer_depth -= 1;
                    if let Some(inferred) = inferred {
                        self.expect_assignable(item.span, inferred, ty);
                    }
                    ty
                }
                (Some(ty), None) => ty,
                (None, Some(initializer)) => {
                    self.initializer_depth += 1;
                    let inferred = self.check_expr(initializer, None);
                    self.initializer_depth -= 1;
                    inferred.unwrap_or(TypeId::ERROR)
                }
                (None, None) => {
                    self.error(
                        item.span,
                        "local variable requires a type or an initializer",
                    );
                    continue;
                }
            };
            // Inside a compile-time block the declared value is a compile-time
            // constant usable by later constant expressions (§11.4).
            if !self.compile_time_constants.is_empty() && self.in_verbatim {
                self.error(item.span, "verbatim forbids compile-time evaluation (§9.8)");
            } else if !self.compile_time_constants.is_empty() {
                if let Some(initializer) = item.init {
                    self.suppress_errors += 1;
                    let constant = self.eval_const_int(initializer);
                    self.suppress_errors -= 1;
                    if let Some(constant) = constant {
                        if let Some(scope) = self.compile_time_constants.last_mut() {
                            scope.insert(item.name.clone(), constant);
                        }
                    }
                }
            }
            let local = self.add_local(
                item.name.clone(),
                ty,
                false,
                None,
                Storage::Value,
                item.span,
            );
            locals.push(local);
        }
        // Variable-length arrays cannot be initialized (§3.6.1).
        for (local, item) in locals.iter().zip(declaration.items.iter()) {
            if item.init.is_some() {
                if let Some(info) = self.local_info(*local) {
                    if matches!(self.types_of(info.ty), Type::Vla { .. }) {
                        self.error(
                            item.span,
                            "a variable-length array cannot have an initializer",
                        );
                    }
                }
            }
        }
        self.result.local_declarations.insert(node, locals);
    }

    fn check_fixed_statement(&mut self, node: NodeId, declaration: &FixedDecl) {
        if let Some(ty_node) = declaration.ty {
            if let Some(ty) = self
                .result
                .node_types
                .get(ty_node as usize)
                .copied()
                .flatten()
            {
                if matches!(self.types_of(ty), Type::Vla { .. }) {
                    self.error(
                        self.span_of(node),
                        "a variable-length array cannot be a constant",
                    );
                }
            }
        }
        // A `fixed` declaration whose type is a function type and whose body is a
        // block is a nested function definition (§5.3, §2.7.9): it declares a
        // function literal bound to a constant name.
        if let (Some(ty_node), Some(Initializer::Block(body))) = (declaration.ty, declaration.init)
        {
            if let NodeKind::FunctionType(function) = self.node_kind(ty_node).clone() {
                self.declare_nested_function(node, declaration, &function, body);
                return;
            }
        }
        let declared_type = declaration.ty.and_then(|ty| self.resolve_type(ty));
        if let Some(ty) = declared_type {
            if declaration.init.is_none() {
                self.error(self.span_of(node), "a constant must be initialized (§5.3)");
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
            let local = self.add_local(
                declaration.name.clone(),
                ty,
                true,
                None,
                Storage::Value,
                self.span_of(node),
            );
            self.result.local_declarations.insert(node, vec![local]);
        } else if let Some(Initializer::Expression(expression)) = declaration.init {
            let ty = self.check_expr(expression, None).unwrap_or(TypeId::ERROR);
            let local = self.add_local(
                declaration.name.clone(),
                ty,
                true,
                None,
                Storage::Value,
                self.span_of(node),
            );
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
                    if let Some(info) = self.local_info(local) {
                        if info.fixed {
                            self.error(self.span_of(node), "cannot assign to a constant");
                        } else if info.storage == Storage::Heap {
                            self.error(self.span_of(node), "cannot reassign heap storage");
                        }
                    }
                }
                Some(Symbol::Global(global)) => {
                    if self.result.globals[global as usize].fixed {
                        self.error(self.span_of(node), "cannot assign to a constant");
                    }
                }
                _ => self.error(self.span_of(node), "expression is not assignable"),
            },
            NodeKind::MemberAccess(_) | NodeKind::PointerMemberAccess(_) => {
                self.check_readonly_assignment(node);
            }
            NodeKind::Index(_) => {}
            NodeKind::Unary(unary) if unary.op == UnaryOp::Dereference => {}
            _ => self.error(self.span_of(node), "expression is not assignable"),
        }
    }

    /// Declares a nested function defined inside a function body (§5.3, §2.7.9):
    /// the declaration becomes a function literal bound to a constant local.
    fn declare_nested_function(
        &mut self,
        node: NodeId,
        declaration: &FixedDecl,
        function: &FunctionType,
        body: NodeId,
    ) {
        let span = self.span_of(node);
        let mut parameters = Vec::new();
        for parameter in &function.parameters {
            let Some(name) = parameter.name.clone() else {
                self.error(
                    parameter.span,
                    "a nested function parameter requires a name",
                );
                return;
            };
            parameters.push(crate::ast::LiteralParameter {
                compile_time: parameter.compile_time,
                name,
                by_ref: parameter.by_ref,
                ty: parameter.ty,
                span: parameter.span,
            });
        }
        let literal = FunctionLiteral {
            parameters,
            calling_convention: function.calling_convention,
            return_type: function.return_type,
            body,
        };
        let literal_node = self
            .ast
            .push(span, NodeKind::FunctionLiteral(literal.clone()));
        let Some(function_type) = self.check_function_literal(literal_node, &literal, None) else {
            return;
        };
        let local = self.add_local(
            declaration.name.clone(),
            function_type,
            true,
            None,
            Storage::Value,
            span,
        );
        self.result.local_declarations.insert(node, vec![local]);
        self.result.nested_functions.insert(node, literal_node);
    }

    /// Enforces `readonly` field assignment rules (§9.6): a readonly field may
    /// only be assigned inside a constructor of the declaring type or of a
    /// derived type.
    fn check_readonly_assignment(&mut self, node: NodeId) {
        let Some(Resolution::Field { type_id, index }) =
            self.result.resolutions[node as usize].clone()
        else {
            return;
        };
        let Type::Named(owner_index) = self.types_of(type_id) else {
            return;
        };
        let (field_name, readonly) = {
            let definition = &self.result.type_defs[owner_index as usize];
            let Some(field) = definition.fields.get(index as usize) else {
                return;
            };
            (field.name.clone(), field.readonly)
        };
        if !readonly {
            return;
        }
        // Walk the base chain to the type that declares the field.
        let mut declaring = owner_index;
        let mut current = self.result.type_defs[owner_index as usize].base;
        while let Some(base) = current {
            let Type::Named(base_index) = self.types_of(base) else {
                break;
            };
            let declares = self.result.type_defs[base_index as usize]
                .fields
                .iter()
                .any(|field| field.name == field_name);
            if declares {
                declaring = base_index;
                current = self.result.type_defs[base_index as usize].base;
            } else {
                break;
            }
        }
        let allowed = self.current_function.is_some_and(|function_index| {
            let function = &self.result.functions[function_index as usize];
            if function.name != "Construct" {
                return false;
            }
            let Some(owner) = function.method_of else {
                return false;
            };
            let Type::Named(mut current) = self.types_of(owner) else {
                return false;
            };
            let mut depth = 0;
            loop {
                if current == declaring {
                    return true;
                }
                depth += 1;
                if depth > 32 {
                    return false;
                }
                let Some(base) = self.result.type_defs[current as usize].base else {
                    return false;
                };
                let Type::Named(base_index) = self.types_of(base) else {
                    return false;
                };
                current = base_index;
            }
        });
        if !allowed {
            self.error(
                self.span_of(node),
                format!("'{field_name}' is readonly and can only be assigned in a constructor"),
            );
        }
    }

    fn check_throw_statement(&mut self, statement: &ThrowStmt) {
        match self.trap_contexts.last() {
            Some(TrapContextKind::Body | TrapContextKind::Catch) => {}
            Some(TrapContextKind::Finally) => {
                self.error(
                    self.span_of(statement.value),
                    "a throw inside 'finally' must terminate the program",
                );
                return;
            }
            None => {
                self.error(
                    self.span_of(statement.value),
                    "'throw' must appear inside a trap or catch block",
                );
                return;
            }
        }
        self.check_expr(statement.value, None);
    }

    fn check_trap_statement(&mut self, node: NodeId, statement: &TrapStmt) {
        if self.current_function.is_none() {
            self.error(
                self.span_of(node),
                "'trap' must appear inside a function body",
            );
        }
        let mut catches = Vec::new();
        let mut saw_wildcard = false;
        for catch in &statement.catches {
            if saw_wildcard {
                self.error(catch.span, "'wildcard' must be the last catch clause");
            }
            if catch.wildcard {
                saw_wildcard = true;
            }
            let catch_type = catch.ty.and_then(|ty| self.resolve_type(ty));
            if !catch.wildcard && catch_type.is_none() {
                self.error(catch.span, "catch requires a type or 'wildcard'");
            }
            self.push_scope();
            let binding = match (&catch.name, catch_type) {
                (Some(name), Some(ty)) => {
                    Some(self.add_local(name.clone(), ty, false, None, Storage::Value, catch.span))
                }
                _ => None,
            };
            self.trap_contexts.push(TrapContextKind::Catch);
            self.check_block(catch.body);
            self.trap_contexts.pop();
            self.pop_scope();
            catches.push(CatchInfo {
                ty: catch_type,
                binding,
                wildcard: catch.wildcard,
                body: catch.body,
            });
        }
        self.result.trap_catches.insert(node, catches);
        self.trap_contexts.push(TrapContextKind::Body);
        self.check_block(statement.body);
        self.trap_contexts.pop();
        if let Some(finally) = statement.finally {
            self.trap_contexts.push(TrapContextKind::Finally);
            self.check_block(finally);
            self.trap_contexts.pop();
        }
    }

    /// Records that a heap allocation may escape the function through `node`
    /// (§12.2), suppressing the never-disposed warning.
    fn note_heap_escape(&mut self, node: NodeId) {
        let name = match self.node_kind(node) {
            NodeKind::Name(name) => name.name.clone(),
            NodeKind::Unary(unary) if unary.op == UnaryOp::Reference => {
                let NodeKind::Name(name) = self.node_kind(unary.operand) else {
                    return;
                };
                name.name.clone()
            }
            _ => return,
        };
        if let Some(Symbol::Local(local)) = self.lookup(&name).cloned() {
            if self.heap_unit_locals.contains_key(&local) {
                for peer in self.heap_group_peers(local) {
                    if let Some(index) = self.heap_unit_locals.get(&peer).copied() {
                        self.heap_units[index].escaped = true;
                    }
                }
            }
        }
    }

    fn check_heap_statement(&mut self, node: NodeId, declaration: &HeapDecl) {
        // An explicit allocator expression must evaluate to an IAllocator (§5.4).
        let mut allocator = None;
        if let Some(allocator_node) = declaration.size {
            let expected = self.lookup("IAllocator").cloned();
            let Some(Symbol::Type(expected)) = expected else {
                self.error(
                    self.span_of(allocator_node),
                    "no IAllocator type is available for heap allocations",
                );
                return;
            };
            let Some(allocator_type) = self.check_expr(allocator_node, Some(expected)) else {
                return;
            };
            if !self.is_assignable(allocator_type, expected) {
                self.error(
                    self.span_of(allocator_node),
                    "a heap allocator must be an IAllocator",
                );
                return;
            }
            allocator = Some(allocator_node);
            self.result.heap_allocators.insert(node, allocator_node);
        } else {
            // The omitted allocator expression uses the global DefaultAllocator
            // (§5.4).
            match self.lookup("DefaultAllocator").cloned() {
                Some(Symbol::Global(_)) => {
                    self.result.heap_default_allocator.insert(node);
                }
                _ => {
                    self.error(
                        self.span_of(node),
                        "no 'DefaultAllocator' global is available for this heap allocation",
                    );
                    return;
                }
            }
        }
        // The type may be omitted when an initializer provides it (§5.4).
        let declared = declaration.ty.map(|type_node| self.resolve_type(type_node));
        let mut ty = match declared {
            Some(Some(ty)) => Some(ty),
            Some(None) => return,
            None => None,
        };
        if let Some(initializer) = declaration.init {
            self.initializer_depth += 1;
            let inferred = self.check_expr(initializer, ty);
            self.initializer_depth -= 1;
            if let Some(inferred) = inferred {
                match ty {
                    Some(ty) => self.expect_assignable(self.span_of(node), inferred, ty),
                    None => ty = Some(inferred),
                }
            }
        }
        let Some(ty) = ty else {
            self.error(
                self.span_of(node),
                "heap definition requires a type or an initializer",
            );
            return;
        };
        let local = self.add_local(
            declaration.name.clone(),
            ty,
            false,
            None,
            Storage::Heap,
            self.span_of(node),
        );
        self.heap_unit_locals.insert(local, self.heap_units.len());
        self.heap_units.push(HeapUnit {
            name: declaration.name.clone(),
            span: self.span_of(node),
            manual: declaration.modifiers.manual,
            disposed: false,
            abandoned: false,
            escaped: false,
            local,
            block: self.current_block,
        });
        if let Some(allocator) = allocator {
            self.result.heap_allocator_locals.insert(local, allocator);
        } else {
            self.result.heap_default_allocator_locals.insert(local);
        }
        self.result.local_declarations.insert(node, vec![local]);
    }

    fn check_dispose_statement(&mut self, statement: &DisposeStmt) {
        if let NodeKind::Name(name) = self.node_kind(statement.target) {
            if let Some(Symbol::Local(local)) = self.lookup(&name.name).cloned() {
                let info = self.local_info(local).cloned();
                if let Some(info) = info {
                    let disposable = matches!(
                        self.types_of(info.ty),
                        Type::Named(index)
                            if matches!(
                                self.result.type_defs[index as usize].kind,
                                TypeDefKind::Class | TypeDefKind::Structure
                            )
                    );
                    if disposable {
                        if !self.disposed_locals.insert(local) {
                            self.warn(
                                self.span_of(statement.target),
                                format!("'{}' is disposed more than once", name.name),
                            );
                        }
                        if let Some(index) = self.heap_unit_locals.get(&local).copied() {
                            self.heap_units[index].disposed = true;
                        }
                        self.result.resolutions[statement.target as usize] =
                            Some(Resolution::Local(local));
                        self.result.node_types[statement.target as usize] = Some(info.ty);
                        return;
                    }
                }
            }
        }
        self.error(
            self.span_of(statement.target),
            "dispose requires a class or structure variable",
        );
    }

    /// Finds the built-in iterator interface behind an iteration target (§5.6).
    ///
    /// Returns `(IIterator<T> instance, needs_accessor, IIterable<T> instance,
    /// element type)`.
    fn find_builtin_iterator(
        &mut self,
        ty: TypeId,
    ) -> Option<(TypeId, bool, Option<TypeId>, TypeId)> {
        let Type::Named(index) = self.types_of(ty) else {
            return None;
        };
        let kind = self.result.type_defs[index as usize].kind;
        if kind == TypeDefKind::Interface {
            let template_name = self.result.type_defs[index as usize]
                .template
                .map(|template| self.result.type_defs[template as usize].name.clone());
            let arguments = self.result.type_defs[index as usize]
                .generic_arguments
                .clone();
            let element = arguments.first().copied();
            match (template_name.as_deref(), element) {
                (Some("IIterator"), Some(element)) => {
                    return Some((ty, false, None, element));
                }
                (Some("IIterable"), Some(element)) => {
                    let iterator_template =
                        self.lookup("IIterator")
                            .cloned()
                            .and_then(|symbol| match symbol {
                                Symbol::Type(iterator) => match self.result.types.get(iterator) {
                                    Type::Named(template) => {
                                        Some(self.result.type_defs[*template as usize].node)
                                    }
                                    _ => None,
                                },
                                _ => None,
                            });
                    let iterator = iterator_template.and_then(|node| {
                        self.instantiate_type(node, vec![element], Span::default())
                    })?;
                    return Some((iterator, true, Some(ty), element));
                }
                _ => {}
            }
            return None;
        }
        if kind != TypeDefKind::Class {
            return None;
        }
        let interfaces = self.result.type_defs[index as usize].interfaces.clone();
        for interface in interfaces {
            let Type::Named(interface_index) = self.types_of(interface) else {
                continue;
            };
            let template_name = self.result.type_defs[interface_index as usize]
                .template
                .map(|template| self.result.type_defs[template as usize].name.clone());
            let arguments = self.result.type_defs[interface_index as usize]
                .generic_arguments
                .clone();
            let element = arguments.first().copied();
            match (template_name.as_deref(), element) {
                (Some("IIterator"), Some(element)) => {
                    return Some((interface, false, None, element));
                }
                (Some("IIterable"), Some(element)) => {
                    let iterator_template =
                        self.lookup("IIterator")
                            .cloned()
                            .and_then(|symbol| match symbol {
                                Symbol::Type(iterator) => match self.result.types.get(iterator) {
                                    Type::Named(template) => {
                                        Some(self.result.type_defs[*template as usize].node)
                                    }
                                    _ => None,
                                },
                                _ => None,
                            });
                    let iterator = iterator_template.and_then(|node| {
                        self.instantiate_type(node, vec![element], Span::default())
                    })?;
                    return Some((iterator, true, Some(interface), element));
                }
                _ => {}
            }
        }
        None
    }

    fn check_for_statement(&mut self, node: NodeId, statement: &ForStmt) {
        let binding_type = self.resolve_type(statement.ty).unwrap_or(TypeId::ERROR);
        if statement.by_ref && matches!(statement.target, ForTarget::Range(_)) {
            self.error(
                self.span_of(node),
                "a by-reference 'for' binding requires an array target",
            );
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
                let saved_expected = self.expected_array_element;
                if matches!(self.node_kind(*target), NodeKind::ArrayLiteral(_)) {
                    self.expected_array_element = Some(binding_type);
                }
                let target_type = self.check_expr(*target, None);
                self.expected_array_element = saved_expected;
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
                        Type::Vla { element, .. } => {
                            self.expect_assignable(self.span_of(node), element, binding_type);
                        }
                        _ => {
                            if let Some((
                                iterator_interface,
                                use_accessor,
                                iterable_interface,
                                element,
                            )) = self.find_builtin_iterator(target_type)
                            {
                                if statement.by_ref {
                                    self.error(
                                        self.span_of(node),
                                        "a by-reference 'for' binding requires an array target",
                                    );
                                    return;
                                }
                                self.expect_assignable(self.span_of(node), element, binding_type);
                                self.result.for_iterators.insert(
                                    node,
                                    ForIteratorInfo {
                                        target: *target,
                                        iterator_interface,
                                        use_accessor,
                                        iterable_interface,
                                        element,
                                    },
                                );
                            } else {
                                self.unsupported(
                                    self.span_of(node),
                                    "iteration over non-array values",
                                );
                            }
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
            if statement.by_ref {
                Storage::Reference
            } else {
                Storage::Value
            },
            self.span_of(node),
        );
        self.result.loop_bindings.insert(node, binding_local);
        self.result.node_types[node as usize] = Some(binding_type);
        self.loop_labels.push(statement.label.clone());
        self.loop_blocks.push(statement.body);
        self.check_block(statement.body);
        self.loop_blocks.pop();
        self.loop_labels.pop();
        self.pop_scope();
    }

    fn check_match_statement(&mut self, node: NodeId, statement: &MatchStmt) {
        let value_type = self.check_expr(statement.value, None);
        self.loop_labels.push(statement.label.clone());
        for (case_index, case) in statement.cases.iter().enumerate() {
            self.push_scope();
            if let Some(pattern) = &case.pattern {
                match pattern {
                    Pattern::Equal(expression) => {
                        if let Some(value_type) = value_type {
                            let ty = self.check_expr(*expression, Some(value_type));
                            if let Some(ty) = ty {
                                // Value equality patterns are the equality
                                // expression of the pattern expression (§5.9.6),
                                // so a user-defined `==` overload is used when
                                // the type has no built-in equality.
                                if self.has_builtin_equality(value_type) {
                                    self.expect_assignable(
                                        self.span_of(*expression),
                                        ty,
                                        value_type,
                                    );
                                } else {
                                    match self.find_operator(value_type, ty, OperatorKind::Equal) {
                                        Some(method) => {
                                            self.result.match_patterns.insert(
                                                (node, case_index as u32),
                                                MatchPatternInfo::Operator { method },
                                            );
                                        }
                                        None => self.error(
                                            self.span_of(*expression),
                                            "no '==' overload matches the match pattern",
                                        ),
                                    }
                                }
                            }
                        } else {
                            self.check_expr(*expression, None);
                        }
                    }
                    Pattern::Comparison {
                        operator, value, ..
                    } => {
                        let ty = self.check_expr(*value, value_type);
                        let op = match operator {
                            crate::lexer::TokenKind::NotEquals => Some(BinaryOp::NotEqual),
                            crate::lexer::TokenKind::Greater => Some(BinaryOp::Greater),
                            crate::lexer::TokenKind::Less => Some(BinaryOp::Less),
                            crate::lexer::TokenKind::GreaterEquals => Some(BinaryOp::GreaterEqual),
                            crate::lexer::TokenKind::LessEquals => Some(BinaryOp::LessEqual),
                            _ => None,
                        };
                        let Some(value_type) = value_type else { return };
                        let Some(ty) = ty else { return };
                        let comparison = match op {
                            Some(BinaryOp::NotEqual) => self
                                .find_operator(value_type, ty, OperatorKind::Equal)
                                .map(|method| MatchPatternInfo::DerivedNotEqual { method }),
                            Some(BinaryOp::Greater) => self
                                .find_operator(value_type, ty, OperatorKind::Greater)
                                .map(|method| MatchPatternInfo::Operator { method }),
                            Some(BinaryOp::Less) => self
                                .find_operator(value_type, ty, OperatorKind::Less)
                                .map(|method| MatchPatternInfo::Operator { method }),
                            Some(BinaryOp::GreaterEqual) => self
                                .find_operator(value_type, ty, OperatorKind::Greater)
                                .and_then(|ordering| {
                                    self.find_operator(value_type, ty, OperatorKind::Equal).map(
                                        |equality| MatchPatternInfo::DerivedComparison {
                                            ordering,
                                            equality,
                                        },
                                    )
                                }),
                            Some(BinaryOp::LessEqual) => self
                                .find_operator(value_type, ty, OperatorKind::Less)
                                .and_then(|ordering| {
                                    self.find_operator(value_type, ty, OperatorKind::Equal).map(
                                        |equality| MatchPatternInfo::DerivedComparison {
                                            ordering,
                                            equality,
                                        },
                                    )
                                }),
                            _ => None,
                        };
                        match comparison {
                            Some(info) => {
                                self.result
                                    .match_patterns
                                    .insert((node, case_index as u32), info);
                            }
                            None => {
                                if self.is_numeric(value_type) {
                                    self.expect_assignable(self.span_of(*value), ty, value_type);
                                } else {
                                    self.error(
                                        self.span_of(*value),
                                        "no comparison overload matches the match pattern",
                                    );
                                }
                            }
                        }
                    }
                    Pattern::Is { name, target } => {
                        self.check_is_pattern(node, case_index, name.clone(), target, value_type)
                    }
                    Pattern::Wildcard => {}
                }
            }
            if let Some(guard) = case.guard {
                self.check_condition(guard);
            }
            self.check_block(case.body);
            self.pop_scope();
        }
        self.loop_labels.pop();
        self.check_match_exhaustiveness(node, statement, value_type);
    }

    /// Warns when a `match` over a variant type neither covers every state nor
    /// has a wildcard case (§5.9.6).
    fn check_match_exhaustiveness(
        &mut self,
        node: NodeId,
        statement: &MatchStmt,
        value_type: Option<TypeId>,
    ) {
        let Some(value_type) = value_type else {
            return;
        };
        let Type::Named(index) = self.types_of(value_type) else {
            return;
        };
        let definition = &self.result.type_defs[index as usize];
        if definition.kind != TypeDefKind::Variant {
            return;
        }
        let state_count = definition.variant_states.len();
        let has_wildcard = statement.cases.iter().any(|case| {
            case.guard.is_none() && matches!(case.pattern, None | Some(Pattern::Wildcard))
        });
        if has_wildcard {
            return;
        }
        let mut covered = Vec::new();
        for case_index in 0..statement.cases.len() as u32 {
            if let Some(MatchPatternInfo::IsVariant { state, .. }) =
                self.result.match_patterns.get(&(node, case_index))
            {
                covered.push(*state);
            }
        }
        let missing = (0..state_count as u32)
            .filter(|state| !covered.contains(state))
            .count();
        if missing > 0 {
            self.warn(
                self.span_of(node),
                format!("match is not exhaustive: {missing} variant state(s) are not covered"),
            );
        }
    }

    /// Checks an `is` pattern and binds its payload/narrowed value.
    fn check_is_pattern(
        &mut self,
        match_node: NodeId,
        case_index: usize,
        binding_name: Option<String>,
        target: &IsTarget,
        value_type: Option<TypeId>,
    ) {
        let Some(value_type) = value_type else {
            return;
        };
        match target {
            IsTarget::Identifier {
                text,
                node: target_node,
            } => {
                let Some(resolved) = self.resolve_is_target(match_node, *target_node, text) else {
                    return;
                };
                match resolved {
                    IsTargetResolved::Type(target_type) => {
                        let target_is_class = matches!(
                            self.types_of(target_type),
                            Type::Named(index)
                                if matches!(
                                    self.result.type_defs[index as usize].kind,
                                    TypeDefKind::Class | TypeDefKind::Interface
                                )
                        );
                        if !target_is_class {
                            self.unsupported(
                                self.span_of(match_node),
                                "is patterns over non-class types",
                            );
                            return;
                        }
                        let binding = binding_name.map(|name| {
                            self.add_local(
                                name,
                                target_type,
                                false,
                                None,
                                Storage::Value,
                                self.span_of(match_node),
                            )
                        });
                        self.result.match_patterns.insert(
                            (match_node, case_index as u32),
                            MatchPatternInfo::IsClass {
                                type_id: target_type,
                                binding,
                            },
                        );
                    }
                    IsTargetResolved::VariantState { type_id, state } => {
                        if self.types_of(value_type) != self.types_of(type_id) {
                            self.error(
                                self.span_of(match_node),
                                "the matched value is not the pattern's variant type",
                            );
                            return;
                        }
                        let Type::Named(index) = self.types_of(type_id) else {
                            return;
                        };
                        let payload_type = self.result.type_defs[index as usize].variant_states
                            [state as usize]
                            .payload;
                        let binding = match (binding_name, payload_type) {
                            (Some(name), Some(payload)) => Some(self.add_local(
                                name,
                                payload,
                                false,
                                None,
                                Storage::Value,
                                self.span_of(match_node),
                            )),
                            (Some(_), None) => {
                                self.error(
                                    self.span_of(match_node),
                                    "this state has no payload to bind",
                                );
                                None
                            }
                            (None, _) => None,
                        };
                        self.result.match_patterns.insert(
                            (match_node, case_index as u32),
                            MatchPatternInfo::IsVariant {
                                type_id,
                                state,
                                binding,
                            },
                        );
                    }
                }
            }
            IsTarget::Illegal => {
                if !matches!(self.types_of(value_type), Type::Float { .. }) {
                    self.error(
                        self.span_of(match_node),
                        "'is illegal' requires a floating-point value",
                    );
                    return;
                }
                self.result
                    .match_patterns
                    .insert((match_node, case_index as u32), MatchPatternInfo::IsIllegal);
            }
            IsTarget::Infinite { negative } => {
                if !matches!(self.types_of(value_type), Type::Float { .. }) {
                    self.error(
                        self.span_of(match_node),
                        "'is infinity' requires a floating-point value",
                    );
                    return;
                }
                self.result.match_patterns.insert(
                    (match_node, case_index as u32),
                    MatchPatternInfo::IsInfinite {
                        negative: *negative,
                    },
                );
            }
        }
    }
}

// ======================================================================
// Expressions
// ======================================================================

impl<'a> Sema<'a> {
    pub(crate) fn check_expr(&mut self, node: NodeId, expected: Option<TypeId>) -> Option<TypeId> {
        let ty = self.check_expr_inner(node, expected);
        if let (Some(ty), Some(expected)) = (ty, expected) {
            if ty != expected && !self.is_assignable(ty, expected) {
                // An `implicitable` conversion operator may be invoked implicitly
                // wherever the types do not match (§9.7).
                if let Some(method) = self.find_implicit_conversion(ty, expected) {
                    self.result.implicit_conversions.insert(node, method);
                    self.result.node_types[node as usize] = Some(expected);
                    return Some(expected);
                }
            }
        }
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
                Some(ty) if self.initializer_depth > 0 => Some(ty),
                Some(_) => {
                    self.error(
                        self.span_of(node),
                        "'uninitialized' may only be used as a storage initial value",
                    );
                    Some(TypeId::ERROR)
                }
                None => {
                    self.error(
                        self.span_of(node),
                        "uninitialized requires a type from context",
                    );
                    None
                }
            },
            NodeKind::Name(name) => self.check_name_expr(node, &name, expected),
            NodeKind::MemberAccess(access) => self.check_member_access(node, &access, false),
            NodeKind::PointerMemberAccess(access) => self.check_member_access(node, &access, true),
            NodeKind::Index(index) => self.check_index_expr(node, &index),
            NodeKind::Call(call) => {
                // Payload-state name elision (§3.7.5):
                // `local v: Maybe = Some(50);` constructs `Maybe.Some(50)`.
                if let Some(handled) = self.check_elided_state_construction(node, &call, expected) {
                    return handled;
                }
                self.check_call(node, &call)
            }
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
            NodeKind::CompileTimeExpr(inner) => {
                if self.in_verbatim {
                    self.error(
                        self.span_of(node),
                        "verbatim forbids compile-time evaluation (§9.8)",
                    );
                    return None;
                }
                let ty = self.check_expr(inner, expected);
                // A '#(...)' expression must be evaluable at compile time (§11.2).
                if self.is_compile_time_constant(inner) {
                    // Record the value so code generation can emit the
                    // compile-time end product directly (§11.4).
                    self.suppress_errors += 1;
                    let value = self.eval_const_int(inner);
                    self.suppress_errors -= 1;
                    if let (Some(ty), Some(value)) = (ty, value) {
                        if self.is_integer(ty) {
                            self.result.compile_time_values.insert(node, value);
                        }
                    }
                } else {
                    self.unsupported(
                        self.span_of(node),
                        "compile-time expressions that depend on run-time values",
                    );
                }
                ty
            }
            NodeKind::Is(is_expression) => self.check_is_expression(node, &is_expression),
            NodeKind::Coalesce(coalesce) => self.check_coalesce(node, &coalesce),
            NodeKind::FunctionLiteral(literal) => {
                self.check_function_literal(node, &literal, expected)
            }
            NodeKind::Monomorphization(monomorphization) => {
                match self.check_monomorphization(node, &monomorphization) {
                    Some(MonomorphizedTarget::Function(index)) => {
                        let parameters = self.result.functions[index as usize]
                            .parameters
                            .iter()
                            .map(|parameter| parameter.ty)
                            .collect::<Vec<_>>();
                        let return_type = self.result.functions[index as usize].return_type;
                        Some(self.result.types.function(
                            parameters,
                            false,
                            self.result.functions[index as usize].calling_convention,
                            return_type,
                        ))
                    }
                    Some(MonomorphizedTarget::Type(_)) => {
                        self.error(
                            self.span_of(node),
                            "a type name cannot be used as a value here",
                        );
                        None
                    }
                    None => None,
                }
            }
            NodeKind::This => {
                let owner = self
                    .current_function
                    .and_then(|index| self.result.functions[index as usize].method_of);
                match owner {
                    Some(owner) => {
                        self.result.resolutions[node as usize] = Some(Resolution::This);
                        Some(owner)
                    }
                    None => {
                        self.error(self.span_of(node), "'this' is only available in methods");
                        None
                    }
                }
            }
            NodeKind::Base => {
                let owner = self
                    .current_function
                    .and_then(|index| self.result.functions[index as usize].method_of);
                match owner.and_then(|owner| match self.types_of(owner) {
                    Type::Named(index) => self.result.type_defs[index as usize].base,
                    _ => None,
                }) {
                    Some(base) => {
                        self.result.resolutions[node as usize] = Some(Resolution::Base);
                        Some(base)
                    }
                    None => {
                        self.error(
                            self.span_of(node),
                            "'base' requires a base class in a method",
                        );
                        None
                    }
                }
            }
            NodeKind::Owner => {
                self.error(
                    self.span_of(node),
                    "'owner' must be followed by '.' and a name",
                );
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

    fn check_name_expr(
        &mut self,
        node: NodeId,
        name: &NameExpr,
        expected: Option<TypeId>,
    ) -> Option<TypeId> {
        let Some(symbol) = self.lookup(&name.name).cloned() else {
            // Top-level compile-time variables keep their final value at runtime
            // (§11.4).
            if let Some((value, ty)) = self.top_level_constants.get(&name.name).copied() {
                self.result.resolutions[node as usize] = Some(Resolution::Constant { ty, value });
                return Some(ty);
            }
            // Compile-time bindings (e.g. array extension dimension bindings,
            // §3.8.3) are available as constants in the body.
            if let Some(value) = self
                .constant_env
                .iter()
                .rev()
                .find_map(|environment| environment.get(&name.name).copied())
            {
                let ty = self.result.types.integer(false, 8);
                self.result.resolutions[node as usize] = Some(Resolution::Constant { ty, value });
                return Some(ty);
            }
            // Name elision for variant states (§3.7.5): in a context that wants a
            // variant, a payload-less state name stands for `Variant.State`.
            if let Some(expected) = expected {
                if let Type::Named(index) = self.types_of(expected) {
                    let definition = &self.result.type_defs[index as usize];
                    if definition.kind == TypeDefKind::Variant {
                        if let Some(state) = definition
                            .variant_states
                            .iter()
                            .position(|state| state.name == name.name)
                        {
                            if definition.variant_states[state].payload.is_some() {
                                self.error(
                                    self.span_of(node),
                                    format!(
                                        "variant state '{}' carries a payload and must be constructed",
                                        name.name
                                    ),
                                );
                                return None;
                            }
                            self.result.resolutions[node as usize] =
                                Some(Resolution::VariantState {
                                    type_id: expected,
                                    state: state as u32,
                                });
                            return Some(expected);
                        }
                    }
                }
            }
            self.error(self.span_of(node), format!("unknown name '{}'", name.name));
            return None;
        };
        self.check_symbol_expr(node, &name.name, symbol)
    }

    /// Records the resolution of an expression node for a resolved symbol and
    /// returns the symbol's value type.
    fn check_symbol_expr(&mut self, node: NodeId, name: &str, symbol: Symbol) -> Option<TypeId> {
        match symbol {
            Symbol::Local(local) => {
                if self.disposed_locals.contains(&local) && self.early_dispose_warned.insert(local)
                {
                    // Using a value after `dispose` is an early dispose (§12.2).
                    let local_name = self
                        .local_info(local)
                        .map_or_else(|| name.to_string(), |info| info.name.clone());
                    self.warn(
                        self.span_of(node),
                        format!("'{local_name}' is used after 'dispose' (early dispose)"),
                    );
                }
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
                    .map(|parameter| parameter.ty)
                    .collect::<Vec<_>>();
                let return_type = self.result.functions[index as usize].return_type;
                Some(self.result.types.function(
                    parameters,
                    false,
                    self.result.functions[index as usize].calling_convention,
                    return_type,
                ))
            }
            Symbol::GenericFunctions(_) => {
                self.error(
                    self.span_of(node),
                    format!("'{name}' is a generic function; type arguments are required"),
                );
                None
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
            Symbol::This { owner } => {
                self.result.resolutions[node as usize] = Some(Resolution::This);
                Some(owner)
            }
        }
    }

    /// Number of `owner` hops in an owner chain base (`owner`, `owner.owner`),
    /// if `node` is such a chain.
    fn owner_levels(&self, node: NodeId) -> Option<usize> {
        let mut levels = 0;
        let mut current = node;
        loop {
            match self.node_kind(current) {
                NodeKind::Owner => return Some(levels + 1),
                NodeKind::MemberAccess(access) if access.name == "owner" => {
                    levels += 1;
                    current = access.base;
                }
                _ => return None,
            }
        }
    }

    fn check_member_access(
        &mut self,
        node: NodeId,
        access: &MemberAccess,
        pointer: bool,
    ) -> Option<TypeId> {
        // Shadowed-name access: `owner.name`, `owner.owner.name` (§6.4).
        if let Some(levels) = self.owner_levels(access.base) {
            let Some(scope) = self.ancestor_scope(self.current_scope(), levels) else {
                self.error(
                    self.span_of(node),
                    "'owner' refers past the outermost scope",
                );
                return None;
            };
            let Some(symbol) = self.lookup_in(scope, &access.name).cloned() else {
                self.error(
                    self.span_of(node),
                    format!(
                        "'{}' is not declared in the scope that 'owner' refers to",
                        access.name
                    ),
                );
                return None;
            };
            return self.check_symbol_expr(node, &access.name, symbol);
        }
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
                        .map(|parameter| parameter.ty)
                        .collect::<Vec<_>>();
                    let return_type = self.result.functions[index as usize].return_type;
                    return Some(self.result.types.function(
                        parameters,
                        false,
                        self.result.functions[index as usize].calling_convention,
                        return_type,
                    ));
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

        // Type-qualified member access: `Status.Running`, `Optional.Value`,
        // `Pixel.RGBA` (§3.7.4-3.7.6).
        if let Some(Symbol::Type(type_id)) = self.try_resolve_name(access.base) {
            let Type::Named(index) = self.type_of(type_id) else {
                self.error(self.span_of(node), "type is not a namespace");
                return None;
            };
            let definition = &self.result.type_defs[index as usize];
            match definition.kind {
                TypeDefKind::Enumeration => {
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
                TypeDefKind::Class | TypeDefKind::Structure => {
                    if let Some(global) = definition.static_fields.get(&access.name).copied() {
                        let accessibility = self.result.globals[global as usize].accessibility;
                        let global_scoped = self.result.globals[global as usize].scoped.clone();
                        if !self.member_accessible(accessibility, type_id)
                            || !self.scope_allows(type_id, &global_scoped)
                        {
                            self.error(
                                self.span_of(node),
                                format!("'{}' is not accessible here", access.name),
                            );
                            return None;
                        }
                        self.result.resolutions[node as usize] = Some(Resolution::Global(global));
                        return Some(self.result.globals[global as usize].ty);
                    }
                    self.error(
                        self.span_of(node),
                        format!(
                            "'{}' is not a static member of '{}'",
                            access.name, definition.qualified_name
                        ),
                    );
                    return None;
                }
                TypeDefKind::Variant => {
                    if let Some(state) = definition
                        .variant_states
                        .iter()
                        .position(|state| state.name == access.name)
                    {
                        if definition.variant_states[state].payload.is_some() {
                            self.error(
                                self.span_of(node),
                                "a state with a payload must be constructed with 'State(value)'",
                            );
                            return None;
                        }
                        self.result.resolutions[node as usize] = Some(Resolution::VariantState {
                            type_id,
                            state: state as u32,
                        });
                        return Some(type_id);
                    }
                    self.error(
                        self.span_of(node),
                        format!(
                            "'{}' is not a state of '{}'",
                            access.name, definition.qualified_name
                        ),
                    );
                    return None;
                }
                TypeDefKind::Union => {
                    self.error(
                        self.span_of(node),
                        "union members are read through an instance, not the type name",
                    );
                    return None;
                }
                _ => {
                    self.error(
                        self.span_of(node),
                        "only enumeration, variant and union members can be accessed through a type name",
                    );
                    return None;
                }
            }
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
                let owner_type_id = self.result.types.intern(Type::Named(index));
                let definition = &self.result.type_defs[index as usize];
                if !matches!(
                    definition.kind,
                    TypeDefKind::Structure
                        | TypeDefKind::Class
                        | TypeDefKind::Union
                        | TypeDefKind::Interface
                        | TypeDefKind::Variant
                ) {
                    self.error(
                        self.span_of(node),
                        format!("'{}' has no fields", definition.qualified_name),
                    );
                    return None;
                }
                if let Some((declaring, field_index)) = self.find_field(owner_type_id, &access.name)
                {
                    let field = {
                        let Type::Named(declaring_index) = self.types_of(declaring) else {
                            return None;
                        };
                        let definition = &self.result.type_defs[declaring_index as usize];
                        definition.fields[field_index as usize].clone()
                    };
                    if !self.member_accessible(field.accessibility, declaring)
                        || !self.scope_allows(declaring, &field.scoped)
                    {
                        self.error(
                            self.span_of(node),
                            format!("'{}' is not accessible here", access.name),
                        );
                        return None;
                    }
                    self.result.resolutions[node as usize] = Some(Resolution::Field {
                        type_id: owner_type_id,
                        index: field_index,
                    });
                    return Some(field.ty);
                }
                if definition.kind == TypeDefKind::Union {
                    if let Some(member_index) = definition
                        .union_members
                        .iter()
                        .position(|member| member.name == access.name)
                    {
                        self.result.resolutions[node as usize] = Some(Resolution::UnionMember {
                            type_id: owner_type_id,
                            member: member_index as u32,
                        });
                        return Some(definition.union_members[member_index].ty);
                    }
                }
                // Accessor property (§3.8.2): reads go through the getter.
                if let Some((declaring, accessor)) = self.find_accessor(owner_type_id, &access.name)
                {
                    if !self.member_accessible(accessor.accessibility, declaring)
                        || !self.scope_allows(declaring, &accessor.scoped)
                    {
                        self.error(
                            self.span_of(node),
                            format!("'{}' is not accessible here", access.name),
                        );
                        return None;
                    }
                    self.result.resolutions[node as usize] = Some(Resolution::Accessor {
                        getter: accessor.getter,
                        setter: accessor.setter,
                    });
                    return Some(accessor.ty);
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
                // Array extension accessors (§3.8.3).
                if matches!(self.result.types.get(base_type), Type::Array { .. }) {
                    self.ensure_array_extensions(base_type);
                    if let Some((declaring, accessor)) = self.find_accessor(base_type, &access.name)
                    {
                        if !self.member_accessible(accessor.accessibility, declaring)
                            || !self.scope_allows(declaring, &accessor.scoped)
                        {
                            self.error(
                                self.span_of(node),
                                format!("'{}' is not accessible here", access.name),
                            );
                            return None;
                        }
                        self.result.resolutions[node as usize] = Some(Resolution::Accessor {
                            getter: accessor.getter,
                            setter: accessor.setter,
                        });
                        return Some(accessor.ty);
                    }
                }
                self.error(
                    self.span_of(node),
                    "member access requires a structure or class value",
                );
                None
            }
        }
    }

    /// Resolves a name chain to a symbol without reporting an error.
    pub(crate) fn try_resolve_name(&mut self, node: NodeId) -> Option<Symbol> {
        // A monomorphized name resolves to the instantiated type or function
        // (§10.2), so `Box<integer(4)>.Member` works like `Box.Member`.
        if let NodeKind::Monomorphization(monomorphization) = self.node_kind(node).clone() {
            return match self.check_monomorphization(node, &monomorphization)? {
                MonomorphizedTarget::Type(type_id) => Some(Symbol::Type(type_id)),
                MonomorphizedTarget::Function(index) => Some(Symbol::Functions(vec![index])),
            };
        }
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

    /// Whether an expression is a compile-time constant (§11.2): literals,
    /// operators over constants, fixed globals and the compiler API.
    fn is_compile_time_constant(&self, node: NodeId) -> bool {
        if let NodeKind::Name(name) = self.node_kind(node) {
            if self.top_level_constants.contains_key(&name.name) {
                return true;
            }
        }
        match self.node_kind(node) {
            NodeKind::IntegerLiteral(_)
            | NodeKind::FloatLiteral(_)
            | NodeKind::CharacterLiteral(_)
            | NodeKind::StringLiteral(_)
            | NodeKind::VerbatimStringLiteral(_)
            | NodeKind::BooleanLiteral(_)
            | NodeKind::InfiniteLiteral
            | NodeKind::IllegalLiteral => true,
            NodeKind::Unary(unary) => self.is_compile_time_constant(unary.operand),
            NodeKind::Binary(binary) => {
                self.is_compile_time_constant(binary.left)
                    && self.is_compile_time_constant(binary.right)
            }
            NodeKind::Conditional(conditional) => {
                self.is_compile_time_constant(conditional.condition)
                    && self.is_compile_time_constant(conditional.then_value)
                    && self.is_compile_time_constant(conditional.else_value)
            }
            NodeKind::Cast(cast) => self.is_compile_time_constant(cast.value),
            NodeKind::CompileTimeExpr(inner) => self.is_compile_time_constant(inner),
            NodeKind::Call(call) => {
                builtin_size_of_type_argument(self.ast, call.callee).is_some()
                    || builtin_namespace_call(self.ast, call.callee, "Platform")
                        .is_some_and(|name| name == "Is")
            }
            NodeKind::Name(name) => match self.lookup(&name.name) {
                Some(Symbol::Global(global)) => {
                    let info = &self.result.globals[*global as usize];
                    info.fixed && info.init.is_some()
                }
                _ => false,
            },
            _ => false,
        }
    }

    /// The value of an integer literal or a negated integer literal, without
    /// reporting diagnostics (used only for optional warnings).
    fn literal_integer(&self, node: NodeId) -> Option<i128> {
        match self.node_kind(node) {
            NodeKind::IntegerLiteral(literal) => literal::decode_integer(&literal.raw)
                .ok()
                .and_then(|value| i128::try_from(value).ok()),
            NodeKind::Unary(unary) if unary.op == UnaryOp::Negate => {
                self.literal_integer(unary.operand).map(|value| -value)
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
                // A provably out-of-bounds constant index is a warning (§4.4.3).
                if let Some(length) = dimensions.first().copied() {
                    if let Some(constant) = self.literal_integer(index.index) {
                        if constant < 0
                            || u64::try_from(constant).map_or(true, |value| value >= length)
                        {
                            self.warn(
                                self.span_of(index.index),
                                format!(
                                    "index {constant} is out of bounds for an array of length {length}"
                                ),
                            );
                        }
                    }
                }
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
            Type::Vla { element, .. } => Some(element),
            Type::Pointer { target: None } => {
                self.error(self.span_of(node), "cannot index a target-less pointer");
                None
            }
            _ => {
                if let Some(indexer) = self.find_indexer(base_type) {
                    self.result.resolutions[node as usize] = Some(Resolution::Indexer {
                        getter: indexer.getter,
                        setter: indexer.setter,
                    });
                    let accessor = indexer.getter.or(indexer.setter);
                    if let Some(accessor) = accessor {
                        let parameter = self.result.functions[accessor as usize]
                            .parameters
                            .get(1)
                            .map(|parameter| parameter.ty);
                        if let Some(parameter) = parameter {
                            let _ = self.check_expr(index.index, Some(parameter));
                        }
                    }
                    let Some(getter) = indexer.getter else {
                        self.error(self.span_of(node), "indexer is write-only");
                        return None;
                    };
                    return self.result.functions[getter as usize].return_type;
                }
                self.error(self.span_of(node), "indexing requires an array or pointer");
                None
            }
        }
    }

    fn check_cast(&mut self, node: NodeId, cast: &CastExpr) -> Option<TypeId> {
        let target = self.resolve_type(cast.type_)?;
        let source = self.check_expr(cast.value, None)?;
        if let Some(method) = self
            .find_conversion_operator(source, source, target)
            .or_else(|| self.find_conversion_operator(target, source, target))
        {
            self.result.resolutions[node as usize] = Some(Resolution::Conversion { method });
            return Some(target);
        }
        if matches!(
            (self.types_of(source), self.types_of(target)),
            (Type::Array { .. }, Type::Array { .. })
        ) && self.size_align(source).0 != self.size_align(target).0
        {
            self.unsupported(
                self.span_of(node),
                "array conversions between different byte sizes",
            );
            return None;
        }
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
        // Pointer-to-pointer conversions only change the target type, so the
        // spec asks for a warning (§3.6.2).
        if matches!(
            (self.types_of(source), self.types_of(target)),
            (Type::Pointer { .. }, Type::Pointer { .. })
        ) {
            self.warn(
                self.span_of(node),
                format!(
                    "pointer conversion from '{}' to '{}' changes only the target type",
                    self.spelling(source),
                    self.spelling(target)
                ),
            );
        }
        // Array/pointer conversions are legal but unsafe, so the spec asks for a
        // warning (§3.6.1).
        if matches!(
            (self.types_of(source), self.types_of(target)),
            (Type::Array { .. }, Type::Pointer { .. }) | (Type::Pointer { .. }, Type::Array { .. })
        ) {
            self.warn(
                self.span_of(node),
                format!(
                    "conversion between '{}' and '{}' does not copy or bounds-check",
                    self.spelling(source),
                    self.spelling(target)
                ),
            );
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
            // Fixed arrays are explicitly convertible regardless of shape
            // (§3.6.1); the conversion reinterprets the same bytes.
            (Type::Array { .. }, Type::Array { .. }) => {
                self.size_align(from).0 == self.size_align(to).0
            }
            // Enumeration <-> underlying numeric type.
            (Type::Named(left), Type::Named(right)) => {
                let left_kind = self.result.type_defs[*left as usize].kind;
                let right_kind = self.result.type_defs[*right as usize].kind;
                matches!(left_kind, TypeDefKind::Class | TypeDefKind::Interface)
                    && matches!(right_kind, TypeDefKind::Class | TypeDefKind::Interface)
            }
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
                if let Some(method) = self.find_operator(ty, ty, OperatorKind::Negate) {
                    let span = self.span_of(node);
                    let method = self.specialize_operator(method, &[unary.operand], span)?;
                    self.result.resolutions[node as usize] = Some(Resolution::Operator { method });
                    return self.result.functions[method as usize]
                        .return_type
                        .or(Some(ty));
                }
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
                let ty = self.check_expr(unary.operand, None)?;
                // Taking the address of a function value yields a pointer to the
                // function value (§2.7.9, §4.5).
                if matches!(self.types_of(ty), Type::Function { .. }) {
                    return Some(self.result.types.pointer(Some(ty)));
                }
                let addressable = matches!(
                    self.node_kind(unary.operand),
                    NodeKind::Name(_)
                        | NodeKind::This
                        | NodeKind::MemberAccess(_)
                        | NodeKind::PointerMemberAccess(_)
                        | NodeKind::Index(_)
                ) || matches!(
                    self.node_kind(unary.operand),
                    NodeKind::Unary(inner) if inner.op == UnaryOp::Dereference
                );
                if !addressable {
                    self.error(
                        self.span_of(node),
                        "unary '&' requires an addressable operand",
                    );
                    return None;
                }
                Some(self.result.types.pointer(Some(ty)))
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

    /// Attempts to resolve a user-defined operator for the operands (§4.9).
    fn check_overloaded_binary(
        &mut self,
        node: NodeId,
        op: BinaryOp,
        left: TypeId,
        right: TypeId,
        left_node: NodeId,
        right_node: NodeId,
    ) -> Option<TypeId> {
        let (kind, ordering) = match op {
            BinaryOp::Add => (OperatorKind::Add, None),
            BinaryOp::Subtract => (OperatorKind::Subtract, None),
            BinaryOp::Multiply => (OperatorKind::Multiply, None),
            BinaryOp::Divide => (OperatorKind::Divide, None),
            BinaryOp::Remainder => (OperatorKind::Remainder, None),
            BinaryOp::Equal => (OperatorKind::Equal, None),
            BinaryOp::Greater => (OperatorKind::Greater, None),
            BinaryOp::Less => (OperatorKind::Less, None),
            BinaryOp::NotEqual => (OperatorKind::Equal, Some(true)),
            BinaryOp::LessEqual => (OperatorKind::Less, Some(false)),
            BinaryOp::GreaterEqual => (OperatorKind::Greater, Some(false)),
            _ => return None,
        };
        match ordering {
            Some(negated) => {
                let method = self.find_operator(left, right, kind)?;
                let span = self.span_of(node);
                let method = self.specialize_operator(method, &[left_node, right_node], span)?;
                if negated {
                    self.result.resolutions[node as usize] =
                        Some(Resolution::DerivedNotEqual { method });
                } else {
                    let Some(equality) = self.find_operator(left, right, OperatorKind::Equal)
                    else {
                        self.error(
                            self.span_of(node),
                            "derived comparison requires an '==' overload",
                        );
                        return None;
                    };
                    let equality =
                        self.specialize_operator(equality, &[left_node, right_node], span)?;
                    self.result.resolutions[node as usize] = Some(Resolution::DerivedComparison {
                        ordering: method,
                        equality,
                    });
                }
                Some(TypeId::BOOLEAN)
            }
            None => {
                let method = self
                    .find_operator(left, right, kind)
                    .or_else(|| self.find_operator(right, left, kind))?;
                let span = self.span_of(node);
                let method = self.specialize_operator(method, &[left_node, right_node], span)?;
                self.result.resolutions[node as usize] = Some(Resolution::Operator { method });
                Some(
                    self.result.functions[method as usize]
                        .return_type
                        .unwrap_or(left),
                )
            }
        }
    }

    /// Finds an operator overload for the argument types along the owner chain.
    /// Result type of a built-in pointer arithmetic expression, if it applies
    /// (§3.6.2).
    fn pointer_arithmetic_result(
        &mut self,
        op: BinaryOp,
        left: TypeId,
        right: TypeId,
    ) -> Option<TypeId> {
        let left_pointer = matches!(self.type_of(left), Type::Pointer { .. });
        let right_pointer = matches!(self.type_of(right), Type::Pointer { .. });
        match op {
            BinaryOp::Add => {
                if left_pointer && self.is_integer(right) {
                    Some(left)
                } else if right_pointer && self.is_integer(left) {
                    Some(right)
                } else {
                    None
                }
            }
            BinaryOp::Subtract => {
                if left_pointer && self.is_integer(right) {
                    Some(left)
                } else if left_pointer && right_pointer && left == right {
                    // The number of elements between the two addresses; the
                    // spec leaves the result type to the type page, so a signed
                    // 64-bit count is used (see docs/spec-notes/_inconsistencies.md).
                    Some(self.result.types.integer(true, 8))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Specializes a constant-parameter operator for the given operand
    /// expressions (§11.5); non-template operators are returned unchanged.
    fn specialize_operator(&mut self, method: u32, operands: &[NodeId], span: Span) -> Option<u32> {
        let is_template = {
            let function = &self.result.functions[method as usize];
            function.template.is_none()
                && function
                    .parameters
                    .iter()
                    .any(|parameter| parameter.compile_time)
        };
        if !is_template {
            return Some(method);
        }
        let parameters = self.result.functions[method as usize].parameters.clone();
        let mut constants = Vec::new();
        for (parameter, operand) in parameters.iter().zip(operands.iter()) {
            if !parameter.compile_time {
                continue;
            }
            if !self.is_compile_time_constant(*operand) {
                self.error(
                    span,
                    "the arguments of a compile-time ('#') parameter must be compile-time constants",
                );
                return None;
            }
            constants.push(self.eval_const_int(*operand)?);
        }
        self.instantiate_constant_function(method, &constants, span)
    }

    fn find_operator(&self, owner: TypeId, other: TypeId, kind: OperatorKind) -> Option<u32> {
        // Non-named owners (array extensions, §3.8.3).
        if !matches!(self.types_of(owner), Type::Named(_)) {
            for operator in self
                .result
                .owner_operators
                .get(&owner)
                .map(Vec::as_slice)
                .unwrap_or_default()
            {
                if operator.kind != kind || operator.method == u32::MAX {
                    continue;
                }
                let parameters = &self.result.functions[operator.method as usize].parameters;
                let expected = if matches!(kind, OperatorKind::Negate | OperatorKind::Coalesce) {
                    1
                } else {
                    2
                };
                if parameters.len() == expected
                    && self.is_assignable(owner, parameters[0].ty)
                    && (expected == 1 || self.is_assignable(other, parameters[1].ty))
                {
                    return Some(operator.method);
                }
            }
            return None;
        }
        let mut current = Some(owner);
        let mut depth = 0;
        while let Some(ty) = current {
            depth += 1;
            if depth > 32 {
                break;
            }
            let Type::Named(index) = self.types_of(ty) else {
                break;
            };
            let definition = &self.result.type_defs[index as usize];
            for operator in &definition.operators {
                if operator.kind != kind || operator.method == u32::MAX {
                    continue;
                }
                let parameters = &self.result.functions[operator.method as usize].parameters;
                let expected = if matches!(kind, OperatorKind::Negate | OperatorKind::Coalesce) {
                    1
                } else {
                    2
                };
                if parameters.len() == expected
                    && self.is_assignable(ty, parameters[0].ty)
                    && (expected == 1 || self.is_assignable(other, parameters[1].ty))
                {
                    return Some(operator.method);
                }
            }
            current = definition.base;
        }
        None
    }

    /// Finds an indexer operator on the owner chain.
    fn find_indexer(&self, owner: TypeId) -> Option<OperatorInfo> {
        let mut current = Some(owner);
        let mut depth = 0;
        while let Some(ty) = current {
            depth += 1;
            if depth > 32 {
                break;
            }
            let Type::Named(index) = self.types_of(ty) else {
                break;
            };
            let definition = &self.result.type_defs[index as usize];
            if let Some(operator) = definition
                .operators
                .iter()
                .find(|operator| operator.kind == OperatorKind::Index)
            {
                return Some(operator.clone());
            }
            current = definition.base;
        }
        None
    }

    /// Finds a conversion operator from `source` to `target` on `owner`.
    /// Finds an `implicitable` conversion operator from `source` to `target`
    /// (§9.7). Conversion operators are declared on the target type, so the
    /// target's declaration chain is searched first and the source's second.
    fn find_implicit_conversion(&self, source: TypeId, target: TypeId) -> Option<u32> {
        for owner in [target, source] {
            let mut current = Some(owner);
            let mut depth = 0;
            while let Some(ty) = current {
                depth += 1;
                if depth > 32 {
                    break;
                }
                let Type::Named(index) = self.types_of(ty) else {
                    break;
                };
                let definition = &self.result.type_defs[index as usize];
                for operator in &definition.operators {
                    if operator.kind != OperatorKind::Conversion
                        || operator.method == u32::MAX
                        || !operator.implicitable
                    {
                        continue;
                    }
                    let function = &self.result.functions[operator.method as usize];
                    if function.return_type == Some(target)
                        && function.parameters.len() == 1
                        && self.is_assignable(source, function.parameters[0].ty)
                    {
                        return Some(operator.method);
                    }
                }
                current = definition.base;
            }
        }
        None
    }

    fn find_conversion_operator(
        &self,
        owner: TypeId,
        source: TypeId,
        target: TypeId,
    ) -> Option<u32> {
        let mut current = Some(owner);
        let mut depth = 0;
        while let Some(ty) = current {
            depth += 1;
            if depth > 32 {
                break;
            }
            let Type::Named(index) = self.types_of(ty) else {
                break;
            };
            let definition = &self.result.type_defs[index as usize];
            for operator in &definition.operators {
                if operator.kind != OperatorKind::Conversion || operator.method == u32::MAX {
                    continue;
                }
                let function = &self.result.functions[operator.method as usize];
                if function.return_type == Some(target)
                    && function.parameters.len() == 1
                    && self.is_assignable(source, function.parameters[0].ty)
                {
                    return Some(operator.method);
                }
            }
            current = definition.base;
        }
        None
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

        // A pointer expectation never types a literal operand (§3.6.2).
        let literal_expected =
            expected.filter(|ty| !matches!(self.type_of(*ty), Type::Pointer { .. }));
        let mut left_type = self.check_expr(
            binary.left,
            if left_is_literal {
                literal_expected
            } else {
                None
            },
        );
        let mut right_type = match left_type {
            // A pointer operand does not type an integer literal: the literal
            // keeps its default integer type for pointer arithmetic (§3.6.2).
            Some(left_type)
                if right_is_literal && matches!(self.type_of(left_type), Type::Pointer { .. }) =>
            {
                self.check_expr(binary.right, None)
            }
            Some(left_type) => self.check_expr(binary.right, Some(left_type)),
            None => self.check_expr(binary.right, expected),
        };
        if left_type != right_type {
            // Pointer operands never type the other operand (§3.6.2): pointer
            // arithmetic accepts any integer type.
            let left_is_pointer =
                left_type.is_some_and(|ty| matches!(self.type_of(ty), Type::Pointer { .. }));
            let right_is_pointer =
                right_type.is_some_and(|ty| matches!(self.type_of(ty), Type::Pointer { .. }));
            if left_is_literal && !right_is_pointer {
                if let Some(right_type) = right_type {
                    left_type = self.check_expr(binary.left, Some(right_type));
                }
            } else if right_is_literal && !left_is_pointer {
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
        if matches!(
            binary.op,
            BinaryOp::Equal
                | BinaryOp::NotEqual
                | BinaryOp::Less
                | BinaryOp::Greater
                | BinaryOp::LessEqual
                | BinaryOp::GreaterEqual
        ) && (matches!(self.node_kind(binary.left), NodeKind::IllegalLiteral)
            || matches!(self.node_kind(binary.right), NodeKind::IllegalLiteral))
        {
            self.error(
                self.span_of(node),
                "the illegal value literal must not be used as a comparison operand",
            );
        }
        // Array extension operators are instantiated for the operand types
        // before overload resolution (§3.8.3).
        for operand in [left_type, right_type] {
            if matches!(self.result.types.get(operand), Type::Array { .. }) {
                self.ensure_array_extensions(operand);
            }
        }
        if let Some(result) = self.check_overloaded_binary(
            node,
            binary.op,
            left_type,
            right_type,
            binary.left,
            binary.right,
        ) {
            return Some(result);
        }
        // Built-in pointer arithmetic (§3.6.2): `pointer + integer`,
        // `integer + pointer`, `pointer - integer` stay pointers, and
        // `pointer - pointer` is an element count.
        if let Some(result) = self.pointer_arithmetic_result(binary.op, left_type, right_type) {
            return Some(result);
        }
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
                let comparable = self.has_builtin_equality(left_type);
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

    /// Checks a `??` expression (§4.9.26): the left operand decides between its
    /// value and the alternative continuation.
    /// Checks a function literal (§2.7.9) and registers a function for it.
    ///
    /// Parameter types may be omitted when the expected type provides them.
    /// Captures are not implemented: identifiers from the enclosing function are
    /// not visible inside the literal (they resolve as unknown names).
    fn check_function_literal(
        &mut self,
        node: NodeId,
        literal: &FunctionLiteral,
        expected: Option<TypeId>,
    ) -> Option<TypeId> {
        let span = self.span_of(node);
        let previous_in_body = self.in_function_body;
        self.in_function_body = false;
        let expected_parameters = expected.and_then(|expected| match self.types_of(expected) {
            Type::Function {
                parameters,
                return_type,
                ..
            } => Some((parameters, return_type)),
            _ => None,
        });
        let mut parameters = Vec::new();
        for (position, parameter) in literal.parameters.iter().enumerate() {
            if parameter.compile_time {
                self.unsupported(parameter.span, "compile-time ('#') parameters");
                return None;
            }
            let ty = match parameter.ty {
                Some(ty_node) => self.resolve_type(ty_node)?,
                None => match expected_parameters
                    .as_ref()
                    .and_then(|(parameters, _)| parameters.get(position).copied())
                {
                    Some(ty) => ty,
                    None => {
                        self.error(
                            parameter.span,
                            "cannot infer the parameter type of this function literal",
                        );
                        return None;
                    }
                },
            };
            parameters.push(ParameterInfo {
                name: parameter.name.clone(),
                ty,
                by_ref: parameter.by_ref,
                default: None,
                compile_time: false,
            });
        }
        let return_type = literal.return_type.and_then(|ty| self.resolve_type(ty));
        let calling_convention = self.resolve_calling_convention(literal.calling_convention);
        let function_type = self.result.types.function(
            parameters.iter().map(|parameter| parameter.ty).collect(),
            false,
            calling_convention,
            return_type,
        );
        self.in_function_body = previous_in_body;
        let body = match self.node_kind(literal.body) {
            NodeKind::Block(_) => FunctionBody::Block(literal.body),
            NodeKind::VerbatimBlock(inner) => FunctionBody::Block(inner),
            _ => FunctionBody::Expression(literal.body),
        };
        // Capture the enclosing function's locals (and `this`) by value (§2.7.9).
        let parameter_names: Vec<String> = literal
            .parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect();
        let mut visible: Vec<(String, Symbol)> = Vec::new();
        let mut scope = Some(self.current_scope());
        while let Some(index) = scope {
            for (name, symbol) in self.scopes[index].symbols.iter() {
                if visible.iter().any(|(existing, _)| existing == name) {
                    continue;
                }
                if parameter_names.iter().any(|parameter| parameter == name) {
                    continue;
                }
                visible.push((name.clone(), symbol.clone()));
            }
            scope = self.scopes[index].parent;
        }
        let mut captures: Vec<CaptureInfo> = Vec::new();
        let mut offset: u64 = 0;
        for (name, symbol) in visible {
            let (ty, source) = match symbol {
                Symbol::Local(local) => match self.local_info(local).cloned() {
                    Some(info) => (info.ty, CaptureSource::Local(local)),
                    None => continue,
                },
                Symbol::This { owner } => (owner, CaptureSource::This),
                _ => continue,
            };
            let (size, alignment) = self.size_align(ty);
            offset = round_up(offset, alignment.max(1));
            captures.push(CaptureInfo {
                name,
                ty,
                local: captures.len() as LocalId,
                offset,
                source,
            });
            offset += size;
        }
        let capture_locals: Vec<LocalInfo> = captures
            .iter()
            .map(|capture| LocalInfo {
                name: capture.name.clone(),
                ty: capture.ty,
                fixed: false,
                parameter: None,
                storage: Storage::Capture(capture.local),
                span,
            })
            .collect();
        let index = self.result.functions.len() as u32;
        let name = format!("lambda{index}");
        let scope = self.scope_path.clone();
        let spellings: Vec<String> = parameters
            .iter()
            .map(|parameter| self.spelling(parameter.ty))
            .collect();
        let mangled = mangle_function(&scope, &name, &spellings);
        let generic_env = self.captured_generic_env();
        let decl_scope = self.current_scope();
        let span_of_function = span;
        // Compile-time bindings are lexically visible inside function literals
        // (§11.5).
        let mut inherited_constants = HashMap::new();
        for environment in &self.constant_env {
            for (name, value) in environment {
                inherited_constants.entry(name.clone()).or_insert(*value);
            }
        }
        self.result.functions.push(FunctionInfo {
            name,
            scope,
            node,
            body: Some(body),
            parameters,
            method_of: None,
            static_owner: None,
            is_virtual: false,
            is_override: false,
            is_sealed: false,
            accessibility: crate::ast::Accessibility::Public,
            calling_convention: None,
            scoped: Vec::new(),
            is_abstract: false,
            return_type,
            mangled,
            exposed_symbol: None,
            generic_parameters: Vec::new(),
            generic_arguments: Vec::new(),
            template: None,
            decl_scope,
            generic_env,
            environment: true,
            captures,
            constant_values: inherited_constants,
            locals: capture_locals,
            span: span_of_function,
        });
        self.result.functions[index as usize].calling_convention = calling_convention;
        self.result.function_literals.insert(node, index);
        Some(function_type)
    }

    fn check_coalesce(&mut self, node: NodeId, coalesce: &CoalesceExpr) -> Option<TypeId> {
        let span = self.span_of(node);
        let operand_type = self.check_expr(coalesce.value, None)?;
        // The decision comes either from a built-in `Result`/`Optional` value or
        // from a user-defined unary `operator ??` (§4.9.26).
        let mut operator = None;
        let mut decision_type = operand_type;
        if let Type::Named(index) = self.types_of(operand_type) {
            match self.result.type_defs[index as usize].builtin_variant {
                Some(BuiltinVariantKind::Result | BuiltinVariantKind::Optional) => {}
                _ => {
                    if self.result.type_defs[index as usize].template.is_none() {
                        match self.find_operator(operand_type, operand_type, OperatorKind::Coalesce)
                        {
                            Some(method) => {
                                operator = Some(method);
                                decision_type = self.result.functions[method as usize]
                                    .return_type
                                    .unwrap_or(TypeId::ERROR);
                            }
                            None => {
                                self.unsupported(span, "?? on types without a decision operator");
                                return None;
                            }
                        }
                    }
                }
            }
        } else {
            match self.find_operator(operand_type, operand_type, OperatorKind::Coalesce) {
                Some(method) => {
                    operator = Some(method);
                    decision_type = self.result.functions[method as usize]
                        .return_type
                        .unwrap_or(TypeId::ERROR);
                }
                None => {
                    self.unsupported(span, "?? on types without a decision operator");
                    return None;
                }
            }
        }
        let template = match self.types_of(decision_type) {
            Type::Named(index) => self.result.type_defs[index as usize].template,
            _ => None,
        };
        let Some(template) = template else {
            self.error(span, "'operator ??' must return a Result value");
            return None;
        };
        let kind = self.result.type_defs[template as usize].builtin_variant;
        let Type::Named(decision_index) = self.types_of(decision_type) else {
            return None;
        };
        let arguments = self.result.type_defs[decision_index as usize]
            .generic_arguments
            .clone();
        let (result_type, alternative_type) = match kind {
            Some(BuiltinVariantKind::Optional) => (arguments[0], TypeId::VOID),
            Some(BuiltinVariantKind::Result) => (arguments[0], arguments[1]),
            None => {
                self.error(
                    span,
                    "the decision value must be a Result or Optional variant",
                );
                return None;
            }
        };
        let states = self.result.type_defs[decision_index as usize]
            .variant_states
            .clone();
        let value_state = states
            .iter()
            .position(|state| state.name == "Value")
            .unwrap_or(0) as u32;
        let alternative_state = if states.len() > 1 { 1 } else { 0 };
        match coalesce.alternative {
            CoalescingAlternative::Expression(expression) => {
                let inferred = self.check_expr(expression, Some(result_type));
                if let Some(inferred) = inferred {
                    self.expect_assignable(self.span_of(expression), inferred, result_type);
                }
            }
            CoalescingAlternative::Throw(exception) => {
                if alternative_type == TypeId::VOID {
                    self.error(
                        span,
                        "the alternative carries no value; '?? throw' requires an exception",
                    );
                    return None;
                }
                if !matches!(
                    self.trap_contexts.last(),
                    Some(TrapContextKind::Body | TrapContextKind::Catch)
                ) {
                    self.error(span, "'?? throw' must appear inside a trap or catch block");
                    return None;
                }
                if let Some(exception) = exception {
                    self.check_expr(exception, None);
                }
            }
            CoalescingAlternative::Return(value) => {
                let return_type = self
                    .current_function
                    .and_then(|index| self.result.functions[index as usize].return_type);
                match value {
                    Some(value) => {
                        let inferred = self.check_expr(value, return_type);
                        if let (Some(inferred), Some(return_type)) = (inferred, return_type) {
                            self.expect_assignable(self.span_of(value), inferred, return_type);
                        }
                    }
                    None => {
                        if return_type != Some(decision_type) {
                            self.error(
                                span,
                                "'?? return' requires the function to return the decision value",
                            );
                            return None;
                        }
                    }
                }
            }
        }
        self.result.coalesce.insert(
            node,
            CoalesceInfo {
                decision_type,
                result_type,
                alternative_type,
                value_state,
                alternative_state,
                operator,
            },
        );
        Some(result_type)
    }

    fn check_is_expression(&mut self, node: NodeId, is_expression: &IsExpr) -> Option<TypeId> {
        let value_type = self.check_expr(is_expression.value, None)?;
        match &is_expression.target {
            IsTarget::Identifier {
                text,
                node: target_node,
            } => {
                let resolved = self.resolve_is_target(node, *target_node, text);
                match resolved {
                    Some(IsTargetResolved::Type(target_type)) => {
                        let value_kind = self.types_of(value_type);
                        let target_kind = self.types_of(target_type);
                        let value_is_class = matches!(&value_kind, Type::Named(index) if matches!(self.result.type_defs[*index as usize].kind, TypeDefKind::Class | TypeDefKind::Interface));
                        let target_is_class = matches!(&target_kind, Type::Named(index) if matches!(self.result.type_defs[*index as usize].kind, TypeDefKind::Class | TypeDefKind::Interface));
                        if !target_is_class {
                            self.unsupported(
                                self.span_of(node),
                                "is checks against this target kind",
                            );
                            return None;
                        }
                        if !value_is_class {
                            // A value that cannot be a class/interface instance can
                            // never match: the result is statically false (§4.8).
                            self.result.resolutions[node as usize] = Some(Resolution::Constant {
                                ty: TypeId::BOOLEAN,
                                value: 0,
                            });
                            return Some(TypeId::BOOLEAN);
                        }
                        self.result.resolutions[node as usize] =
                            Some(Resolution::Type(target_type));
                        Some(TypeId::BOOLEAN)
                    }
                    Some(IsTargetResolved::VariantState { type_id, state }) => {
                        if !self.is_assignable(value_type, type_id) {
                            self.error(
                                self.span_of(node),
                                "the checked value is not a variant instance",
                            );
                            return None;
                        }
                        self.result.resolutions[node as usize] =
                            Some(Resolution::VariantState { type_id, state });
                        Some(TypeId::BOOLEAN)
                    }
                    None => None,
                }
            }
            IsTarget::Illegal => {
                if !matches!(self.types_of(value_type), Type::Float { .. }) {
                    self.error(
                        self.span_of(node),
                        "checking for 'illegal' requires a floating-point value",
                    );
                }
                Some(TypeId::BOOLEAN)
            }
            IsTarget::Infinite { .. } => {
                if !matches!(self.types_of(value_type), Type::Float { .. }) {
                    self.error(
                        self.span_of(node),
                        "checking for 'infinite' requires a floating-point value",
                    );
                }
                Some(TypeId::BOOLEAN)
            }
        }
    }

    /// Resolves the target of an `is` expression or pattern: a class type or a
    /// variant state (`Variant.State`).
    /// Resolves an `is` target, preferring the parsed name chain so that
    /// monomorphized targets (`Variant<Arguments>.State`, §10.2) resolve.
    fn resolve_is_target(
        &mut self,
        report_node: NodeId,
        target_node: NodeId,
        text: &str,
    ) -> Option<IsTargetResolved> {
        if let NodeKind::MemberAccess(access) = self.node_kind(target_node).clone() {
            if let NodeKind::Monomorphization(monomorphization) =
                self.node_kind(access.base).clone()
            {
                return match self.check_monomorphization(access.base, &monomorphization) {
                    Some(MonomorphizedTarget::Type(type_id)) => {
                        let state = match self.types_of(type_id) {
                            Type::Named(index)
                                if self.result.type_defs[index as usize].kind
                                    == TypeDefKind::Variant =>
                            {
                                self.result.type_defs[index as usize]
                                    .variant_states
                                    .iter()
                                    .position(|state| state.name == access.name)
                            }
                            _ => None,
                        };
                        match state {
                            Some(state) => Some(IsTargetResolved::VariantState {
                                type_id,
                                state: state as u32,
                            }),
                            None => {
                                self.error(
                                    self.span_of(report_node),
                                    format!("'{text}' is not a variant state"),
                                );
                                None
                            }
                        }
                    }
                    Some(MonomorphizedTarget::Function(_)) => {
                        self.error(
                            self.span_of(report_node),
                            "a generic function cannot be used as an is-target",
                        );
                        None
                    }
                    None => None,
                };
            }
        }
        self.resolve_is_target_name(report_node, text)
    }

    fn resolve_is_target_name(&mut self, node: NodeId, text: &str) -> Option<IsTargetResolved> {
        let parts: Vec<&str> = text.split('.').collect();
        if parts.is_empty() {
            return None;
        }
        if let Some(Symbol::Type(type_id)) = self.resolve_symbol_path(&parts) {
            return Some(IsTargetResolved::Type(type_id));
        }
        if parts.len() >= 2 {
            let type_parts = &parts[..parts.len() - 1];
            if let Some(Symbol::Type(type_id)) = self.resolve_symbol_path(type_parts) {
                if let Type::Named(index) = self.types_of(type_id) {
                    let definition = &self.result.type_defs[index as usize];
                    if definition.kind == TypeDefKind::Variant {
                        let state_name = parts[parts.len() - 1];
                        if let Some(state) = definition
                            .variant_states
                            .iter()
                            .position(|state| state.name == state_name)
                        {
                            return Some(IsTargetResolved::VariantState {
                                type_id,
                                state: state as u32,
                            });
                        }
                    }
                }
            }
        }
        self.error(self.span_of(node), format!("unknown is-target '{text}'"));
        None
    }

    fn resolve_symbol_path(&mut self, parts: &[&str]) -> Option<Symbol> {
        let mut symbol = self.lookup(parts.first()?).cloned();
        for part in parts.iter().skip(1) {
            symbol = match symbol {
                Some(Symbol::Module(scope)) => self.lookup_in(scope, part).cloned(),
                _ => None,
            };
        }
        symbol
    }

    fn check_variant_construction(
        &mut self,
        node: NodeId,
        type_id: TypeId,
        state_name: &str,
        arguments: &[Argument],
    ) -> Option<TypeId> {
        let Type::Named(index) = self.types_of(type_id) else {
            return None;
        };
        let definition = &self.result.type_defs[index as usize];
        let Some(state) = definition
            .variant_states
            .iter()
            .position(|state| state.name == state_name)
        else {
            self.error(
                self.span_of(node),
                format!(
                    "'{state_name}' is not a state of '{}'",
                    definition.qualified_name
                ),
            );
            return None;
        };
        let payload = definition.variant_states[state].payload;
        let Some(payload_type) = payload else {
            self.error(
                self.span_of(node),
                "a state without a payload is not constructed with a value",
            );
            return None;
        };
        if arguments.len() != 1 {
            self.error(
                self.span_of(node),
                "a variant state with a payload takes exactly one value",
            );
            return None;
        }
        let argument_type = self.check_expr(arguments[0].value, Some(payload_type));
        if let Some(argument_type) = argument_type {
            self.expect_assignable(arguments[0].span, argument_type, payload_type);
        }
        self.result.resolutions[node as usize] = Some(Resolution::ConstructVariant {
            type_id,
            state: state as u32,
        });
        Some(type_id)
    }

    fn check_union_construction(
        &mut self,
        node: NodeId,
        type_id: TypeId,
        member_name: &str,
        arguments: &[Argument],
    ) -> Option<TypeId> {
        let Type::Named(index) = self.types_of(type_id) else {
            return None;
        };
        let definition = &self.result.type_defs[index as usize];
        let Some(member) = definition
            .union_members
            .iter()
            .position(|member| member.name == member_name)
        else {
            self.error(
                self.span_of(node),
                format!(
                    "'{member_name}' is not a member of '{}'",
                    definition.qualified_name
                ),
            );
            return None;
        };
        let member_type = definition.union_members[member].ty;
        if arguments.len() != 1 {
            self.error(
                self.span_of(node),
                "a union member construction takes exactly one value",
            );
            return None;
        }
        let argument_type = self.check_expr(arguments[0].value, Some(member_type));
        if let Some(argument_type) = argument_type {
            self.expect_assignable(arguments[0].span, argument_type, member_type);
        }
        self.result.resolutions[node as usize] = Some(Resolution::ConstructUnion {
            type_id,
            member: member as u32,
        });
        Some(type_id)
    }

    /// Checks a `NOLAA.*` heap lifetime control call (§15.3). These are compiler
    /// hints and lower to no code.
    fn check_nolaa_call(
        &mut self,
        node: NodeId,
        name: &str,
        arguments: &[Argument],
    ) -> Option<TypeId> {
        let span = self.span_of(node);
        let expected = match name {
            "Abandon" | "AssumeNoDemandNext" => 1,
            "Bind" => 2,
            _ => {
                self.error(span, format!("unknown compiler API 'NOLAA.{name}'"));
                return None;
            }
        };
        if arguments.len() != expected {
            self.error(span, format!("'NOLAA.{name}' takes {expected} argument(s)"));
            return None;
        }
        // Every argument must point at a NOLAA-trackable heap object (§12.2).
        let mut locals: Vec<LocalId> = Vec::new();
        for argument in arguments {
            let trackable = match self.node_kind(argument.value) {
                NodeKind::Name(argument_name) => match self.lookup(&argument_name.name).cloned() {
                    Some(Symbol::Local(local)) => self.heap_unit_locals.contains_key(&local),
                    _ => false,
                },
                _ => false,
            };
            if !trackable {
                self.error(
                    self.span_of(argument.value),
                    "NOLAA arguments must point to a trackable heap object",
                );
                return None;
            }
            let NodeKind::Name(argument_name) = self.node_kind(argument.value) else {
                return None;
            };
            let Some(Symbol::Local(local)) = self.lookup(&argument_name.name).cloned() else {
                return None;
            };
            locals.push(local);
        }
        if name == "Abandon" {
            if let Some(index) = self.heap_unit_locals.get(&locals[0]).copied() {
                self.heap_units[index].abandoned = true;
            }
        }
        if name == "Bind" {
            let (required, required_by) = (locals[0], locals[1]);
            if required != required_by {
                let (mut group, other) = match self
                    .heap_groups
                    .iter()
                    .position(|group| group.contains(&required))
                {
                    Some(index) => {
                        let group = self.heap_groups.remove(index);
                        let other = self
                            .heap_groups
                            .iter()
                            .position(|group| group.contains(&required_by))
                            .map(|index| self.heap_groups.remove(index));
                        (group, other)
                    }
                    None => (vec![required], {
                        self.heap_groups
                            .iter()
                            .position(|group| group.contains(&required_by))
                            .map(|index| self.heap_groups.remove(index))
                    }),
                };
                group.extend(other.unwrap_or_else(|| vec![required_by]));
                group.sort_unstable();
                group.dedup();
                self.heap_groups.push(group);
            }
        }
        self.result.resolutions[node as usize] = Some(Resolution::CompileTimeNoOp);
        Some(TypeId::VOID)
    }

    /// Evaluates `Platform.Is(...)` against the compilation target (§15.3).
    fn check_platform_is(&mut self, node: NodeId, arguments: &[Argument]) -> Option<TypeId> {
        let span = self.span_of(node);
        if arguments.len() != 1 {
            self.error(span, "Platform.Is takes exactly one argument");
            return None;
        }
        let argument = &arguments[0];
        let matches = match self.node_kind(argument.value) {
            NodeKind::StringLiteral(literal) => match literal::decode_string(&literal.raw) {
                Ok(decoded) => {
                    let name = String::from_utf8_lossy(&decoded.bytes).into_owned();
                    // Names outside the canonical set answer false (§15.3).
                    Self::platform_kind_from_name(&name)
                        .is_some_and(|kind| kind == Self::host_platform_kind())
                }
                Err(message) => {
                    self.error(self.span_of(argument.value), message);
                    return None;
                }
            },
            NodeKind::MemberAccess(access) => {
                // `PlatformKind.Member`
                let Some(Symbol::Type(type_id)) = self.try_resolve_name(access.base) else {
                    self.error(self.span_of(node), "Platform.Is expects a platform kind");
                    return None;
                };
                let Type::Named(index) = self.types_of(type_id) else {
                    return None;
                };
                let definition = &self.result.type_defs[index as usize];
                if definition.kind != TypeDefKind::Enumeration {
                    self.error(self.span_of(node), "Platform.Is expects a platform kind");
                    return None;
                }
                match definition
                    .enum_members
                    .iter()
                    .find(|(member, _)| *member == access.name)
                {
                    Some((_, value)) => *value as u32 == Self::host_platform_kind(),
                    None => {
                        self.error(
                            self.span_of(node),
                            format!("'{}' is not a platform kind", access.name),
                        );
                        return None;
                    }
                }
            }
            _ => {
                self.unsupported(
                    self.span_of(argument.value),
                    "Platform.Is with non-constant arguments",
                );
                return None;
            }
        };
        self.result.resolutions[node as usize] = Some(Resolution::Constant {
            ty: TypeId::BOOLEAN,
            value: i128::from(matches),
        });
        Some(TypeId::BOOLEAN)
    }

    /// Handles the compile-time diagnostics API (§15.3).
    fn check_compiler_call(
        &mut self,
        node: NodeId,
        name: String,
        arguments: &[Argument],
    ) -> Option<TypeId> {
        let span = self.span_of(node);
        if self.compile_time_constants.is_empty() {
            self.error(
                span,
                format!("'Compiler.{name}' is only available in compile-time code"),
            );
            return None;
        }
        let message = |me: &mut Self, argument: &Argument| -> Option<String> {
            let NodeKind::StringLiteral(literal) = me.node_kind(argument.value) else {
                me.unsupported(me.span_of(argument.value), "computed compile-time messages");
                return None;
            };
            match literal::decode_string(&literal.raw) {
                Ok(decoded) => Some(String::from_utf8_lossy(&decoded.bytes).into_owned()),
                Err(message) => {
                    me.error(me.span_of(argument.value), message);
                    None
                }
            }
        };
        match name.as_str() {
            "LogNote" | "LogWarning" | "LogError" => {
                let Some(argument) = arguments.first() else {
                    self.error(span, format!("'Compiler.{name}' requires a message"));
                    return None;
                };
                let message = message(self, argument)?;
                match name.as_str() {
                    "LogNote" => self.note(span, message),
                    "LogWarning" => self.warn(span, message),
                    _ => self.error(span, message),
                }
            }
            "Assert" => {
                let Some(condition) = arguments.first() else {
                    self.error(span, "'Compiler.Assert' requires a condition");
                    return None;
                };
                match self.eval_compile_time_bool(condition.value, 0) {
                    Some(true) => {}
                    Some(false) => {
                        let detail = match arguments
                            .get(1)
                            .and_then(|argument| message(self, argument))
                        {
                            Some(message) => format!("compile-time assertion failed: {message}"),
                            None => "compile-time assertion failed".to_string(),
                        };
                        self.error(span, detail);
                    }
                    None => self.unsupported(
                        self.span_of(condition.value),
                        "compile-time assertions with non-constant conditions",
                    ),
                }
            }
            _ => {
                self.error(span, format!("unknown compiler API function '{name}'"));
                return None;
            }
        }
        self.result.resolutions[node as usize] = Some(Resolution::CompileTimeNoOp);
        Some(TypeId::VOID)
    }

    /// Handles `State(payload)` where `State` is a variant state name elided in a
    /// context that expects the variant type (§3.7.5). Returns `None` when the
    /// callee is not an elided variant state.
    fn check_elided_state_construction(
        &mut self,
        node: NodeId,
        call: &CallExpr,
        expected: Option<TypeId>,
    ) -> Option<Option<TypeId>> {
        let expected = expected?;
        let NodeKind::Name(name) = self.node_kind(call.callee) else {
            return None;
        };
        if self.lookup(&name.name).is_some() {
            return None;
        }
        let Type::Named(index) = self.types_of(expected) else {
            return None;
        };
        if self.result.type_defs[index as usize].kind != TypeDefKind::Variant {
            return None;
        }
        let state = self.result.type_defs[index as usize]
            .variant_states
            .iter()
            .position(|state| state.name == name.name)?;
        let payload_type = self.result.type_defs[index as usize].variant_states[state].payload?;
        if call.arguments.len() != 1 {
            self.error(
                self.span_of(node),
                format!(
                    "variant state '{}' expects exactly one payload argument",
                    name.name
                ),
            );
            return Some(None);
        }
        let argument = &call.arguments[0];
        let value_type = self.check_expr(argument.value, Some(payload_type));
        if let Some(value_type) = value_type {
            self.expect_assignable(argument.span, value_type, payload_type);
        }
        self.result.resolutions[node as usize] = Some(Resolution::ConstructVariant {
            type_id: expected,
            state: state as u32,
        });
        Some(Some(expected))
    }

    /// Resolves a generic method call `receiver.Method<TypeArgs>(args)` by
    /// instantiating the method template (§10.1, §10.2).
    fn check_generic_method_call(
        &mut self,
        node: NodeId,
        call: &CallExpr,
        monomorphization: &MonomorphizationExpr,
        access: &MemberAccess,
    ) -> Option<TypeId> {
        let span = self.span_of(node);
        let base_type = self.check_expr(access.base, None)?;
        let Type::Named(index) = self.types_of(base_type) else {
            self.error(span, "a generic method call requires a named receiver");
            return None;
        };
        let name = access.name.clone();
        let template = self.result.type_defs[index as usize]
            .methods
            .get(&name)
            .and_then(|overloads| {
                overloads.iter().copied().find(|candidate| {
                    !self.result.functions[*candidate as usize]
                        .generic_parameters
                        .is_empty()
                })
            });
        let Some(template) = template else {
            self.error(span, format!("'{name}' has no generic form"));
            return None;
        };
        let mut arguments = Vec::new();
        for argument in &monomorphization.arguments {
            let Some(argument) = argument else {
                self.error(span, "generic method calls require explicit type arguments");
                return None;
            };
            arguments.push(self.resolve_type(*argument)?);
        }
        let instance = self.instantiate_function(template, arguments, span)?;
        let parameters = self.result.functions[instance as usize].parameters.clone();
        if parameters.is_empty() || !self.is_this_compatible(base_type, parameters[0].ty) {
            self.error(span, "the receiver type does not match the generic method");
            return None;
        }
        let mapped = self.match_call_arguments(&parameters[1..], &call.arguments, span)?;
        for (value, parameter) in mapped.iter().zip(parameters.iter().skip(1)) {
            let ty = self.check_expr(*value, Some(parameter.ty));
            if let Some(ty) = ty {
                self.expect_assignable(self.span_of(*value), ty, parameter.ty);
            }
        }
        self.result.call_arguments.insert(node, mapped);
        self.result.resolutions[node as usize] = Some(Resolution::Function(instance));
        self.result.functions[instance as usize].return_type
    }

    fn check_call(&mut self, node: NodeId, call: &CallExpr) -> Option<TypeId> {
        if self.in_verbatim {
            for namespace in ["Platform", "Compiler", "NOLAA"] {
                if builtin_namespace_call(self.ast, call.callee, namespace).is_some() {
                    self.error(
                        self.span_of(node),
                        "verbatim forbids compile-time evaluation (§9.8)",
                    );
                }
            }
            if builtin_size_of_type_argument(self.ast, call.callee).is_some() {
                self.error(
                    self.span_of(node),
                    "verbatim forbids compile-time evaluation (§9.8)",
                );
            }
        }
        // Compile-time API calls (Platform/Compiler/NOLAA, Type.SizeOf) are
        // no-ops at run time and must not count as heap escapes (§15.3).
        let compile_time_api = ["Platform", "Compiler", "NOLAA"]
            .iter()
            .any(|namespace| builtin_namespace_call(self.ast, call.callee, namespace).is_some())
            || builtin_size_of_type_argument(self.ast, call.callee).is_some();
        if !compile_time_api {
            for argument in &call.arguments {
                self.note_heap_escape(argument.value);
            }
        }
        // Generic method calls: `receiver.Method<TypeArgs>(...)` (§10.2).
        if builtin_size_of_type_argument(self.ast, call.callee).is_none() {
            if let NodeKind::Monomorphization(monomorphization) =
                self.node_kind(call.callee).clone()
            {
                if let NodeKind::MemberAccess(access) =
                    self.node_kind(monomorphization.base).clone()
                {
                    return self.check_generic_method_call(node, call, &monomorphization, &access);
                }
            }
        }
        // Compiler API: `Platform.Is(PlatformKind)` / `Platform.Is("name")`
        // evaluates against the compilation target (§15.3).
        if let Some(name) = builtin_namespace_call(self.ast, call.callee, "Platform") {
            if name == "Is" {
                return self.check_platform_is(node, &call.arguments);
            }
        }
        // Heap lifetime analysis controls are advisory no-ops at run time (§15.3).
        if let Some(name) = builtin_namespace_call(self.ast, call.callee, "NOLAA") {
            return self.check_nolaa_call(node, &name, &call.arguments);
        }
        // Compile-time diagnostics: `Compiler.LogNote/LogWarning/LogError/Assert`
        // (§15.3).
        if let Some(name) = builtin_compiler_function(self.ast, call.callee) {
            return self.check_compiler_call(node, name, &call.arguments);
        }
        // Compiler API: `Type.SizeOf<T>()` yields `T`'s memory size as a
        // `cardinal(8)` value (§11.8).
        if let Some(type_argument) = builtin_size_of_type_argument(self.ast, call.callee) {
            let ty = self.resolve_type(type_argument)?;
            self.result.resolutions[node as usize] = Some(Resolution::SizeOf { ty });
            if call.arguments.is_empty() {
                return Some(self.result.types.integer(false, 8));
            }
            self.error(self.span_of(node), "Type.SizeOf<T>() takes no arguments");
            return None;
        }
        // Monomorphized call or construction: `Generic<Arguments>(...)` (§10.2).
        if let NodeKind::Monomorphization(monomorphization) = self.node_kind(call.callee).clone() {
            return match self.check_monomorphization(call.callee, &monomorphization) {
                Some(MonomorphizedTarget::Type(type_id)) => {
                    self.check_construction(node, type_id, &call.arguments)
                }
                Some(MonomorphizedTarget::Function(index)) => {
                    self.resolve_overload(node, &[index], &call.arguments)
                }
                None => None,
            };
        }
        // Method call: `value.Method(arguments)` (§8.4).
        if let NodeKind::MemberAccess(access) = self.node_kind(call.callee) {
            let namespace = matches!(
                self.try_resolve_name(access.base),
                Some(Symbol::Module(_)) | Some(Symbol::Type(_))
            );
            if !namespace {
                if let Some(base_type) = self.check_expr(access.base, None) {
                    if let Some(result) =
                        self.try_method_call(node, base_type, &access.name, &call.arguments)
                    {
                        return result;
                    }
                }
            }
        }
        // Member construction: `Variant.State(value)` / `Union.Member(value)`.
        if let NodeKind::MemberAccess(access) = self.node_kind(call.callee) {
            if let Some(Symbol::Type(type_id)) = self.try_resolve_name(access.base) {
                if let Type::Named(index) = self.types_of(type_id) {
                    match self.result.type_defs[index as usize].kind {
                        TypeDefKind::Variant => {
                            return self.check_variant_construction(
                                node,
                                type_id,
                                &access.name,
                                &call.arguments,
                            );
                        }
                        TypeDefKind::Union => {
                            return self.check_union_construction(
                                node,
                                type_id,
                                &access.name,
                                &call.arguments,
                            );
                        }
                        _ => {}
                    }
                }
            }
        }
        // Static method call: `Class.Method(arguments)` (§9.15).
        if let NodeKind::MemberAccess(access) = self.node_kind(call.callee).clone() {
            if let Some(Symbol::Type(type_id)) = self.try_resolve_name(access.base) {
                if let Type::Named(index) = self.types_of(type_id) {
                    if let Some(overloads) = self.result.type_defs[index as usize]
                        .static_methods
                        .get(&access.name)
                        .cloned()
                    {
                        return self.resolve_overload(node, &overloads, &call.arguments);
                    }
                }
            }
        }
        // Construction: `Type(arguments)` (§4.6).
        if let Some(Symbol::Type(type_id)) = self.try_callee_symbol(call.callee) {
            return self.check_construction(node, type_id, &call.arguments);
        }
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

    /// Creates an accessor declared by an array extension template (§3.8.3).
    fn create_array_extension_accessor(
        &mut self,
        node: NodeId,
        declaration: &AccessorDecl,
        owner: TypeId,
    ) {
        let Some(ty) = self.resolve_type(declaration.ty) else {
            return;
        };
        let scoped =
            self.resolve_scoped_targets(&declaration.modifiers, Some(owner), self.span_of(node));
        let mut accessor = AccessorInfo {
            accessibility: declaration.modifiers.accessibility,
            scoped,
            name: declaration.name.clone(),
            ty,
            getter: None,
            setter: None,
        };
        for member in &declaration.members {
            let implementation = match (member.expression, member.body) {
                // `get = (this: &array(length) T) cardinal(4) => { ... }`:
                // the function literal's block/expression is the implementation
                // and its `this` parameter matches the hidden receiver.
                (Some(expression), None) => match self.node_kind(expression).clone() {
                    NodeKind::FunctionLiteral(literal) => {
                        Some(match self.node_kind(literal.body) {
                            NodeKind::Block(_) => FunctionBody::Block(literal.body),
                            NodeKind::VerbatimBlock(inner) => FunctionBody::Block(inner),
                            _ => FunctionBody::Expression(literal.body),
                        })
                    }
                    _ => Some(FunctionBody::Expression(expression)),
                },
                (None, Some(block)) => Some(FunctionBody::Block(block)),
                (Some(_), Some(_)) => {
                    self.error(
                        member.span,
                        "an accessor member takes either an expression or a block",
                    );
                    None
                }
                (None, None) => None,
            };
            if implementation.is_none() {
                self.error(member.span, "accessor member requires an implementation");
                continue;
            }
            let method = self.create_accessor_method(
                owner,
                &declaration.name,
                member.is_getter,
                ty,
                implementation,
                member.span,
            );
            if member.is_getter {
                accessor.getter = method;
            } else {
                accessor.setter = method;
            }
        }
        self.result
            .owner_accessors
            .entry(owner)
            .or_default()
            .push(accessor);
    }

    /// Instantiates the `extension for array(...)` templates that match
    /// `type_id` (§3.8.3).
    fn ensure_array_extensions(&mut self, type_id: TypeId) {
        let Type::Array {
            dimensions,
            element,
        } = self.result.types.get(type_id).clone()
        else {
            return;
        };
        let templates = self.result.array_extension_templates.clone();
        for template in templates {
            if self
                .result
                .array_extension_instances
                .contains_key(&(template.extension, type_id))
            {
                continue;
            }
            if dimensions.len() != template.bindings.len() {
                continue;
            }
            let mut lengths = HashMap::new();
            let mut compatible = true;
            for ((name, bound), length) in template.bindings.iter().zip(dimensions.iter()) {
                if !self.is_integer(*bound) {
                    compatible = false;
                    break;
                }
                lengths.insert(name.clone(), *length);
            }
            if !compatible {
                continue;
            }
            if !self.check_generic_constraints(
                std::slice::from_ref(&template.parameter),
                &[element],
                self.current_scope(),
                self.span_of(template.extension),
            ) {
                continue;
            }
            let mut environment = HashMap::new();
            environment.insert(template.parameter.name.clone(), element);
            self.generic_env.push(environment);
            self.array_binding_lengths.push(lengths);
            let members = match self.node_kind(template.extension).clone() {
                NodeKind::Extension(declaration) => declaration.members,
                _ => Vec::new(),
            };
            let mut created = Vec::new();
            for member in members {
                // Each instance owns a deep clone of the member AST: bodies are
                // checked per instantiation (bound length, element type), so
                // sharing the template's nodes would alias their resolutions.
                let cloned = self.clone_declaration(member);
                match self.node_kind(cloned).clone() {
                    NodeKind::Fixed(declaration) => {
                        if let Some(index) = self.create_method(cloned, &declaration, type_id) {
                            created.push(index);
                        }
                    }
                    NodeKind::Accessor(declaration) => {
                        self.create_array_extension_accessor(cloned, &declaration, type_id);
                    }
                    NodeKind::Operator(_) => {
                        self.resolve_operator_declaration(cloned, Some(type_id));
                    }
                    _ => {}
                }
            }
            self.array_binding_lengths.pop();
            self.generic_env.pop();
            self.result
                .array_extension_instances
                .insert((template.extension, type_id), created);
        }
    }

    /// Collects methods named `name` from `type_id` and its base classes.
    fn collect_methods(&self, type_id: TypeId, name: &str) -> Vec<u32> {
        if !matches!(self.result.types.get(type_id), Type::Named(_)) {
            // Array extension methods (§3.8.3) are registered per owner type.
            let found: Vec<u32> = self
                .result
                .owner_methods
                .get(&type_id)
                .map(|methods| {
                    methods
                        .iter()
                        .copied()
                        .filter(|method| self.result.functions[*method as usize].name == name)
                        .collect()
                })
                .unwrap_or_default();
            return found;
        }
        let mut methods = Vec::new();
        let mut current = Some(type_id);
        let mut depth = 0;
        while let Some(ty) = current {
            depth += 1;
            if depth > 32 {
                break;
            }
            let Type::Named(index) = self.result.types.get(ty) else {
                break;
            };
            let definition = &self.result.type_defs[*index as usize];
            if let Some(list) = definition.methods.get(name) {
                // Methods of the most derived class hide base declarations of
                // the same name (overrides replace them, §9.13).
                methods.extend(list.iter().copied());
                break;
            }
            current = definition.base;
        }
        methods
    }

    fn is_this_compatible(&self, actual: TypeId, expected: TypeId) -> bool {
        if actual == expected {
            return true;
        }
        let mut current = Some(actual);
        let mut depth = 0;
        while let Some(ty) = current {
            depth += 1;
            if depth > 32 {
                break;
            }
            if ty == expected {
                return true;
            }
            let Type::Named(index) = self.result.types.get(ty) else {
                return false;
            };
            current = self.result.type_defs[*index as usize].base;
        }
        false
    }

    fn try_method_call(
        &mut self,
        node: NodeId,
        base_type: TypeId,
        name: &str,
        arguments: &[Argument],
    ) -> Option<Option<TypeId>> {
        if matches!(self.result.types.get(base_type), Type::Array { .. }) {
            self.ensure_array_extensions(base_type);
        }
        let all_candidates = self.collect_methods(base_type, name);
        if all_candidates.is_empty() {
            return None;
        }
        let all_candidates: Vec<u32> = all_candidates
            .into_iter()
            .filter(|candidate| {
                self.result.functions[*candidate as usize]
                    .generic_parameters
                    .is_empty()
            })
            .collect();
        if all_candidates.is_empty() {
            self.error(
                self.span_of(node),
                format!("a generic method requires type arguments: '{name}'"),
            );
            return Some(None);
        }
        let candidates: Vec<u32> = all_candidates
            .iter()
            .copied()
            .filter(|candidate| {
                let function = &self.result.functions[*candidate as usize];
                let declaring = function.method_of.unwrap_or(base_type);
                self.member_accessible(function.accessibility, declaring)
                    && self.scope_allows(declaring, &function.scoped)
            })
            .collect();
        if candidates.is_empty() {
            self.error(
                self.span_of(node),
                format!("method '{name}' is not accessible here"),
            );
            return Some(None);
        }
        let span = self.span_of(node);
        // Methods with '#' parameters are templates: specialize them per
        // constant argument tuple (§11.5).
        let mut template_candidates = Vec::with_capacity(candidates.len());
        for &candidate in &candidates {
            let function = &self.result.functions[candidate as usize];
            if function.is_template() && function.generic_parameters.is_empty() {
                let parameters = function.parameters.clone();
                self.suppress_errors += 1;
                let mapped = self.match_call_arguments(&parameters[1..], arguments, span);
                let mut constants = Vec::new();
                let mut ok = mapped.is_some();
                if let Some(mapped) = &mapped {
                    for (parameter, value) in parameters.iter().skip(1).zip(mapped.iter()) {
                        if !parameter.compile_time {
                            continue;
                        }
                        if !self.is_compile_time_constant(*value) {
                            ok = false;
                            break;
                        }
                        match self.eval_const_int(*value) {
                            Some(constant) => constants.push(constant),
                            None => {
                                ok = false;
                                break;
                            }
                        }
                    }
                }
                self.suppress_errors -= 1;
                if ok {
                    if let Some(instance) =
                        self.instantiate_constant_function(candidate, &constants, span)
                    {
                        template_candidates.push(instance);
                    }
                }
            } else {
                template_candidates.push(candidate);
            }
        }
        let candidates = template_candidates;
        let mut exact = Vec::new();
        for &candidate in &candidates {
            let parameters = self.result.functions[candidate as usize].parameters.clone();
            if !self.is_this_compatible(base_type, parameters[0].ty) {
                continue;
            }
            self.suppress_errors += 1;
            let mapped = self.match_call_arguments(&parameters[1..], arguments, span);
            let matches = mapped.is_some_and(|mapped| {
                mapped
                    .iter()
                    .zip(parameters.iter().skip(1))
                    .all(|(value, parameter)| {
                        self.check_expr(*value, Some(parameter.ty)) == Some(parameter.ty)
                    })
            });
            self.suppress_errors -= 1;
            if matches {
                exact.push(candidate);
            }
        }
        let selected = if exact.len() == 1 {
            exact[0]
        } else if exact.is_empty() {
            if candidates.len() == 1 {
                let parameters = self.result.functions[candidates[0] as usize]
                    .parameters
                    .clone();
                self.match_call_arguments(&parameters[1..], arguments, span);
            }
            self.error(span, "no method overload matches the argument types");
            return Some(None);
        } else {
            self.error(span, "ambiguous method call: several overloads match");
            return Some(None);
        };
        let parameters = self.result.functions[selected as usize].parameters.clone();
        let Some(mapped) = self.match_call_arguments(&parameters[1..], arguments, span) else {
            return Some(None);
        };
        for (value, parameter) in mapped.iter().zip(parameters.iter().skip(1)) {
            let ty = self.check_expr(*value, Some(parameter.ty));
            if let Some(ty) = ty {
                self.expect_assignable(self.span_of(*value), ty, parameter.ty);
            }
        }
        self.result.call_arguments.insert(node, mapped);
        self.result.resolutions[node as usize] = Some(Resolution::Function(selected));
        Some(self.result.functions[selected as usize].return_type)
    }

    fn check_construction(
        &mut self,
        node: NodeId,
        type_id: TypeId,
        arguments: &[Argument],
    ) -> Option<TypeId> {
        let Type::Named(index) = self.result.types.get(type_id).clone() else {
            self.error(self.span_of(node), "only named types can be constructed");
            return None;
        };
        match self.result.type_defs[index as usize].kind {
            TypeDefKind::Structure | TypeDefKind::Class => {}
            TypeDefKind::Enumeration => {
                self.error(
                    self.span_of(node),
                    "enumeration values are written as members, not constructed",
                );
                return None;
            }
            TypeDefKind::Variant | TypeDefKind::Union => {
                self.error(
                    self.span_of(node),
                    "variants and unions are constructed through a member construction expression",
                );
                return None;
            }
            _ => {
                self.unsupported(self.span_of(node), "construction of this type");
                return None;
            }
        }
        if self.result.type_defs[index as usize].is_static {
            let class_name = self.result.type_defs[index as usize].qualified_name.clone();
            self.error(
                self.span_of(node),
                format!("cannot instantiate the static class '{class_name}'"),
            );
            return None;
        }
        if self.result.type_defs[index as usize].is_abstract {
            let class_name = self.result.type_defs[index as usize].qualified_name.clone();
            self.error(
                self.span_of(node),
                format!("cannot instantiate the abstract class '{class_name}'"),
            );
            return None;
        }
        let all_constructors = self.collect_methods(type_id, "Construct");
        let candidates: Vec<u32> = all_constructors
            .iter()
            .copied()
            .filter(|candidate| {
                let function = &self.result.functions[*candidate as usize];
                let declaring = function.method_of.unwrap_or(type_id);
                self.member_accessible(function.accessibility, declaring)
                    && self.scope_allows(declaring, &function.scoped)
            })
            .collect();
        if !all_constructors.is_empty() && candidates.is_empty() {
            self.error(
                self.span_of(node),
                "this constructor is not accessible here",
            );
            return None;
        }
        if candidates.is_empty() {
            if arguments.is_empty() {
                self.result.resolutions[node as usize] = Some(Resolution::Construct {
                    type_id,
                    method: None,
                });
                return Some(type_id);
            }
            // The Built-In Library exception classes declare
            // `Construct(this, innerException: Optional<Exception> = Null)` (§15.2).
            let class_name = self.result.type_defs[index as usize].name.clone();
            let field_type = self.result.type_defs[index as usize]
                .fields
                .first()
                .map(|field| field.ty);
            if BUILTIN_EXCEPTION_NAMES.contains(&class_name.as_str()) {
                if let Some(field_type) = field_type {
                    if arguments.len() != 1 {
                        self.error(
                            self.span_of(node),
                            "this exception constructor takes one InnerException argument",
                        );
                        return None;
                    }
                    let argument = &arguments[0];
                    if let Some(name) = &argument.name {
                        if name != "innerException" {
                            self.error(argument.span, "unknown parameter name");
                            return None;
                        }
                    }
                    let ty = self.check_expr(argument.value, Some(field_type));
                    if let Some(ty) = ty {
                        self.expect_assignable(argument.span, ty, field_type);
                    }
                    self.result
                        .call_arguments
                        .insert(node, vec![argument.value]);
                    self.result.resolutions[node as usize] = Some(Resolution::Construct {
                        type_id,
                        method: None,
                    });
                    return Some(type_id);
                }
            }
            self.error(
                self.span_of(node),
                "this type declares no constructor that accepts arguments",
            );
            return None;
        }
        let span = self.span_of(node);
        let mut exact = Vec::new();
        for &candidate in &candidates {
            let parameters = self.result.functions[candidate as usize].parameters.clone();
            if !self.is_this_compatible(type_id, parameters[0].ty) {
                continue;
            }
            self.suppress_errors += 1;
            let mapped = self.match_call_arguments(&parameters[1..], arguments, span);
            let matches = mapped.is_some_and(|mapped| {
                mapped
                    .iter()
                    .zip(parameters.iter().skip(1))
                    .all(|(value, parameter)| {
                        self.check_expr(*value, Some(parameter.ty)) == Some(parameter.ty)
                    })
            });
            self.suppress_errors -= 1;
            if matches {
                exact.push(candidate);
            }
        }
        if exact.is_empty() {
            // Second phase: implicit and interface conversions.
            for &candidate in &candidates {
                let parameters = self.result.functions[candidate as usize].parameters.clone();
                if !self.is_this_compatible(type_id, parameters[0].ty) {
                    continue;
                }
                self.suppress_errors += 1;
                let mapped = self.match_call_arguments(&parameters[1..], arguments, span);
                let matches = mapped.is_some_and(|mapped| {
                    mapped.iter().zip(parameters.iter().skip(1)).all(
                        |(value, parameter)| match self.check_expr(*value, Some(parameter.ty)) {
                            Some(ty) => self.is_assignable(ty, parameter.ty),
                            None => false,
                        },
                    )
                });
                self.suppress_errors -= 1;
                if matches {
                    exact.push(candidate);
                }
            }
        }
        if exact.len() != 1 {
            if candidates.len() == 1 {
                let parameters = self.result.functions[candidates[0] as usize]
                    .parameters
                    .clone();
                self.match_call_arguments(&parameters[1..], arguments, span);
            }
            self.error(span, "no constructor overload matches the argument types");
            return None;
        }
        let selected = exact[0];
        let parameters = self.result.functions[selected as usize].parameters.clone();
        let mapped = self.match_call_arguments(&parameters[1..], arguments, span)?;
        for (value, parameter) in mapped.iter().zip(parameters.iter().skip(1)) {
            let ty = self.check_expr(*value, Some(parameter.ty));
            if let Some(ty) = ty {
                self.expect_assignable(self.span_of(*value), ty, parameter.ty);
            }
        }
        self.result.call_arguments.insert(node, mapped);
        self.result.resolutions[node as usize] = Some(Resolution::Construct {
            type_id,
            method: Some(selected),
        });
        Some(type_id)
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

    /// Maps call arguments to parameters by position and name and fills omitted
    /// trailing parameters with their defaults (§4.5, §8.2). `parameters` must
    /// not include a receiver.
    fn match_call_arguments(
        &mut self,
        parameters: &[ParameterInfo],
        arguments: &[Argument],
        fallback_span: Span,
    ) -> Option<Vec<NodeId>> {
        let mut mapped: Vec<Option<NodeId>> = vec![None; parameters.len()];
        let mut next_positional = 0usize;
        for argument in arguments {
            match &argument.name {
                Some(name) => {
                    let position = parameters
                        .iter()
                        .position(|parameter| &parameter.name == name);
                    let Some(position) = position else {
                        if self.suppress_errors == 0 {
                            self.error(argument.span, format!("unknown parameter name '{name}'"));
                        }
                        return None;
                    };
                    if mapped[position].is_some() {
                        if self.suppress_errors == 0 {
                            self.error(
                                argument.span,
                                format!("parameter '{name}' is given more than once"),
                            );
                        }
                        return None;
                    }
                    mapped[position] = Some(argument.value);
                }
                None => {
                    while next_positional < parameters.len() && mapped[next_positional].is_some() {
                        next_positional += 1;
                    }
                    if next_positional >= parameters.len() {
                        if self.suppress_errors == 0 {
                            self.error(argument.span, "too many arguments");
                        }
                        return None;
                    }
                    mapped[next_positional] = Some(argument.value);
                    next_positional += 1;
                }
            }
        }
        let mut ordered = Vec::with_capacity(parameters.len());
        for (position, slot) in mapped.into_iter().enumerate() {
            match slot {
                Some(value) => ordered.push(value),
                None => match parameters[position].default {
                    Some(default) => ordered.push(default),
                    None => {
                        if self.suppress_errors == 0 {
                            self.error(
                                fallback_span,
                                format!(
                                    "missing argument for parameter '{}'",
                                    parameters[position].name
                                ),
                            );
                        }
                        return None;
                    }
                },
            }
        }
        Some(ordered)
    }

    fn resolve_overload(
        &mut self,
        node: NodeId,
        overloads: &[u32],
        arguments: &[Argument],
    ) -> Option<TypeId> {
        let span = self.span_of(node);
        // Functions with `#` parameters are templates: specialize them per
        // constant argument tuple (§11.5).
        let mut candidates = Vec::with_capacity(overloads.len());
        let mut saw_constant_template = false;
        for &candidate in overloads {
            let function = &self.result.functions[candidate as usize];
            if function.is_template() && function.generic_parameters.is_empty() {
                saw_constant_template = true;
                let parameters = function.parameters.clone();
                self.suppress_errors += 1;
                let mapped = self.match_call_arguments(&parameters, arguments, span);
                let mut constants = Vec::new();
                let mut ok = mapped.is_some();
                if let Some(mapped) = &mapped {
                    for (parameter, value) in parameters.iter().zip(mapped.iter()) {
                        if !parameter.compile_time {
                            continue;
                        }
                        if !self.is_compile_time_constant(*value) {
                            ok = false;
                            break;
                        }
                        match self.eval_const_int(*value) {
                            Some(constant) => constants.push(constant),
                            None => {
                                ok = false;
                                break;
                            }
                        }
                    }
                }
                self.suppress_errors -= 1;
                if ok {
                    if let Some(instance) =
                        self.instantiate_constant_function(candidate, &constants, span)
                    {
                        candidates.push(instance);
                    }
                }
            } else {
                candidates.push(candidate);
            }
        }
        if candidates.is_empty() && saw_constant_template && self.suppress_errors == 0 {
            self.error(
                span,
                "the arguments of a compile-time ('#') parameter must be compile-time constants",
            );
            return None;
        }
        let mut exact = Vec::new();
        for &candidate in &candidates {
            let parameters = self.result.functions[candidate as usize].parameters.clone();
            self.suppress_errors += 1;
            let mapped = self.match_call_arguments(&parameters, arguments, span);
            let matches = mapped.is_some_and(|mapped| {
                mapped
                    .iter()
                    .zip(parameters.iter())
                    .all(|(value, parameter)| {
                        self.check_expr(*value, Some(parameter.ty)) == Some(parameter.ty)
                    })
            });
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
                for &candidate in &candidates {
                    let parameters = self.result.functions[candidate as usize].parameters.clone();
                    self.suppress_errors += 1;
                    let mapped = self.match_call_arguments(&parameters, arguments, span);
                    let matches = mapped.is_some_and(|mapped| {
                        mapped
                            .iter()
                            .zip(parameters.iter())
                            .all(|(value, parameter)| {
                                match self.check_expr(*value, Some(parameter.ty)) {
                                    Some(ty) => self.is_assignable(ty, parameter.ty),
                                    None => false,
                                }
                            })
                    });
                    self.suppress_errors -= 1;
                    if matches {
                        convertible.push(candidate);
                    }
                }
                if convertible.len() == 1 {
                    convertible[0]
                } else if convertible.is_empty() {
                    // Re-run the mapping without suppression on the single
                    // overload so malformed argument lists are reported precisely.
                    if candidates.len() == 1 {
                        let parameters = self.result.functions[candidates[0] as usize]
                            .parameters
                            .clone();
                        self.match_call_arguments(&parameters, arguments, span);
                    }
                    self.error(span, "no overload matches the argument types");
                    return None;
                } else {
                    self.error(
                        span,
                        "ambiguous call: several overloads match the argument types",
                    );
                    return None;
                }
            }
            _ => {
                self.error(
                    span,
                    "ambiguous call: several overloads match the argument types",
                );
                return None;
            }
        };

        let parameters = self.result.functions[selected as usize].parameters.clone();
        let mapped = self.match_call_arguments(&parameters, arguments, span)?;
        for (value, parameter) in mapped.iter().zip(parameters.iter()) {
            let ty = self.check_expr(*value, Some(parameter.ty));
            if let Some(ty) = ty {
                self.expect_assignable(self.span_of(*value), ty, parameter.ty);
            }
        }
        self.result.call_arguments.insert(node, mapped);
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

        let mut element_type = expected_element.or(self.expected_array_element);
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
                    let element_type = match element_type {
                        Some(element_type) => element_type,
                        None => {
                            // Infer from the range start expression's type.
                            let ty = self.check_expr(range.start, None)?;
                            element_type = Some(ty);
                            ty
                        }
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

/// The resolved target of an `is` expression or pattern.
enum IsTargetResolved {
    /// A declared type (class checks).
    Type(TypeId),
    /// A variant state check.
    VariantState {
        /// Variant type.
        type_id: TypeId,
        /// State index.
        state: u32,
    },
}

/// Detects the compiler API call `Type.SizeOf<T>()` (§11.8) and returns its type
/// argument. `Type` is a built-in compiler namespace, not a declared module.
fn builtin_size_of_type_argument(ast: &Ast, callee: NodeId) -> Option<NodeId> {
    let NodeKind::Monomorphization(monomorphization) = ast.node(callee).kind.clone() else {
        return None;
    };
    let NodeKind::MemberAccess(access) = ast.node(monomorphization.base).kind.clone() else {
        return None;
    };
    if access.name != "SizeOf" {
        return None;
    }
    let NodeKind::Name(namespace) = ast.node(access.base).kind.clone() else {
        return None;
    };
    if namespace.name != "Type" {
        return None;
    }
    match monomorphization.arguments.as_slice() {
        [Some(argument)] => Some(*argument),
        _ => None,
    }
}

/// Detects a `Compiler.<name>(...)` call from the compile-time library (§15.3).
fn builtin_compiler_function(ast: &Ast, callee: NodeId) -> Option<String> {
    let NodeKind::MemberAccess(access) = ast.node(callee).kind.clone() else {
        return None;
    };
    let NodeKind::Name(namespace) = ast.node(access.base).kind.clone() else {
        return None;
    };
    if namespace.name != "Compiler" {
        return None;
    }
    Some(access.name)
}

/// Detects a call `Namespace.<name>(...)` on a built-in compiler namespace
/// (§15.3).
fn builtin_namespace_call(ast: &Ast, callee: NodeId, namespace: &str) -> Option<String> {
    let NodeKind::MemberAccess(access) = ast.node(callee).kind.clone() else {
        return None;
    };
    let NodeKind::Name(base) = ast.node(access.base).kind.clone() else {
        return None;
    };
    if base.name != namespace {
        return None;
    }
    Some(access.name)
}
