//! Deep cloning of AST subtrees.
//!
//! The AST is arena-allocated and [`NodeKind`] variants refer to other nodes by
//! [`NodeId`], so the derived shallow `Clone` of a node kind would leave the
//! copy pointing back into the original tree. The methods here rebuild a whole
//! subtree, remapping every contained id to a freshly appended node.
//!
//! Cloning is structural: nothing is shared with the source subtree, spans are
//! preserved verbatim, and a node reachable through several paths is cloned
//! once per path.

use crate::ast::*;

impl Ast {
    /// Deep-clones the subtree rooted at `root`, appending new nodes and
    /// returning the new root id.
    ///
    /// Every node receives a fresh id; the copies are complete and independent
    /// of the originals. All spans are preserved exactly.
    pub fn clone_subtree(&mut self, root: NodeId) -> NodeId {
        let span = self.span(root);
        let kind = self.node(root).kind.clone();
        let cloned = self.clone_kind(&kind);
        self.push(span, cloned)
    }

    /// Deep-clones several root subtrees, in order.
    ///
    /// Each root is cloned independently, so shared nodes are duplicated rather
    /// than deduplicated.
    pub fn clone_subtrees(&mut self, roots: &[NodeId]) -> Vec<NodeId> {
        roots.iter().map(|&root| self.clone_subtree(root)).collect()
    }

