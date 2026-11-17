//! Semantic analysis: name resolution, type checking, layouts and mangled names.
//!
//! Analysis runs in three passes:
//! 1. declarations: modules, named types (`class`, `structure`, `enumeration`,
//!    `variant`, `union`, `interface`) and function/global signatures,
//! 2. layouts and signatures: field types, structure layout, enumeration values,
//! 3. bodies: scopes, local variables, expression/statement type checking.
//!
//! The result is consumed by code generation, which needs the inferred type of
//! every expression, the resolution of every name, and the layout of every
//! declared type.

mod check;
pub mod types;

use std::collections::HashMap;

use crate::ast::*;
use crate::diagnostics::DiagnosticEngine;
use crate::source::{SourceManager, Span};
use types::{mangle_function, TypeId, TypeStore};

pub use types::{Type, TypeId as Ty};

/// Identity of a declared type (class, structure, enumeration, ...).
pub type TypeDefId = u32;
/// Identity of a local variable.
pub type LocalId = u32;

/// What kind of type a declaration introduces (§3.7).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TypeDefKind {
    /// `structure`
    Structure,
    /// `class`
    Class,
    /// `interface`
    Interface,
    /// `enumeration`
    Enumeration,
    /// `variant`
    Variant,
    /// `union`
    Union,
}

/// One field of a structure or class (§3.8.1).
#[derive(Clone, Debug)]
pub struct FieldInfo {
    /// Field name.
    pub name: String,
    /// Field type.
    pub ty: TypeId,
    /// Byte offset from the start of the instance.
    pub offset: u64,
}

/// One state of a variant (§3.7.5).
#[derive(Clone, Debug)]
pub struct VariantStateInfo {
    /// State name.
    pub name: String,
    /// Payload type; `None` for a payload-less state.
    pub payload: Option<TypeId>,
}

/// One member of a union (§3.7.6).
#[derive(Clone, Debug)]
pub struct UnionMemberInfo {
    /// Member name.
    pub name: String,
    /// Member type.
    pub ty: TypeId,
    /// Offset from the start of the instance.
    pub offset: u64,
    /// Explicit `aligned(N)` modifier, if any.
    pub aligned: Option<u32>,
}

/// A declared type.
#[derive(Debug)]
pub struct TypeDef {
    /// Declared name.
    pub name: String,
    /// Fully qualified name (`Outer.Inner`).
    pub qualified_name: String,
    /// What kind of declaration this is.
    pub kind: TypeDefKind,
    /// The declaration's AST node.
    pub node: NodeId,
    /// Fields, in declaration order (structures/classes).
    pub fields: Vec<FieldInfo>,
    /// Field initializers, parallel to `fields` (§3.8.1).
    pub field_initializers: Vec<Option<NodeId>>,
    /// Methods contributed by extension blocks, grouped by name.
    pub methods: std::collections::HashMap<String, Vec<u32>>,
    /// Methods in declaration order.
    pub method_list: Vec<u32>,
    /// Accessor properties declared in extension blocks.
    pub accessors: Vec<AccessorInfo>,
    /// Operator definitions declared in extension blocks.
    pub operators: Vec<OperatorInfo>,
    /// Interfaces this class implements (§3.7.3/§3.7.7).
    pub interfaces: Vec<TypeId>,
    /// Per-interface dispatch tables: implementation method id per interface
    /// method slot, in interface declaration order.
    pub interface_dispatch: Vec<(TypeId, Vec<u32>)>,
    /// Whether the class is sealed (§9.11).
    pub sealed: bool,
    /// Virtual dispatch table: method id per slot (classes only).
    pub vtable: Vec<u32>,
    /// Whether the vtable has been computed.
    pub vtable_ready: bool,
    /// Base class, if any.
    pub base: Option<TypeId>,
    /// Variant states, in declaration order.
    pub variant_states: Vec<VariantStateInfo>,
    /// Union members with their offsets.
    pub union_members: Vec<UnionMemberInfo>,
    /// Underlying type of a variant's state space.
    pub tag_type: Option<TypeId>,
    /// Offset of a variant's payload space.
    pub payload_offset: u64,
    /// Size of a variant's payload space.
    pub payload_size: u64,
    /// Size of an instance including the class header (classes) and inherited
    /// fields; equals `size` for structures.
    pub instance_size: u64,
    /// Alignment of an instance.
    pub instance_align: u64,
    /// Enumeration members with their values.
    pub enum_members: Vec<(String, i128)>,
    /// Underlying type of an enumeration.
    pub underlying: Option<TypeId>,
    /// Byte size of the type.
    pub size: u64,
    /// Alignment of the type (the spec uses member type size as default
    /// alignment, §3.7.2).
    pub align: u64,
}

