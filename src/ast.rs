//! The NovaSharp abstract syntax tree.
//!
//! The AST is arena-allocated: every node lives in [`Ast::nodes`] and is
//! referred to by a small [`NodeId`]. This keeps the tree cheap to copy and
//! traverse, avoids `Box`/`Rc` proliferation, and makes later rewrites
//! (sugar expansion, monomorphization) straightforward.
//!
//! Construction sugar defined by the spec is *not* represented separately:
//! - identifier definition shorthands (`Name(params): T`, `Name[dims]: T`) are
//!   expanded by the parser into an ordinary type ([§2.5])
//! - `fixed Name(params): T { ... }` becomes a `Fixed` definition whose type is
//!   a function type and whose initializer is the block ([§8.1])
//!
//! Every node carries the [`Span`] it was parsed from so that later phases can
//! report precise diagnostics.

use crate::source::{FileId, Span};

/// Index of a node inside [`Ast::nodes`].
pub type NodeId = u32;

/// The parsed representation of one source file.
#[derive(Debug, Default)]
pub struct Ast {
    /// All nodes, in creation order.
    pub nodes: Vec<Node>,
    /// Top-level statements of the file body.
    pub roots: Vec<NodeId>,
    /// File this AST was parsed from.
    pub file: FileId,
}

impl Ast {
    /// Creates an empty AST for `file`.
    #[must_use]
    pub fn new(file: FileId) -> Self {
        Self {
            nodes: Vec::new(),
            roots: Vec::new(),
            file,
        }
    }

    /// Returns the node with `id`.
    #[must_use]
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }

    /// Returns the node with `id`.
    #[must_use]
    pub fn node_mut(&mut self, id: NodeId) -> &mut Node {
        &mut self.nodes[id as usize]
    }

    /// Returns the span of `id`.
    #[must_use]
    pub fn span(&self, id: NodeId) -> Span {
        self.nodes[id as usize].span
    }

    /// Appends a node and returns its id.
    pub fn push(&mut self, span: Span, kind: NodeKind) -> NodeId {
        let id = u32::try_from(self.nodes.len()).expect("AST node count exceeds u32::MAX");
        self.nodes.push(Node { span, kind });
        id
    }
}

/// One AST node.
#[derive(Debug)]
pub struct Node {
    /// Source range this node was parsed from.
    pub span: Span,
    /// What the node is.
    pub kind: NodeKind,
}

