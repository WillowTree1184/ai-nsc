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
use self::check::{
    builtin_compiler_function, builtin_namespace_call, builtin_size_of_type_argument,
};
mod generics;
pub mod types;

pub(crate) use generics::MonomorphizedTarget;

use std::collections::{HashMap, HashSet};

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

/// Kind of built-in generic interface (§15.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BuiltinInterfaceKind {
    /// `interface IIterator<T>` with `Next` and `Reset`.
    Iterator,
    /// `interface IIterable<T>` with an `Iterator` getter.
    Iterable,
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

/// One resolved `scoped(...)` accessibility target (§9.5).
#[derive(Clone, Debug)]
pub struct ScopedInfo {
    /// What the target clips to.
    pub kind: ScopedKind,
    /// Whether the target's inherited items are cancelled.
    pub sealed: bool,
}

/// Resolved form of a `scoped` target (§9.5).
#[derive(Clone, Debug)]
pub enum ScopedKind {
    /// A named type target.
    Type(TypeId),
    /// The declaring type's operators.
    Operator,
    /// The declaring type's operators for one symbol/conversion target (§9.5).
    OperatorSymbol(String),
    /// A member (and its overrides); resolved after declarations.
    Member(u32),
    /// A member name awaiting declaration resolution.
    Pending(String),
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
    /// Base accessibility of the field (§9.1); `Unspecified` is private.
    pub accessibility: crate::ast::Accessibility,
    /// Whether the field is `readonly` (§9.6).
    pub readonly: bool,
    /// Accessibility clipping targets (§9.5).
    pub scoped: Vec<ScopedInfo>,
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
    /// Built-in generic interface kind, for `IIterator`/`IIterable` (§15.2).
    pub builtin_interface: Option<BuiltinInterfaceKind>,
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
    /// Static accessors (§3.8.2, §9.15).
    pub static_accessors: Vec<AccessorInfo>,
    /// Modules declared inside the type body (identifier organization only,
    /// §7): module name to scope index.
    pub modules: HashMap<String, usize>,
    /// Operator definitions declared in extension blocks.
    pub operators: Vec<OperatorInfo>,
    /// Interfaces this class implements (§3.7.3/§3.7.7).
    pub interfaces: Vec<TypeId>,
    /// Interfaces this interface extends (§3.7.3).
    pub interface_bases: Vec<TypeId>,
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
    /// Whether the parameter list ends with a `wildcard` (§3.6.3).
    pub wildcard: bool,
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
    /// Base accessibility of the function (§9.1).
    pub accessibility: crate::ast::Accessibility,
    /// Explicit calling convention, if declared (§3.6.3).
    pub calling_convention: Option<i32>,
    /// Accessibility clipping targets (§9.5).
    pub scoped: Vec<ScopedInfo>,
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
    /// Built-In Library `DefaultAllocator.Allocate` (§15.2).
    BuiltinAllocate,
    /// Built-In Library `DefaultAllocator.Deallocate` (§15.2).
    BuiltinDeallocate,
    /// Read a module-level storage from a free accessor (§3.8.2).
    FreeBackingGet(u32),
    /// Write a module-level storage from a free accessor (§3.8.2).
    FreeBackingSet(u32),
    /// Read a local backing storage through a by-reference parameter (§3.8.2).
    LocalBackingGet {
        /// Parameter index of the backing pointer.
        parameter: u32,
    },
    /// Write a local backing storage through a by-reference parameter (§3.8.2).
    LocalBackingSet {
        /// Parameter index of the backing pointer.
        parameter: u32,
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
    /// Accessibility clipping targets (§9.5).
    pub scoped: Vec<ScopedInfo>,
    /// Base accessibility of the accessor (§9.1).
    pub accessibility: crate::ast::Accessibility,
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

/// Iterator-based `for` iteration (§5.6).
#[derive(Clone, Copy, Debug)]
pub struct ForIteratorInfo {
    /// Expression producing the iterable/iterator value.
    pub target: NodeId,
    /// Interface used to dispatch `Next` (an `IIterator<T>` instance).
    pub iterator_interface: TypeId,
    /// Whether the target must first be converted through the `Iterator` accessor
    /// (`IIterable<T>`).
    pub use_accessor: bool,
    /// Interface of the iterable target, when `use_accessor` is set.
    pub iterable_interface: Option<TypeId>,
    /// Payload type of `Next()`.
    pub element: TypeId,
}

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
    /// Whether the unit was returned to the caller (ownership transfer).
    pub(crate) returned: bool,
    /// Whether the unit was passed to a `linkable` call, which makes NOLAA
    /// abandon its lifetime analysis (§12.2) and report it as never disposed.
    pub(crate) linkable_abandoned: bool,
    /// The local holding the heap pointer.
    pub(crate) local: LocalId,
    /// Block containing the declaration.
    pub(crate) block: NodeId,
}

/// A module-level variable or constant.
#[derive(Debug)]
pub struct GlobalInfo {
    /// Declared name.
    pub name: String,
    /// Base accessibility of the storage (§9.1).
    pub accessibility: crate::ast::Accessibility,
    /// Whether the storage is declared `verbatim` (§9.8).
    pub verbatim: bool,
    /// Accessibility clipping targets (§9.5).
    pub scoped: Vec<ScopedInfo>,
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
    /// A free accessor read or write (§3.8.2).
    FreeAccessor {
        /// Getter function, if the accessor provides one.
        getter: Option<u32>,
        /// Setter function, if the accessor provides one.
        setter: Option<u32>,
        /// Local variable backing the accessor, passed by reference (§3.8.2).
        local_backing: Option<LocalId>,
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
    /// A top-level compile-time variable (§11.4). The final value is looked up
    /// at code-generation time so that re-execution (interpreter/JIT) is visible.
    TopLevelConstant {
        /// Variable name.
        name: String,
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
    /// `is illegal` pattern over a floating-point value.
    IsIllegal,
    /// `is infinity` / `is -infinity` pattern over a floating-point value.
    IsInfinite {
        /// Whether the negative infinity is tested.
        negative: bool,
    },
}

/// An accessor declared outside type bodies and extension blocks (§3.8.2):
/// its members are function constants and its name acts as the property.
#[derive(Clone, Debug)]
pub struct FreeAccessorInfo {
    /// Accessor name.
    pub name: String,
    /// Scope path where it was declared.
    pub scope: Vec<String>,
    /// Base accessibility.
    pub accessibility: Accessibility,
    /// Property type.
    pub ty: TypeId,
    /// Getter function constant.
    pub getter: Option<u32>,
    /// Setter function constant.
    pub setter: Option<u32>,
    /// Local variable backing the accessor, passed by reference (§3.8.2).
    pub local_backing: Option<LocalId>,
}

/// An `extension for array(length: cardinal(4)) T` template (§3.8.3).
#[derive(Clone, Debug)]
pub struct ArrayExtensionTemplate {
    /// The extension declaration node.
    pub extension: NodeId,
    /// Bound dimension names and their bound types.
    pub bindings: Vec<(String, TypeId)>,
    /// The element type parameter.
    pub parameter: GenericParameterInfo,
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
    /// Explicit heap allocator expressions, keyed by the heap statement node
    /// (§5.4).
    pub heap_allocators: HashMap<NodeId, NodeId>,
    /// Explicit heap allocator used for each heap local, for deallocation.
    pub heap_allocator_locals: HashMap<LocalId, NodeId>,
    /// Iteration over `IIterator`/`IIterable` targets, keyed by the `for` node
    /// (§5.6).
    pub for_iterators: HashMap<NodeId, ForIteratorInfo>,
    /// Index of the platform-provided `DefaultAllocator` global (§5.4).
    pub default_allocator_global: Option<u32>,
    /// Heap statements that use `DefaultAllocator` (allocator omitted).
    pub heap_default_allocator: std::collections::HashSet<NodeId>,
    /// Heap locals allocated through `DefaultAllocator`.
    pub heap_default_allocator_locals: std::collections::HashSet<u32>,
    /// Heap locals that NOLAA disposes at the end of a block (§12.2).
    pub auto_dispose: HashMap<NodeId, Vec<LocalId>>,
    /// Nested function declarations inside bodies (§5.3), mapped to the
    /// function literal expression that creates their value.
    pub nested_functions: HashMap<NodeId, NodeId>,
    /// Heap locals NOLAA disposes before a `return` jumps out of their scope
    /// (§12.2), keyed by the return statement.
    pub jump_dispose: HashMap<NodeId, Vec<LocalId>>,
    /// Integer values of compile-time expressions that can be emitted as
    /// constants (§11.4: keep only the required end products).
    pub compile_time_values: HashMap<NodeId, i128>,
    /// `#{}` blocks that have no run-time end products and need no code
    /// generation (§11.4).
    pub compile_time_only_blocks: HashSet<NodeId>,
    /// Locals whose declaring compile-time block is dropped: code generation must
    /// not allocate storage for them (§11.4).
    pub dead_locals: HashSet<(u32, LocalId)>,
    /// Locals referencing a heap unit returned by another function with the
    /// default allocator (§5.8/§12.2).
    pub heap_reference_locals: HashSet<LocalId>,
    /// Locals referencing a returned explicit-allocator heap class instance.
    pub heap_custom_reference_locals: HashSet<LocalId>,
    /// Methods belonging to non-named owners (array extensions, §3.8.3).
    pub owner_methods: HashMap<TypeId, Vec<u32>>,
    /// Accessors belonging to non-named owners (array extensions, §3.8.3).
    pub owner_accessors: HashMap<TypeId, Vec<AccessorInfo>>,
    /// Operators belonging to non-named owners (array extensions, §3.8.3).
    pub owner_operators: HashMap<TypeId, Vec<OperatorInfo>>,
    /// User-defined operator implementations selected for compound assignments
    /// (§5.7), keyed by the assignment statement.
    pub compound_operators: HashMap<NodeId, u32>,
    /// Accessors declared outside type bodies and extension blocks (§3.8.2).
    pub free_accessors: Vec<FreeAccessorInfo>,
    /// `extension for array(...)` templates.
    pub array_extension_templates: Vec<ArrayExtensionTemplate>,
    /// Instantiated array extension methods per target array type.
    pub array_extension_instances: HashMap<(NodeId, TypeId), Vec<u32>>,
    /// Final values of top-level compile-time variables (§11.4).
    pub compile_time_variables: HashMap<String, (i128, TypeId)>,
    /// Arguments matched by a function's trailing `wildcard` (§3.6.3).
    pub wildcard_arguments: HashMap<NodeId, Vec<NodeId>>,
    /// Number of errors reported.
    pub error_count: u32,
}

impl SemaResult {
    /// Size and alignment of a type in bytes.
    ///
    /// Panics only on an internal invariant violation (missing named type).
    #[must_use]
    pub fn size_align(&self, ty: TypeId) -> (u64, u64) {
        let layout = self.types.layout();
        match self.types.get(ty) {
            Type::Error | Type::Void => (0, 1),
            Type::Boolean => (1, 1),
            Type::Integer { bytes, .. } => {
                let size = u64::from(*bytes);
                // 64-bit integers on i686 are aligned to 4 bytes (§13).
                let align = if *bytes == 8 && layout.integer64_align == 4 {
                    4
                } else {
                    size
                };
                (size, align)
            }
            Type::Float { bytes } => {
                let size = u64::from(*bytes);
                let align = if *bytes == 8 && layout.float64_align == 4 {
                    4
                } else {
                    size
                };
                (size, align)
            }
            Type::Array {
                dimensions,
                element,
            } => {
                let (element_size, element_align) = self.size_align(*element);
                let count: u64 = dimensions.iter().product();
                (element_size * count, element_align)
            }
            Type::Vla { length, .. } => {
                let pointer = u64::from(layout.pointer_size);
                let (length_size, length_align) = self.size_align(*length);
                let align = pointer.max(length_align);
                let offset = round_up_size(pointer, length_align.max(1));
                let size = round_up_size(offset + length_size, align);
                (size, align)
            }
            Type::Pointer { .. } | Type::Function { .. } => {
                let pointer = u64::from(layout.pointer_size);
                (pointer, pointer)
            }
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
            Type::Vla { element, .. } => {
                out.push_str("array(runtime)");
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
    /// An accessor declared outside type bodies and extension blocks (§3.8.2).
    FreeAccessor(u32),
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
    analyze_with_layout(
        ast,
        sources,
        engine,
        crate::sema::types::TargetLayout::HOST64,
    )
}

/// Runs semantic analysis for an explicit target data layout.
pub fn analyze_with_layout(
    ast: &mut Ast,
    sources: &SourceManager,
    engine: &mut DiagnosticEngine,
    layout: crate::sema::types::TargetLayout,
) -> SemaResult {
    analyze_with_options(ast, sources, engine, layout, true, false, None)
}

/// Runs semantic analysis for an explicit target layout and host-JIT choice.
pub fn analyze_with_options(
    ast: &mut Ast,
    sources: &SourceManager,
    engine: &mut DiagnosticEngine,
    layout: crate::sema::types::TargetLayout,
    jit_allowed: bool,
    freestanding: bool,
    target_triple: Option<&str>,
) -> SemaResult {
    let node_count = ast.nodes.len();
    let mut sema = Sema {
        ast,
        sources,
        engine,
        result: SemaResult {
            types: TypeStore::with_layout(layout),
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
        heap_alias_units: HashMap::new(),
        heap_returning_functions: HashSet::new(),
        heap_custom_return_functions: HashSet::new(),
        heap_reference_locals: HashSet::new(),
        heap_custom_reference_locals: HashSet::new(),
        heap_demand_edges: HashMap::new(),
        heap_unit_locals: HashMap::new(),
        early_dispose_warned: std::collections::HashSet::new(),
        current_owner: None,
        current_block: NodeId::default(),
        in_function_body: false,
        expected_array_element: None,
        top_level_constants: HashMap::new(),
        compile_time_heap: HashMap::new(),
        two_phase_future_fixed: std::collections::HashSet::new(),
        two_phase_future_locals: std::collections::HashSet::new(),
        two_phase_pending: Vec::new(),
        compile_time_block_stack: Vec::new(),
        compile_time_block_local_owners: HashMap::new(),
        compile_time_block_records: Vec::new(),
        in_verbatim: false,
        compile_time_depth: 0,
        compile_time_expression_depth: 0,
        compile_time_local_ids: Vec::new(),
        in_constant_initializer: false,
        compile_call_depth: 0,
        top_level_blocks: Vec::new(),
        top_level_units: Vec::new(),
        unknown_compile_time_names: std::collections::HashSet::new(),
        jit_allowed,
        freestanding,
        target_platform_kind: Sema::platform_kind_for_triple(target_triple),
        compile_time_phase_b: false,
        compile_time_deferred: false,
        compile_time_aborted: false,
        array_binding_lengths: Vec::new(),
        heap_groups: Vec::new(),
        pending_assume_no_demand: None,
        active_assume_no_demand: None,
        block_stack: Vec::new(),
        loop_blocks: Vec::new(),
    };
    sema.run();
    sema.result.error_count = sema.engine.error_count();
    sema.result
}

/// A deferred top-level/module `#if`, together with the scope that must receive
/// its selected branch's declarations (§11.7).
#[derive(Clone, Debug)]
struct PendingCompileTimeIf {
    node: NodeId,
    scope_stack: Vec<usize>,
    scope_path: Vec<String>,
}

/// Control flow produced by a compile-time statement (§11.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CompileTimeFlow {
    Normal,
    Break,
    Continue,
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
    /// Alias locals mapped to their heap allocation unit (§12.2).
    pub(crate) heap_alias_units: HashMap<LocalId, u32>,
    /// Functions whose returned reference points to a heap unit allocated with
    /// the default allocator (§5.8/§12.2 interprocedural ownership).
    pub(crate) heap_returning_functions: HashSet<u32>,
    /// Functions returning heap units allocated through an explicit allocator.
    pub(crate) heap_custom_return_functions: HashSet<u32>,
    /// Locals known to reference a returned default-allocator heap unit.
    pub(crate) heap_reference_locals: HashSet<LocalId>,
    /// Locals known to reference a returned custom-allocator heap unit.
    pub(crate) heap_custom_reference_locals: HashSet<LocalId>,
    /// Demand edges between heap units discovered from field stores (§12.2).
    pub(crate) heap_demand_edges: HashMap<u32, Vec<u32>>,
    /// Index into `heap_units` for each heap local.
    pub(crate) heap_unit_locals: HashMap<u32, usize>,
    /// Locals already reported for use-after-dispose (§12.2).
    pub(crate) early_dispose_warned: std::collections::HashSet<u32>,
    /// Type whose body is currently being checked, for accessibility (§9.1).
    pub(crate) current_owner: Option<TypeId>,
    /// Block whose statements are currently being checked (§12.2).
    pub(crate) current_block: NodeId,
    /// Whether an array type construct is inside a function body, where runtime
    /// dimensions are allowed (VLA, §3.6.1).
    pub(crate) in_function_body: bool,
    /// Element type expected for array literals without an explicit expected
    /// array type (e.g. `for` targets, §5.9.5).
    pub(crate) expected_array_element: Option<TypeId>,
    /// Compile-time variables produced by top-level `#{}` blocks, in execution
    /// order (§11.2, §11.4).
    pub(crate) top_level_constants: HashMap<String, (i128, TypeId)>,
    /// Scalar heap units allocated during compile-time execution (§5.4/§11.8);
    /// they are freed when the compile-time execution ends and never enter the
    /// runtime artifact.
    pub(crate) compile_time_heap: HashMap<String, (i128, TypeId)>,
    /// Immutable bindings produced by `#{}` blocks anywhere in the program,
    /// used to detect forward dependencies for lazy two-phase evaluation (§11.7).
    two_phase_future_fixed: std::collections::HashSet<String>,
    /// `local` bindings produced by `#{}` blocks; these may not be forward
    /// dependencies (§11.7).
    two_phase_future_locals: std::collections::HashSet<String>,
    /// Deferred top-level/module `#if` conditions awaiting settlement.
    two_phase_pending: Vec<PendingCompileTimeIf>,
    /// Stack of `#{}` blocks currently being checked in a function body.
    compile_time_block_stack: Vec<NodeId>,
    /// Locals declared by function-body `#{}` blocks, mapped to their block.
    compile_time_block_local_owners: HashMap<(u32, LocalId), NodeId>,
    /// Function-body `#{}` blocks with their function and declared locals.
    compile_time_block_records: Vec<(NodeId, u32, Vec<LocalId>)>,
    /// Whether the code being checked is inside a `verbatim` target (§9.8).
    pub(crate) in_verbatim: bool,
    /// Nesting depth of `#{}` compile-time blocks (§11.7).
    pub(crate) compile_time_depth: usize,
    /// Nesting depth of `#(...)`/`#if` compile-time expressions (§11.3).
    pub(crate) compile_time_expression_depth: usize,
    /// Locals declared inside compile-time blocks, keyed by function.
    pub(crate) compile_time_local_ids: Vec<(u32, LocalId)>,
    /// Whether the target is freestanding (no host C library, e.g. UEFI): the
    /// platform `DefaultAllocator` is then the developer's responsibility.
    pub(crate) freestanding: bool,
    /// Compilation target's `PlatformKind` value for `Platform.Is` (§11.9.3).
    pub(crate) target_platform_kind: u32,
    /// Whether a constant initializer is being checked (§5.3).
    pub(crate) in_constant_initializer: bool,
    /// Nesting depth of compile-time function interpretation (§11.4).
    pub(crate) compile_call_depth: usize,
    /// Top-level `#{}` blocks in execution order (§11.2).
    pub(crate) top_level_blocks: Vec<NodeId>,
    /// Top-level `#{}` blocks and `#()` expression statements in source order
    /// (§11.2, §11.4).
    pub(crate) top_level_units: Vec<NodeId>,
    /// Compile-time variable names whose final values are not known yet.
    pub(crate) unknown_compile_time_names: std::collections::HashSet<String>,
    /// Whether host JIT execution may be used for compile-time code (§11.4).
    pub(crate) jit_allowed: bool,
    /// Whether top-level compile-time blocks are being re-executed after all
    /// declarations are resolved (§11.2).
    pub(crate) compile_time_phase_b: bool,
    /// Set when a phase-A top-level block contains a call that can only be
    /// resolved after declaration resolution; the block is re-run in phase B.
    pub(crate) compile_time_deferred: bool,
    /// Set once compile-time execution reported an error; all further
    /// compile-time execution stops (§11.4: an error aborts compile-time code).
    pub(crate) compile_time_aborted: bool,
    /// Bound array dimension values while instantiating an array extension
    /// (§3.8.3).
    pub(crate) array_binding_lengths: Vec<HashMap<String, u64>>,
    /// NOLAA existence-demand groups created by `NOLAA.Bind` (§12.2): each
    /// group is disposed at the same safe point.
    pub(crate) heap_groups: Vec<Vec<LocalId>>,
    /// Pending `NOLAA.AssumeNoDemandNext` target for the next statement (§12.2).
    pub(crate) pending_assume_no_demand: Option<LocalId>,
    /// `NOLAA.AssumeNoDemandNext` target active for the statement being checked.
    pub(crate) active_assume_no_demand: Option<LocalId>,
    /// Enclosing code blocks (innermost last), for NOLAA jump safe points.
    pub(crate) block_stack: Vec<NodeId>,
    /// Loop body blocks (innermost last), for NOLAA break/continue safe points.
    pub(crate) loop_blocks: Vec<NodeId>,
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

    /// The `PlatformKind` value for a target triple (§15.2, §11.9.3): 0 `Other`,
    /// 1 `Windows`, 2 `Linux`, 3 `MacOS`, 4 `FreeBSD`. `None` means the host.
    pub(crate) fn platform_kind_for_triple(triple: Option<&str>) -> u32 {
        let triple = triple.map_or_else(
            || crate::HOST_TRIPLE.to_ascii_lowercase(),
            |triple| triple.to_ascii_lowercase(),
        );
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
        // to them regardless of declaration order (§6.3). Lazy two-phase `#if`
        // evaluation needs the immutable bindings of the whole program, so the
        // future candidate set is collected before declaration collection
        // (§11.7).
        let roots = self.ast.roots.clone();
        let (future_fixed, future_locals) = self.collect_future_compile_time_bindings(&roots);
        self.two_phase_future_fixed = future_fixed;
        self.two_phase_future_locals = future_locals;
        self.declare_types(&roots, true);
        if !self.two_phase_pending.is_empty() {
            self.settle_two_phase_ifs();
        }
        self.two_phase_pending.clear();

        // Pass 2: resolve field types, layouts, enumeration values and
        // function/global signatures.
        self.resolve_declarations(&roots);

        // Resolve `scoped` member-name targets now that all members exist (§9.5).
        self.resolve_pending_scoped();

        // Interface inheritance needs every interface's members resolved
        // (§3.7.3): flatten inherited method lists, expand class interface lists
        // and verify implementations.
        self.finalize_interface_inheritance();

        // Compile-time function calls need checked bodies before they can be
        // JIT-executed, so bodies are checked before the second compile-time
        // execution pass (§11.2, §11.4).
        self.finalize_variant_union_layouts();
        self.build_vtables();
        self.register_compile_time_names();
        self.check_function_bodies();

        // Pass 2 re-executes top-level compile-time blocks now that every
        // declaration (including functions) is resolvable (§11.2).
        self.rerun_top_level_compile_time();

        // Pass 3: check initializers.
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

    /// Pointer size used for built-in class/interface reference layout (§3.4).
    fn builtin_pointer_size(&self) -> u64 {
        u64::from(self.result.types.pointer_size())
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
            builtin_interface: None,
            fields: Vec::new(),
            field_initializers: Vec::new(),
            methods: std::collections::HashMap::new(),
            method_list: Vec::new(),
            accessors: Vec::new(),
            operators: Vec::new(),
            interfaces: Vec::new(),
            interface_dispatch: Vec::new(),
            interface_bases: Vec::new(),
            static_accessors: Vec::new(),
            modules: HashMap::new(),
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
            instance_size: 3 * self.builtin_pointer_size(),
            instance_align: self.builtin_pointer_size(),
            enum_members: Vec::new(),
            underlying: None,
            size: self.builtin_pointer_size(),
            align: self.builtin_pointer_size(),
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
            builtin_interface: None,
            fields: Vec::new(),
            field_initializers: Vec::new(),
            methods: std::collections::HashMap::new(),
            method_list: Vec::new(),
            accessors: Vec::new(),
            operators: Vec::new(),
            interfaces: Vec::new(),
            interface_dispatch: Vec::new(),
            interface_bases: Vec::new(),
            static_accessors: Vec::new(),
            modules: HashMap::new(),
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

    /// Pushes a synthetic interface declaration (Built-In Library, §15.2).
    fn push_builtin_interface(&mut self, name: &str) -> TypeId {
        let index = self.result.type_defs.len() as TypeDefId;
        let node = NodeId::MAX - 400 - index;
        self.result.type_defs.push(TypeDef {
            name: name.to_string(),
            qualified_name: name.to_string(),
            kind: TypeDefKind::Interface,
            node,
            generic_parameters: Vec::new(),
            generic_arguments: Vec::new(),
            template: None,
            decl_scope: 0,
            static_fields: std::collections::HashMap::new(),
            static_methods: std::collections::HashMap::new(),
            builtin_variant: None,
            builtin_interface: None,
            fields: Vec::new(),
            field_initializers: Vec::new(),
            methods: std::collections::HashMap::new(),
            method_list: Vec::new(),
            accessors: Vec::new(),
            operators: Vec::new(),
            interfaces: Vec::new(),
            interface_dispatch: Vec::new(),
            interface_bases: Vec::new(),
            static_accessors: Vec::new(),
            modules: HashMap::new(),
            sealed: false,
            is_abstract: true,
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
            size: self.builtin_pointer_size(),
            align: self.builtin_pointer_size(),
        });
        let type_id = self.result.types.intern(Type::Named(index));
        self.declare(name.to_string(), Symbol::Type(type_id), Span::default());
        type_id
    }

    /// Pushes a synthetic generic interface template (Built-In Library, §15.2).
    fn push_builtin_interface_template(
        &mut self,
        name: &str,
        kind: BuiltinInterfaceKind,
        parameters: &[&str],
    ) -> TypeId {
        let index = self.result.type_defs.len() as TypeDefId;
        let node = NodeId::MAX - 500 - index;
        self.result.type_defs.push(TypeDef {
            name: name.to_string(),
            qualified_name: name.to_string(),
            kind: TypeDefKind::Interface,
            node,
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
            builtin_variant: None,
            builtin_interface: Some(kind),
            fields: Vec::new(),
            field_initializers: Vec::new(),
            methods: std::collections::HashMap::new(),
            method_list: Vec::new(),
            accessors: Vec::new(),
            operators: Vec::new(),
            interfaces: Vec::new(),
            interface_dispatch: Vec::new(),
            interface_bases: Vec::new(),
            static_accessors: Vec::new(),
            modules: HashMap::new(),
            sealed: false,
            is_abstract: true,
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
            size: self.builtin_pointer_size(),
            align: self.builtin_pointer_size(),
        });
        let type_id = self.result.types.intern(Type::Named(index));
        self.declare(name.to_string(), Symbol::Type(type_id), Span::default());
        type_id
    }

    /// Adds a concrete method with a built-in body to a synthetic class.
    fn push_builtin_class_method(
        &mut self,
        owner: TypeId,
        name: &str,
        parameters: &[(&str, TypeId)],
        return_type: Option<TypeId>,
        body: FunctionBody,
    ) -> u32 {
        let Type::Named(owner_index) = self.result.types.get(owner) else {
            return 0;
        };
        let owner_index = *owner_index;
        let qualified = self.result.type_defs[owner_index as usize]
            .qualified_name
            .clone();
        let scope: Vec<String> = qualified.split('.').map(str::to_string).collect();
        let mut all_parameters = vec![ParameterInfo {
            name: "this".to_string(),
            ty: owner,
            by_ref: true,
            default: None,
            compile_time: false,
        }];
        for (name, ty) in parameters {
            all_parameters.push(ParameterInfo {
                name: (*name).to_string(),
                ty: *ty,
                by_ref: false,
                default: None,
                compile_time: false,
            });
        }
        let spellings: Vec<String> = all_parameters
            .iter()
            .map(|parameter| self.spelling(parameter.ty))
            .collect();
        let mangled = mangle_function(&scope, name, &spellings);
        let index = self.result.functions.len() as u32;
        self.result.functions.push(FunctionInfo {
            name: name.to_string(),
            scope,
            node: NodeId::default(),
            body: Some(body),
            parameters: all_parameters,

            wildcard: false,
            method_of: Some(owner),
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
            decl_scope: 0,
            generic_env: HashMap::new(),
            environment: false,
            captures: Vec::new(),
            constant_values: HashMap::new(),
            locals: Vec::new(),
            span: Span::default(),
        });
        let definition = &mut self.result.type_defs[owner_index as usize];
        definition
            .methods
            .entry(name.to_string())
            .or_default()
            .push(index);
        definition.method_list.push(index);
        index
    }

    /// Adds an abstract method to a synthetic interface (Built-In Library).
    fn push_builtin_interface_method(
        &mut self,
        interface: TypeId,
        name: &str,
        parameters: &[(&str, TypeId)],
        return_type: Option<TypeId>,
    ) -> u32 {
        let Type::Named(interface_index) = self.result.types.get(interface) else {
            return 0;
        };
        let interface_index = *interface_index;
        let qualified = self.result.type_defs[interface_index as usize]
            .qualified_name
            .clone();
        let scope: Vec<String> = qualified.split('.').map(str::to_string).collect();
        let mut all_parameters = vec![ParameterInfo {
            name: "this".to_string(),
            ty: interface,
            by_ref: true,
            default: None,
            compile_time: false,
        }];
        for (name, ty) in parameters {
            all_parameters.push(ParameterInfo {
                name: (*name).to_string(),
                ty: *ty,
                by_ref: false,
                default: None,
                compile_time: false,
            });
        }
        let spellings: Vec<String> = all_parameters
            .iter()
            .map(|parameter| self.spelling(parameter.ty))
            .collect();
        let mangled = mangle_function(&scope, name, &spellings);
        let index = self.result.functions.len() as u32;
        self.result.functions.push(FunctionInfo {
            name: name.to_string(),
            scope,
            node: NodeId::default(),
            body: None,
            parameters: all_parameters,

            wildcard: false,
            method_of: Some(interface),
            static_owner: None,
            is_virtual: false,
            is_override: false,
            is_sealed: false,
            accessibility: crate::ast::Accessibility::Public,
            calling_convention: None,
            scoped: Vec::new(),
            is_abstract: true,
            return_type,
            mangled,
            exposed_symbol: None,
            generic_parameters: Vec::new(),
            generic_arguments: Vec::new(),
            template: None,
            decl_scope: 0,
            generic_env: HashMap::new(),
            environment: false,
            captures: Vec::new(),
            constant_values: HashMap::new(),
            locals: Vec::new(),
            span: Span::default(),
        });
        let definition = &mut self.result.type_defs[interface_index as usize];
        definition
            .methods
            .entry(name.to_string())
            .or_default()
            .push(index);
        definition.method_list.push(index);
        index
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
                builtin_interface: None,
                fields: Vec::new(),
                field_initializers: Vec::new(),
                methods: std::collections::HashMap::new(),
                method_list: Vec::new(),
                accessors: Vec::new(),
                operators: Vec::new(),
                interfaces: Vec::new(),
                interface_dispatch: Vec::new(),
                interface_bases: Vec::new(),
                static_accessors: Vec::new(),
                modules: HashMap::new(),
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
                instance_size: 3 * self.builtin_pointer_size(),
                instance_align: self.builtin_pointer_size(),
                enum_members: Vec::new(),
                underlying: None,
                size: self.builtin_pointer_size(),
                align: self.builtin_pointer_size(),
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
                    let pointer_size = self.builtin_pointer_size();
                    let definition = &mut self.result.type_defs[*index as usize];
                    definition.fields = vec![FieldInfo {
                        name: "InnerException".to_string(),
                        ty: optional_exception,
                        offset: 3 * pointer_size,
                        accessibility: crate::ast::Accessibility::Public,
                        readonly: false,
                        scoped: Vec::new(),
                    }];
                    definition.field_initializers = vec![None];
                    definition.instance_size = 5 * pointer_size;
                    definition.instance_align = pointer_size;
                }
            }
            // IAllocator: the heap allocation protocol (§15.2).
            let allocator = self.push_builtin_interface("IAllocator");
            let pointer = self.result.types.pointer(None);
            let result_template = self
                .lookup("Result")
                .cloned()
                .and_then(|symbol| match symbol {
                    Symbol::Type(ty) => match self.result.types.get(ty) {
                        Type::Named(index) => Some(self.result.type_defs[*index as usize].node),
                        _ => None,
                    },
                    _ => None,
                });
            let named_exception = |me: &mut Self, name: &str| -> Option<TypeId> {
                match me.lookup(name).cloned() {
                    Some(Symbol::Type(ty)) => Some(ty),
                    _ => None,
                }
            };
            let allocate_failed = named_exception(self, "AllocateFailedException");
            let deallocate_failed = named_exception(self, "DeallocateFailedException");
            if let Some(template_node) = result_template {
                let cardinal8 = self.result.types.integer(false, 8);
                let allocate_result = allocate_failed.and_then(|failure| {
                    self.instantiate_type(template_node, vec![pointer, failure], Span::default())
                });
                let deallocate_result = deallocate_failed.and_then(|failure| {
                    self.instantiate_type(
                        template_node,
                        vec![TypeId::VOID, failure],
                        Span::default(),
                    )
                });
                let mut allocate_method = None;
                if let Some(allocate_result) = allocate_result {
                    allocate_method = Some(self.push_builtin_interface_method(
                        allocator,
                        "Allocate",
                        &[("size", cardinal8), ("alignment", cardinal8)],
                        Some(allocate_result),
                    ));
                }
                let mut deallocate_method = None;
                if let Some(deallocate_result) = deallocate_result {
                    deallocate_method = Some(self.push_builtin_interface_method(
                        allocator,
                        "Deallocate",
                        &[("block", pointer)],
                        Some(deallocate_result),
                    ));
                }
                // Freestanding targets (UEFI) have no platform allocator: the
                // developer supplies an `IAllocator` and uses explicit `heap`
                // allocator expressions (§5.4).
                if !self.freestanding {
                    // Platform-provided DefaultAllocator (§5.4, §15.2).
                    let default_allocator =
                        self.push_builtin_class("DefaultAllocatorImpl", None, false);
                    let mut implementations = Vec::new();
                    if let (Some(allocate_result), Some(method)) =
                        (allocate_result, allocate_method)
                    {
                        let _ = method;
                        let allocate = self.push_builtin_class_method(
                            default_allocator,
                            "Allocate",
                            &[("size", cardinal8), ("alignment", cardinal8)],
                            Some(allocate_result),
                            FunctionBody::BuiltinAllocate,
                        );
                        implementations.push(allocate);
                    }
                    if let (Some(deallocate_result), Some(method)) =
                        (deallocate_result, deallocate_method)
                    {
                        let _ = method;
                        let deallocate = self.push_builtin_class_method(
                            default_allocator,
                            "Deallocate",
                            &[("block", pointer)],
                            Some(deallocate_result),
                            FunctionBody::BuiltinDeallocate,
                        );
                        implementations.push(deallocate);
                    }
                    if let Type::Named(default_index) = self.result.types.get(default_allocator) {
                        let definition = &mut self.result.type_defs[*default_index as usize];
                        definition.interfaces.push(allocator);
                        definition
                            .interface_dispatch
                            .push((allocator, implementations));
                    }
                    let spelling = self.spelling(allocator);
                    let mangled = mangle_function(&[], "DefaultAllocator", &[spelling]);
                    let global_index = self.result.globals.len() as u32;
                    self.result.globals.push(GlobalInfo {
                        accessibility: crate::ast::Accessibility::Public,
                        scoped: Vec::new(),
                        verbatim: false,
                        name: "DefaultAllocator".to_string(),
                        scope: Vec::new(),
                        ty: allocator,
                        init: None,
                        fixed: true,
                        mangled,
                        node: NodeId::MAX - 700 - global_index,
                        span: Span::default(),
                    });
                    self.declare(
                        "DefaultAllocator".to_string(),
                        Symbol::Global(global_index),
                        Span::default(),
                    );
                    self.result.default_allocator_global = Some(global_index);
                }
            }
            // Generic iterator interfaces (§15.2).
            self.push_builtin_interface_template(
                "IIterator",
                BuiltinInterfaceKind::Iterator,
                &["T"],
            );
            self.push_builtin_interface_template(
                "IIterable",
                BuiltinInterfaceKind::Iterable,
                &["T"],
            );
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
                builtin_interface: None,
                fields: Vec::new(),
                field_initializers: Vec::new(),
                methods: std::collections::HashMap::new(),
                method_list: Vec::new(),
                accessors: Vec::new(),
                operators: Vec::new(),
                interfaces: Vec::new(),
                interface_dispatch: Vec::new(),
                interface_bases: Vec::new(),
                static_accessors: Vec::new(),
                modules: HashMap::new(),
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

    /// Whether a node is a declaration allowed at file/module top level (§5,
    /// §11.4). Statements and runtime expressions are function-body-only; a
    /// top-level `#()` compile-time expression statement is allowed.
    fn top_level_declaration_allowed(&self, node: NodeId) -> bool {
        match self.node_kind(node) {
            NodeKind::Module(_)
            | NodeKind::Class(_)
            | NodeKind::Structure(_)
            | NodeKind::Interface(_)
            | NodeKind::Enumeration(_)
            | NodeKind::Variant(_)
            | NodeKind::Union(_)
            | NodeKind::Extension(_)
            | NodeKind::Accessor(_)
            | NodeKind::Operator(_)
            | NodeKind::Local(_)
            | NodeKind::Fixed(_)
            | NodeKind::Heap(_)
            | NodeKind::Linkable(_)
            | NodeKind::Use(_)
            | NodeKind::CompileTimeBlock(_)
            | NodeKind::CompileTimeIf(_) => true,
            NodeKind::ExpressionStmt(expression) => {
                matches!(self.node_kind(expression), NodeKind::CompileTimeExpr(_))
            }
            _ => false,
        }
    }

    /// Pass 1: walk the declaration structure creating module scopes and named
    /// type stubs.
    fn declare_types(&mut self, nodes: &[NodeId], top_level: bool) {
        for &node in nodes {
            if top_level && !self.top_level_declaration_allowed(node) {
                self.error(
                    self.span_of(node),
                    "statements and expressions are only allowed inside function bodies (§5)",
                );
                continue;
            }
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
                    self.declare_types(&members, top_level);
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
                NodeKind::CompileTimeIf(statement) => {
                    if top_level {
                        // §11.7: defer conditions that depend on a not-yet
                        // produced immutable binding; reject `local` forward
                        // dependencies immediately.
                        let (fixed_dependencies, local_dependencies) = self
                            .condition_forward_bindings(
                                statement.condition,
                                &self.two_phase_future_fixed.clone(),
                                &self.two_phase_future_locals.clone(),
                            );
                        if !local_dependencies.is_empty() {
                            self.error(
                                self.span_of(statement.condition),
                                format!(
                                    "the compile-time 'local' '{}' may not be a forward dependency (§11.7)",
                                    local_dependencies[0]
                                ),
                            );
                            continue;
                        }
                        if !fixed_dependencies.is_empty() {
                            self.two_phase_pending.push(PendingCompileTimeIf {
                                node,
                                scope_stack: self.scope_stack.clone(),
                                scope_path: self.scope_path.clone(),
                            });
                            continue;
                        }
                    }
                    let condition = statement.condition;
                    let taken = self
                        .eval_compile_time_bool(condition, 0)
                        .unwrap_or_else(|| {
                            self.error(
                                self.span_of(condition),
                                "the '#if' condition must be a compile-time constant",
                            );
                            false
                        });
                    self.result.compile_time_conditions.insert(node, taken);
                    if taken {
                        let members = self.block_statements(statement.then_block);
                        self.declare_types(&members, top_level);
                    } else if let Some(else_block) = statement.else_block {
                        let members = self.block_statements(else_block);
                        self.declare_types(&members, top_level);
                    }
                }
                NodeKind::CompileTimeBlock(block) => {
                    self.top_level_blocks.push(node);
                    self.top_level_units.push(node);
                    self.execute_top_level_compile_time(node, block);
                }
                NodeKind::ExpressionStmt(expression)
                    if matches!(self.node_kind(expression), NodeKind::CompileTimeExpr(_)) =>
                {
                    self.top_level_units.push(node);
                    self.execute_top_level_compile_time_expression(node);
                }
                _ => {}
            }
            // Modules directly inside a type body organize identifiers (§3.7.1).
            if matches!(
                self.node_kind(node),
                NodeKind::Class(_)
                    | NodeKind::Structure(_)
                    | NodeKind::Interface(_)
                    | NodeKind::Enumeration(_)
                    | NodeKind::Variant(_)
                    | NodeKind::Union(_)
            ) {
                self.declare_type_body_modules(node);
            }
        }
    }

    /// Collects `fixed`/`local` names declared inside `#{}` blocks reachable from
    /// the top level, including blocks inside conditional branches (§11.7).
    fn collect_future_compile_time_bindings(
        &self,
        nodes: &[NodeId],
    ) -> (
        std::collections::HashSet<String>,
        std::collections::HashSet<String>,
    ) {
        let mut fixed = std::collections::HashSet::new();
        let mut locals = std::collections::HashSet::new();
        for &node in nodes {
            self.collect_future_bindings_in_node(node, false, &mut fixed, &mut locals);
        }
        (fixed, locals)
    }

    fn collect_future_bindings_in_node(
        &self,
        node: NodeId,
        inside_block: bool,
        fixed: &mut std::collections::HashSet<String>,
        locals: &mut std::collections::HashSet<String>,
    ) {
        match self.node_kind(node) {
            NodeKind::Module(declaration) => {
                for &member in &declaration.members {
                    self.collect_future_bindings_in_node(member, false, fixed, locals);
                }
            }
            NodeKind::CompileTimeIf(statement) => {
                for block in [Some(statement.then_block), statement.else_block]
                    .into_iter()
                    .flatten()
                {
                    for statement in self.block_statements(block) {
                        self.collect_future_bindings_in_node(
                            statement,
                            inside_block,
                            fixed,
                            locals,
                        );
                    }
                }
            }
            NodeKind::CompileTimeBlock(block) => {
                for statement in self.block_statements(block) {
                    self.collect_future_bindings_in_node(statement, true, fixed, locals);
                }
            }
            NodeKind::Fixed(declaration) if inside_block => {
                if declaration.init.is_some() {
                    fixed.insert(declaration.name.clone());
                }
            }
            NodeKind::Local(declaration) if inside_block => {
                for item in &declaration.items {
                    locals.insert(item.name.clone());
                }
            }
            NodeKind::Block(statements) => {
                for statement in statements {
                    self.collect_future_bindings_in_node(statement, inside_block, fixed, locals);
                }
            }
            NodeKind::If(statement) => {
                for branch in &statement.branches {
                    for inner in self.block_statements(branch.body) {
                        self.collect_future_bindings_in_node(inner, inside_block, fixed, locals);
                    }
                }
                if let Some(else_block) = statement.else_block {
                    for inner in self.block_statements(else_block) {
                        self.collect_future_bindings_in_node(inner, inside_block, fixed, locals);
                    }
                }
            }
            NodeKind::While(statement) => {
                for inner in self.block_statements(statement.body) {
                    self.collect_future_bindings_in_node(inner, inside_block, fixed, locals);
                }
            }
            NodeKind::For(statement) => {
                for inner in self.block_statements(statement.body) {
                    self.collect_future_bindings_in_node(inner, inside_block, fixed, locals);
                }
            }
            _ => {}
        }
    }

    /// The forward-dependency bindings referenced by a `#if` condition: names not
    /// currently available that a later `#{}` produces as `fixed`/`local`.
    fn condition_forward_bindings(
        &self,
        condition: NodeId,
        future_fixed: &std::collections::HashSet<String>,
        future_locals: &std::collections::HashSet<String>,
    ) -> (Vec<String>, Vec<String>) {
        let mut names = Vec::new();
        self.collect_condition_names(condition, &mut names);
        let mut fixed = Vec::new();
        let mut locals = Vec::new();
        for (name, _) in names {
            if self.compile_time_binding_available(&name) {
                continue;
            }
            if future_fixed.contains(&name) {
                fixed.push(name);
            } else if future_locals.contains(&name) {
                locals.push(name);
            }
        }
        (fixed, locals)
    }

    fn compile_time_binding_available(&self, name: &str) -> bool {
        self.top_level_constants.contains_key(name)
            || self
                .constant_env
                .iter()
                .any(|environment| environment.contains_key(name))
            || self.lookup(name).is_some()
    }

    fn collect_condition_names(&self, node: NodeId, out: &mut Vec<(String, NodeId)>) {
        match self.node_kind(node) {
            NodeKind::Name(name) => out.push((name.name.clone(), node)),
            NodeKind::Unary(unary) => self.collect_condition_names(unary.operand, out),
            NodeKind::Binary(binary) => {
                self.collect_condition_names(binary.left, out);
                self.collect_condition_names(binary.right, out);
            }
            NodeKind::Cast(cast) => self.collect_condition_names(cast.value, out),
            NodeKind::Is(expression) => self.collect_condition_names(expression.value, out),
            NodeKind::CompileTimeExpr(inner) | NodeKind::VerbatimExpression(inner) => {
                self.collect_condition_names(inner, out);
            }
            NodeKind::Call(call) => {
                self.collect_condition_names(call.callee, out);
                for argument in &call.arguments {
                    self.collect_condition_names(argument.value, out);
                }
            }
            NodeKind::MemberAccess(access) => self.collect_condition_names(access.base, out),
            _ => {}
        }
    }

    /// Settles deferred top-level `#if` conditions (settlement phase, §11.7).
    fn settle_two_phase_ifs(&mut self) {
        loop {
            if self.two_phase_pending.is_empty() {
                break;
            }
            let batch = std::mem::take(&mut self.two_phase_pending);
            let mut deferred = Vec::new();
            let mut progressed = false;
            for entry in batch {
                let node = entry.node;
                if self.result.compile_time_conditions.contains_key(&node) {
                    continue;
                }
                let NodeKind::CompileTimeIf(statement) = self.node_kind(node).clone() else {
                    continue;
                };
                let future_fixed = self.two_phase_future_fixed.clone();
                let future_locals = self.two_phase_future_locals.clone();
                let (fixed_dependencies, local_dependencies) = self.condition_forward_bindings(
                    statement.condition,
                    &future_fixed,
                    &future_locals,
                );
                if !local_dependencies.is_empty() {
                    self.error(
                        self.span_of(statement.condition),
                        format!(
                            "the compile-time 'local' '{}' may not be a forward dependency (§11.7)",
                            local_dependencies[0]
                        ),
                    );
                    continue;
                }
                if !fixed_dependencies.is_empty() {
                    // A later settlement round may produce these bindings.
                    deferred.push(entry);
                    continue;
                }
                self.suppress_errors += 1;
                let decision = self.eval_compile_time_bool(statement.condition, 0);
                self.suppress_errors -= 1;
                if let Some(taken) = decision {
                    self.result.compile_time_conditions.insert(node, taken);
                    let members = if taken {
                        self.block_statements(statement.then_block)
                    } else {
                        statement
                            .else_block
                            .map(|else_block| self.block_statements(else_block))
                            .unwrap_or_default()
                    };
                    // Collect declarations and execute the selected branch's
                    // compile-time blocks exactly once in the scope where the
                    // `#if` appeared (§11.7 settlement).
                    let saved_stack = self.scope_stack.clone();
                    let saved_path = self.scope_path.clone();
                    self.scope_stack = entry.scope_stack;
                    self.scope_path = entry.scope_path;
                    self.declare_types(&members, true);
                    self.scope_stack = saved_stack;
                    self.scope_path = saved_path;
                    progressed = true;
                    continue;
                }
                self.error(
                    self.span_of(statement.condition),
                    "the '#if' condition must be a compile-time constant",
                );
            }
            if !self.two_phase_pending.is_empty() {
                // Nested `#if`s encountered while declaring a selected branch
                // joined the queue; settle them before giving up on deferred
                // conditions.
                self.two_phase_pending.extend(deferred);
                continue;
            }
            if deferred.is_empty() {
                break;
            }
            if !progressed {
                for entry in &deferred {
                    let span = match self.node_kind(entry.node) {
                        NodeKind::CompileTimeIf(statement) => self.span_of(statement.condition),
                        _ => self.span_of(entry.node),
                    };
                    self.error(
                        span,
                        "two-phase compile-time evaluation cannot make progress: the '#if' forward dependency is never produced (§11.7)",
                    );
                }
                break;
            }
            self.two_phase_pending = deferred;
        }
    }

    /// Executes a compile-time assignment (`X = value` / `X += value`) in a
    /// top-level `#{}` block (§11.4): the compile-time variable's stored value is
    /// updated so later compile-time code and run-time constants see it.
    fn execute_top_level_assignment(
        &mut self,
        target: &NodeId,
        value: &NodeId,
        compound: Option<crate::ast::BinaryOp>,
    ) {
        let NodeKind::Name(name) = self.node_kind(*target).clone() else {
            if self.compile_time_phase_b {
                self.unsupported(
                    self.span_of(*target),
                    "compile-time assignments to non-identifier targets",
                );
            }
            return;
        };
        let from_heap = self.compile_time_heap.contains_key(&name.name);
        let Some((current, ty)) = self
            .compile_time_heap
            .get(&name.name)
            .copied()
            .or_else(|| self.top_level_constants.get(&name.name).copied())
        else {
            if self.compile_time_phase_b {
                self.error(
                    self.span_of(*target),
                    format!("'{}' is not a compile-time variable (§11.4)", name.name),
                );
            }
            return;
        };
        self.suppress_errors += 1;
        let evaluated = self.eval_const_int(*value);
        self.suppress_errors -= 1;
        let Some(evaluated) = evaluated else {
            if self.compile_time_phase_b {
                self.unsupported(
                    self.span_of(*value),
                    "non-constant values in compile-time assignments",
                );
            }
            return;
        };
        let updated = match compound {
            None => evaluated,
            Some(operator) => {
                use crate::ast::BinaryOp;
                match operator {
                    BinaryOp::Add => current.wrapping_add(evaluated),
                    BinaryOp::Subtract => current.wrapping_sub(evaluated),
                    BinaryOp::Multiply => current.wrapping_mul(evaluated),
                    BinaryOp::Divide if evaluated != 0 => current.wrapping_div(evaluated),
                    BinaryOp::Remainder if evaluated != 0 => current.wrapping_rem(evaluated),
                    BinaryOp::ShiftLeft => current.wrapping_shl(evaluated as u32),
                    BinaryOp::ShiftRight => ((current as u128) >> (evaluated as u32 & 127)) as i128,
                    BinaryOp::BitwiseAnd => current & evaluated,
                    BinaryOp::BitwiseOr => current | evaluated,
                    BinaryOp::BitwiseXor => current ^ evaluated,
                    _ => {
                        self.unsupported(
                            self.span_of(*value),
                            "this compound assignment in a compile-time block",
                        );
                        return;
                    }
                }
            }
        };
        if from_heap {
            self.compile_time_heap
                .insert(name.name.clone(), (updated, ty));
        } else {
            self.top_level_constants
                .insert(name.name.clone(), (updated, ty));
        }
        self.unknown_compile_time_names.remove(&name.name);
    }

    /// Returns the statements of a block node.
    fn block_statements(&self, node: NodeId) -> Vec<NodeId> {
        match self.node_kind(node) {
            NodeKind::Block(statements) => statements.clone(),
            NodeKind::VerbatimBlock(inner) => self.block_statements(inner),
            _ => Vec::new(),
        }
    }

    /// Executes a top-level compile-time block during declaration collection
    /// (§11.2, §11.4): constant locals become top-level compile-time variables
    /// available to later blocks, `#if`s and constant expressions.
    /// Re-executes all top-level `#{}` blocks after declaration resolution so
    /// compile-time function calls can be interpreted (§11.2). The `#if`
    /// decisions already taken during declaration collection are preserved.
    fn rerun_top_level_compile_time(&mut self) {
        if self.top_level_blocks.is_empty() || self.compile_time_aborted {
            return;
        }
        self.top_level_constants.clear();
        self.compile_time_heap.clear();
        self.compile_time_phase_b = true;
        self.compile_time_deferred = false;
        let units = self.top_level_units.clone();
        for node in units {
            match self.node_kind(node) {
                NodeKind::CompileTimeBlock(block) => {
                    self.execute_top_level_compile_time(node, block);
                }
                NodeKind::ExpressionStmt(expression)
                    if matches!(self.node_kind(expression), NodeKind::CompileTimeExpr(_)) =>
                {
                    self.execute_top_level_compile_time_expression(node);
                }
                _ => {}
            }
        }
        self.compile_time_phase_b = false;
        self.compile_time_deferred = false;
        self.result.compile_time_variables = self.top_level_constants.clone();
        self.unknown_compile_time_names.clear();
    }

    /// Records the names and declared types of top-level compile-time variables
    /// before function bodies are checked, so bodies can refer to them while
    /// final values are still being computed (§11.4).
    fn register_compile_time_names(&mut self) {
        let blocks = self.top_level_blocks.clone();
        for node in blocks {
            let block = match self.node_kind(node) {
                NodeKind::CompileTimeBlock(block) => block,
                _ => continue,
            };
            let statements = self.block_statements(block);
            self.register_compile_time_names_in_statements(&statements);
        }
        self.result.compile_time_variables = self.top_level_constants.clone();
    }

    fn register_compile_time_names_in_statements(&mut self, statements: &[NodeId]) {
        for statement in statements {
            match self.node_kind(*statement).clone() {
                NodeKind::Local(declaration) => {
                    for item in &declaration.items {
                        let Some(ty_node) = item.ty else { continue };
                        let Some(ty) = self.resolve_type(ty_node) else {
                            continue;
                        };
                        if !self.top_level_constants.contains_key(&item.name) {
                            self.top_level_constants.insert(item.name.clone(), (0, ty));
                            self.unknown_compile_time_names.insert(item.name.clone());
                        }
                    }
                }
                NodeKind::Fixed(declaration) => {
                    if let Some(Initializer::Expression(_)) = declaration.init {
                        if let Some(ty) = declaration.ty.and_then(|ty| self.resolve_type(ty)) {
                            if !self.top_level_constants.contains_key(&declaration.name) {
                                self.top_level_constants
                                    .insert(declaration.name.clone(), (0, ty));
                                self.unknown_compile_time_names
                                    .insert(declaration.name.clone());
                            }
                        }
                    }
                }
                NodeKind::Block(inner) => {
                    let inner = inner.clone();
                    self.register_compile_time_names_in_statements(&inner);
                }
                NodeKind::CompileTimeBlock(inner) => {
                    let statements = self.block_statements(inner);
                    self.register_compile_time_names_in_statements(&statements);
                }
                NodeKind::If(if_statement) => {
                    for branch in &if_statement.branches {
                        let statements = self.block_statements(branch.body);
                        self.register_compile_time_names_in_statements(&statements);
                    }
                    if let Some(else_block) = if_statement.else_block {
                        let statements = self.block_statements(else_block);
                        self.register_compile_time_names_in_statements(&statements);
                    }
                }
                NodeKind::While(loop_statement) => {
                    let statements = self.block_statements(loop_statement.body);
                    self.register_compile_time_names_in_statements(&statements);
                }
                NodeKind::For(loop_statement) => {
                    let statements = self.block_statements(loop_statement.body);
                    self.register_compile_time_names_in_statements(&statements);
                }
                _ => {}
            }
        }
    }

    /// Evaluates a compile-time constant initializer to its integer payload plus
    /// type; boolean constants are folded to 0/1 with the boolean type (§11.4).
    fn eval_compile_time_value(
        &mut self,
        expression: NodeId,
        declared: Option<TypeId>,
    ) -> Option<(i128, TypeId)> {
        if let Some(value) = self.eval_const_int(expression) {
            return Some((
                value,
                declared.unwrap_or_else(|| self.inferred_integer_type(value)),
            ));
        }
        let boolean = self.eval_compile_time_bool(expression, 0)?;
        Some((i128::from(boolean), declared.unwrap_or(TypeId::BOOLEAN)))
    }

    fn execute_top_level_compile_time(&mut self, node: NodeId, block: NodeId) {
        if !self.compile_time_phase_b {
            self.compile_time_deferred = false;
        }
        if self.compile_time_aborted {
            return;
        }
        let statements = self.block_statements(block);
        let flow = self.execute_compile_time_statements(&statements, false);
        if matches!(flow, CompileTimeFlow::Break | CompileTimeFlow::Continue) {
            self.error(
                self.span_of(node),
                "'break'/'continue' outside a compile-time loop",
            );
        }
    }

    /// Executes a top-level `#( expression )` statement (§11.2, §11.4). Pure
    /// non-API expressions are evaluated in the second pass after declarations
    /// are resolved; compiler API calls run in the declaration pass.
    fn execute_top_level_compile_time_expression(&mut self, statement: NodeId) {
        if self.compile_time_aborted {
            return;
        }
        if !self.compile_time_phase_b {
            self.compile_time_deferred = false;
        }
        let flow = self.execute_compile_time_statements(&[statement], false);
        self.compile_time_deferred = false;
        if matches!(flow, CompileTimeFlow::Break | CompileTimeFlow::Continue) {
            self.error(
                self.span_of(statement),
                "'break'/'continue' outside a compile-time loop",
            );
        }
    }

    /// Executes statements of a top-level `#{}` block at compile time (§11.4).
    /// Number of errors reported so far by the diagnostic engine.
    fn error_count(&self) -> u32 {
        self.engine.error_count()
    }

    fn execute_compile_time_statements(
        &mut self,
        statements: &[NodeId],
        in_loop: bool,
    ) -> CompileTimeFlow {
        let errors_before = self.error_count();
        for statement in statements {
            if self.compile_time_aborted {
                return CompileTimeFlow::Normal;
            }
            if !self.compile_time_phase_b && self.compile_time_deferred {
                return CompileTimeFlow::Normal;
            }
            let flow = match self.node_kind(*statement).clone() {
                NodeKind::CompileTimeIf(if_statement) => {
                    if self.compile_time_phase_b {
                        // The declaration-selection decision was already made.
                        continue;
                    }
                    let condition = if_statement.condition;
                    let taken = self
                        .eval_compile_time_bool(condition, 0)
                        .unwrap_or_else(|| {
                            self.error(
                                self.span_of(condition),
                                "the '#if' condition must be a compile-time constant",
                            );
                            false
                        });
                    self.result
                        .compile_time_conditions
                        .insert(*statement, taken);
                    continue;
                }
                NodeKind::CompileTimeBlock(inner) => {
                    let statements = self.block_statements(inner);
                    let flow = self.execute_compile_time_statements(&statements, in_loop);
                    if matches!(flow, CompileTimeFlow::Break | CompileTimeFlow::Continue) {
                        return flow;
                    }
                    continue;
                }
                NodeKind::Block(inner) => {
                    let flow = self.execute_compile_time_statements(&inner, in_loop);
                    if matches!(flow, CompileTimeFlow::Break | CompileTimeFlow::Continue) {
                        return flow;
                    }
                    continue;
                }
                NodeKind::Local(declaration) => {
                    for item in &declaration.items {
                        let Some(init) = item.init else {
                            if self.compile_time_phase_b {
                                self.unsupported(item.span, "uninitialized compile-time variables");
                            }
                            continue;
                        };
                        self.suppress_errors += 1;
                        let declared = item.ty.and_then(|ty| self.resolve_type(ty));
                        let value = self.eval_compile_time_value(init, declared);
                        self.suppress_errors -= 1;
                        match value {
                            Some((value, ty)) => {
                                self.top_level_constants
                                    .insert(item.name.clone(), (value, ty));
                                self.unknown_compile_time_names.remove(&item.name);
                            }
                            None => {
                                if self.compile_time_phase_b {
                                    self.unsupported(
                                        item.span,
                                        "non-constant values in top-level compile-time blocks",
                                    );
                                }
                            }
                        }
                    }
                    continue;
                }
                NodeKind::Fixed(declaration) => {
                    if let Some(Initializer::Expression(expression)) = declaration.init {
                        self.suppress_errors += 1;
                        let declared = declaration.ty.and_then(|ty| self.resolve_type(ty));
                        let value = self.eval_compile_time_value(expression, declared);
                        self.suppress_errors -= 1;
                        if let Some((value, ty)) = value {
                            self.top_level_constants
                                .insert(declaration.name.clone(), (value, ty));
                        }
                    }
                    continue;
                }
                NodeKind::ExpressionStmt(expression) => {
                    let is_api = builtin_compiler_function(self.ast, expression).is_some()
                        || ["Platform", "NOLAA"].iter().any(|namespace| {
                            builtin_namespace_call(self.ast, expression, namespace).is_some()
                        })
                        || builtin_size_of_type_argument(self.ast, expression).is_some();
                    if !self.compile_time_phase_b {
                        // Compiler API statements execute in phase A; any other
                        // call needs resolved declarations, so the whole block is
                        // deferred to phase B instead of reporting an unknown name
                        // after a collection pass.
                        if !is_api {
                            self.compile_time_deferred = true;
                            return CompileTimeFlow::Normal;
                        }
                        self.compile_time_constants
                            .push(std::collections::HashMap::new());
                        self.check_expr(expression, None);
                        self.compile_time_constants.pop();
                        continue;
                    }
                    if is_api {
                        // Already executed during phase A.
                        continue;
                    }
                    let errors_before = self.engine.error_count();
                    self.compile_time_constants
                        .push(std::collections::HashMap::new());
                    self.check_expr(expression, None);
                    self.compile_time_constants.pop();
                    if let Some(Resolution::Function(index)) =
                        self.result.resolutions[expression as usize].clone()
                    {
                        let (is_external, function_name) = {
                            let function = &self.result.functions[index as usize];
                            (
                                function.body.is_none() && function.exposed_symbol.is_some(),
                                function.name.clone(),
                            )
                        };
                        if is_external {
                            // §5.5: compile-time use of an external library needs
                            // the library to be provided to the compiler. On the
                            // host target the compiler process itself provides the
                            // C library through the JIT; other targets have no
                            // compile-time library.
                            if !self.jit_allowed {
                                self.error(
                                    self.span_of(expression),
                                    "no compile-time library provides this external symbol (§5.5): compile-time code may only call linkable functions supplied by a compile-time library (cross-target compilation has no host compile-time library)",
                                );
                                continue;
                            }
                            if let Err(message) =
                                self.execute_external_compile_time_call(expression, index)
                            {
                                self.error(
                                    self.span_of(expression),
                                    format!(
                                        "compile-time external call to '{}' failed: {message}",
                                        function_name
                                    ),
                                );
                            }
                            continue;
                        }
                        self.suppress_errors += 1;
                        let value = self.eval_const_int(expression);
                        self.suppress_errors -= 1;
                        if value.is_none() && self.engine.error_count() == errors_before {
                            self.unsupported(
                                self.span_of(expression),
                                "this compile-time function call",
                            );
                        }
                    }
                    continue;
                }
                NodeKind::Set(set_statement) => {
                    self.execute_top_level_assignment(
                        &set_statement.target,
                        &set_statement.value,
                        None,
                    );
                    continue;
                }
                NodeKind::CompoundSet(compound) => {
                    self.execute_top_level_assignment(
                        &compound.target,
                        &compound.value,
                        Some(compound.op),
                    );
                    continue;
                }
                NodeKind::If(if_statement) => {
                    self.execute_compile_time_branches(&if_statement, in_loop)
                }
                NodeKind::While(while_statement) => {
                    self.execute_compile_time_while(&while_statement)
                }
                NodeKind::DoWhile(do_while) => self.execute_compile_time_do_while(&do_while),
                NodeKind::Match(match_statement) => {
                    self.execute_compile_time_match(&match_statement, in_loop)
                }
                NodeKind::Heap(declaration) => {
                    self.execute_compile_time_heap(*statement, &declaration);
                    continue;
                }
                NodeKind::Dispose(dispose) => {
                    if let NodeKind::Name(name) = self.node_kind(dispose.target) {
                        if self.compile_time_heap.remove(&name.name).is_some() {
                            continue;
                        }
                    }
                    self.unsupported(
                        self.span_of(*statement),
                        "compile-time dispose of a non-compile-time heap unit",
                    );
                    continue;
                }
                NodeKind::For(for_statement) => self.execute_compile_time_for(&for_statement),
                NodeKind::Break(_) => {
                    if !in_loop {
                        self.error(
                            self.span_of(*statement),
                            "'break' outside a compile-time loop",
                        );
                    }
                    CompileTimeFlow::Break
                }
                NodeKind::Continue(_) => {
                    if !in_loop {
                        self.error(
                            self.span_of(*statement),
                            "'continue' outside a compile-time loop",
                        );
                    }
                    CompileTimeFlow::Continue
                }
                _ => {
                    self.unsupported(
                        self.span_of(*statement),
                        "this statement in a top-level compile-time block",
                    );
                    continue;
                }
            };
            if matches!(flow, CompileTimeFlow::Break | CompileTimeFlow::Continue) {
                return flow;
            }
        }
        if self.error_count() > errors_before {
            self.compile_time_aborted = true;
        }
        CompileTimeFlow::Normal
    }

    /// Executes an external `linkable` call at compile time on the host through
    /// a JIT trampoline (§5.5: the host C library is the compile-time library).
    fn execute_external_compile_time_call(
        &mut self,
        expression: NodeId,
        index: u32,
    ) -> Result<(), String> {
        let (parameters, return_type, symbol) = {
            let function = &self.result.functions[index as usize];
            (
                function.parameters.clone(),
                function.return_type,
                function.exposed_symbol.clone(),
            )
        };
        let symbol = symbol.ok_or_else(|| "missing external symbol name".to_string())?;
        if !parameters.is_empty() {
            return Err(
                "fixed parameters of compile-time external calls are not supported".to_string(),
            );
        }
        let NodeKind::Call(call) = self.node_kind(expression).clone() else {
            return Err("expected a call expression".to_string());
        };
        let mut arguments = Vec::with_capacity(call.arguments.len());
        for argument in &call.arguments {
            let node = argument.value;
            let Some(ty) = self.result.node_types.get(node as usize).copied().flatten() else {
                return Err("argument type is not known".to_string());
            };
            match self.result.types.get(ty).clone() {
                Type::Integer { bytes, signed } if bytes <= 8 => {
                    self.suppress_errors += 1;
                    let value = self.eval_const_int(node);
                    self.suppress_errors -= 1;
                    let Some(value) = value else {
                        return Err("argument is not a compile-time constant".to_string());
                    };
                    arguments.push(crate::codegen::JitExternalArgument::Integer {
                        value,
                        bytes,
                        signed,
                    });
                }
                Type::Boolean => {
                    self.suppress_errors += 1;
                    let value = self.eval_const_int(node);
                    self.suppress_errors -= 1;
                    let Some(value) = value else {
                        return Err("argument is not a compile-time constant".to_string());
                    };
                    arguments.push(crate::codegen::JitExternalArgument::Integer {
                        value,
                        bytes: 4,
                        signed: false,
                    });
                }
                Type::Float { bytes } if bytes == 4 || bytes == 8 => {
                    let Some(value) = self.eval_const_float(node) else {
                        return Err("argument is not a compile-time constant".to_string());
                    };
                    arguments.push(crate::codegen::JitExternalArgument::Float { value, bytes });
                }
                Type::Array { .. } => {
                    let bytes = match self.node_kind(node).clone() {
                        NodeKind::StringLiteral(literal)
                        | NodeKind::VerbatimStringLiteral(literal) => {
                            crate::literal::decode_string(&literal.raw)
                                .map_err(|message| format!("invalid string literal: {message}"))?
                                .bytes
                        }
                        _ => {
                            return Err(
                                "only string literals are supported as array arguments".to_string()
                            )
                        }
                    };
                    arguments.push(crate::codegen::JitExternalArgument::Bytes(bytes));
                }
                _ => {
                    return Err(format!(
                        "argument type '{}' is not supported in compile-time external calls",
                        self.spelling(ty)
                    ))
                }
            }
        }
        let return_signature = match return_type {
            None => None,
            Some(ty) => match self.result.types.get(ty) {
                Type::Integer { bytes, signed } if *bytes <= 8 => Some((*bytes, *signed)),
                _ => {
                    return Err(
                        "compile-time external calls require a void or integer return type"
                            .to_string(),
                    )
                }
            },
        };
        crate::codegen::jit_call_external(&symbol, &arguments, return_signature).map(|_| ())
    }

    fn execute_compile_time_branches(
        &mut self,
        statement: &crate::ast::IfStmt,
        in_loop: bool,
    ) -> CompileTimeFlow {
        for branch in &statement.branches {
            let Some(taken) = self.eval_compile_time_bool(branch.condition, 0) else {
                self.error(
                    self.span_of(branch.condition),
                    "compile-time blocks cannot branch on run-time values (§11.4)",
                );
                return CompileTimeFlow::Normal;
            };
            if taken {
                let statements = self.block_statements(branch.body);
                return self.execute_compile_time_statements(&statements, in_loop);
            }
        }
        if let Some(else_block) = statement.else_block {
            let statements = self.block_statements(else_block);
            return self.execute_compile_time_statements(&statements, in_loop);
        }
        CompileTimeFlow::Normal
    }

    fn execute_compile_time_while(&mut self, statement: &crate::ast::LoopStmt) -> CompileTimeFlow {
        let mut iterations = 0usize;
        while self
            .eval_compile_time_bool(statement.condition, 0)
            .unwrap_or_else(|| {
                self.error(
                    self.span_of(statement.condition),
                    "compile-time blocks cannot branch on run-time values (§11.4)",
                );
                false
            })
        {
            iterations += 1;
            let _ = iterations;
            let statements = self.block_statements(statement.body);
            let errors_before = self.engine.error_count();
            match self.execute_compile_time_statements(&statements, true) {
                CompileTimeFlow::Break => break,
                CompileTimeFlow::Normal | CompileTimeFlow::Continue => {}
            }
            if (!self.compile_time_phase_b && self.compile_time_deferred)
                || self.engine.error_count() > errors_before
            {
                break;
            }
        }
        CompileTimeFlow::Normal
    }

    /// Executes a compile-time `do { ... } while (...)` loop (§5.9.4, §11.4).
    fn execute_compile_time_do_while(
        &mut self,
        statement: &crate::ast::DoWhileStmt,
    ) -> CompileTimeFlow {
        loop {
            let statements = self.block_statements(statement.body);
            let errors_before = self.engine.error_count();
            match self.execute_compile_time_statements(&statements, true) {
                CompileTimeFlow::Break => break,
                CompileTimeFlow::Normal | CompileTimeFlow::Continue => {}
            }
            if (!self.compile_time_phase_b && self.compile_time_deferred)
                || self.engine.error_count() > errors_before
            {
                break;
            }
            let condition = self
                .eval_compile_time_bool(statement.condition, 0)
                .unwrap_or_else(|| {
                    self.error(
                        self.span_of(statement.condition),
                        "compile-time blocks cannot branch on run-time values (§11.4)",
                    );
                    false
                });
            if !condition {
                break;
            }
        }
        CompileTimeFlow::Normal
    }

    /// Evaluates a compile-time match operand/pattern to an integer payload and
    /// records whether the value is boolean (for type agreement).
    fn compile_time_match_value(&mut self, node: NodeId) -> Option<(i128, bool)> {
        self.suppress_errors += 1;
        let integer = self.eval_const_int(node);
        let boolean = if integer.is_none() {
            self.eval_compile_time_bool(node, 0)
        } else {
            None
        };
        self.suppress_errors -= 1;
        match (integer, boolean) {
            (Some(value), _) => Some((value, false)),
            (None, Some(value)) => Some((i128::from(value), true)),
            (None, None) => None,
        }
    }

    /// Executes a scalar `heap` definition in a compile-time block (§5.4,
    /// §11.4). Compile-time allocation units never enter the runtime artifact;
    /// non-scalar compile-time heap objects are not representable by the
    /// constant interpreter and are diagnosed.
    fn execute_compile_time_heap(&mut self, node: NodeId, declaration: &crate::ast::HeapDecl) {
        let declared = declaration.ty.and_then(|ty| self.resolve_type(ty));
        let Some(expression) = declaration.init else {
            self.unsupported(
                self.span_of(node),
                "uninitialized compile-time heap storage",
            );
            return;
        };
        self.suppress_errors += 1;
        let value = self.eval_compile_time_value(expression, declared);
        self.suppress_errors -= 1;
        let Some((value, ty)) = value else {
            self.unsupported(
                self.span_of(expression),
                "non-scalar compile-time heap initializers",
            );
            return;
        };
        let allowed = ty == TypeId::BOOLEAN
            || matches!(
                self.result.types.get(ty),
                crate::sema::types::Type::Integer { .. }
            );
        if !allowed {
            self.unsupported(
                self.span_of(expression),
                "compile-time heap allocation of non-scalar types",
            );
            return;
        }
        self.compile_time_heap
            .insert(declaration.name.clone(), (value, ty));
        self.unknown_compile_time_names.remove(&declaration.name);
    }

    /// Executes a `match` statement at compile time over constant values
    /// (§5.9.6, §11.4). Only value equality/comparison/wildcard patterns are
    /// supported in compile-time code; `is` patterns require run-time type data.
    fn execute_compile_time_match(
        &mut self,
        statement: &crate::ast::MatchStmt,
        in_loop: bool,
    ) -> CompileTimeFlow {
        let Some((value, boolean)) = self.compile_time_match_value(statement.value) else {
            self.error(
                self.span_of(statement.value),
                "compile-time 'match' requires a constant matched value",
            );
            return CompileTimeFlow::Normal;
        };
        for case in &statement.cases {
            let matched = match &case.pattern {
                None | Some(crate::ast::Pattern::Wildcard) => true,
                Some(crate::ast::Pattern::Equal(pattern)) => {
                    let Some((pattern_value, pattern_boolean)) =
                        self.compile_time_match_value(*pattern)
                    else {
                        self.error(
                            self.span_of(*pattern),
                            "compile-time match pattern must be constant",
                        );
                        return CompileTimeFlow::Normal;
                    };
                    if pattern_boolean != boolean {
                        self.error(
                            self.span_of(*pattern),
                            "compile-time match pattern has a different type",
                        );
                        return CompileTimeFlow::Normal;
                    }
                    pattern_value == value
                }
                Some(crate::ast::Pattern::Comparison {
                    operator,
                    value: rhs,
                    span,
                }) => {
                    if boolean {
                        self.error(*span, "compile-time match comparison requires integers");
                        return CompileTimeFlow::Normal;
                    }
                    let Some(rhs) = self.eval_const_int(*rhs) else {
                        self.error(*span, "compile-time match pattern must be constant");
                        return CompileTimeFlow::Normal;
                    };
                    match operator {
                        crate::lexer::TokenKind::Less => value < rhs,
                        crate::lexer::TokenKind::Greater => value > rhs,
                        crate::lexer::TokenKind::LessEquals => value <= rhs,
                        crate::lexer::TokenKind::GreaterEquals => value >= rhs,
                        crate::lexer::TokenKind::EqualsEquals => value == rhs,
                        crate::lexer::TokenKind::NotEquals => value != rhs,
                        _ => {
                            self.error(*span, "unsupported compile-time match comparison");
                            return CompileTimeFlow::Normal;
                        }
                    }
                }
                Some(crate::ast::Pattern::Is { .. }) => {
                    self.unsupported(self.span_of(case.body), "compile-time match 'is' patterns");
                    return CompileTimeFlow::Normal;
                }
            };
            if !matched {
                continue;
            }
            if let Some(guard) = case.guard {
                match self.eval_compile_time_bool(guard, 0) {
                    Some(true) => {}
                    Some(false) => continue,
                    None => {
                        self.error(
                            self.span_of(guard),
                            "compile-time match guard must be constant",
                        );
                        return CompileTimeFlow::Normal;
                    }
                }
            }
            let statements = self.block_statements(case.body);
            let errors_before = self.engine.error_count();
            let flow = self.execute_compile_time_statements(&statements, in_loop);
            if matches!(flow, CompileTimeFlow::Break | CompileTimeFlow::Continue)
                || self.engine.error_count() > errors_before
            {
                return flow;
            }
            break;
        }
        CompileTimeFlow::Normal
    }

    /// Executes a compile-time `for` loop over a range (§5.9.5, §11.4).
    fn execute_compile_time_for(&mut self, statement: &crate::ast::ForStmt) -> CompileTimeFlow {
        let crate::ast::ForTarget::Range(range) = statement.target.clone() else {
            if self.compile_time_phase_b {
                self.unsupported(
                    self.span_of(statement.body),
                    "compile-time 'for' loops over arrays/iterables",
                );
            }
            return CompileTimeFlow::Normal;
        };
        if statement.by_ref {
            if self.compile_time_phase_b {
                self.unsupported(
                    self.span_of(statement.body),
                    "by-reference compile-time 'for' bindings",
                );
            }
            return CompileTimeFlow::Normal;
        }
        let binding_type = match statement.ty {
            Some(ty_node) => match self.resolve_type(ty_node) {
                Some(ty) => Some(ty),
                None => return CompileTimeFlow::Normal,
            },
            None => self.result.node_types[range.start as usize],
        };
        let binding_name = statement.name.clone();
        self.suppress_errors += 1;
        let start = self.eval_const_int(range.start);
        let end = self.eval_const_int(range.end);
        let explicit_step = range.step.map(|step| self.eval_const_int(step));
        self.suppress_errors -= 1;
        let (Some(start), Some(end)) = (start, end) else {
            if self.compile_time_phase_b {
                self.error(
                    self.span_of(range.start),
                    "compile-time range bounds must be constant (§11.4)",
                );
            }
            return CompileTimeFlow::Normal;
        };
        let step = match explicit_step {
            Some(Some(step)) => step,
            Some(None) => {
                if self.compile_time_phase_b {
                    self.error(
                        self.span_of(range.start),
                        "compile-time range step must be constant (§11.4)",
                    );
                }
                return CompileTimeFlow::Normal;
            }
            None => match range.direction {
                crate::ast::RangeDirection::Ascending => 1,
                crate::ast::RangeDirection::Descending => -1,
                crate::ast::RangeDirection::Auto => {
                    if end >= start {
                        1
                    } else {
                        -1
                    }
                }
            },
        };
        if step == 0 {
            if self.compile_time_phase_b {
                self.error(self.span_of(range.start), "range step must not be zero");
            }
            return CompileTimeFlow::Normal;
        }
        let previous = binding_name
            .as_ref()
            .and_then(|name| self.top_level_constants.get(name).copied());
        let mut value = start;
        let mut iterations = 0usize;
        loop {
            let in_range = match range.direction {
                crate::ast::RangeDirection::Ascending => {
                    if range.inclusive {
                        value <= end
                    } else {
                        value < end
                    }
                }
                crate::ast::RangeDirection::Descending => {
                    if range.inclusive {
                        value >= end
                    } else {
                        value > end
                    }
                }
                crate::ast::RangeDirection::Auto => {
                    if range.inclusive == (step > 0) {
                        if step > 0 {
                            value <= end
                        } else {
                            value >= end
                        }
                    } else if step > 0 {
                        value < end
                    } else {
                        value > end
                    }
                }
            };
            if !in_range {
                break;
            }
            iterations += 1;
            let _ = iterations;
            if let (Some(name), Some(binding_type)) = (binding_name.as_ref(), binding_type) {
                self.top_level_constants
                    .insert(name.clone(), (value, binding_type));
            }
            let statements = self.block_statements(statement.body);
            match self.execute_compile_time_statements(&statements, true) {
                CompileTimeFlow::Break => break,
                CompileTimeFlow::Normal | CompileTimeFlow::Continue => {}
            }
            value = value.wrapping_add(step);
        }
        if let Some(name) = binding_name {
            if let Some(previous) = previous {
                self.top_level_constants.insert(name, previous);
            } else {
                self.top_level_constants.remove(&name);
            }
        }
        CompileTimeFlow::Normal
    }

    /// Smallest signed integer type able to represent `value`, for inferred
    /// compile-time variables (§5.2).
    fn inferred_integer_type(&mut self, value: i128) -> TypeId {
        let bytes = if i8::try_from(value).is_ok() {
            1
        } else if i16::try_from(value).is_ok() {
            2
        } else if i32::try_from(value).is_ok() {
            4
        } else if i64::try_from(value).is_ok() {
            8
        } else {
            16
        };
        self.result.types.integer(true, bytes)
    }

    /// Declares modules that appear directly inside a type body (§3.7.1): they
    /// only organize identifiers, so their members are declared in a fresh scope
    /// reachable through the type name.
    fn declare_type_body_modules(&mut self, node: NodeId) {
        let members: Vec<NodeId> = match self.node_kind(node) {
            NodeKind::Class(declaration) => declaration.members.clone(),
            NodeKind::Structure(declaration) => declaration.members.clone(),
            NodeKind::Interface(declaration) => declaration.members.clone(),
            NodeKind::Enumeration(declaration) => declaration.extensions.clone(),
            NodeKind::Variant(declaration) => declaration.extensions.clone(),
            NodeKind::Union(declaration) => declaration.extensions.clone(),
            _ => Vec::new(),
        };
        let Some(type_id) = self.type_id_for_node(node) else {
            return;
        };
        let Type::Named(index) = self.types_of(type_id) else {
            return;
        };
        for member in members {
            let NodeKind::Module(module) = self.node_kind(member).clone() else {
                continue;
            };
            let module_scope = self.create_child_scope();
            self.declare(
                module.name.clone(),
                Symbol::Module(module_scope),
                self.span_of(member),
            );
            self.result.type_defs[index as usize]
                .modules
                .insert(module.name.clone(), module_scope);
            self.scope_stack.push(module_scope);
            let nested = module.members;
            self.declare_types(&nested, false);
            self.scope_stack.pop();
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
            builtin_interface: None,
            fields: Vec::new(),
            field_initializers: Vec::new(),
            methods: std::collections::HashMap::new(),
            method_list: Vec::new(),
            accessors: Vec::new(),
            operators: Vec::new(),
            interfaces: Vec::new(),
            interface_dispatch: Vec::new(),
            interface_bases: Vec::new(),
            static_accessors: Vec::new(),
            modules: HashMap::new(),
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
                    self.resolve_type_body_modules(node);
                }
                NodeKind::Class(declaration) => {
                    self.resolve_structure_like(node, declaration.name.clone(), TypeDefKind::Class);
                    self.resolve_type_body_modules(node);
                }
                NodeKind::Enumeration(declaration) => {
                    self.resolve_enumeration(node, declaration.name.clone());
                    self.resolve_type_body_modules(node);
                }
                NodeKind::Variant(_declaration) => {
                    self.resolve_variant(node);
                    self.resolve_type_body_modules(node);
                }
                NodeKind::Union(_declaration) => {
                    self.resolve_union(node);
                    self.resolve_type_body_modules(node);
                }
                NodeKind::Interface(_declaration) => {
                    self.resolve_interface(node);
                    self.resolve_type_body_modules(node);
                }
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
                NodeKind::Accessor(declaration) => {
                    self.resolve_free_accessor(node, &declaration);
                }
                NodeKind::CompileTimeIf(statement) => {
                    let taken = self
                        .result
                        .compile_time_conditions
                        .get(&node)
                        .copied()
                        .unwrap_or(false);
                    let statements = if taken {
                        self.block_statements(statement.then_block)
                    } else {
                        statement
                            .else_block
                            .map(|else_block| self.block_statements(else_block))
                            .unwrap_or_default()
                    };
                    self.resolve_declarations(&statements);
                }
                // Top-level `#{}` blocks were executed during declaration
                // collection (§11.2); they declare no runtime members.
                NodeKind::CompileTimeBlock(_) => {}
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

/// Rounds `value` up to a multiple of `alignment`.
fn round_up_size(value: u64, alignment: u64) -> u64 {
    if alignment <= 1 {
        return value;
    }
    value.div_ceil(alignment) * alignment
}