    /// Clones `kind`, remapping every [`NodeId`] it contains.
    ///
    /// This match is deliberately exhaustive with no wildcard arm: adding a new
    /// [`NodeKind`] variant must fail to compile until it is handled here.
    fn clone_kind(&mut self, kind: &NodeKind) -> NodeKind {
        match kind {
            // -- Structure -------------------------------------------------
            NodeKind::Block(statements) => NodeKind::Block(self.clone_ids(statements)),
            NodeKind::VerbatimBlock(block) => NodeKind::VerbatimBlock(self.clone_subtree(*block)),
            NodeKind::VerbatimExpression(inner) => {
                NodeKind::VerbatimExpression(self.clone_subtree(*inner))
            }

            // -- Declarations ----------------------------------------------
            NodeKind::Class(decl) => NodeKind::Class(self.clone_class_decl(decl)),
            NodeKind::Structure(decl) => NodeKind::Structure(self.clone_class_decl(decl)),
            NodeKind::Interface(decl) => NodeKind::Interface(self.clone_class_decl(decl)),
            NodeKind::Enumeration(decl) => NodeKind::Enumeration(self.clone_enumeration_decl(decl)),
            NodeKind::Variant(decl) => NodeKind::Variant(self.clone_variant_decl(decl)),
            NodeKind::Union(decl) => NodeKind::Union(self.clone_union_decl(decl)),
            NodeKind::Module(decl) => NodeKind::Module(self.clone_module_decl(decl)),
            NodeKind::Extension(decl) => NodeKind::Extension(self.clone_extension_decl(decl)),
            NodeKind::ExtensionTarget(target) => {
                NodeKind::ExtensionTarget(self.clone_extension_target(target))
            }
            NodeKind::Field(decl) => NodeKind::Field(self.clone_field_decl(decl)),
            NodeKind::Accessor(decl) => NodeKind::Accessor(self.clone_accessor_decl(decl)),
            NodeKind::Operator(decl) => NodeKind::Operator(self.clone_operator_decl(decl)),
            NodeKind::Local(decl) => NodeKind::Local(self.clone_local_decl(decl)),
            NodeKind::Heap(decl) => NodeKind::Heap(self.clone_heap_decl(decl)),
            NodeKind::Fixed(decl) => NodeKind::Fixed(self.clone_fixed_decl(decl)),
            NodeKind::Linkable(decl) => NodeKind::Linkable(self.clone_linkable_decl(decl)),

            // -- Statements ------------------------------------------------
            NodeKind::Use(stmt) => NodeKind::Use(self.clone_use_stmt(stmt)),
            NodeKind::Set(stmt) => NodeKind::Set(self.clone_set_stmt(stmt)),
            NodeKind::CompoundSet(stmt) => {
                NodeKind::CompoundSet(self.clone_compound_set_stmt(stmt))
            }
            NodeKind::Dispose(stmt) => NodeKind::Dispose(self.clone_dispose_stmt(stmt)),
            NodeKind::If(stmt) => NodeKind::If(self.clone_if_stmt(stmt)),
            NodeKind::While(stmt) => NodeKind::While(self.clone_loop_stmt(stmt)),
            NodeKind::DoWhile(stmt) => NodeKind::DoWhile(self.clone_do_while_stmt(stmt)),
            NodeKind::For(stmt) => NodeKind::For(self.clone_for_stmt(stmt)),
            NodeKind::Match(stmt) => NodeKind::Match(self.clone_match_stmt(stmt)),
            NodeKind::Break(stmt) => NodeKind::Break(self.clone_break_stmt(stmt)),
            NodeKind::Continue(stmt) => NodeKind::Continue(self.clone_continue_stmt(stmt)),
            NodeKind::Return(stmt) => NodeKind::Return(self.clone_return_stmt(stmt)),
            NodeKind::Throw(stmt) => NodeKind::Throw(self.clone_throw_stmt(stmt)),
            NodeKind::Trap(stmt) => NodeKind::Trap(self.clone_trap_stmt(stmt)),
            NodeKind::CompileTimeBlock(block) => {
                NodeKind::CompileTimeBlock(self.clone_subtree(*block))
            }
            NodeKind::CompileTimeIf(stmt) => {
                NodeKind::CompileTimeIf(self.clone_compile_time_if_stmt(stmt))
            }
            NodeKind::ExpressionStmt(expr) => NodeKind::ExpressionStmt(self.clone_subtree(*expr)),

            // -- Expressions -----------------------------------------------
            NodeKind::Name(expr) => NodeKind::Name(expr.clone()),
            NodeKind::MemberAccess(access) => {
                NodeKind::MemberAccess(self.clone_member_access(access))
            }
            NodeKind::PointerMemberAccess(access) => {
                NodeKind::PointerMemberAccess(self.clone_member_access(access))
            }
            NodeKind::Index(expr) => NodeKind::Index(self.clone_index_expr(expr)),
            NodeKind::Call(expr) => NodeKind::Call(self.clone_call_expr(expr)),
            NodeKind::Construct(expr) => NodeKind::Construct(self.clone_construct_expr(expr)),
            NodeKind::MemberConstruct(expr) => {
                NodeKind::MemberConstruct(self.clone_member_construct_expr(expr))
            }
            NodeKind::Cast(expr) => NodeKind::Cast(self.clone_cast_expr(expr)),
            NodeKind::Is(expr) => NodeKind::Is(self.clone_is_expr(expr)),
            NodeKind::Unary(expr) => NodeKind::Unary(self.clone_unary_expr(expr)),
            NodeKind::Binary(expr) => NodeKind::Binary(self.clone_binary_expr(expr)),
            NodeKind::Conditional(expr) => NodeKind::Conditional(self.clone_conditional_expr(expr)),
            NodeKind::Coalesce(expr) => NodeKind::Coalesce(self.clone_coalesce_expr(expr)),
            NodeKind::ArrayLiteral(literal) => {
                NodeKind::ArrayLiteral(self.clone_array_literal(literal))
            }
            NodeKind::FunctionLiteral(literal) => {
                NodeKind::FunctionLiteral(self.clone_function_literal(literal))
            }
            NodeKind::CompileTimeExpr(expr) => NodeKind::CompileTimeExpr(self.clone_subtree(*expr)),
            NodeKind::Monomorphization(expr) => {
                NodeKind::Monomorphization(self.clone_monomorphization_expr(expr))
            }

            // -- Literals --------------------------------------------------
            NodeKind::IntegerLiteral(literal) => NodeKind::IntegerLiteral(literal.clone()),
            NodeKind::FloatLiteral(literal) => NodeKind::FloatLiteral(literal.clone()),
            NodeKind::CharacterLiteral(literal) => NodeKind::CharacterLiteral(literal.clone()),
            NodeKind::StringLiteral(literal) => NodeKind::StringLiteral(literal.clone()),
            NodeKind::VerbatimStringLiteral(literal) => {
                NodeKind::VerbatimStringLiteral(literal.clone())
            }
            NodeKind::BooleanLiteral(value) => NodeKind::BooleanLiteral(*value),
            NodeKind::InfiniteLiteral => NodeKind::InfiniteLiteral,
            NodeKind::IllegalLiteral => NodeKind::IllegalLiteral,
            NodeKind::UninitializedLiteral => NodeKind::UninitializedLiteral,

            // -- Special names ---------------------------------------------
            NodeKind::This => NodeKind::This,
            NodeKind::Owner => NodeKind::Owner,
            NodeKind::Base => NodeKind::Base,

            // -- Types -----------------------------------------------------
            NodeKind::BasicType(ty) => NodeKind::BasicType(ty.clone()),
            NodeKind::ArrayType(ty) => NodeKind::ArrayType(self.clone_array_type(ty)),
            NodeKind::PointerType(target) => NodeKind::PointerType(self.clone_opt_id(*target)),
            NodeKind::FunctionType(ty) => NodeKind::FunctionType(self.clone_function_type(ty)),
            NodeKind::NamedType(ty) => NodeKind::NamedType(self.clone_named_type(ty)),
        }
    }

    /// Clones every id in `ids`, in order.
    fn clone_ids(&mut self, ids: &[NodeId]) -> Vec<NodeId> {
        ids.iter().map(|&id| self.clone_subtree(id)).collect()
    }