/// Every AST node kind.
///
/// The variants are grouped: structure, declarations, statements, expressions,
/// literals, special names and types.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum NodeKind {
    // ------------------------------------------------------------------
    // Structure
    // ------------------------------------------------------------------
    /// A `{ ... }` block of statements. The vector holds the statement ids, in
    /// order.
    Block(Vec<NodeId>),
    /// A `verbatim { ... }` block (§9.8): same statements, optimization disabled.
    VerbatimBlock(NodeId),
    /// A no-optimize expression (`@(Expression)`, §14.3.1).
    VerbatimExpression(NodeId),

    // ------------------------------------------------------------------
    // Declarations (§3, §5.2-§5.5, §7, §8.1, §3.8)
    // ------------------------------------------------------------------
    /// `class` declaration.
    Class(ClassDecl),
    /// `structure` declaration.
    Structure(ClassDecl),
    /// `interface` declaration.
    Interface(ClassDecl),
    /// `enumeration` declaration.
    Enumeration(EnumerationDecl),
    /// `variant` declaration.
    Variant(VariantDecl),
    /// `union` declaration.
    Union(UnionDecl),
    /// `module` declaration.
    Module(ModuleDecl),
    /// `extension [for target] { ... }` declaration.
    Extension(ExtensionDecl),
    /// The target of an `extension for ...` declaration (§3.8.3).
    ExtensionTarget(ExtensionTarget),
    /// Type-body field definition.
    Field(FieldDecl),
    /// `accessor` definition.
    Accessor(AccessorDecl),
    /// `operator` definition.
    Operator(OperatorDecl),
    /// `local` definition (may declare several names).
    Local(LocalDecl),
    /// `heap` definition.
    Heap(HeapDecl),
    /// `fixed` definition (constant).
    Fixed(FixedDecl),
    /// `linkable` definition.
    Linkable(LinkableDecl),

    // ------------------------------------------------------------------
    // Statements (§5)
    // ------------------------------------------------------------------
    /// `use` statement.
    Use(UseStmt),
    /// `target = value` statement.
    Set(SetStmt),
    /// `target op= value` statement.
    CompoundSet(CompoundSetStmt),
    /// `assembly ...` inline-assembly statement (§5.2).
    Assembly(AssemblyStmt),
    /// `dispose target` statement.
    Dispose(DisposeStmt),
    /// `if` statement.
    If(IfStmt),
    /// `while` statement.
    While(LoopStmt),
    /// `do ... while` statement.
    DoWhile(DoWhileStmt),
    /// `for` statement.
    For(ForStmt),
    /// `match` statement.
    Match(MatchStmt),
    /// `break [label]` statement.
    Break(BreakStmt),
    /// `continue [label]` statement.
    Continue(ContinueStmt),
    /// `return [value]` statement.
    Return(ReturnStmt),
    /// `throw value` statement.
    Throw(ThrowStmt),
    /// `trap` statement.
    Trap(TrapStmt),
    /// `#{ ... }` compile-time block.
    CompileTimeBlock(NodeId),
    /// `#if cond { ... } [else { ... }]`.
    CompileTimeIf(CompileTimeIfStmt),
    /// An expression evaluated for its effect (only function calls are legal).
    ExpressionStmt(NodeId),

    // ------------------------------------------------------------------
    // Expressions (§4)
    // ------------------------------------------------------------------
    /// A bare name reference.
    Name(NameExpr),
    /// `base.name` / `expr.name`.
    MemberAccess(MemberAccess),
    /// `base->name` / `expr->name`.
    PointerMemberAccess(MemberAccess),
    /// `base[index]`.
    Index(IndexExpr),
    /// `callee(arguments)`.
    Call(CallExpr),
    /// `Type(arguments)` construction.
    Construct(ConstructExpr),
    /// `Type.member(value)` member construction.
    MemberConstruct(MemberConstructExpr),
    /// `Cast<Type>(expr)`.
    Cast(CastExpr),
    /// `expr is target`.
    Is(IsExpr),
    /// Prefix unary operator.
    Unary(UnaryExpr),
    /// Infix binary operator.
    Binary(BinaryExpr),
    /// `cond ? then : else`.
    Conditional(ConditionalExpr),
    /// `expr ?? alternative`.
    Coalesce(CoalesceExpr),
    /// `{ e1, e2, [value : count], [start..end] }`.
    ArrayLiteral(ArrayLiteral),
    /// `(params) Type => { ... }`.
    FunctionLiteral(FunctionLiteral),
    /// `#(expr)` compile-time expression.
    CompileTimeExpr(NodeId),
    /// `target<args>` monomorphization.
    Monomorphization(MonomorphizationExpr),

    // ------------------------------------------------------------------
    // Literals (§2.7)
    // ------------------------------------------------------------------
    /// Integer literal; the value is decoded by [`crate::literal`].
    IntegerLiteral(IntLiteral),
    /// Float literal.
    FloatLiteral(FloatLiteral),
    /// Character literal.
    CharacterLiteral(CharacterLiteral),
    /// Normal string literal.
    StringLiteral(StringLiteral),
    /// Verbatim string literal.
    VerbatimStringLiteral(StringLiteral),
    /// `true` / `false`.
    BooleanLiteral(bool),
    /// `float(N).NotANumber` / `float(N).Infinite` (§3.3).
    FloatSpecialLiteral(FloatSpecialLiteral),
    /// `uninitialized`.
    UninitializedLiteral,

    // ------------------------------------------------------------------
    // Special names (§2.6)
    // ------------------------------------------------------------------
    /// `this`.
    This,
    /// `owner`.
    Owner,
    /// `base`.
    Base,

    // ------------------------------------------------------------------
    // Types (§3)
    // ------------------------------------------------------------------
    /// `integer(N)`, `cardinal(N)`, `float(N)` or `boolean`.
    BasicType(BasicType),
    /// `array(dims...) element`.
    ArrayType(ArrayType),
    /// `pointer [target]` (§3.6.2; the target may be omitted).
    PointerType(Option<NodeId>),
    /// `function(...) return_type`.
    FunctionType(FunctionType),
    /// A named type, optionally monomorphized.
    NamedType(NamedType),
}

// ======================================================================
// Modifiers (§9)
// ======================================================================

/// Which base accessibility modifier was written, if any.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Accessibility {
    /// No base accessibility modifier.
    #[default]
    Unspecified,
    /// `public`
    Public,
    /// `private`
    Private,
    /// `protected`
    Protected,
}

/// A `scoped(target, ...)` modifier target (§9.5).
#[derive(Clone, Debug)]
pub enum ScopedTarget {
    /// `[sealed] Identifier`
    Named {
        /// Whether the `sealed` keyword was written.
        sealed: bool,
        /// The target name (a member access chain is allowed by §9.5).
        name: String,
    },
    /// `operator [Identifier | OperatorSymbol]`
    Operator {
        /// The operator's name, if one was written.
        name: Option<String>,
    },
}

/// The set of modifiers that can precede a definition (§9).
#[derive(Clone, Debug, Default)]
pub struct Modifiers {
    /// Base accessibility.
    pub accessibility: Accessibility,
    /// `scoped(...)` targets.
    pub scoped: Vec<ScopedTarget>,
    /// `readonly`
    pub readonly: bool,
    /// `implicitable`
    pub implicitable: bool,
    /// `verbatim`
    pub verbatim: bool,
    /// `manual`
    pub manual: bool,
    /// `abstract`
    pub abstract_: bool,
    /// `sealed`
    pub sealed: bool,
    /// `virtual`
    pub virtual_: bool,
    /// `override`
    pub override_: bool,
    /// `aligned(N)`
    pub aligned: Option<u32>,
    /// `static`
    pub static_: bool,
}

