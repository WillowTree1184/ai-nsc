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
mod generics;
pub mod types;

pub(crate) use generics::MonomorphizedTarget;

use std::collections::HashMap;

use crate::ast::*;
use crate::diagnostics::{DiagnosticEngine, Severity};
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

/// A constraint on a generic parameter (§10.1).
#[derive(Clone, Debug)]
pub enum GenericConstraint {
    /// A type-kind keyword (`class`, `structure`, `interface`, `enumeration`,
    /// `variant`, `union`, `basic`).
    Kind(GenericConstraintKind),
    /// A named type; the argument must inherit, implement or equal it.
    Named(String),
}

/// The type-kind keywords allowed as generic constraints (§10.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GenericConstraintKind {
    /// `class`
    Class,
    /// `structure`
    Structure,
    /// `interface`
    Interface,
    /// `enumeration`
    Enumeration,
    /// `variant`
    Variant,
    /// `union`
    Union,
    /// `basic` (any basic type, §3.2)
    Basic,
}

/// One generic parameter of a declaration (§10.1).
#[derive(Clone, Debug)]
pub struct GenericParameterInfo {
    /// Parameter name.
    pub name: String,
    /// Constraints; an empty list accepts any type.
    pub constraints: Vec<GenericConstraint>,
    /// Span of the declaration.
    pub span: Span,
}

impl GenericParameterInfo {
    /// Builds the semantic view of a parsed generic parameter.
    #[must_use]
    pub fn from_ast(parameter: &GenericParameter) -> Self {
        let constraints = parameter
            .constraints
            .iter()
            .map(|constraint| match constraint.as_str() {
                "class" => GenericConstraint::Kind(GenericConstraintKind::Class),
                "structure" => GenericConstraint::Kind(GenericConstraintKind::Structure),
                "interface" => GenericConstraint::Kind(GenericConstraintKind::Interface),
                "enumeration" => GenericConstraint::Kind(GenericConstraintKind::Enumeration),
                "variant" => GenericConstraint::Kind(GenericConstraintKind::Variant),
                "union" => GenericConstraint::Kind(GenericConstraintKind::Union),
                "basic" => GenericConstraint::Kind(GenericConstraintKind::Basic),
                name => GenericConstraint::Named(name.to_string()),
            })
            .collect();
        Self {
            name: parameter.name.clone(),
            constraints,
            span: parameter.span,
        }
    }
}

/// One accessor declaration waiting for its owner's layout (§3.8.2).
#[derive(Clone, Debug)]
pub(crate) struct PendingAccessor {
    /// The accessor declaration node.
    pub(crate) node: NodeId,
    /// The type the accessor belongs to.
    pub(crate) owner: TypeId,
    /// Generic substitution environment of the owner's instantiation (§10.1).
    pub(crate) generic_env: HashMap<String, TypeId>,
}

/// A built-in generic variant type (§15.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BuiltinVariantKind {
    /// `Optional<T>`: `Value: T` or a payload-less `Null`.
    Optional,
    /// `Result<TValue; TAlternative>`: `Value: TValue` or `Alternative: TAlternative`.
    Result,
}

/// A module-level extension with a generic target (`extension for Box<T>`,
/// §3.8.3). Such extensions are instantiated together with the targeted type.
#[derive(Clone, Debug)]
pub(crate) struct PendingGenericExtension {
    /// The `extension` declaration node.
    pub(crate) node: NodeId,
    /// Declaration node of the targeted generic type.
    pub(crate) template_node: NodeId,
    /// Generic parameters written in the target, positionally matched against
    /// the target type's parameters.
    pub(crate) parameters: Vec<GenericParameterInfo>,
    /// Lexical scope of the extension declaration, for constraint lookups.
    pub(crate) decl_scope: usize,
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
    /// Generic parameters of the declaration (§10.1); empty both for
    /// non-generic declarations and for instantiations.
    pub generic_parameters: Vec<GenericParameterInfo>,
    /// Substituted type arguments of an instantiation, in parameter order.
    pub generic_arguments: Vec<TypeId>,
    /// The generic declaration this type instantiated, if any.
    pub template: Option<TypeDefId>,
    /// Lexical scope the declaration appeared in (constraint lookups).
    pub decl_scope: usize,
    /// Static ("class-level") fields, by name: index into `globals` (§9.15).
    pub static_fields: std::collections::HashMap<String, u32>,
    /// Static methods, by name: indices into `functions` (§9.15).
    pub static_methods: std::collections::HashMap<String, Vec<u32>>,
    /// Built-in variant kind, for `Optional`/`Result` (§15.2).
    pub builtin_variant: Option<BuiltinVariantKind>,
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
    /// Whether the class is abstract and therefore cannot be instantiated (§9.10).
    pub is_abstract: bool,
    /// Whether the class is `static`: no instances, class-level members (§9.15).
    pub is_static: bool,
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
    /// Default-value expression used when a call omits the argument (§8.2).
    pub default: Option<NodeId>,
    /// Whether the argument must be a compile-time constant (§11.5).
    pub compile_time: bool,
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
    /// The class owning a static method: its static fields are in scope in the
    /// body (§9.15).
    pub static_owner: Option<TypeId>,
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
    /// External symbol name for `linkable` declarations, if written.
    pub exposed_symbol: Option<String>,
    /// Generic parameters declared by a generic function (template only, §10.1).
    pub generic_parameters: Vec<GenericParameterInfo>,
    /// Substituted type arguments of a generic function instance.
    pub generic_arguments: Vec<TypeId>,
    /// The generic function this instance was created from, if any.
    pub template: Option<u32>,
    /// Lexical scope of the declaration (constraint lookups).
    pub decl_scope: usize,
    /// Substitution environment active when the function was created, i.e. the
    /// generic arguments of its enclosing instantiated type (§10.1).
    pub generic_env: HashMap<String, TypeId>,
    /// Whether the function takes a hidden leading closure-environment pointer
    /// (§2.7.9; function literals).
    pub environment: bool,
    /// Variables captured from the enclosing function (function literals).
    pub captures: Vec<CaptureInfo>,
    /// Compile-time constants bound to `#` parameters of this specialization
    /// (§11.5).
    pub constant_values: HashMap<String, i128>,
    /// Locals declared in the body, in declaration order.
    pub locals: Vec<LocalInfo>,
    /// Span of the declaration, for diagnostics.
    pub span: Span,
}