    /// Clones an optional id.
    fn clone_opt_id(&mut self, id: Option<NodeId>) -> Option<NodeId> {
        id.map(|id| self.clone_subtree(id))
    }

    /// Clones optional monomorphization arguments (`None` means "infer").
    fn clone_opt_ids(&mut self, ids: &Option<Vec<Option<NodeId>>>) -> Option<Vec<Option<NodeId>>> {
        ids.as_ref().map(|ids| {
            ids.iter()
                .map(|&id| self.clone_opt_id(id))
                .collect::<Vec<_>>()
        })
    }

    /// Clones a `class` / `structure` / `interface` declaration.
    fn clone_class_decl(&mut self, decl: &ClassDecl) -> ClassDecl {
        ClassDecl {
            modifiers: decl.modifiers.clone(),
            name: decl.name.clone(),
            generics: decl.generics.clone(),
            bases: self.clone_ids(&decl.bases),
            members: self.clone_ids(&decl.members),
        }
    }

    /// Clones an `enumeration` declaration.
    fn clone_enumeration_decl(&mut self, decl: &EnumerationDecl) -> EnumerationDecl {
        EnumerationDecl {
            modifiers: decl.modifiers.clone(),
            name: decl.name.clone(),
            underlying: self.clone_subtree(decl.underlying),
            members: decl
                .members
                .iter()
                .map(|member| EnumerationMember {
                    name: member.name.clone(),
                    value: self.clone_opt_id(member.value),
                    span: member.span,
                })
                .collect(),
            extensions: self.clone_ids(&decl.extensions),
        }
    }

    /// Clones a `variant` declaration.
    fn clone_variant_decl(&mut self, decl: &VariantDecl) -> VariantDecl {
        VariantDecl {
            modifiers: decl.modifiers.clone(),
            name: decl.name.clone(),
            generics: decl.generics.clone(),
            members: decl
                .members
                .iter()
                .map(|member| VariantMember {
                    name: member.name.clone(),
                    payload: self.clone_opt_id(member.payload),
                    span: member.span,
                })
                .collect(),
            extensions: self.clone_ids(&decl.extensions),
        }
    }

    /// Clones a `union` declaration.
    fn clone_union_decl(&mut self, decl: &UnionDecl) -> UnionDecl {
        UnionDecl {
            modifiers: decl.modifiers.clone(),
            name: decl.name.clone(),
            generics: decl.generics.clone(),
            members: decl
                .members
                .iter()
                .map(|member| UnionMember {
                    modifiers: member.modifiers.clone(),
                    name: member.name.clone(),
                    ty: self.clone_subtree(member.ty),
                    span: member.span,
                })
                .collect(),
            extensions: self.clone_ids(&decl.extensions),
        }
    }

    /// Clones a `module` declaration.
    fn clone_module_decl(&mut self, decl: &ModuleDecl) -> ModuleDecl {
        ModuleDecl {
            modifiers: decl.modifiers.clone(),
            name: decl.name.clone(),
            members: self.clone_ids(&decl.members),
        }
    }

    /// Clones an `extension` declaration.
    fn clone_extension_decl(&mut self, decl: &ExtensionDecl) -> ExtensionDecl {
        ExtensionDecl {
            target: self.clone_opt_id(decl.target),
            members: self.clone_ids(&decl.members),
        }
    }

    /// Clones an `extension for ...` target.
    fn clone_extension_target(&mut self, target: &ExtensionTarget) -> ExtensionTarget {
        match target {
            ExtensionTarget::Type(ty) => ExtensionTarget::Type(self.clone_subtree(*ty)),
            ExtensionTarget::Generic { name, generics } => ExtensionTarget::Generic {
                name: name.clone(),
                generics: generics.clone(),
            },
            ExtensionTarget::Array {
                bindings,
                parameter,
            } => ExtensionTarget::Array {
                bindings: bindings
                    .iter()
                    .map(|(name, ty)| (name.clone(), self.clone_subtree(*ty)))
                    .collect(),
                parameter: parameter.clone(),
            },
        }
    }

    /// Clones a type-body field definition.
    fn clone_field_decl(&mut self, decl: &FieldDecl) -> FieldDecl {
        FieldDecl {
            modifiers: decl.modifiers.clone(),
            name: decl.name.clone(),
            ty: self.clone_opt_id(decl.ty),
            init: self.clone_initializer(decl.init),
        }
    }

    /// Clones an initializer, which may be an expression or a block.
    fn clone_initializer(&mut self, init: Option<Initializer>) -> Option<Initializer> {
        init.map(|init| match init {
            Initializer::Expression(expr) => Initializer::Expression(self.clone_subtree(expr)),
            Initializer::Block(block) => Initializer::Block(self.clone_subtree(block)),
        })
    }