// ======================================================================
// Declarations
// ======================================================================

/// One generic parameter definition (§10.1).
#[derive(Clone, Debug)]
pub struct GenericParameter {
    /// Parameter name.
    pub name: String,
    /// Constraints (`class`, `structure`, `interface`, `enumeration`,
    /// `variant`, `union`, `basic`, or a named type).
    pub constraints: Vec<String>,
    /// Span of the definition.
    pub span: Span,
}

/// `class` / `structure` / `interface` declaration (§3.7.1-3.7.3).
#[derive(Clone, Debug)]
pub struct ClassDecl {
    /// Modifiers preceding the keyword.
    pub modifiers: Modifiers,
    /// Declared name.
    pub name: String,
    /// Generic parameters.
    pub generics: Vec<GenericParameter>,
    /// Base list (superclass and implemented interfaces).
    pub bases: Vec<NodeId>,
    /// Members (fields, accessors, modules, extensions, operators, ...).
    pub members: Vec<NodeId>,
}

/// `enumeration` declaration (§3.7.4).
#[derive(Clone, Debug)]
pub struct EnumerationDecl {
    /// Modifiers.
    pub modifiers: Modifiers,
    /// Declared name.
    pub name: String,
    /// Underlying basic type.
    pub underlying: NodeId,
    /// Members (`Name = Expression;`).
    pub members: Vec<EnumerationMember>,
    /// Embedded extension declarations.
    pub extensions: Vec<NodeId>,
}

/// One `Name = Expression;` enumeration member.
#[derive(Clone, Debug)]
pub struct EnumerationMember {
    /// Member name.
    pub name: String,
    /// Member value expression; `None` means the value is implicit.
    pub value: Option<NodeId>,
    /// Span of the member.
    pub span: Span,
}

/// `variant` declaration (§3.7.5).
#[derive(Clone, Debug)]
pub struct VariantDecl {
    /// Modifiers.
    pub modifiers: Modifiers,
    /// Declared name.
    pub name: String,
    /// Generic parameters.
    pub generics: Vec<GenericParameter>,
    /// State declarations (`Name [":" Type]`).
    pub members: Vec<VariantMember>,
    /// Embedded extensions.
    pub extensions: Vec<NodeId>,
}

/// One variant state.
#[derive(Clone, Debug)]
pub struct VariantMember {
    /// State name.
    pub name: String,
    /// Optional payload type.
    pub payload: Option<NodeId>,
    /// Span of the member.
    pub span: Span,
}

/// `union` declaration (§3.7.6).
#[derive(Clone, Debug)]
pub struct UnionDecl {
    /// Modifiers.
    pub modifiers: Modifiers,
    /// Declared name.
    pub name: String,
    /// Generic parameters.
    pub generics: Vec<GenericParameter>,
    /// Members (`{Modifier} Name ":" Type ";"`).
    pub members: Vec<UnionMember>,
    /// Embedded extensions.
    pub extensions: Vec<NodeId>,
}

/// One union member.
#[derive(Clone, Debug)]
pub struct UnionMember {
    /// Member modifiers.
    pub modifiers: Modifiers,
    /// Member name.
    pub name: String,
    /// Member type (required).
    pub ty: NodeId,
    /// Span of the member.
    pub span: Span,
}

/// `module` declaration (§7).
#[derive(Clone, Debug)]
pub struct ModuleDecl {
    /// Modifiers.
    pub modifiers: Modifiers,
    /// Module name.
    pub name: String,
    /// Members.
    pub members: Vec<NodeId>,
}

/// `extension [for target] { ... }` (§3.8.3).
#[derive(Clone, Debug)]
pub struct ExtensionDecl {
    /// Extension target, if written (`extension for T`).
    pub target: Option<NodeId>,
    /// Members.
    pub members: Vec<NodeId>,
}

/// Type-body field definition (§3.8.1).
#[derive(Clone, Debug)]
pub struct FieldDecl {
    /// Modifiers.
    pub modifiers: Modifiers,
    /// Field name.
    pub name: String,
    /// Field type, if written.
    pub ty: Option<NodeId>,
    /// Initializer (expression or block).
    pub init: Option<Initializer>,
}

/// An initializer: either an expression or a block (for function-typed fields).
#[derive(Clone, Copy, Debug)]
pub enum Initializer {
    /// `= Expression`
    Expression(NodeId),
    /// `Block`
    Block(NodeId),
}

/// `accessor Name: Type [=> backing] { get/set ... }` (§3.8.2).
#[derive(Clone, Debug)]
pub struct AccessorDecl {
    /// Modifiers.
    pub modifiers: Modifiers,
    /// Accessor name.
    pub name: String,
    /// Accessor type.
    pub ty: NodeId,
    /// Backing field, if written.
    pub backing: Option<NodeId>,
    /// `get` / `set` members.
    pub members: Vec<AccessorMember>,
}