impl FunctionInfo {
    /// Whether this function is a generic template that is only lowered through
    /// its instantiations (§10.1).
    #[must_use]
    pub fn is_template(&self) -> bool {
        !self.generic_parameters.is_empty()
            || (self.template.is_none()
                && self
                    .parameters
                    .iter()
                    .any(|parameter| parameter.compile_time))
    }
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
    /// Unary decision operator `??` (§4.9.26).
    Coalesce,
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
    /// Whether a conversion operator may be invoked implicitly (§9.7).
    pub implicitable: bool,
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

/// Names of the Built-In Library exception classes (§15.2).
pub(crate) const BUILTIN_EXCEPTION_NAMES: [&str; 6] = [
    "Exception",
    "NotImplementedException",
    "DivideByZeroException",
    "OutOfRangeException",
    "AllocateFailedException",
    "DeallocateFailedException",
];

/// How a local's storage is allocated (§12.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Storage {
    /// Storage inside the current scope (stack or global data).
    Value,
    /// Heap storage reached through a pointer (`heap` definitions).
    Heap,
    /// By-reference parameter storage: the slot holds a pointer to the caller's
    /// value (§8.2).
    Reference,
    /// Storage inside a closure environment (captured variable, §2.7.9).
    Capture(u32),
}

/// Where a captured variable comes from when a function literal is created.
#[derive(Clone, Copy, Debug)]
pub enum CaptureSource {
    /// A local (or parameter) of the enclosing function.
    Local(LocalId),
    /// The enclosing method's `this` reference.
    This,
}

