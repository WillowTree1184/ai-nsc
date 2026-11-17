//! Generic declarations and monomorphization (§10).
//!
//! A generic declaration is a *template*: semantic analysis records it (name,
//! generic parameters, constraints) but never resolves its members directly,
//! because member types mention the parameters. Every use with concrete type
//! arguments — a type position (`local b: Box<integer(4)>`), a construction
//! (`Box<integer(4)>(5)`), a call (`Echo<integer(4)>(42)`) or a member
//! construction — *instantiates* the template (§10.2):
//!
//! 1. the argument list is validated against the parameter list and against the
//!    parameters' constraints (§10.1),
//! 2. the declaration's AST subtree is deep-cloned so that the instance gets its
//!    own nodes (types, resolutions and locals are per-node tables),
//! 3. the clone is resolved with a substitution environment (`T -> integer(4)`)
//!    in place, which gives the instance its fields, methods, accessors,
//!    operators and layouts,
//! 4. the instance is cached, so a second use of the same argument list reuses
//!    it and each argument list is compiled exactly once (monomorphization).
//!
//! Instance names carry their arguments (`Box<integer(4)>`) and instance methods
//! are mangled from that name and their parameter spellings, so several
//! instantiations coexist in one program (§13.2).

use super::types::{mangle_function, TypeId};
use super::*;
use crate::ast::*;

/// What a monomorphization expression resolved to (§10.2).
#[derive(Clone, Copy, Debug)]
pub(crate) enum MonomorphizedTarget {
    /// A generic type instantiated with concrete arguments.
    Type(TypeId),
    /// A generic function instantiated with concrete arguments.
    Function(u32),
}

impl<'a> Sema<'a> {
    // ------------------------------------------------------------------
    // Declarations
    // ------------------------------------------------------------------

    /// Whether the type declaration at `node` carries generic parameters and is
    /// therefore a template (§10.1).
    pub(crate) fn is_generic_type_declaration(&self, node: NodeId) -> bool {
        match self.node_kind(node) {
            NodeKind::Class(declaration)
            | NodeKind::Structure(declaration)
            | NodeKind::Interface(declaration) => !declaration.generics.is_empty(),
            NodeKind::Variant(declaration) => !declaration.generics.is_empty(),
            NodeKind::Union(declaration) => !declaration.generics.is_empty(),
            _ => false,
        }
    }

    /// Records a generic `fixed`/`linkable` function as a template.
    ///
    /// The signature and body are *not* resolved here: they mention generic
    /// parameters and are resolved per instantiation instead.
    pub(crate) fn declare_generic_function(&mut self, node: NodeId, declaration: &FixedDecl) {
        let Some(ty_node) = declaration.ty else {
            self.error(
                self.span_of(node),
                "a generic declaration requires a function type",
            );
            return;
        };
        if !matches!(self.node_kind(ty_node), NodeKind::FunctionType(_)) {
            self.error(
                self.span_of(node),
                "only functions may declare generic parameters",
            );
            return;
        }
        if !matches!(declaration.init, Some(Initializer::Block(_))) {
            self.error(self.span_of(node), "a generic function requires a body");
            return;
        }
        self.push_function_template(
            node,
            declaration.name.clone(),
            None,
            declaration.generics.as_slice(),
        );
    }

    /// Records a generic `linkable` function as a template.
    pub(crate) fn declare_generic_linkable(&mut self, node: NodeId, declaration: &LinkableDecl) {
        let Some(ty_node) = declaration.ty else {
            self.error(
                self.span_of(node),
                "a generic declaration requires a function type",
            );
            return;
        };
        if !matches!(self.node_kind(ty_node), NodeKind::FunctionType(_)) {
            self.error(
                self.span_of(node),
                "only functions may declare generic parameters",
            );
            return;
        }
        if !matches!(declaration.init, Some(Initializer::Block(_))) {
            self.error(self.span_of(node), "a generic function requires a body");
            return;
        }
        let exposed = declaration
            .symbol
            .clone()
            .unwrap_or_else(|| declaration.name.clone());
        self.push_function_template(
            node,
            declaration.name.clone(),
            Some(exposed),
            declaration.generics.as_slice(),
        );
    }