    /// Clones an `accessor` definition.
    fn clone_accessor_decl(&mut self, decl: &AccessorDecl) -> AccessorDecl {
        AccessorDecl {
            modifiers: decl.modifiers.clone(),
            name: decl.name.clone(),
            ty: self.clone_subtree(decl.ty),
            backing: self.clone_opt_id(decl.backing),
            members: decl
                .members
                .iter()
                .map(|member| self.clone_accessor_member(member))
                .collect(),
        }
    }

    /// Clones one `get` / `set` accessor member.
    fn clone_accessor_member(&mut self, member: &AccessorMember) -> AccessorMember {
        AccessorMember {
            modifiers: member.modifiers.clone(),
            is_getter: member.is_getter,
            ty: self.clone_opt_id(member.ty),
            expression: self.clone_opt_id(member.expression),
            this_type: self.clone_opt_id(member.this_type),
            this_by_ref: member.this_by_ref,
            value_type: self.clone_opt_id(member.value_type),
            body: self.clone_opt_id(member.body),
            span: member.span,
        }
    }

    /// Clones an `operator` definition.
    fn clone_operator_decl(&mut self, decl: &OperatorDecl) -> OperatorDecl {
        OperatorDecl {
            modifiers: decl.modifiers.clone(),
            head: self.clone_operator_head(&decl.head),
            body: self.clone_operator_body(&decl.body),
        }
    }

    /// Clones an operator head.
    fn clone_operator_head(&mut self, head: &OperatorHead) -> OperatorHead {
        match head {
            OperatorHead::Shorthand {
                symbol,
                parameters,
                return_type,
            } => OperatorHead::Shorthand {
                symbol: symbol.clone(),
                parameters: self.clone_parameters(parameters),
                return_type: self.clone_opt_id(*return_type),
            },
            OperatorHead::Notation {
                symbol,
                return_type,
            } => OperatorHead::Notation {
                symbol: symbol.clone(),
                return_type: self.clone_opt_id(*return_type),
            },
            OperatorHead::Indexer {
                parameters,
                return_type,
            } => OperatorHead::Indexer {
                parameters: self.clone_parameters(parameters),
                return_type: self.clone_opt_id(*return_type),
            },
            OperatorHead::Conversion {
                type_,
                parameters,
                return_type,
            } => OperatorHead::Conversion {
                type_: self.clone_subtree(*type_),
                parameters: self.clone_parameters(parameters),
                return_type: self.clone_opt_id(*return_type),
            },
        }
    }

    /// Clones an operator body.
    fn clone_operator_body(&mut self, body: &OperatorBody) -> OperatorBody {
        match body {
            OperatorBody::None => OperatorBody::None,
            OperatorBody::Expression(expr) => OperatorBody::Expression(self.clone_subtree(*expr)),
            OperatorBody::Block(block) => OperatorBody::Block(self.clone_subtree(*block)),
            OperatorBody::AccessorMembers(members) => OperatorBody::AccessorMembers(
                members
                    .iter()
                    .map(|member| self.clone_accessor_member(member))
                    .collect(),
            ),
        }
    }

    /// Clones a parameter list.
    fn clone_parameters(&mut self, parameters: &[Parameter]) -> Vec<Parameter> {
        parameters
            .iter()
            .map(|parameter| Parameter {
                compile_time: parameter.compile_time,
                name: parameter.name.clone(),
                by_ref: parameter.by_ref,
                ty: self.clone_opt_id(parameter.ty),
                default: self.clone_opt_id(parameter.default),
                span: parameter.span,
            })
            .collect()
    }

    /// Clones a `local` definition.
    fn clone_local_decl(&mut self, decl: &LocalDecl) -> LocalDecl {
        LocalDecl {
            modifiers: decl.modifiers.clone(),
            items: decl
                .items
                .iter()
                .map(|item| LocalItem {
                    verbatim: item.verbatim,
                    name: item.name.clone(),
                    ty: self.clone_opt_id(item.ty),
                    init: self.clone_opt_id(item.init),
                    span: item.span,
                })
                .collect(),
        }
    }

    /// Clones a `heap` definition.
    fn clone_heap_decl(&mut self, decl: &HeapDecl) -> HeapDecl {
        HeapDecl {
            modifiers: decl.modifiers.clone(),
            size: self.clone_opt_id(decl.size),
            name: decl.name.clone(),
            ty: self.clone_opt_id(decl.ty),
            init: self.clone_opt_id(decl.init),
        }
    }

    /// Clones a `fixed` definition.
    fn clone_fixed_decl(&mut self, decl: &FixedDecl) -> FixedDecl {
        FixedDecl {
            modifiers: decl.modifiers.clone(),
            name: decl.name.clone(),
            generics: decl.generics.clone(),
            ty: self.clone_opt_id(decl.ty),
            init: self.clone_initializer(decl.init),
        }
    }