/// One `get` / `set` member of an accessor.
#[derive(Clone, Debug)]
pub struct AccessorMember {
    /// Modifiers.
    pub modifiers: Modifiers,
    /// Whether this is the getter (`true`) or setter (`false`).
    pub is_getter: bool,
    /// Explicit member type, if written.
    pub ty: Option<NodeId>,
    /// `= Expression` form.
    pub expression: Option<NodeId>,
    /// Block form with explicit `this` parameter.
    pub this_type: Option<NodeId>,
    /// Whether the explicit `this` parameter was written with `&`.
    pub this_by_ref: bool,
    /// Block form's value parameter type (setter only).
    pub value_type: Option<NodeId>,
    /// Block form body.
    pub body: Option<NodeId>,
    /// Span of the member.
    pub span: Span,
}

/// `operator ...` definition (§4.9).
#[derive(Clone, Debug)]
pub struct OperatorDecl {
    /// Modifiers.
    pub modifiers: Modifiers,
    /// The operator head.
    pub head: OperatorHead,
    /// `= Expression`, block body or accessor members.
    pub body: OperatorBody,
}

/// The head of an operator definition (§4.9).
#[derive(Clone, Debug)]
pub enum OperatorHead {
    /// `+ (params): Type` shorthand form.
    Shorthand {
        /// The operator symbol.
        symbol: String,
        /// Parameter types (names optional).
        parameters: Vec<Parameter>,
        /// Return type.
        return_type: Option<NodeId>,
    },
    /// `+ : Type` notation form.
    Notation {
        /// The operator symbol.
        symbol: String,
        /// Return type.
        return_type: Option<NodeId>,
    },
    /// `[params]` indexer notation.
    Indexer {
        /// Parameters.
        parameters: Vec<Parameter>,
        /// Return type.
        return_type: Option<NodeId>,
    },
    /// Conversion operator notation (`ConstructedTypeName`).
    Conversion {
        /// The constructed type name.
        type_: NodeId,
        /// Parameters.
        parameters: Vec<Parameter>,
        /// Return type.
        return_type: Option<NodeId>,
    },
}

/// Body of an operator definition.
#[derive(Clone, Debug)]
pub enum OperatorBody {
    /// No body.
    None,
    /// `= Expression`
    Expression(NodeId),
    /// A block body.
    Block(NodeId),
    /// Accessor members (indexer `get`/`set`).
    AccessorMembers(Vec<AccessorMember>),
}

/// `local Name: Type = value, ...` (§5.2).
#[derive(Clone, Debug)]
pub struct LocalDecl {
    /// Modifiers.
    pub modifiers: Modifiers,
    /// Declared items.
    pub items: Vec<LocalItem>,
}

/// One item of a `local` definition.
#[derive(Clone, Debug)]
pub struct LocalItem {
    /// Whether the storage is marked no-optimize (`local @Name`, §14.3.1).
    pub verbatim: bool,
    /// Name.
    pub name: String,
    /// Type, if written.
    pub ty: Option<NodeId>,
    /// Initializer, if written.
    pub init: Option<NodeId>,
    /// Span of the item.
    pub span: Span,
}

/// `heap (size) Name: Type = value` (§5.4).
#[derive(Clone, Debug)]
pub struct HeapDecl {
    /// Modifiers.
    pub modifiers: Modifiers,
    /// Optional explicit allocation size.
    pub size: Option<NodeId>,
    /// Name.
    pub name: String,
    /// Type, if written.
    pub ty: Option<NodeId>,
    /// Initializer, if written.
    pub init: Option<NodeId>,
}

/// `fixed Name [generics] [: Type] [= value | Block]` (§5.3).
#[derive(Clone, Debug)]
pub struct FixedDecl {
    /// Modifiers.
    pub modifiers: Modifiers,
    /// Name.
    pub name: String,
    /// Generic parameters.
    pub generics: Vec<GenericParameter>,
    /// Type, if written.
    pub ty: Option<NodeId>,
    /// Initializer.
    pub init: Option<Initializer>,
}

/// `linkable [(symbol)] Name ...` (§5.5).
#[derive(Clone, Debug)]
pub struct LinkableDecl {
    /// Modifiers.
    pub modifiers: Modifiers,
    /// Explicit external symbol name, if written.
    pub symbol: Option<String>,
    /// Name.
    pub name: String,
    /// Generic parameters.
    pub generics: Vec<GenericParameter>,
    /// Type, if written.
    pub ty: Option<NodeId>,
    /// Initializer.
    pub init: Option<Initializer>,
}