    fn push_function_template(
        &mut self,
        node: NodeId,
        name: String,
        exposed_symbol: Option<String>,
        generics: &[GenericParameter],
    ) {
        let parameters: Vec<GenericParameterInfo> = generics
            .iter()
            .map(GenericParameterInfo::from_ast)
            .collect();
        // A declaration that cannot be instantiated by any argument list is
        // almost always a mistake; report unknown constraint names eagerly.
        for parameter in &parameters {
            for constraint in &parameter.constraints {
                if let GenericConstraint::Named(constraint_name) = constraint {
                    if self.lookup(constraint_name).is_none() {
                        self.error(
                            parameter.span,
                            format!("unknown constraint type '{constraint_name}'"),
                        );
                    }
                }
            }
        }
        let index = self.result.functions.len() as u32;
        let span = self.span_of(node);
        self.result.functions.push(FunctionInfo {
            name: name.clone(),
            scope: self.scope_path.clone(),
            node,
            body: None,
            parameters: Vec::new(),
            method_of: None,
            is_virtual: false,
            is_override: false,
            is_sealed: false,
            is_abstract: false,
            return_type: None,
            mangled: String::new(),
            exposed_symbol,
            generic_parameters: parameters,
            generic_arguments: Vec::new(),
            template: None,
            decl_scope: self.current_scope(),
            generic_env: HashMap::new(),
            locals: Vec::new(),
            span,
        });
        self.declare_generic_functions(name, index);
    }

    // ------------------------------------------------------------------
    // Environment and cloning
    // ------------------------------------------------------------------

    /// Looks a name up in the active generic substitution environments (§10.1).
    ///
    /// Generic parameters live in the declaration's scope and shadow outer
    /// names, so the innermost environment is consulted first.
    pub(crate) fn lookup_generic_parameter(&self, name: &str) -> Option<TypeId> {
        for layer in self.generic_env.iter().rev() {
            if let Some(ty) = layer.get(name) {
                return Some(*ty);
            }
        }
        None
    }

    /// Deep-clones the declaration subtree at `node` and grows the per-node
    /// result tables so the new nodes can be annotated.
    pub(crate) fn clone_declaration(&mut self, node: NodeId) -> NodeId {
        let cloned = self.ast.clone_subtree(node);
        let count = self.ast.nodes.len();
        if self.result.node_types.len() < count {
            self.result.node_types.resize(count, None);
        }
        if self.result.resolutions.len() < count {
            self.result.resolutions.resize(count, None);
        }
        cloned
    }

    /// Resolves a generic argument list (§10.2). An omitted argument denotes the
    /// empty type.
    pub(crate) fn resolve_type_arguments(
        &mut self,
        arguments: &[Option<NodeId>],
    ) -> Option<Vec<TypeId>> {
        let mut resolved = Vec::with_capacity(arguments.len());
        for argument in arguments {
            match argument {
                Some(node) => resolved.push(self.resolve_type(*node)?),
                None => resolved.push(TypeId::VOID),
            }
        }
        Some(resolved)
    }

    // ------------------------------------------------------------------
    // Constraints (§10.1)
    // ------------------------------------------------------------------

    /// Checks the constraints of `parameters` against `arguments`.
    ///
    /// Reports one diagnostic per violated constraint and returns whether all
    /// constraints hold.
    fn check_generic_constraints(
        &mut self,
        parameters: &[GenericParameterInfo],
        arguments: &[TypeId],
        decl_scope: usize,
        span: Span,
    ) -> bool {
        let mut satisfied = true;
        for (parameter, argument) in parameters.iter().zip(arguments.iter()) {
            for constraint in &parameter.constraints {
                if self.type_satisfies_constraint(*argument, constraint, decl_scope) {
                    continue;
                }
                satisfied = false;
                let argument_spelling = self.spelling(*argument);
                let constraint_spelling = match constraint {
                    GenericConstraint::Kind(kind) => constraint_kind_keyword(*kind).to_string(),
                    GenericConstraint::Named(name) => name.clone(),
                };
                self.error(
                    span,
                    format!(
                        "type argument '{argument_spelling}' does not satisfy constraint \
                         '{constraint_spelling}' of generic parameter '{}'",
                        parameter.name
                    ),
                );
            }
        }
        satisfied
    }