/// One declared parameter.
#[derive(Clone, Debug)]
pub struct ParameterInfo {
    /// Declared name.
    pub name: String,
    /// Parameter type.
    pub ty: TypeId,
    /// Whether the parameter is passed by reference (`&`, §8.2).
    pub by_ref: bool,
}

/// A function definition (`fixed Name : function(...) T { ... }`, §8.1).
#[derive(Debug)]
pub struct FunctionInfo {
    /// Declared name.
    pub name: String,
    /// Module path the declaration lives in.
    pub scope: Vec<String>,
    /// The `fixed` declaration node.
    pub node: NodeId,
    /// The function body; `None` for abstract interface methods.
    pub body: Option<FunctionBody>,
    /// Parameters, including `this` first for methods.
    pub parameters: Vec<ParameterInfo>,
    /// The type this function is a method of, if it came from an extension.
    pub method_of: Option<TypeId>,
    /// Whether the method is virtual (`virtual` or `override`, §9.12/§9.13).
    pub is_virtual: bool,
    /// Whether the method overrides a base method.
    pub is_override: bool,
    /// Whether the method or class is sealed (§9.11).
    pub is_sealed: bool,
    /// Whether the method is abstract (an interface method without a body).
    pub is_abstract: bool,
    /// Return type; `None` is the empty type.
    pub return_type: Option<TypeId>,
    /// Mangled symbol name (§13.2).
    pub mangled: String,
    /// Locals declared in the body, in declaration order.
    pub locals: Vec<LocalInfo>,
    /// Span of the declaration, for diagnostics.
    pub span: Span,
}

/// The lowered body of a function or method.
#[derive(Clone, Copy, Debug)]
pub enum FunctionBody {
    /// A statement block.
    Block(NodeId),
    /// A single expression whose value is returned.
    Expression(NodeId),
    /// Read a field through `this` (accessor default getter).
    BackingGet {
        /// Type owning the field.
        owner: TypeId,
        /// Field index.
        field_index: u32,
    },
    /// Write a field through `this` (accessor default setter).
    BackingSet {
        /// Type owning the field.
        owner: TypeId,
        /// Field index.
        field_index: u32,
    },
}

/// The overloadable operator kinds (§4.9.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OperatorKind {
    /// Binary `+`.
    Add,
    /// Binary `-`.
    Subtract,
    /// Binary `*`.
    Multiply,
    /// Binary `/`.
    Divide,
    /// Binary `%`.
    Remainder,
    /// Binary `==`.
    Equal,
    /// Binary `>`.
    Greater,
    /// Binary `<`.
    Less,
    /// Unary `-`.
    Negate,
    /// Indexer `[...]`.
    Index,
    /// Conversion operator.
    Conversion,
}

/// One operator definition attached to a type (§4.9).
#[derive(Clone, Debug)]
pub struct OperatorInfo {
    /// Operator kind.
    pub kind: OperatorKind,
    /// Implementation method (non-indexer operators).
    pub method: u32,
    /// Indexer getter.
    pub getter: Option<u32>,
    /// Indexer setter.
    pub setter: Option<u32>,
}

/// An accessor property (§3.8.2).
#[derive(Clone, Debug)]
pub struct AccessorInfo {
    /// Accessor name.
    pub name: String,
    /// Value type.
    pub ty: TypeId,
    /// Getter method, if defined.
    pub getter: Option<u32>,
    /// Setter method, if defined.
    pub setter: Option<u32>,
}

/// How a local's storage is allocated (§12.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Storage {
    /// Storage inside the current scope (stack or global data).
    Value,
    /// Heap storage reached through a pointer (`heap` definitions).
    Heap,
}

/// A local variable (also used for parameters' storage).
#[derive(Clone, Debug)]
pub struct LocalInfo {
    /// Declared name.
    pub name: String,
    /// Type of the variable.
    pub ty: TypeId,
    /// Whether the variable is a constant (`fixed`).
    pub fixed: bool,
    /// Parameter index in the LLVM signature, when this local stores one.
    pub parameter: Option<u32>,
    /// Storage allocation of this variable.
    pub storage: Storage,
    /// Span of the declaration.
    pub span: Span,
}