/// A parameter type in a `function` type (§8.2).
#[derive(Clone, Debug)]
pub struct Parameter {
    /// Whether the parameter is a compile-time constant (`#`).
    pub compile_time: bool,
    /// Parameter name, if written.
    pub name: Option<String>,
    /// Whether the parameter is passed by reference (`&`).
    pub by_ref: bool,
    /// Parameter type.
    pub ty: Option<NodeId>,
    /// Default value, if written.
    pub default: Option<NodeId>,
    /// Span of the parameter.
    pub span: Span,
}

// ======================================================================
// Statements
// ======================================================================

/// `use a, b.c` (§5.6).
#[derive(Clone, Debug)]
pub struct UseStmt {
    /// Imported identifiers (name chains represented as AST expression nodes).
    pub targets: Vec<NodeId>,
}

/// Assignment statement (§5.7).
#[derive(Clone, Debug)]
pub struct SetStmt {
    /// Assignment target.
    pub target: NodeId,
    /// Assigned value.
    pub value: NodeId,
}

/// Compound assignment statement (§5.7).
#[derive(Clone, Debug)]
pub struct CompoundSetStmt {
    /// Assignment target.
    pub target: NodeId,
    /// Assigned value.
    pub value: NodeId,
    /// The arithmetic/bitwise operator combined with the assignment.
    pub op: BinaryOp,
}

/// Operand direction of an inline-assembly placeholder (§5.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssemblyDirection {
    /// No marker: read then write.
    ReadWrite,
    /// `:in`: input only.
    In,
    /// `:out`: output only.
    Out,
}

/// One piece of inline-assembly content (§5.2).
#[derive(Clone, Debug)]
pub enum AssemblySegment {
    /// Literal assembly text (escapes already decoded).
    Text(String),
    /// `@[Expression(:in|:out)]`.
    Placeholder {
        /// Operand expression.
        expression: NodeId,
        /// Operand direction.
        direction: AssemblyDirection,
    },
}

/// `assembly [with clobber(...)] { ... }` statement (§5.2).
#[derive(Clone, Debug)]
pub struct AssemblyStmt {
    /// Clobber declarations, if written.
    pub clobbers: Option<Vec<String>>,
    /// Assembly text and operand placeholders in source order.
    pub segments: Vec<AssemblySegment>,
}

/// `dispose target` (§5.8).
#[derive(Clone, Debug)]
pub struct DisposeStmt {
    /// Target to dispose.
    pub target: NodeId,
}

/// `if` statement (§5.9.2).
#[derive(Clone, Debug)]
pub struct IfStmt {
    /// `if` / `else if` branches, in order.
    pub branches: Vec<IfBranch>,
    /// Trailing `else` block, if any.
    pub else_block: Option<NodeId>,
}

/// One `if`/`else if` branch.
#[derive(Clone, Debug)]
pub struct IfBranch {
    /// Branch condition.
    pub condition: NodeId,
    /// Branch body.
    pub body: NodeId,
    /// Span of the branch header.
    pub span: Span,
}

/// `while condition [: label] { ... }` (§5.9.3).
#[derive(Clone, Debug)]
pub struct LoopStmt {
    /// Loop condition.
    pub condition: NodeId,
    /// Loop label.
    pub label: Option<String>,
    /// Loop body.
    pub body: NodeId,
}

/// `do [: label] { ... } while condition` (§5.9.4).
#[derive(Clone, Debug)]
pub struct DoWhileStmt {
    /// Loop label.
    pub label: Option<String>,
    /// Loop body.
    pub body: NodeId,
    /// Loop condition.
    pub condition: NodeId,
}

/// What a `for` statement traverses (§5.9.5).
#[derive(Clone, Debug)]
pub enum ForTarget {
    /// Traverse any expression (array or iterable).
    Expression(NodeId),
    /// Traverse an explicit range.
    Range(RangeExpr),
}

/// Direction imposed by a range operator (§5.9.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeDirection {
    /// `..` / `..!`: the direction follows the start/end comparison.
    Auto,
    /// `..<` / `..<=`: ascending.
    Ascending,
    /// `..>` / `..>=`: descending.
    Descending,
}

/// An explicit `for` range (§5.9.5).
#[derive(Clone, Debug)]
pub struct RangeExpr {
    /// Range start.
    pub start: NodeId,
    /// Explicit step, if written.
    pub step: Option<NodeId>,
    /// Range end.
    pub end: NodeId,
    /// Whether the end is included.
    pub inclusive: bool,
    /// Operator-imposed direction.
    pub direction: RangeDirection,
}

/// `for [&] Name : Type in target [: label] { ... }` (§5.9.5).
#[derive(Clone, Debug)]
pub struct ForStmt {
    /// Whether the iteration variable is taken by reference.
    pub by_ref: bool,
    /// Iteration variable name (`None` when the binding is omitted).
    pub name: Option<String>,
    /// Iteration variable type (`None` when omitted).
    pub ty: Option<NodeId>,
    /// Traversed target.
    pub target: ForTarget,
    /// Loop label.
    pub label: Option<String>,
    /// Loop body.
    pub body: NodeId,
}