    /// Whether `argument` satisfies one constraint.
    fn type_satisfies_constraint(
        &self,
        argument: TypeId,
        constraint: &GenericConstraint,
        decl_scope: usize,
    ) -> bool {
        match constraint {
            GenericConstraint::Kind(kind) => self.type_matches_kind(argument, *kind),
            GenericConstraint::Named(name) => {
                // Unknown constraint names are reported when the template is
                // declared; treat them as satisfied to avoid cascades.
                let Some(Symbol::Type(expected)) = self.lookup_in(decl_scope, name).cloned() else {
                    return true;
                };
                self.is_assignable(argument, expected)
            }
        }
    }

    fn type_matches_kind(&self, argument: TypeId, kind: GenericConstraintKind) -> bool {
        match kind {
            GenericConstraintKind::Basic => matches!(
                self.result.types.get(argument),
                Type::Boolean | Type::Integer { .. } | Type::Float { .. }
            ),
            _ => match self.result.types.get(argument) {
                Type::Named(index) => {
                    type_kind_matches(self.result.type_defs[*index as usize].kind, kind)
                }
                _ => false,
            },
        }
    }

    // ------------------------------------------------------------------
    // Type instantiation (§10.1/§10.2)
    // ------------------------------------------------------------------

    /// Instantiates the generic type declared at `template_node` with
    /// `arguments`, reusing a cached instance when possible.
    pub(crate) fn instantiate_type(
        &mut self,
        template_node: NodeId,
        arguments: Vec<TypeId>,
        span: Span,
    ) -> Option<TypeId> {
        let Some(template_index) = self.result.type_defs.iter().position(|definition| {
            definition.node == template_node && definition.template.is_none()
        }) else {
            self.error(span, "internal error: generic type declaration not found");
            return None;
        };
        let template_index = template_index as TypeDefId;
        let (parameters, kind, name, template_name, decl_scope) = {
            let definition = &self.result.type_defs[template_index as usize];
            (
                definition.generic_parameters.clone(),
                definition.kind,
                definition.name.clone(),
                definition.qualified_name.clone(),
                definition.decl_scope,
            )
        };
        if parameters.is_empty() {
            self.error(
                span,
                format!("'{template_name}' does not declare generic parameters"),
            );
            return None;
        }
        if parameters.len() != arguments.len() {
            self.error(
                span,
                format!(
                    "generic type '{template_name}' expects {} type argument(s), found {}",
                    parameters.len(),
                    arguments.len()
                ),
            );
            return None;
        }
        let key = (template_node, arguments.clone());
        if let Some(existing) = self.type_instantiations.get(&key) {
            return Some(*existing);
        }
        if !self.check_generic_constraints(&parameters, &arguments, decl_scope, span) {
            return None;
        }

        let spelled: Vec<String> = arguments
            .iter()
            .map(|argument| self.spelling(*argument))
            .collect();
        let qualified_name = format!("{template_name}<{}>", spelled.join(";"));
        let instance_index = self.result.type_defs.len() as TypeDefId;
        self.result.type_defs.push(TypeDef {
            name: name.clone(),
            qualified_name,
            kind,
            node: template_node,
            generic_parameters: Vec::new(),
            generic_arguments: arguments.clone(),
            template: Some(template_index),
            decl_scope,
            fields: Vec::new(),
            field_initializers: Vec::new(),
            methods: HashMap::new(),
            method_list: Vec::new(),
            accessors: Vec::new(),
            operators: Vec::new(),
            interfaces: Vec::new(),
            interface_dispatch: Vec::new(),
            sealed: false,
            is_abstract: false,
            vtable: Vec::new(),
            vtable_ready: false,
            base: None,
            variant_states: Vec::new(),
            union_members: Vec::new(),
            tag_type: None,
            payload_offset: 0,
            payload_size: 0,
            instance_size: 0,
            instance_align: 1,
            enum_members: Vec::new(),
            underlying: None,
            size: 0,
            align: 1,
        });
        let instance_type = self.result.types.intern(Type::Named(instance_index));
        self.type_instantiations.insert(key, instance_type);
        self.instances_by_template
            .entry(template_node)
            .or_default()
            .push(arguments.clone());

        // Resolve a clone of the declaration with the substitution in place. The
        // instance is registered before its members are resolved so that
        // recursive uses of the same instantiation terminate.
        let cloned = self.clone_declaration(template_node);
        self.result.type_defs[instance_index as usize].node = cloned;
        let environment: HashMap<String, TypeId> = parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .zip(arguments.iter().copied())
            .collect();
        self.generic_env.push(environment);
        match self.node_kind(cloned) {
            NodeKind::Structure(_) => {
                self.resolve_structure_like(cloned, name, TypeDefKind::Structure);
            }
            NodeKind::Class(_) => {
                self.resolve_structure_like(cloned, name, TypeDefKind::Class);
            }
            NodeKind::Interface(_) => self.resolve_interface(cloned),
            NodeKind::Variant(_) => self.resolve_variant(cloned),
            NodeKind::Union(_) => self.resolve_union(cloned),
            _ => {}
        }
        self.generic_env.pop();

        self.apply_generic_extensions(template_node, instance_type, &arguments);
        Some(instance_type)
    }