    /// Clones a `linkable` definition.
    fn clone_linkable_decl(&mut self, decl: &LinkableDecl) -> LinkableDecl {
        LinkableDecl {
            modifiers: decl.modifiers.clone(),
            symbol: decl.symbol.clone(),
            name: decl.name.clone(),
            generics: decl.generics.clone(),
            ty: self.clone_opt_id(decl.ty),
            init: self.clone_initializer(decl.init),
        }
    }

    /// Clones a `use` statement.
    fn clone_use_stmt(&mut self, stmt: &UseStmt) -> UseStmt {
        UseStmt {
            targets: self.clone_ids(&stmt.targets),
        }
    }

    /// Clones an assignment statement.
    fn clone_set_stmt(&mut self, stmt: &SetStmt) -> SetStmt {
        SetStmt {
            target: self.clone_subtree(stmt.target),
            value: self.clone_subtree(stmt.value),
        }
    }

    /// Clones a compound assignment statement.
    fn clone_compound_set_stmt(&mut self, stmt: &CompoundSetStmt) -> CompoundSetStmt {
        CompoundSetStmt {
            target: self.clone_subtree(stmt.target),
            value: self.clone_subtree(stmt.value),
            op: stmt.op,
        }
    }

    /// Clones a `dispose` statement.
    fn clone_dispose_stmt(&mut self, stmt: &DisposeStmt) -> DisposeStmt {
        DisposeStmt {
            target: self.clone_subtree(stmt.target),
        }
    }

    /// Clones an `if` statement.
    fn clone_if_stmt(&mut self, stmt: &IfStmt) -> IfStmt {
        IfStmt {
            branches: stmt
                .branches
                .iter()
                .map(|branch| IfBranch {
                    condition: self.clone_subtree(branch.condition),
                    body: self.clone_subtree(branch.body),
                    span: branch.span,
                })
                .collect(),
            else_block: self.clone_opt_id(stmt.else_block),
        }
    }

    /// Clones a `while` statement.
    fn clone_loop_stmt(&mut self, stmt: &LoopStmt) -> LoopStmt {
        LoopStmt {
            condition: self.clone_subtree(stmt.condition),
            label: stmt.label.clone(),
            body: self.clone_subtree(stmt.body),
        }
    }

    /// Clones a `do ... while` statement.
    fn clone_do_while_stmt(&mut self, stmt: &DoWhileStmt) -> DoWhileStmt {
        DoWhileStmt {
            label: stmt.label.clone(),
            body: self.clone_subtree(stmt.body),
            condition: self.clone_subtree(stmt.condition),
        }
    }

    /// Clones a `for` statement.
    fn clone_for_stmt(&mut self, stmt: &ForStmt) -> ForStmt {
        ForStmt {
            by_ref: stmt.by_ref,
            name: stmt.name.clone(),
            ty: self.clone_opt_id(stmt.ty),
            target: self.clone_for_target(&stmt.target),
            label: stmt.label.clone(),
            body: self.clone_subtree(stmt.body),
        }
    }

    /// Clones what a `for` statement traverses.
    fn clone_for_target(&mut self, target: &ForTarget) -> ForTarget {
        match target {
            ForTarget::Expression(expr) => ForTarget::Expression(self.clone_subtree(*expr)),
            ForTarget::Range(range) => ForTarget::Range(self.clone_range_expr(range)),
        }
    }

    /// Clones an explicit range.
    fn clone_range_expr(&mut self, range: &RangeExpr) -> RangeExpr {
        RangeExpr {
            start: self.clone_subtree(range.start),
            step: self.clone_opt_id(range.step),
            end: self.clone_subtree(range.end),
            inclusive: range.inclusive,
        }
    }

    /// Clones a `match` statement.
    fn clone_match_stmt(&mut self, stmt: &MatchStmt) -> MatchStmt {
        MatchStmt {
            value: self.clone_subtree(stmt.value),
            label: stmt.label.clone(),
            cases: stmt
                .cases
                .iter()
                .map(|case| MatchCase {
                    pattern: self.clone_pattern(case.pattern.as_ref()),
                    guard: self.clone_opt_id(case.guard),
                    body: self.clone_subtree(case.body),
                    span: case.span,
                })
                .collect(),
        }
    }

    /// Clones a `match` pattern.
    fn clone_pattern(&mut self, pattern: Option<&Pattern>) -> Option<Pattern> {
        pattern.map(|pattern| match pattern {
            Pattern::Equal(value) => Pattern::Equal(self.clone_subtree(*value)),
            Pattern::Comparison {
                operator,
                value,
                span,
            } => Pattern::Comparison {
                operator: *operator,
                value: self.clone_subtree(*value),
                span: *span,
            },
            Pattern::Is { name, target } => Pattern::Is {
                name: name.clone(),
                target: target.clone(),
            },
            Pattern::Wildcard => Pattern::Wildcard,
        })
    }