/// `match value [: label] { case ... }` (§5.9.6).
#[derive(Clone, Debug)]
pub struct MatchStmt {
    /// Matched expression.
    pub value: NodeId,
    /// Match label.
    pub label: Option<String>,
    /// Cases.
    pub cases: Vec<MatchCase>,
}

/// One `match` case.
#[derive(Clone, Debug)]
pub struct MatchCase {
    /// Pattern, if written.
    pub pattern: Option<Pattern>,
    /// `when` guard, if written.
    pub guard: Option<NodeId>,
    /// Case body.
    pub body: NodeId,
    /// Span of the case header.
    pub span: Span,
}

/// A `match` pattern (§5.9.6).
#[derive(Clone, Debug)]
pub enum Pattern {
    /// Equality pattern (`Expression`).
    Equal(NodeId),
    /// Comparison pattern (`op Expression`).
    Comparison {
        /// The comparison operator token kind.
        operator: crate::lexer::TokenKind,
        /// The compared value.
        value: NodeId,
        /// Span of the operator.
        span: Span,
    },
    /// `is [Name :] IsTarget`.
    Is {
        /// Optional binding name.
        name: Option<String>,
        /// The `is` target.
        target: IsTarget,
    },
    /// `wildcard`
    Wildcard,
}

/// Target of an `is` expression or pattern (§4.8).
#[derive(Clone, Debug)]
pub enum IsTarget {
    /// A named type (or special float target).
    Identifier {
        /// Source text of the target (`Type`, `Module.Type`, `Variant.State`).
        text: String,
        /// The parsed name/member-access chain; lets monomorphized targets such
        /// as `Box<integer(4)>.Some` resolve (§10.2).
        node: NodeId,
    },
    /// `float(N).NotANumber` (§3.3, §4.8)
    FloatSpecial {
        /// Width in bytes (4 or 8).
        bytes: u8,
    },
}

/// `break [label]` (§5.9.7).
#[derive(Clone, Debug)]
pub struct BreakStmt {
    /// Label, if written.
    pub label: Option<String>,
}

/// `continue [label]` (§5.9.8).
#[derive(Clone, Debug)]
pub struct ContinueStmt {
    /// Label, if written.
    pub label: Option<String>,
}

/// `return [value]` (§5.10.2).
#[derive(Clone, Debug)]
pub struct ReturnStmt {
    /// Returned value, if written.
    pub value: Option<NodeId>,
}

/// `throw value` (§5.10.3).
#[derive(Clone, Debug)]
pub struct ThrowStmt {
    /// Thrown value.
    pub value: NodeId,
}

/// `trap { ... } catch ... finally { ... }` (§5.10.4).
#[derive(Clone, Debug)]
pub struct TrapStmt {
    /// Protected body.
    pub body: NodeId,
    /// Catch clauses.
    pub catches: Vec<CatchClause>,
    /// Finally block, if written.
    pub finally: Option<NodeId>,
}

/// One `catch` clause.
#[derive(Clone, Debug)]
pub struct CatchClause {
    /// Binding name, if written.
    pub name: Option<String>,
    /// Caught type, if written.
    pub ty: Option<NodeId>,
    /// Whether the clause is `catch wildcard`.
    pub wildcard: bool,
    /// Handler body.
    pub body: NodeId,
    /// Span of the clause header.
    pub span: Span,
}

/// `#if condition { ... } [else { ... }]` (§11.6).
#[derive(Clone, Debug)]
pub struct CompileTimeIfStmt {
    /// Condition.
    pub condition: NodeId,
    /// Taken branch.
    pub then_block: NodeId,
    /// Alternative branch, if written.
    pub else_block: Option<NodeId>,
}

// ======================================================================
// Expressions
// ======================================================================

/// A bare name expression.
#[derive(Clone, Debug)]
pub struct NameExpr {
    /// The name as written.
    pub name: String,
}

/// Member access (`base.member` or `base->member`).
#[derive(Clone, Debug)]
pub struct MemberAccess {
    /// The base: a name, type or expression.
    pub base: NodeId,
    /// The accessed member name.
    pub name: String,
}

/// Index access (`base[index]`).
#[derive(Clone, Debug)]
pub struct IndexExpr {
    /// The indexed value.
    pub base: NodeId,
    /// The index expression.
    pub index: NodeId,
}

/// One argument of a call (§8.3).
#[derive(Clone, Debug)]
pub struct Argument {
    /// Explicit parameter name, if written.
    pub name: Option<String>,
    /// The argument value.
    pub value: NodeId,
    /// Span of the argument.
    pub span: Span,
}

/// A function call.
#[derive(Clone, Debug)]
pub struct CallExpr {
    /// The callee.
    pub callee: NodeId,
    /// Call arguments.
    pub arguments: Vec<Argument>,
}

/// A construction expression (`Type(args)`).
#[derive(Clone, Debug)]
pub struct ConstructExpr {
    /// The constructed type.
    pub type_: NodeId,
    /// Construction arguments.
    pub arguments: Vec<Argument>,
}