/// A module-level variable or constant.
#[derive(Debug)]
pub struct GlobalInfo {
    /// Declared name.
    pub name: String,
    /// Module path.
    pub scope: Vec<String>,
    /// Type of the storage.
    pub ty: TypeId,
    /// Initializer expression, if any.
    pub init: Option<NodeId>,
    /// Whether the storage is a constant (`fixed`).
    pub fixed: bool,
    /// Mangled symbol name.
    pub mangled: String,
    /// The declaration's AST node.
    pub node: NodeId,
    /// Span of the declaration.
    pub span: Span,
}

/// What a name resolved to.
#[derive(Clone, Debug)]
pub enum Resolution {
    /// A local variable or parameter.
    Local(LocalId),
    /// A module-level variable.
    Global(u32),
    /// A function overload set; the index is the selected overload for calls.
    Function(u32),
    /// A declared type.
    Type(TypeId),
    /// An enumeration member.
    EnumMember {
        /// The enumeration type.
        type_id: TypeId,
        /// Index into the enumeration's member list.
        index: u32,
    },
    /// The `this` parameter of a method.
    This,
    /// The `base` keyword inside a method (§3.7.1).
    Base,
    /// A user-defined operator invocation (§4.9).
    Operator {
        /// Operator implementation.
        method: u32,
    },
    /// `!=` desugared to `!(==)` (§4.9.20).
    DerivedNotEqual {
        /// `==` implementation.
        method: u32,
    },
    /// `<=`/`>=` desugared to `<`/`>` or `==` (§4.9.15–4.9.17).
    DerivedComparison {
        /// `<` or `>` implementation.
        ordering: u32,
        /// `==` implementation.
        equality: u32,
    },
    /// Indexer access (§4.4.3).
    Indexer {
        /// Getter.
        getter: Option<u32>,
        /// Setter.
        setter: Option<u32>,
    },
    /// Conversion operator invocation (§4.9.12).
    Conversion {
        /// Implementation.
        method: u32,
    },
    /// An accessor member read or write (§3.8.2).
    Accessor {
        /// Getter method, if readable.
        getter: Option<u32>,
        /// Setter method, if writable.
        setter: Option<u32>,
    },
    /// A construction expression (`T(args)`, §4.6); `method` is the selected
    /// `Construct` overload, if one exists.
    Construct {
        /// Constructed type.
        type_id: TypeId,
        /// Selected constructor.
        method: Option<u32>,
    },
    /// A member construction of a variant state (`Variant.State(value)`).
    ConstructVariant {
        /// Variant type.
        type_id: TypeId,
        /// State index.
        state: u32,
    },
    /// A member construction of a union (`Union.Member(value)`).
    ConstructUnion {
        /// Union type.
        type_id: TypeId,
        /// Member index.
        member: u32,
    },
    /// A variant instance of a payload-less state (`Variant.State`).
    VariantState {
        /// Variant type.
        type_id: TypeId,
        /// State index.
        state: u32,
    },
    /// A union member access (`unionValue.Member`).
    UnionMember {
        /// Union type.
        type_id: TypeId,
        /// Member index.
        member: u32,
    },
    /// A structure or class field.
    Field {
        /// The declaring type.
        type_id: TypeId,
        /// Index into the type's field list.
        index: u32,
    },
}

/// One checked `catch` clause of a `trap` statement (§5.10.4).
#[derive(Clone, Debug)]
pub struct CatchInfo {
    /// Caught type; `None` for `wildcard`.
    pub ty: Option<TypeId>,
    /// Local receiving the thrown value, if a name was written.
    pub binding: Option<LocalId>,
    /// Whether this is `catch wildcard`.
    pub wildcard: bool,
    /// Handler body block.
    pub body: NodeId,
}

/// Lexical exception-handling context used while checking statements.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrapContextKind {
    /// Inside a `trap` body.
    Body,
    /// Inside a `catch` block.
    Catch,
    /// Inside a `finally` block.
    Finally,
}