    /// Clones a `break` statement.
    fn clone_break_stmt(&mut self, stmt: &BreakStmt) -> BreakStmt {
        BreakStmt {
            label: stmt.label.clone(),
        }
    }

    /// Clones a `continue` statement.
    fn clone_continue_stmt(&mut self, stmt: &ContinueStmt) -> ContinueStmt {
        ContinueStmt {
            label: stmt.label.clone(),
        }
    }

    /// Clones a `return` statement.
    fn clone_return_stmt(&mut self, stmt: &ReturnStmt) -> ReturnStmt {
        ReturnStmt {
            value: self.clone_opt_id(stmt.value),
        }
    }

    /// Clones a `throw` statement.
    fn clone_throw_stmt(&mut self, stmt: &ThrowStmt) -> ThrowStmt {
        ThrowStmt {
            value: self.clone_subtree(stmt.value),
        }
    }

    /// Clones a `trap` statement.
    fn clone_trap_stmt(&mut self, stmt: &TrapStmt) -> TrapStmt {
        TrapStmt {
            body: self.clone_subtree(stmt.body),
            catches: stmt
                .catches
                .iter()
                .map(|clause| CatchClause {
                    name: clause.name.clone(),
                    ty: self.clone_opt_id(clause.ty),
                    wildcard: clause.wildcard,
                    body: self.clone_subtree(clause.body),
                    span: clause.span,
                })
                .collect(),
            finally: self.clone_opt_id(stmt.finally),
        }
    }

    /// Clones a `#if` statement.
    fn clone_compile_time_if_stmt(&mut self, stmt: &CompileTimeIfStmt) -> CompileTimeIfStmt {
        CompileTimeIfStmt {
            condition: self.clone_subtree(stmt.condition),
            then_block: self.clone_subtree(stmt.then_block),
            else_block: self.clone_opt_id(stmt.else_block),
        }
    }

    /// Clones a member access.
    fn clone_member_access(&mut self, access: &MemberAccess) -> MemberAccess {
        MemberAccess {
            base: self.clone_subtree(access.base),
            name: access.name.clone(),
        }
    }

    /// Clones an index expression.
    fn clone_index_expr(&mut self, expr: &IndexExpr) -> IndexExpr {
        IndexExpr {
            base: self.clone_subtree(expr.base),
            index: self.clone_subtree(expr.index),
        }
    }

    /// Clones a call expression.
    fn clone_call_expr(&mut self, expr: &CallExpr) -> CallExpr {
        CallExpr {
            callee: self.clone_subtree(expr.callee),
            arguments: self.clone_arguments(&expr.arguments),
        }
    }

    /// Clones call/construction arguments.
    fn clone_arguments(&mut self, arguments: &[Argument]) -> Vec<Argument> {
        arguments
            .iter()
            .map(|argument| Argument {
                name: argument.name.clone(),
                value: self.clone_subtree(argument.value),
                span: argument.span,
            })
            .collect()
    }

    /// Clones a construction expression.
    fn clone_construct_expr(&mut self, expr: &ConstructExpr) -> ConstructExpr {
        ConstructExpr {
            type_: self.clone_subtree(expr.type_),
            arguments: self.clone_arguments(&expr.arguments),
        }
    }

    /// Clones a member construction expression.
    fn clone_member_construct_expr(&mut self, expr: &MemberConstructExpr) -> MemberConstructExpr {
        MemberConstructExpr {
            type_: self.clone_subtree(expr.type_),
            member: expr.member.clone(),
            value: self.clone_subtree(expr.value),
        }
    }

    /// Clones a cast expression.
    fn clone_cast_expr(&mut self, expr: &CastExpr) -> CastExpr {
        CastExpr {
            type_: self.clone_subtree(expr.type_),
            value: self.clone_subtree(expr.value),
        }
    }

    /// Clones an `is` expression, remapping the target's name chain.
    fn clone_is_expr(&mut self, expr: &IsExpr) -> IsExpr {
        IsExpr {
            value: self.clone_subtree(expr.value),
            target: self.clone_is_target(&expr.target),
        }
    }

    /// Clones an `is` target.
    fn clone_is_target(&mut self, target: &IsTarget) -> IsTarget {
        match target {
            IsTarget::Identifier { text, node } => IsTarget::Identifier {
                text: text.clone(),
                node: self.clone_subtree(*node),
            },
            IsTarget::Illegal => IsTarget::Illegal,
            IsTarget::Infinite { negative } => IsTarget::Infinite {
                negative: *negative,
            },
        }
    }

    /// Clones a unary expression.
    fn clone_unary_expr(&mut self, expr: &UnaryExpr) -> UnaryExpr {
        UnaryExpr {
            op: expr.op,
            operand: self.clone_subtree(expr.operand),
        }
    }

    /// Clones a binary expression.
    fn clone_binary_expr(&mut self, expr: &BinaryExpr) -> BinaryExpr {
        BinaryExpr {
            op: expr.op,
            left: self.clone_subtree(expr.left),
            right: self.clone_subtree(expr.right),
        }
    }