/// A member construction expression (`Type.member(args)`).
#[derive(Clone, Debug)]
pub struct MemberConstructExpr {
    /// The constructed type.
    pub type_: NodeId,
    /// The member name.
    pub member: String,
    /// The single value argument.
    pub value: NodeId,
}

/// An explicit conversion (`Cast<Type>(value)`).
#[derive(Clone, Debug)]
pub struct CastExpr {
    /// Target type.
    pub type_: NodeId,
    /// Converted value.
    pub value: NodeId,
}

/// An `is` expression.
#[derive(Clone, Debug)]
pub struct IsExpr {
    /// Tested value.
    pub value: NodeId,
    /// Test target.
    pub target: IsTarget,
}

/// Prefix unary operators.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UnaryOp {
    /// `-`
    Negate,
    /// `!`
    LogicalNot,
    /// `^` (bitwise not)
    BitwiseNot,
    /// `&` (take reference)
    Reference,
    /// `*` (dereference)
    Dereference,
}

/// A prefix unary expression.
#[derive(Clone, Debug)]
pub struct UnaryExpr {
    /// The operator.
    pub op: UnaryOp,
    /// The operand.
    pub operand: NodeId,
}

/// Infix binary operators and their precedence (§4.9.1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BinaryOp {
    /// `*`
    Multiply,
    /// `/`
    Divide,
    /// `%`
    Remainder,
    /// `+`
    Add,
    /// `-`
    Subtract,
    /// `<<`
    ShiftLeft,
    /// `>>`
    ShiftRight,
    /// `<<<`
    CircularShiftLeft,
    /// `>>>`
    CircularShiftRight,
    /// `<`
    Less,
    /// `>`
    Greater,
    /// `<=`
    LessEqual,
    /// `>=`
    GreaterEqual,
    /// `==`
    Equal,
    /// `!=`
    NotEqual,
    /// `&`
    BitwiseAnd,
    /// `^`
    BitwiseXor,
    /// `|`
    BitwiseOr,
    /// `&&`
    LogicalAnd,
    /// `||`
    LogicalOr,
}

impl BinaryOp {
    /// The operator's precedence: higher binds tighter (§4.9.1).
    #[must_use]
    pub const fn precedence(self) -> u8 {
        use BinaryOp::{
            Add, BitwiseAnd, BitwiseOr, BitwiseXor, CircularShiftLeft, CircularShiftRight, Divide,
            Equal, Greater, GreaterEqual, Less, LessEqual, LogicalAnd, LogicalOr, Multiply,
            NotEqual, Remainder, ShiftLeft, ShiftRight, Subtract,
        };
        match self {
            Multiply | Divide | Remainder => 12,
            Add | Subtract => 11,
            ShiftLeft | ShiftRight | CircularShiftLeft | CircularShiftRight => 10,
            Less | Greater | LessEqual | GreaterEqual => 9,
            Equal | NotEqual => 8,
            BitwiseAnd => 7,
            BitwiseXor => 6,
            BitwiseOr => 5,
            LogicalAnd => 4,
            LogicalOr => 3,
        }
    }
}

/// An infix binary expression.
#[derive(Clone, Debug)]
pub struct BinaryExpr {
    /// The operator.
    pub op: BinaryOp,
    /// Left operand.
    pub left: NodeId,
    /// Right operand.
    pub right: NodeId,
}

/// A conditional expression.
#[derive(Clone, Debug)]
pub struct ConditionalExpr {
    /// Condition.
    pub condition: NodeId,
    /// Value when the condition holds.
    pub then_value: NodeId,
    /// Value when the condition fails.
    pub else_value: NodeId,
}

/// What follows `??` (§4.9.26).
#[derive(Clone, Debug)]
pub enum CoalescingAlternative {
    /// An ordinary alternative expression.
    Expression(NodeId),
    /// `throw [Expression]`
    Throw(Option<NodeId>),
    /// `return [Expression]`
    Return(Option<NodeId>),
}

/// A null-coalescing expression.
#[derive(Clone, Debug)]
pub struct CoalesceExpr {
    /// Left operand.
    pub value: NodeId,
    /// Alternative.
    pub alternative: CoalescingAlternative,
}

/// One element of an array literal (§2.7.8).
#[derive(Clone, Debug)]
pub enum ArrayElement {
    /// A single value.
    Value(NodeId),
    /// `[value : count]`
    Repeat {
        /// Repeated value.
        value: NodeId,
        /// Repeat count.
        count: NodeId,
    },
    /// `[start..step..end]` / `[start..!end]`
    Range(RangeExpr),
}

/// An array literal.
#[derive(Clone, Debug)]
pub struct ArrayLiteral {
    /// Elements, in order.
    pub elements: Vec<ArrayElement>,
}