/// Type information recorded for a checked `match` pattern.
#[derive(Clone, Debug)]
pub enum MatchPatternInfo {
    /// `is name: Variant.State` pattern.
    IsVariant {
        /// Variant type.
        type_id: TypeId,
        /// State index.
        state: u32,
        /// Local receiving the payload, if the pattern binds one.
        binding: Option<LocalId>,
    },
    /// `is name: Class` pattern over a class hierarchy.
    IsClass {
        /// Target class type.
        type_id: TypeId,
        /// Local receiving the narrowed reference, if bound.
        binding: Option<LocalId>,
    },
}

/// Everything the later phases need from semantic analysis.
#[derive(Debug, Default)]
pub struct SemaResult {
    /// Interned types.
    pub types: TypeStore,
    /// Declared types.
    pub type_defs: Vec<TypeDef>,
    /// Inferred type of every AST node.
    pub node_types: Vec<Option<TypeId>>,
    /// What every name expression resolved to.
    pub resolutions: Vec<Option<Resolution>>,
    /// Function definitions in declaration order.
    pub functions: Vec<FunctionInfo>,
    /// Module-level variables in declaration order.
    pub globals: Vec<GlobalInfo>,
    /// Locals declared by a given declaration node (Local/Fixed statements).
    pub local_declarations: HashMap<NodeId, Vec<LocalId>>,
    /// The binding local of a `for` statement, by statement node.
    pub loop_bindings: HashMap<NodeId, LocalId>,
    /// Type-checked `is` patterns, keyed by match statement node and case index.
    pub match_patterns: HashMap<(NodeId, u32), MatchPatternInfo>,
    /// Checked catch clauses, keyed by trap statement node.
    pub trap_catches: HashMap<NodeId, Vec<CatchInfo>>,
    /// Number of errors reported.
    pub error_count: u32,
}

impl SemaResult {
    /// Size and alignment of a type in bytes.
    ///
    /// Panics only on an internal invariant violation (missing named type).
    #[must_use]
    pub fn size_align(&self, ty: TypeId) -> (u64, u64) {
        match self.types.get(ty) {
            Type::Error | Type::Void => (0, 1),
            Type::Boolean => (1, 1),
            Type::Integer { bytes, .. } | Type::Float { bytes } => {
                let size = u64::from(*bytes);
                (size, size)
            }
            Type::Array {
                dimensions,
                element,
            } => {
                let (element_size, element_align) = self.size_align(*element);
                let count: u64 = dimensions.iter().product();
                (element_size * count, element_align)
            }
            Type::Pointer { .. } | Type::Function { .. } => (8, 8),
            Type::Named(index) => {
                let definition = &self.type_defs[*index as usize];
                (definition.size, definition.align)
            }
        }
    }

    /// Returns a declared type by id.
    #[must_use]
    pub fn type_def(&self, id: TypeDefId) -> &TypeDef {
        &self.type_defs[id as usize]
    }

    /// Returns the canonical spelling of a type, using declared names when
    /// available.
    #[must_use]
    pub fn spelling(&self, ty: TypeId) -> String {
        match self.types.get(ty) {
            Type::Named(index) => self.type_defs[*index as usize].qualified_name.clone(),
            _ => self.types.spelling(ty),
        }
    }
}

// ---------------------------------------------------------------------------
// Analysis
// ---------------------------------------------------------------------------

/// A lexical scope.
#[derive(Debug, Default)]
pub(crate) struct Scope {
    parent: Option<usize>,
    symbols: HashMap<String, Symbol>,
}

/// What a name in a scope binds to.
#[derive(Clone, Debug)]
pub(crate) enum Symbol {
    Type(TypeId),
    Functions(Vec<u32>),
    Local(LocalId),
    Global(u32),
    Module(usize),
    This { owner: TypeId },
}

/// Runs semantic analysis over one parsed file.
///
/// Errors are reported through `engine`; the returned result is still usable for
/// code generation when `error_count` is zero.
pub fn analyze(ast: &Ast, sources: &SourceManager, engine: &mut DiagnosticEngine) -> SemaResult {
    let mut sema = Sema {
        ast,
        sources,
        engine,
        result: SemaResult {
            types: TypeStore::new(),
            node_types: vec![None; ast.nodes.len()],
            resolutions: vec![None; ast.nodes.len()],
            ..SemaResult::default()
        },
        scopes: vec![Scope::default()],
        scope_stack: vec![0],
        scope_path: Vec::new(),
        current_function: None,
        suppress_errors: 0,
        loop_labels: Vec::new(),
        pending_accessors: Vec::new(),
        trap_contexts: Vec::new(),
    };
    sema.run();
    sema.result.error_count = sema.engine.error_count();
    sema.result
}