    // ------------------------------------------------------------------
    // Function instantiation (§10.1/§10.2)
    // ------------------------------------------------------------------

    /// Instantiates the generic function template `template_index` with
    /// `arguments`, reusing a cached instance when possible.
    pub(crate) fn instantiate_function(
        &mut self,
        template_index: u32,
        arguments: Vec<TypeId>,
        span: Span,
    ) -> Option<u32> {
        let (parameters, node, scope, decl_scope, name, exposed_symbol) = {
            let template = &self.result.functions[template_index as usize];
            (
                template.generic_parameters.clone(),
                template.node,
                template.scope.clone(),
                template.decl_scope,
                template.name.clone(),
                template.exposed_symbol.clone(),
            )
        };
        if parameters.len() != arguments.len() {
            self.error(
                span,
                format!(
                    "generic function '{name}' expects {} type argument(s), found {}",
                    parameters.len(),
                    arguments.len()
                ),
            );
            return None;
        }
        let key = (node, arguments.clone());
        if let Some(existing) = self.function_instantiations.get(&key) {
            return Some(*existing);
        }
        if !self.check_generic_constraints(&parameters, &arguments, decl_scope, span) {
            return None;
        }

        let cloned = self.clone_declaration(node);
        let NodeKind::Fixed(declaration) = self.node_kind(cloned).clone() else {
            self.error(
                span,
                "internal error: generic function declaration not found",
            );
            return None;
        };
        let Some(ty_node) = declaration.ty else {
            self.error(self.span_of(cloned), "a generic function requires a type");
            return None;
        };
        let NodeKind::FunctionType(function) = self.node_kind(ty_node).clone() else {
            self.error(
                self.span_of(cloned),
                "a generic function requires a function type",
            );
            return None;
        };
        let Some(Initializer::Block(body)) = declaration.init else {
            self.error(self.span_of(cloned), "a generic function requires a body");
            return None;
        };
        let environment: HashMap<String, TypeId> = parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .zip(arguments.iter().copied())
            .collect();
        self.generic_env.push(environment.clone());
        let resolved = self.resolve_parameters(&function).map(|parameters| {
            let return_type = function.return_type.and_then(|ty| self.resolve_type(ty));
            (parameters, return_type)
        });
        self.generic_env.pop();
        let (resolved_parameters, return_type) = resolved?;
        let spellings: Vec<String> = resolved_parameters
            .iter()
            .map(|parameter| self.spelling(parameter.ty))
            .collect();
        let mangled = exposed_symbol
            .clone()
            .unwrap_or_else(|| mangle_function(&scope, &name, &spellings));
        let index = self.result.functions.len() as u32;
        self.result.functions.push(FunctionInfo {
            name: name.clone(),
            scope,
            node: cloned,
            body: Some(FunctionBody::Block(body)),
            parameters: resolved_parameters,
            method_of: None,
            is_virtual: false,
            is_override: false,
            is_sealed: false,
            is_abstract: false,
            return_type,
            mangled,
            exposed_symbol,
            generic_parameters: Vec::new(),
            generic_arguments: arguments.clone(),
            template: Some(template_index),
            decl_scope,
            generic_env: environment,
            locals: Vec::new(),
            span: self.span_of(cloned),
        });
        self.function_instantiations.insert(key, index);
        Some(index)
    }