    /// Clones a conditional expression.
    fn clone_conditional_expr(&mut self, expr: &ConditionalExpr) -> ConditionalExpr {
        ConditionalExpr {
            condition: self.clone_subtree(expr.condition),
            then_value: self.clone_subtree(expr.then_value),
            else_value: self.clone_subtree(expr.else_value),
        }
    }

    /// Clones a null-coalescing expression.
    fn clone_coalesce_expr(&mut self, expr: &CoalesceExpr) -> CoalesceExpr {
        CoalesceExpr {
            value: self.clone_subtree(expr.value),
            alternative: self.clone_coalescing_alternative(&expr.alternative),
        }
    }

    /// Clones what follows `??`.
    fn clone_coalescing_alternative(
        &mut self,
        alternative: &CoalescingAlternative,
    ) -> CoalescingAlternative {
        match alternative {
            CoalescingAlternative::Expression(expr) => {
                CoalescingAlternative::Expression(self.clone_subtree(*expr))
            }
            CoalescingAlternative::Throw(value) => {
                CoalescingAlternative::Throw(self.clone_opt_id(*value))
            }
            CoalescingAlternative::Return(value) => {
                CoalescingAlternative::Return(self.clone_opt_id(*value))
            }
        }
    }

    /// Clones an array literal.
    fn clone_array_literal(&mut self, literal: &ArrayLiteral) -> ArrayLiteral {
        ArrayLiteral {
            elements: literal
                .elements
                .iter()
                .map(|element| self.clone_array_element(element))
                .collect(),
        }
    }

    /// Clones one array literal element.
    fn clone_array_element(&mut self, element: &ArrayElement) -> ArrayElement {
        match element {
            ArrayElement::Value(value) => ArrayElement::Value(self.clone_subtree(*value)),
            ArrayElement::Repeat { value, count } => ArrayElement::Repeat {
                value: self.clone_subtree(*value),
                count: self.clone_subtree(*count),
            },
            ArrayElement::Range(range) => ArrayElement::Range(self.clone_range_expr(range)),
        }
    }

    /// Clones a function literal.
    fn clone_function_literal(&mut self, literal: &FunctionLiteral) -> FunctionLiteral {
        FunctionLiteral {
            parameters: literal
                .parameters
                .iter()
                .map(|parameter| LiteralParameter {
                    compile_time: parameter.compile_time,
                    name: parameter.name.clone(),
                    by_ref: parameter.by_ref,
                    ty: self.clone_opt_id(parameter.ty),
                    span: parameter.span,
                })
                .collect(),
            calling_convention: self.clone_opt_id(literal.calling_convention),
            return_type: self.clone_opt_id(literal.return_type),
            body: self.clone_subtree(literal.body),
        }
    }

    /// Clones a monomorphization expression.
    fn clone_monomorphization_expr(&mut self, expr: &MonomorphizationExpr) -> MonomorphizationExpr {
        MonomorphizationExpr {
            base: self.clone_subtree(expr.base),
            arguments: expr
                .arguments
                .iter()
                .map(|&argument| self.clone_opt_id(argument))
                .collect(),
        }
    }

    /// Clones an array type.
    fn clone_array_type(&mut self, ty: &ArrayType) -> ArrayType {
        ArrayType {
            dimensions: ty
                .dimensions
                .iter()
                .map(|dimension| match dimension {
                    ArrayDimension::Size(size) => ArrayDimension::Size(self.clone_subtree(*size)),
                    ArrayDimension::Binding { name, ty } => ArrayDimension::Binding {
                        name: name.clone(),
                        ty: self.clone_subtree(*ty),
                    },
                })
                .collect(),
            element: self.clone_subtree(ty.element),
        }
    }

    /// Clones a function type.
    fn clone_function_type(&mut self, ty: &FunctionType) -> FunctionType {
        FunctionType {
            parameters: self.clone_parameters(&ty.parameters),
            wildcard: ty.wildcard,
            calling_convention: self.clone_opt_id(ty.calling_convention),
            return_type: self.clone_opt_id(ty.return_type),
        }
    }