pub(crate) struct Sema<'a> {
    pub(crate) ast: &'a Ast,
    pub(crate) sources: &'a SourceManager,
    pub(crate) engine: &'a mut DiagnosticEngine,
    pub(crate) result: SemaResult,
    pub(crate) scopes: Vec<Scope>,
    pub(crate) scope_stack: Vec<usize>,
    pub(crate) scope_path: Vec<String>,
    pub(crate) current_function: Option<u32>,
    pub(crate) suppress_errors: u32,
    pub(crate) loop_labels: Vec<Option<String>>,
    /// Accessor declarations waiting for their owner's field layout.
    pub(crate) pending_accessors: Vec<(NodeId, TypeId)>,
    /// Lexical trap context stack for validating `throw` (§5.10).
    pub(crate) trap_contexts: Vec<TrapContextKind>,
}

impl<'a> Sema<'a> {
    // -- diagnostics -------------------------------------------------------

    pub(crate) fn error(&mut self, span: Span, message: impl Into<String>) {
        if self.suppress_errors == 0 {
            let mut stderr = std::io::stderr();
            let _ = self.engine.report(
                &mut stderr,
                self.sources,
                crate::diagnostics::Severity::Error,
                Some(span),
                &message.into(),
            );
        }
    }

    pub(crate) fn span_of(&self, node: NodeId) -> Span {
        self.ast.span(node)
    }

    pub(crate) fn node_kind(&self, node: NodeId) -> NodeKind {
        self.ast.node(node).kind.clone()
    }

    // -- scopes ------------------------------------------------------------

    pub(crate) fn current_scope(&self) -> usize {
        *self.scope_stack.last().expect("scope stack is never empty")
    }

    fn create_child_scope(&mut self) -> usize {
        let parent = self.current_scope();
        let id = self.scopes.len();
        self.scopes.push(Scope {
            parent: Some(parent),
            symbols: HashMap::new(),
        });
        id
    }

    pub(crate) fn push_scope(&mut self) -> usize {
        let id = self.create_child_scope();
        self.scope_stack.push(id);
        id
    }

    pub(crate) fn pop_scope(&mut self) {
        self.scope_stack.pop();
    }

    /// Declares a non-function symbol in the current scope.
    pub(crate) fn declare(&mut self, name: String, symbol: Symbol, span: Span) {
        let scope = self.current_scope();
        if self.scopes[scope].symbols.contains_key(&name) {
            self.error(span, format!("'{name}' is already declared in this scope"));
            return;
        }
        self.scopes[scope].symbols.insert(name, symbol);
    }

    pub(crate) fn declare_functions(&mut self, name: String, function: u32) {
        let scope = self.current_scope();
        match self.scopes[scope].symbols.get_mut(&name) {
            Some(Symbol::Functions(list)) => list.push(function),
            Some(_) => {
                let span = self.result.functions[function as usize].span;
                self.error(span, format!("'{name}' is already declared in this scope"));
            }
            None => {
                self.scopes[scope]
                    .symbols
                    .insert(name, Symbol::Functions(vec![function]));
            }
        }
    }

    pub(crate) fn lookup(&self, name: &str) -> Option<&Symbol> {
        let mut scope = Some(self.current_scope());
        while let Some(index) = scope {
            if let Some(symbol) = self.scopes[index].symbols.get(name) {
                return Some(symbol);
            }
            scope = self.scopes[index].parent;
        }
        None
    }

    pub(crate) fn lookup_in(&self, scope: usize, name: &str) -> Option<&Symbol> {
        self.scopes[scope].symbols.get(name)
    }

    // -- passes ------------------------------------------------------------

    fn run(&mut self) {
        // Pass 1: declare modules and named types so that signatures can refer
        // to them regardless of declaration order (§6.3).
        let roots = self.ast.roots.clone();
        self.declare_types(&roots);

        // Pass 2: resolve field types, layouts, enumeration values and
        // function/global signatures.
        self.resolve_declarations(&roots);

        // Pass 3: check bodies and initializers.
        self.finalize_variant_union_layouts();
        self.build_vtables();
        self.check_function_bodies();
        self.check_global_initializers();
    }