    // ------------------------------------------------------------------
    // Module-level generic extensions (§3.8.3)
    // ------------------------------------------------------------------

    /// Records an `extension for Generic<T>` declaration. Such an extension is
    /// instantiated together with the targeted type, and only for instances that
    /// satisfy the extension's own constraints.
    pub(crate) fn register_generic_extension(&mut self, node: NodeId, target: NodeId) {
        let NodeKind::ExtensionTarget(ExtensionTarget::Generic { name, generics }) =
            self.node_kind(target).clone()
        else {
            return;
        };
        let parameters: Vec<GenericParameterInfo> = generics
            .iter()
            .map(GenericParameterInfo::from_ast)
            .collect();
        let Some(Symbol::Type(type_id)) = self.lookup(&name).cloned() else {
            self.error(
                self.span_of(target),
                format!("unknown generic target '{name}'"),
            );
            return;
        };
        let Type::Named(index) = self.types_of(type_id) else {
            self.error(
                self.span_of(target),
                "an extension target must be a declared type",
            );
            return;
        };
        if self.result.type_defs[index as usize]
            .generic_parameters
            .is_empty()
        {
            self.error(
                self.span_of(target),
                format!("'{name}' does not declare generic parameters"),
            );
            return;
        }
        let template_node = self.result.type_defs[index as usize].node;
        let extension = PendingGenericExtension {
            node,
            template_node,
            parameters,
            decl_scope: self.current_scope(),
        };
        self.pending_generic_extensions.push(extension.clone());
        // Instances that already exist must receive the extension too.
        let instances = self
            .instances_by_template
            .get(&template_node)
            .cloned()
            .unwrap_or_default();
        for arguments in instances {
            if let Some(instance_type) = self
                .type_instantiations
                .get(&(template_node, arguments.clone()))
                .copied()
            {
                self.apply_generic_extension(&extension, instance_type, &arguments);
            }
        }
    }

    /// Applies one recorded generic extension to an instance of its target.
    fn apply_generic_extension(
        &mut self,
        extension: &PendingGenericExtension,
        instance_type: TypeId,
        arguments: &[TypeId],
    ) {
        let span = self.span_of(extension.node);
        if extension.parameters.len() != arguments.len() {
            self.error(
                span,
                format!(
                    "generic extension declares {} type parameter(s) but its target has {}",
                    extension.parameters.len(),
                    arguments.len()
                ),
            );
            return;
        }
        for (parameter, argument) in extension.parameters.iter().zip(arguments.iter()) {
            for constraint in &parameter.constraints {
                if !self.type_satisfies_constraint(*argument, constraint, extension.decl_scope) {
                    return;
                }
            }
        }
        let environment: HashMap<String, TypeId> = extension
            .parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .zip(arguments.iter().copied())
            .collect();
        let cloned = self.clone_declaration(extension.node);
        self.generic_env.push(environment);
        self.resolve_extension_declaration(cloned, Some(instance_type));
        self.generic_env.pop();
    }