/// One variable captured by a function literal (§2.7.9).
#[derive(Clone, Debug)]
pub struct CaptureInfo {
    /// Captured name.
    pub name: String,
    /// Captured type.
    pub ty: TypeId,
    /// Local that receives the captured value inside the literal.
    pub local: LocalId,
    /// Byte offset inside the closure environment record.
    pub offset: u64,
    /// Where the value is read from when the closure is created.
    pub source: CaptureSource,
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

/// One heap allocation unit tracked for NOLAA diagnostics (§12.2).
#[derive(Clone, Debug)]
pub(crate) struct HeapUnit {
    /// Variable name.
    pub(crate) name: String,
    /// Declaration span.
    pub(crate) span: Span,
    /// Whether the unit was declared `manual`.
    pub(crate) manual: bool,
    /// Whether an explicit `dispose` was seen.
    pub(crate) disposed: bool,
    /// Whether the unit was abandoned through `NOLAA.Abandon`.
    pub(crate) abandoned: bool,
    /// Whether the pointer escaped the function (argument, return, store...).
    pub(crate) escaped: bool,
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
    /// A compiler-API call that produces no run-time code (§15.3).
    CompileTimeNoOp,
    /// A compile-time constant value (the compiler API evaluates it).
    Constant {
        /// Type of the constant.
        ty: TypeId,
        /// Integer value (booleans are 0/1).
        value: i128,
    },
    /// `Type.SizeOf<T>()` from the compiler API (§11.8): a compile-time size.
    SizeOf {
        /// The type whose memory size is produced.
        ty: TypeId,
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

/// Type information recorded for a checked `??` expression (§4.9.26).
#[derive(Clone, Copy, Debug)]
pub struct CoalesceInfo {
    /// The decision variant type (`Optional<T>` / `Result<...>` or a user type).
    pub decision_type: TypeId,
    /// The value type produced by the expression (`TValue`).
    pub result_type: TypeId,
    /// The alternative type carried when the decision is not `Value`.
    pub alternative_type: TypeId,
    /// State index of `Value`.
    pub value_state: u32,
    /// State index of `Alternative`/`Null`.
    pub alternative_state: u32,
    /// User-defined `operator ??` implementation, if the decision is user-made.
    pub operator: Option<u32>,
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
    /// A value-equality pattern lowered through a user-defined `==` (§5.9.6).
    Operator {
        /// Operator implementation.
        method: u32,
    },
    /// A `!=` pattern lowered through a user-defined `==` (§5.9.6).
    DerivedNotEqual {
        /// `==` implementation.
        method: u32,
    },
    /// A `<=`/`>=` pattern lowered through user-defined `<`/`>` and `==`.
    DerivedComparison {
        /// `<` or `>` implementation.
        ordering: u32,
        /// `==` implementation.
        equality: u32,
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
    /// Compile-time `#if` decisions, keyed by statement node.
    pub compile_time_conditions: HashMap<NodeId, bool>,
    /// Checked `??` expressions, keyed by expression node (§4.9.26).
    pub coalesce: HashMap<NodeId, CoalesceInfo>,
    /// Function literals, keyed by literal expression node (§2.7.9).
    pub function_literals: HashMap<NodeId, u32>,
    /// Ordered argument expressions of a call, after applying named arguments and
    /// default values (§4.5). Keyed by the call node.
    pub call_arguments: HashMap<NodeId, Vec<NodeId>>,
    /// Implicit conversion operators invoked at a value node (`implicitable`,
    /// §9.7). Keyed by the converted value expression.
    pub implicit_conversions: HashMap<NodeId, u32>,
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

    /// Returns the canonical spelling of a type (§13.2), using declared names
    /// at every nesting depth.
    #[must_use]
    pub fn spelling(&self, ty: TypeId) -> String {
        let mut out = String::new();
        self.write_spelling(ty, &mut out);
        out
    }

    fn write_spelling(&self, ty: TypeId, out: &mut String) {
        use std::fmt::Write as _;
        match self.types.get(ty) {
            Type::Named(index) => {
                out.push_str(&self.type_defs[*index as usize].qualified_name);
            }
            Type::Array {
                dimensions,
                element,
            } => {
                out.push_str("array(");
                for (position, dimension) in dimensions.iter().enumerate() {
                    if position > 0 {
                        out.push(',');
                    }
                    let _ = write!(out, "{dimension}");
                }
                out.push(')');
                self.write_spelling(*element, out);
            }
            Type::Pointer { target } => {
                out.push_str("pointer");
                if let Some(target) = target {
                    self.write_spelling(*target, out);
                }
            }
            Type::Function {
                parameters,
                wildcard,
                calling_convention,
                return_type,
            } => {
                out.push_str("function(");
                for (position, parameter) in parameters.iter().enumerate() {
                    if position > 0 {
                        out.push(',');
                    }
                    self.write_spelling(*parameter, out);
                }
                if *wildcard {
                    out.push_str(",wildcard");
                }
                if let Some(convention) = calling_convention {
                    let _ = write!(out, ";{convention}");
                }
                out.push(')');
                if let Some(return_type) = return_type {
                    self.write_spelling(*return_type, out);
                }
            }
            Type::Error | Type::Void => out.push_str("void"),
            Type::Boolean => out.push_str("boolean"),
            Type::Integer { signed, bytes } => {
                let name = if *signed { "integer" } else { "cardinal" };
                let _ = write!(out, "{name}({bytes})");
            }
            Type::Float { bytes } => {
                let _ = write!(out, "float({bytes})");
            }
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
    /// Module scopes whose members are imported by a `use` statement in this
    /// scope (§5.6). Later imports are searched first.
    imports: Vec<usize>,
}

/// What a name in a scope binds to.
#[derive(Clone, Debug)]
pub(crate) enum Symbol {
    Type(TypeId),
    Functions(Vec<u32>),
    /// Generic function templates; callable only through a monomorphization
    /// expression (§10.2).
    GenericFunctions(Vec<u32>),
    Local(LocalId),
    Global(u32),
    Module(usize),
    This {
        owner: TypeId,
    },
}

/// Runs semantic analysis over one parsed file.
///
/// Errors are reported through `engine`; the returned result is still usable for
/// code generation when `error_count` is zero.
pub fn analyze(
    ast: &mut Ast,
    sources: &SourceManager,
    engine: &mut DiagnosticEngine,
) -> SemaResult {
    let node_count = ast.nodes.len();
    let mut sema = Sema {
        ast,
        sources,
        engine,
        result: SemaResult {
            types: TypeStore::new(),
            node_types: vec![None; node_count],
            resolutions: vec![None; node_count],
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
        generic_env: Vec::new(),
        type_instantiations: HashMap::new(),
        instances_by_template: HashMap::new(),
        function_instantiations: HashMap::new(),
        pending_generic_extensions: Vec::new(),
        disposed_locals: std::collections::HashSet::new(),
        compile_time_constants: Vec::new(),
        initializer_depth: 0,
        bodies_checked: 0,
        constant_env: Vec::new(),
        constant_instantiations: HashMap::new(),
        heap_units: Vec::new(),
        heap_unit_locals: HashMap::new(),
        early_dispose_warned: std::collections::HashSet::new(),
    };
    sema.run();
    sema.result.error_count = sema.engine.error_count();
    sema.result
}

pub(crate) struct Sema<'a> {
    pub(crate) ast: &'a mut Ast,
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
    pub(crate) pending_accessors: Vec<PendingAccessor>,
    /// Lexical trap context stack for validating `throw` (§5.10).
    pub(crate) trap_contexts: Vec<TrapContextKind>,
    /// Active generic substitution environments (§10.1); outer scopes first.
    pub(crate) generic_env: Vec<HashMap<String, TypeId>>,
    /// Instantiated generic types, keyed by declaration node and arguments.
    pub(crate) type_instantiations: HashMap<(NodeId, Vec<TypeId>), TypeId>,
    /// Instantiation arguments of every generic type, by declaration node.
    pub(crate) instances_by_template: HashMap<NodeId, Vec<Vec<TypeId>>>,
    /// Instantiated generic functions, keyed by declaration node and arguments.
    pub(crate) function_instantiations: HashMap<(NodeId, Vec<TypeId>), u32>,
    /// Module-level extensions with generic targets (§3.8.3).
    pub(crate) pending_generic_extensions: Vec<PendingGenericExtension>,
    /// Locals that a `dispose` statement already disposed (§5.8), for the
    /// double-dispose warning.
    pub(crate) disposed_locals: std::collections::HashSet<LocalId>,
    /// Constant values of locals declared inside compile-time blocks (§11.4);
    /// inner blocks are searched first.
    pub(crate) compile_time_constants: Vec<HashMap<String, i128>>,
    /// Nesting depth of storage-initializer expressions; `uninitialized` is only
    /// legal there (§2.7.12).
    pub(crate) initializer_depth: u32,
    /// Number of function bodies already type-checked, so later passes can pick
    /// up literals discovered in initializers (§2.7.9).
    pub(crate) bodies_checked: usize,
    /// Compile-time constant bindings of `#` parameters (§11.5), innermost last.
    pub(crate) constant_env: Vec<HashMap<String, i128>>,
    /// Specializations of constant-parameter templates, keyed by declaration and
    /// constant argument tuple.
    pub(crate) constant_instantiations: HashMap<(NodeId, Vec<i128>), u32>,
    /// Heap allocation units of the function body being checked (§12.2).
    pub(crate) heap_units: Vec<HeapUnit>,
    /// Index into `heap_units` for each heap local.
    pub(crate) heap_unit_locals: HashMap<u32, usize>,
    /// Locals already reported for use-after-dispose (§12.2).
    pub(crate) early_dispose_warned: std::collections::HashSet<u32>,
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

    /// Reports a non-fatal warning.
    pub(crate) fn warn(&mut self, span: Span, message: impl Into<String>) {
        if self.suppress_errors == 0 {
            let mut stderr = std::io::stderr();
            let _ = self.engine.report(
                &mut stderr,
                self.sources,
                Severity::Warning,
                Some(span),
                &message.into(),
            );
        }
    }

    /// Reports an informational note.
    pub(crate) fn note(&mut self, span: Span, message: impl Into<String>) {
        if self.suppress_errors == 0 {
            let mut stderr = std::io::stderr();
            let _ = self.engine.report(
                &mut stderr,
                self.sources,
                Severity::Note,
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

    /// The host platform kind as the numeric value of `PlatformKind` (§15.2).
    pub(crate) fn host_platform_kind() -> u32 {
        let triple = crate::HOST_TRIPLE.to_ascii_lowercase();
        if triple.contains("windows") {
            1
        } else if triple.contains("linux") {
            2
        } else if triple.contains("darwin") || triple.contains("macos") || triple.contains("apple")
        {
            3
        } else if triple.contains("freebsd") {
            4
        } else {
            0
        }
    }

    /// Resolves a canonical platform name to a `PlatformKind` value (§15.3).
    /// Canonical names are matched byte-exactly and are case-sensitive; `Other`
    /// has no canonical name, so unknown names map to `None` (the query answers
    /// false rather than reporting an error).
    pub(crate) fn platform_kind_from_name(name: &str) -> Option<u32> {
        match name {
            "Windows" => Some(1),
            "Linux" => Some(2),
            "MacOS" => Some(3),
            "FreeBSD" => Some(4),
            _ => None,
        }
    }

    /// The `levels`-th ancestor of `scope` (its parent for 1), if it exists.
    pub(crate) fn ancestor_scope(&self, mut scope: usize, levels: usize) -> Option<usize> {
        for _ in 0..levels {
            scope = self.scopes[scope].parent?;
        }
        Some(scope)
    }

    fn create_child_scope(&mut self) -> usize {
        let parent = self.current_scope();
        let id = self.scopes.len();
        self.scopes.push(Scope {
            parent: Some(parent),
            symbols: HashMap::new(),
            imports: Vec::new(),
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

    /// Declares a generic function template so that it can be found by name and
    /// instantiated by a monomorphization expression (§10.2).
    pub(crate) fn declare_generic_functions(&mut self, name: String, function: u32) {
        let scope = self.current_scope();
        match self.scopes[scope].symbols.get_mut(&name) {
            Some(Symbol::GenericFunctions(list)) => list.push(function),
            Some(_) => {
                let span = self.result.functions[function as usize].span;
                self.error(
                    span,
                    format!("'{name}' mixes generic and non-generic declarations in one scope"),
                );
            }
            None => {
                self.scopes[scope]
                    .symbols
                    .insert(name, Symbol::GenericFunctions(vec![function]));
            }
        }
    }

    pub(crate) fn declare_functions(&mut self, name: String, function: u32) {
        let scope = self.current_scope();
        // Two functions with the same name and the same parameter types are an
        // invalid overload set (§8.5).
        let parameters = self.result.functions[function as usize].parameters.clone();
        if let Some(Symbol::Functions(list)) = self.scopes[scope].symbols.get(&name) {
            let duplicate = list.iter().any(|existing| {
                let existing_parameters =
                    self.result.functions[*existing as usize].parameters.clone();
                existing_parameters.len() == parameters.len()
                    && existing_parameters
                        .iter()
                        .zip(parameters.iter())
                        .all(|(left, right)| left.ty == right.ty)
            });
            if duplicate {
                let span = self.result.functions[function as usize].span;
                self.error(span, format!("duplicate function declaration '{name}'"));
            }
        }
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
            for imported in self.scopes[index].imports.iter().rev() {
                if let Some(symbol) = self.scopes[*imported].symbols.get(name) {
                    return Some(symbol);
                }
            }
            scope = self.scopes[index].parent;
        }
        None
    }

    /// Registers the imports of a `use` statement in the current scope (§5.6).
    ///
    /// Imports only affect the statements that follow the `use` in the same
    /// scope, which holds naturally because statements are checked in order.
    pub(crate) fn resolve_use_statement(&mut self, statement: &UseStmt) {
        for target in &statement.targets {
            let span = self.span_of(*target);
            match self.try_resolve_name(*target) {
                Some(Symbol::Module(scope)) => {
                    let current = self.current_scope();
                    if !self.scopes[current].imports.contains(&scope) {
                        self.scopes[current].imports.push(scope);
                    }
                }
                Some(Symbol::Type(_)) => {
                    self.unsupported(span, "use of a type's static members");
                }
                _ => self.error(span, "'use' requires a module name"),
            }
        }
    }

    pub(crate) fn lookup_in(&self, scope: usize, name: &str) -> Option<&Symbol> {
        self.scopes[scope].symbols.get(name)
    }

    // -- passes ------------------------------------------------------------

    fn run(&mut self) {
        self.declare_builtin_variants();
        self.declare_builtin_exceptions();
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
        // Initializers may contain function literals; check their bodies too.
        self.check_function_bodies();
        // Monomorphization can happen while bodies and initializers are checked;
        // instances created there still need layouts and virtual dispatch.
        self.finalize_variant_union_layouts();
        self.build_vtables();
        self.check_entry_point();
        self.check_constructor_chains();
    }

    /// Pushes a synthetic class declaration (used by the Built-In Library).
    fn push_builtin_class(
        &mut self,
        name: &str,
        base: Option<TypeId>,
        is_abstract: bool,
    ) -> TypeId {
        let index = self.result.type_defs.len() as TypeDefId;
        let node = NodeId::MAX - 100 - index;
        self.result.type_defs.push(TypeDef {
            name: name.to_string(),
            qualified_name: name.to_string(),
            kind: TypeDefKind::Class,
            node,
            generic_parameters: Vec::new(),
            generic_arguments: Vec::new(),
            template: None,
            decl_scope: 0,
            static_fields: std::collections::HashMap::new(),
            static_methods: std::collections::HashMap::new(),
            builtin_variant: None,
            fields: Vec::new(),
            field_initializers: Vec::new(),
            methods: std::collections::HashMap::new(),
            method_list: Vec::new(),
            accessors: Vec::new(),
            operators: Vec::new(),
            interfaces: Vec::new(),
            interface_dispatch: Vec::new(),
            sealed: false,
            is_abstract,
            is_static: false,
            vtable: Vec::new(),
            vtable_ready: true,
            base,
            variant_states: Vec::new(),
            union_members: Vec::new(),
            tag_type: None,
            payload_offset: 0,
            payload_size: 0,
            instance_size: 16,
            instance_align: 8,
            enum_members: Vec::new(),
            underlying: None,
            size: 8,
            align: 8,
        });
        let type_id = self.result.types.intern(Type::Named(index));
        self.declare(name.to_string(), Symbol::Type(type_id), Span::default());
        type_id
    }

    /// Pushes a synthetic enumeration declaration (used by the Built-In Library).
    fn push_builtin_enumeration(
        &mut self,
        name: &str,
        signed: bool,
        bytes: u8,
        members: &[&str],
    ) -> TypeId {
        let index = self.result.type_defs.len() as TypeDefId;
        let node = NodeId::MAX - 200 - index;
        let underlying = self.result.types.integer(signed, bytes);
        let size = u64::from(bytes);
        self.result.type_defs.push(TypeDef {
            name: name.to_string(),
            qualified_name: name.to_string(),
            kind: TypeDefKind::Enumeration,
            node,
            generic_parameters: Vec::new(),
            generic_arguments: Vec::new(),
            template: None,
            decl_scope: 0,
            static_fields: std::collections::HashMap::new(),
            static_methods: std::collections::HashMap::new(),
            builtin_variant: None,
            fields: Vec::new(),
            field_initializers: Vec::new(),
            methods: std::collections::HashMap::new(),
            method_list: Vec::new(),
            accessors: Vec::new(),
            operators: Vec::new(),
            interfaces: Vec::new(),
            interface_dispatch: Vec::new(),
            sealed: false,
            is_abstract: false,
            is_static: false,
            vtable: Vec::new(),
            vtable_ready: true,
            base: None,
            variant_states: Vec::new(),
            union_members: Vec::new(),
            tag_type: None,
            payload_offset: 0,
            payload_size: 0,
            instance_size: 0,
            instance_align: 1,
            enum_members: members
                .iter()
                .enumerate()
                .map(|(position, member)| ((*member).to_string(), position as i128))
                .collect(),
            underlying: Some(underlying),
            size,
            align: size,
        });
        let type_id = self.result.types.intern(Type::Named(index));
        self.declare(name.to_string(), Symbol::Type(type_id), Span::default());
        type_id
    }

    /// Declares the Built-In Library exception class hierarchy (§15.2):
    /// `Exception` and the runtime exception subclasses.
    fn declare_builtin_exceptions(&mut self) {
        // Built-In Library roots (§15.2): the Object class and the two built-in
        // enumerations.
        let object = self.push_builtin_class("Object", None, true);
        self.push_builtin_enumeration(
            "CallingConvention",
            true,
            4,
            &[
                "PlatformDefault",
                "CDeclaration",
                "StandardCall",
                "FastCall",
                "SystemVAMD64",
                "ThisCall",
                "VectorCall",
                "MicrosoftX64",
            ],
        );
        self.push_builtin_enumeration(
            "PlatformKind",
            false,
            4,
            &["Other", "Windows", "Linux", "MacOS", "FreeBSD"],
        );
        const NAMES: [&str; 6] = [
            "Exception",
            "NotImplementedException",
            "DivideByZeroException",
            "OutOfRangeException",
            "AllocateFailedException",
            "DeallocateFailedException",
        ];
        for name in NAMES {
            let index = self.result.type_defs.len() as TypeDefId;
            self.result.type_defs.push(TypeDef {
                name: name.to_string(),
                qualified_name: name.to_string(),
                kind: TypeDefKind::Class,
                // Synthetic declaration: must not collide with a real AST node
                // (which `type_id_for_node` matches on).
                node: NodeId::MAX - 300 - index,
                generic_parameters: Vec::new(),
                generic_arguments: Vec::new(),
                template: None,
                decl_scope: 0,
                static_fields: std::collections::HashMap::new(),
                static_methods: std::collections::HashMap::new(),
                builtin_variant: None,
                fields: Vec::new(),
                field_initializers: Vec::new(),
                methods: std::collections::HashMap::new(),
                method_list: Vec::new(),
                accessors: Vec::new(),
                operators: Vec::new(),
                interfaces: Vec::new(),
                interface_dispatch: Vec::new(),
                sealed: false,
                is_abstract: false,
                is_static: false,
                vtable: Vec::new(),
                vtable_ready: true,
                base: None,
                variant_states: Vec::new(),
                union_members: Vec::new(),
                tag_type: None,
                payload_offset: 0,
                payload_size: 0,
                instance_size: 16,
                instance_align: 8,
                enum_members: Vec::new(),
                underlying: None,
                size: 8,
                align: 8,
            });
            let type_id = self.result.types.intern(Type::Named(index));
            self.declare(name.to_string(), Symbol::Type(type_id), Span::default());
        }
        let exception = self.lookup("Exception").cloned();
        if let Some(Symbol::Type(exception)) = exception {
            if let Type::Named(index) = self.result.types.get(exception) {
                self.result.type_defs[*index as usize].base = Some(object);
            }
            for name in &NAMES[1..] {
                if let Some(Symbol::Type(type_id)) = self.lookup(name).cloned() {
                    if let Type::Named(index) = self.result.types.get(type_id) {
                        self.result.type_defs[*index as usize].base = Some(exception);
                    }
                }
            }
            // Exception.InnerException: Optional<Exception> (§15.2), inherited by
            // every built-in exception class (fields are flattened per class).
            let optional_exception = self
                .lookup("Optional")
                .cloned()
                .and_then(|symbol| match symbol {
                    Symbol::Type(ty) => match self.result.types.get(ty) {
                        Type::Named(index) => Some(self.result.type_defs[*index as usize].node),
                        _ => None,
                    },
                    _ => None,
                })
                .and_then(|template_node| {
                    self.instantiate_type(template_node, vec![exception], Span::default())
                });
            if let Some(optional_exception) = optional_exception {
                for name in NAMES {
                    let Some(Symbol::Type(type_id)) = self.lookup(name).cloned() else {
                        continue;
                    };
                    let Type::Named(index) = self.result.types.get(type_id) else {
                        continue;
                    };
                    let definition = &mut self.result.type_defs[*index as usize];
                    definition.fields = vec![FieldInfo {
                        name: "InnerException".to_string(),
                        ty: optional_exception,
                        offset: 16,
                    }];
                    definition.field_initializers = vec![None];
                    definition.instance_size = 32;
                    definition.instance_align = 8;
                }
            }
        }
    }

    /// An executable program declares exactly one `Main` function (§14.1).
    fn check_entry_point(&mut self) {
        let mut entry = None;
        for index in 0..self.result.functions.len() {
            let function = &self.result.functions[index];
            if function.is_template() || function.method_of.is_some() || function.name != "Main" {
                continue;
            }
            match entry {
                None => entry = Some(index),
                Some(_) => {
                    let span = function.span;
                    self.error(span, "a program must declare exactly one 'Main' function");
                }
            }
        }
    }

    /// Validates constructor chains (§3.7.1).
    ///
    /// When a base class constructor has parameters or a non-empty body, every
    /// derived class must declare a constructor, and each constructor must call
    /// the base constructor with `base.Construct(...)` as its first statement.
    fn check_constructor_chains(&mut self) {
        for index in 0..self.result.type_defs.len() {
            let definition = &self.result.type_defs[index];
            if definition.kind != TypeDefKind::Class || !definition.generic_parameters.is_empty() {
                continue;
            }
            let Some(base) = definition.base else {
                continue;
            };
            let Type::Named(base_index) = self.result.types.get(base) else {
                continue;
            };
            let base_index = *base_index as usize;
            let base_constructors = self.result.type_defs[base_index]
                .methods
                .get("Construct")
                .cloned()
                .unwrap_or_default();
            let base_is_non_trivial = base_constructors.iter().any(|constructor| {
                let function = &self.result.functions[*constructor as usize];
                function.parameters.len() > 1
                    || match function.body {
                        Some(FunctionBody::Block(block)) => !self.block_is_empty(block),
                        _ => true,
                    }
            });
            if !base_is_non_trivial {
                continue;
            }
            let class_name = self.result.type_defs[index].qualified_name.clone();
            let class_node = self.result.type_defs[index].node;
            let constructors = self.result.type_defs[index]
                .methods
                .get("Construct")
                .cloned()
                .unwrap_or_default();
            if constructors.is_empty() {
                self.error(
                    self.span_of(class_node),
                    format!(
                        "class '{class_name}' must declare a constructor because its base \
                         class constructor is not empty"
                    ),
                );
                continue;
            }
            for constructor in constructors {
                let function = &self.result.functions[constructor as usize];
                let span = function.span;
                let starts_with_base_call = match function.body {
                    Some(FunctionBody::Block(block)) => match self.node_kind(block) {
                        NodeKind::Block(statements) => statements
                            .first()
                            .is_some_and(|statement| self.is_base_construct_call(*statement)),
                        _ => false,
                    },
                    _ => false,
                };
                if !starts_with_base_call {
                    self.error(
                        span,
                        "a constructor must call the base class constructor with \
                         'base.Construct(...)' as its first statement",
                    );
                }
            }
        }
    }

    /// Whether a statement is a call of the form `base.Construct(...)`.
    fn is_base_construct_call(&self, statement: NodeId) -> bool {
        let NodeKind::ExpressionStmt(expression) = self.node_kind(statement) else {
            return false;
        };
        let NodeKind::Call(call) = self.node_kind(expression) else {
            return false;
        };
        let NodeKind::MemberAccess(access) = self.node_kind(call.callee) else {
            return false;
        };
        access.name == "Construct" && matches!(self.node_kind(access.base), NodeKind::Base)
    }

    /// Whether a block has no statements.
    fn block_is_empty(&self, block: NodeId) -> bool {
        matches!(self.node_kind(block), NodeKind::Block(statements) if statements.is_empty())
    }

    /// Declares the Built-In Library generic variants `Optional<T>` and
    /// `Result<TValue; TAlternative>` (§15.2) as templates. Their instances are
    /// built directly, without AST cloning: `instantiate_builtin_variant`.
    fn declare_builtin_variants(&mut self) {
        for (name, kind, parameters) in [
            ("Optional", BuiltinVariantKind::Optional, vec!["T"]),
            (
                "Result",
                BuiltinVariantKind::Result,
                vec!["TValue", "TAlternative"],
            ),
        ] {
            let index = self.result.type_defs.len() as TypeDefId;
            self.result.type_defs.push(TypeDef {
                name: name.to_string(),
                qualified_name: name.to_string(),
                kind: TypeDefKind::Variant,
                // Unique synthetic node; also used as the instantiation-cache key
                // (`Sema::ast_placeholder_node`).
                node: NodeId::MAX - 1 - index,
                generic_parameters: parameters
                    .iter()
                    .map(|parameter| GenericParameterInfo {
                        name: (*parameter).to_string(),
                        constraints: Vec::new(),
                        span: Span::default(),
                    })
                    .collect(),
                generic_arguments: Vec::new(),
                template: None,
                decl_scope: 0,
                static_fields: std::collections::HashMap::new(),
                static_methods: std::collections::HashMap::new(),
                builtin_variant: Some(kind),
                fields: Vec::new(),
                field_initializers: Vec::new(),
                methods: std::collections::HashMap::new(),
                method_list: Vec::new(),
                accessors: Vec::new(),
                operators: Vec::new(),
                interfaces: Vec::new(),
                interface_dispatch: Vec::new(),
                sealed: false,
                is_abstract: false,
                is_static: false,
                vtable: Vec::new(),
                vtable_ready: true,
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
            self.declare(name.to_string(), Symbol::Type(type_id), Span::default());
        }
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
        let generic_parameters: Vec<GenericParameterInfo> = match self.node_kind(node) {
            NodeKind::Class(declaration)
            | NodeKind::Structure(declaration)
            | NodeKind::Interface(declaration) => declaration
                .generics
                .iter()
                .map(GenericParameterInfo::from_ast)
                .collect(),
            NodeKind::Variant(declaration) => declaration
                .generics
                .iter()
                .map(GenericParameterInfo::from_ast)
                .collect(),
            NodeKind::Union(declaration) => declaration
                .generics
                .iter()
                .map(GenericParameterInfo::from_ast)
                .collect(),
            _ => Vec::new(),
        };
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
            generic_parameters,
            generic_arguments: Vec::new(),
            template: None,
            decl_scope: self.current_scope(),
            static_fields: std::collections::HashMap::new(),
            static_methods: std::collections::HashMap::new(),
            builtin_variant: None,
            fields: Vec::new(),
            field_initializers: Vec::new(),
            methods: std::collections::HashMap::new(),
            method_list: Vec::new(),
            accessors: Vec::new(),
            operators: Vec::new(),
            interfaces: Vec::new(),
            interface_dispatch: Vec::new(),
            sealed: false,
            is_abstract: false,
            is_static: false,
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
            // Generic declarations are templates: their members are resolved
            // per instantiation (§10.1).
            if self.is_generic_type_declaration(node) {
                continue;
            }
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
                NodeKind::Fixed(declaration) => {
                    if declaration.generics.is_empty() {
                        self.resolve_function_signature(node, &declaration);
                    } else {
                        self.declare_generic_function(node, &declaration);
                    }
                }
                NodeKind::Linkable(declaration) => {
                    if declaration.generics.is_empty() {
                        self.resolve_linkable_signature(node, &declaration);
                    } else {
                        self.declare_generic_linkable(node, &declaration);
                    }
                }
                NodeKind::Local(_) => self.resolve_global_local(node),
                NodeKind::Extension(_) => self.resolve_extension_declaration(node, None),
                NodeKind::Use(statement) => self.resolve_use_statement(&statement),
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