    /// Pass 1: walk the declaration structure creating module scopes and named
    /// type stubs.
    fn declare_types(&mut self, nodes: &[NodeId]) {
        for &node in nodes {
            match self.node_kind(node) {
                NodeKind::Module(declaration) => {
                    let module_scope = self.create_child_scope();
                    self.declare(
                        declaration.name.clone(),
                        Symbol::Module(module_scope),
                        self.span_of(node),
                    );
                    self.scope_stack.push(module_scope);
                    self.scope_path.push(declaration.name.clone());
                    let members = declaration.members.clone();
                    self.declare_types(&members);
                    self.scope_path.pop();
                    self.scope_stack.pop();
                }
                NodeKind::Class(declaration) => {
                    self.declare_named_type(node, declaration.name.clone(), TypeDefKind::Class)
                }
                NodeKind::Structure(declaration) => {
                    self.declare_named_type(node, declaration.name.clone(), TypeDefKind::Structure)
                }
                NodeKind::Interface(declaration) => {
                    self.declare_named_type(node, declaration.name.clone(), TypeDefKind::Interface)
                }
                NodeKind::Enumeration(declaration) => self.declare_named_type(
                    node,
                    declaration.name.clone(),
                    TypeDefKind::Enumeration,
                ),
                NodeKind::Variant(declaration) => {
                    self.declare_named_type(node, declaration.name.clone(), TypeDefKind::Variant)
                }
                NodeKind::Union(declaration) => {
                    self.declare_named_type(node, declaration.name.clone(), TypeDefKind::Union)
                }
                _ => {}
            }
        }
    }

    fn declare_named_type(&mut self, node: NodeId, name: String, kind: TypeDefKind) {
        let qualified_name = if self.scope_path.is_empty() {
            name.clone()
        } else {
            format!("{}.{}", self.scope_path.join("."), name)
        };
        let index = self.result.type_defs.len() as TypeDefId;
        self.result.type_defs.push(TypeDef {
            name: name.clone(),
            qualified_name,
            kind,
            node,
            fields: Vec::new(),
            field_initializers: Vec::new(),
            methods: std::collections::HashMap::new(),
            method_list: Vec::new(),
            accessors: Vec::new(),
            operators: Vec::new(),
            interfaces: Vec::new(),
            interface_dispatch: Vec::new(),
            sealed: false,
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
        let type_id = self.result.types.intern(Type::Named(index));
        self.declare(name, Symbol::Type(type_id), self.span_of(node));
    }

    /// Pass 2: resolve declared type bodies and signatures.
    fn resolve_declarations(&mut self, nodes: &[NodeId]) {
        for &node in nodes {
            match self.node_kind(node) {
                NodeKind::Module(declaration) => {
                    self.enter_module_scope(&declaration.name);
                    let members = declaration.members.clone();
                    self.resolve_declarations(&members);
                    self.exit_module_scope();
                }
                NodeKind::Structure(declaration) => {
                    self.resolve_structure_like(
                        node,
                        declaration.name.clone(),
                        TypeDefKind::Structure,
                    );
                }
                NodeKind::Class(declaration) => {
                    self.resolve_structure_like(node, declaration.name.clone(), TypeDefKind::Class);
                }
                NodeKind::Enumeration(declaration) => {
                    self.resolve_enumeration(node, declaration.name.clone());
                }
                NodeKind::Variant(_declaration) => self.resolve_variant(node),
                NodeKind::Union(_declaration) => self.resolve_union(node),
                NodeKind::Interface(_declaration) => self.resolve_interface(node),
                NodeKind::Fixed(declaration) => self.resolve_function_signature(node, &declaration),
                NodeKind::Linkable(declaration) => {
                    self.resolve_linkable_signature(node, &declaration);
                }
                NodeKind::Local(_) => self.resolve_global_local(node),
                NodeKind::Extension(_) => self.resolve_extension_declaration(node, None),
                _ => {}
            }
        }
        self.resolve_pending_accessors();
    }

    pub(crate) fn unsupported(&mut self, span: Span, feature: &str) {
        self.error(span, format!("{feature} are not implemented yet"));
    }

    fn enter_module_scope(&mut self, name: &str) {
        // The module scope was created in pass 1 and bound in the parent scope.
        if let Some(Symbol::Module(scope)) = self.lookup(name).cloned() {
            self.scope_stack.push(scope);
            self.scope_path.push(name.to_string());
        }
    }

    fn exit_module_scope(&mut self) {
        self.scope_stack.pop();
        self.scope_path.pop();
    }
}