/// A function literal (§2.7.9).
#[derive(Clone, Debug)]
pub struct FunctionLiteral {
    /// Parameters.
    pub parameters: Vec<LiteralParameter>,
    /// Explicit calling convention expression, if written.
    pub calling_convention: Option<NodeId>,
    /// Return type, if written.
    pub return_type: Option<NodeId>,
    /// Function body: a block, or a single expression (implicit return).
    pub body: NodeId,
}

/// A parameter of a function literal (§8.2).
#[derive(Clone, Debug)]
pub struct LiteralParameter {
    /// Whether the parameter is a compile-time constant (`#`).
    pub compile_time: bool,
    /// Parameter name (required for literals).
    pub name: String,
    /// Whether the parameter is passed by reference (`&`).
    pub by_ref: bool,
    /// Parameter type, if written.
    pub ty: Option<NodeId>,
    /// Span of the parameter.
    pub span: Span,
}

/// A monomorphization expression (`target<T; U>`, §10.2).
#[derive(Clone, Debug)]
pub struct MonomorphizationExpr {
    /// Base identifier or expression.
    pub base: NodeId,
    /// Type arguments; `None` means "let the compiler infer".
    pub arguments: Vec<Option<NodeId>>,
}

// ======================================================================
// Literals
// ======================================================================

/// An integer literal token, kept as written.
#[derive(Clone, Debug)]
pub struct IntLiteral {
    /// Raw literal text (including base prefix and `_` separators).
    pub raw: String,
}

/// A float literal token, kept as written.
#[derive(Clone, Debug)]
pub struct FloatLiteral {
    /// Raw literal text.
    pub raw: String,
}

/// A float special value (§3.3).
#[derive(Clone, Debug)]
pub struct FloatSpecialLiteral {
    /// Width in bytes (4 or 8).
    pub bytes: u8,
    /// Whether this is `Infinite` (otherwise `NotANumber`).
    pub infinite: bool,
}

/// A character literal, kept as written (encoding tag included).
#[derive(Clone, Debug)]
pub struct CharacterLiteral {
    /// Raw literal text.
    pub raw: String,
}

/// A string literal, kept as written (encoding tag included).
#[derive(Clone, Debug)]
pub struct StringLiteral {
    /// Raw literal text.
    pub raw: String,
}

// ======================================================================
// Types
// ======================================================================

/// Which basic type constructor (§3.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BasicKind {
    /// `integer(N)`
    Integer,
    /// `cardinal(N)`
    Cardinal,
    /// `float(N)`
    Float,
    /// `boolean`
    Boolean,
}

/// A basic type (§3.2).
#[derive(Clone, Debug)]
pub struct BasicType {
    /// Which basic type.
    pub kind: BasicKind,
    /// The `N` type argument, for `integer`/`cardinal`/`float`.
    pub size: Option<u32>,
    /// Span of the `N` literal, for diagnostics.
    pub size_span: Option<Span>,
}

/// One dimension of an array type (§3.6.1).
#[derive(Clone, Debug)]
pub enum ArrayDimension {
    /// A concrete size expression, e.g. `array(5)`.
    Size(NodeId),
    /// A named compile-time binding, e.g. `array(length: cardinal(4))`.
    Binding {
        /// Binding name.
        name: String,
        /// Bound type (usually `cardinal(N)`).
        ty: NodeId,
    },
}

/// An array type (`array(dims) element`, §3.6.1).
#[derive(Clone, Debug)]
pub struct ArrayType {
    /// Dimensions.
    pub dimensions: Vec<ArrayDimension>,
    /// Element type.
    pub element: NodeId,
}

/// A function type (`function(params) return`, §3.6.3).
#[derive(Clone, Debug)]
pub struct FunctionType {
    /// Parameters.
    pub parameters: Vec<Parameter>,
    /// Whether the parameter list ended in `wildcard`.
    pub wildcard: bool,
    /// Explicit calling convention expression, if written.
    pub calling_convention: Option<NodeId>,
    /// Return type; absent means the empty type.
    pub return_type: Option<NodeId>,
}

/// The target of an `extension for ...` declaration (§3.8.3).
#[derive(Clone, Debug)]
pub enum ExtensionTarget {
    /// A concrete type, e.g. `extension for Vector3`.
    Type(NodeId),
    /// A generic target, e.g. `extension for MyClass<T>`.
    Generic {
        /// Target name.
        name: String,
        /// Generic parameters declared by the target.
        generics: Vec<GenericParameter>,
    },
    /// An array target with bound dimensions, e.g.
    /// `extension for array(length: cardinal(4)) T`.
    Array {
        /// Bound dimensions (`length: cardinal(4)`).
        bindings: Vec<(String, NodeId)>,
        /// Element type parameter.
        parameter: GenericParameter,
    },
}

/// A named type (§3.1).
#[derive(Clone, Debug)]
pub struct NamedType {
    /// Name or member access chain.
    pub name: NodeId,
    /// Monomorphization type arguments, if written.
    pub arguments: Option<Vec<Option<NodeId>>>,
}