    /// Clones a named type.
    fn clone_named_type(&mut self, ty: &NamedType) -> NamedType {
        NamedType {
            name: self.clone_subtree(ty.name),
            arguments: self.clone_opt_ids(&ty.arguments),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;
    use crate::parser;
    use crate::source::{FileId, SourceManager, Span};

    fn parse(source: &str) -> Ast {
        let mut sources = SourceManager::new();
        let file = sources.add_buffer("test.ns", source);
        let lexed = lexer::tokenize(file, source);
        assert!(lexed.errors.is_empty(), "lex errors: {:?}", lexed.errors);
        let parsed = parser::parse(file, source, &lexed.tokens);
        assert!(
            parsed.errors.is_empty(),
            "parse errors: {:?}",
            parsed.errors
        );
        parsed.ast
    }

    fn node_count(ast: &Ast) -> NodeId {
        u32::try_from(ast.nodes.len()).expect("test AST fits in u32 ids")
    }

    #[test]
    fn clone_preserves_shape() {
        let mut ast = parse(
            "structure Point { public X: integer(4); public Y: integer(4); }\n\
             fixed Sum(value: integer(4)): integer(4) {\n\
                 local doubled: integer(4) = value * 2;\n\
                 if (doubled > 4) { return doubled; }\n\
                 return value;\n\
             }",
        );
        assert_eq!(ast.roots.len(), 2);
        let roots = ast.roots.clone();
        let original_count = node_count(&ast);

        for &root in &roots {
            let original_span = ast.span(root);
            let original_kind = std::mem::discriminant(&ast.node(root).kind);
            let cloned = ast.clone_subtree(root);
            assert_ne!(cloned, root);
            assert!(cloned >= original_count);
            assert_eq!(ast.span(cloned), original_span);
            assert_eq!(
                std::mem::discriminant(&ast.node(cloned).kind),
                original_kind
            );
        }

        assert!(ast.nodes.len() > original_count as usize);
    }

    #[test]
    fn clone_is_independent() {
        let mut ast = parse(
            "fixed First(value: integer(4)): integer(4) { return value; }\n\
             fixed Second(value: integer(4)): integer(4) { return value + 1; }",
        );
        assert_eq!(ast.roots.len(), 2);
        let first = ast.roots[0];
        let second = ast.roots[1];
        let original_count = node_count(&ast);
        let first_span = ast.span(first);
        let second_span = ast.span(second);
        let second_kind = std::mem::discriminant(&ast.node(second).kind);

        let cloned = ast.clone_subtree(first);

        assert_ne!(cloned, first);
        assert!(cloned >= original_count);
        assert_eq!(ast.span(cloned), first_span);
        // Cloning `first` must not disturb the other function.
        assert_eq!(ast.span(second), second_span);
        assert_eq!(std::mem::discriminant(&ast.node(second).kind), second_kind);
    }

    #[test]
    fn clone_nested_children_are_remapped() {
        let mut ast = Ast::new(FileId::default());
        let value = ast.push(Span::default(), NodeKind::BooleanLiteral(true));
        let statement = ast.push(
            Span::default(),
            NodeKind::Return(ReturnStmt { value: Some(value) }),
        );
        let block = ast.push(Span::default(), NodeKind::Block(vec![statement]));
        let original_count = node_count(&ast);

        let cloned_block = ast.clone_subtree(block);

        assert_ne!(cloned_block, block);
        assert!(cloned_block >= original_count);
        let statements = match &ast.node(cloned_block).kind {
            NodeKind::Block(statements) => statements,
            other => panic!("expected a block, got {other:?}"),
        };
        assert_eq!(statements.len(), 1);
        let cloned_statement = statements[0];
        assert_ne!(cloned_statement, statement);
        assert!(cloned_statement >= original_count);

        let returned = match &ast.node(cloned_statement).kind {
            NodeKind::Return(returned) => returned,
            other => panic!("expected a return statement, got {other:?}"),
        };
        let cloned_value = returned.value.expect("the return value should be cloned");
        assert_ne!(cloned_value, value);
        assert!(cloned_value >= original_count);
        assert!(matches!(
            ast.node(cloned_value).kind,
            NodeKind::BooleanLiteral(true)
        ));
    }

    #[test]
    fn clone_subtrees_clones_each_root_independently() {
        let mut ast = Ast::new(FileId::default());
        let shared = ast.push(Span::default(), NodeKind::BooleanLiteral(false));
        let first = ast.push(Span::default(), NodeKind::ExpressionStmt(shared));
        let second = ast.push(Span::default(), NodeKind::ExpressionStmt(shared));
        let original_count = node_count(&ast);

        let cloned = ast.clone_subtrees(&[first, second]);

        assert_eq!(cloned.len(), 2);
        assert_ne!(cloned[0], cloned[1]);
        assert!(cloned[0] < cloned[1]);
        assert_ne!(cloned[0], first);
        assert_ne!(cloned[1], second);
        assert!(cloned[0] >= original_count);
        assert!(cloned[1] >= original_count);

        let child_of = |root: NodeId| match &ast.node(root).kind {
            NodeKind::ExpressionStmt(child) => *child,
            other => panic!("expected an expression statement, got {other:?}"),
        };
        assert_eq!(child_of(first), shared);
        assert_eq!(child_of(second), shared);
        assert_ne!(child_of(cloned[0]), child_of(cloned[1]));
        assert_ne!(child_of(cloned[0]), shared);
        assert_ne!(child_of(cloned[1]), shared);
    }
}