    /// Applies every recorded generic extension to a new instance.
    fn apply_generic_extensions(
        &mut self,
        template_node: NodeId,
        instance_type: TypeId,
        arguments: &[TypeId],
    ) {
        let extensions: Vec<PendingGenericExtension> = self
            .pending_generic_extensions
            .iter()
            .filter(|extension| extension.template_node == template_node)
            .cloned()
            .collect();
        for extension in extensions {
            self.apply_generic_extension(&extension, instance_type, arguments);
        }
    }

    // ------------------------------------------------------------------
    // Monomorphization expressions (§10.2)
    // ------------------------------------------------------------------

    /// Resolves `target<arguments>` to a concrete type or function (§10.2).
    pub(crate) fn check_monomorphization(
        &mut self,
        node: NodeId,
        expression: &MonomorphizationExpr,
    ) -> Option<MonomorphizedTarget> {
        let span = self.span_of(node);
        let arguments = self.resolve_type_arguments(&expression.arguments)?;
        let Some(symbol) = self.try_resolve_name(expression.base) else {
            self.error(
                span,
                "expected a generic type or generic function before '<'",
            );
            return None;
        };
        match symbol {
            Symbol::Type(type_id) => {
                let Type::Named(index) = self.types_of(type_id) else {
                    self.error(span, "monomorphization requires a declared type");
                    return None;
                };
                let definition = &self.result.type_defs[index as usize];
                if definition.generic_parameters.is_empty() {
                    let name = definition.qualified_name.clone();
                    self.error(
                        span,
                        format!("'{name}' does not declare generic parameters"),
                    );
                    return None;
                }
                let template_node = definition.node;
                let instance = self.instantiate_type(template_node, arguments, span)?;
                self.result.resolutions[node as usize] = Some(Resolution::Type(instance));
                Some(MonomorphizedTarget::Type(instance))
            }
            Symbol::GenericFunctions(templates) => {
                let mut matching = Vec::new();
                for template in templates {
                    let expected = self.result.functions[template as usize]
                        .generic_parameters
                        .len();
                    if expected == arguments.len() {
                        matching.push(template);
                    }
                }
                let template = match matching.as_slice() {
                    [single] => *single,
                    [] => {
                        self.error(
                            span,
                            "no generic function with this number of type arguments",
                        );
                        return None;
                    }
                    _ => {
                        self.error(span, "ambiguous generic function instantiation");
                        return None;
                    }
                };
                let instance = self.instantiate_function(template, arguments, span)?;
                self.result.resolutions[node as usize] = Some(Resolution::Function(instance));
                Some(MonomorphizedTarget::Function(instance))
            }
            _ => {
                self.error(
                    span,
                    "only generic types and generic functions can be monomorphized",
                );
                None
            }
        }
    }
}

/// Maps a type-kind constraint to the corresponding declaration kind.
fn type_kind_matches(kind: TypeDefKind, constraint: GenericConstraintKind) -> bool {
    matches!(
        (kind, constraint),
        (TypeDefKind::Class, GenericConstraintKind::Class)
            | (TypeDefKind::Structure, GenericConstraintKind::Structure)
            | (TypeDefKind::Interface, GenericConstraintKind::Interface)
            | (TypeDefKind::Enumeration, GenericConstraintKind::Enumeration)
            | (TypeDefKind::Variant, GenericConstraintKind::Variant)
            | (TypeDefKind::Union, GenericConstraintKind::Union)
    )
}

/// The keyword spelling of a type-kind constraint.
fn constraint_kind_keyword(kind: GenericConstraintKind) -> &'static str {
    match kind {
        GenericConstraintKind::Class => "class",
        GenericConstraintKind::Structure => "structure",
        GenericConstraintKind::Interface => "interface",
        GenericConstraintKind::Enumeration => "enumeration",
        GenericConstraintKind::Variant => "variant",
        GenericConstraintKind::Union => "union",
        GenericConstraintKind::Basic => "basic",
    }
}
