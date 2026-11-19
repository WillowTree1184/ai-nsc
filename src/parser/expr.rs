//! Expression parsing (§4) and the literals it builds.

use super::Parser;
use crate::ast::*;
use crate::lexer::TokenKind;
use crate::source::Span;

impl<'a> Parser<'a> {
    /// Parses a full expression.
    pub(crate) fn parse_expression(&mut self) -> Option<NodeId> {
        self.parse_conditional()
    }

    /// Parses `condition ? then : else` (lowest precedence, right associative).
    fn parse_conditional(&mut self) -> Option<NodeId> {
        let start_token = self.position;
        let condition = self.parse_coalesce()?;
        if !self.eat(TokenKind::Question) {
            return Some(condition);
        }
        let then_value = self.parse_expression()?;
        self.expect(TokenKind::Colon, "':' in conditional expression");
        let else_value = self.parse_expression()?;
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(
            span,
            NodeKind::Conditional(ConditionalExpr {
                condition,
                then_value,
                else_value,
            }),
        ))
    }

    /// Parses `value ?? alternative` (right associative, §4.9.26).
    fn parse_coalesce(&mut self) -> Option<NodeId> {
        let start_token = self.position;
        let value = self.parse_binary(0)?;
        if !self.eat(TokenKind::QuestionQuestion) {
            return Some(value);
        }
        let alternative = if self.eat(TokenKind::Throw) {
            CoalescingAlternative::Throw(if self.at_expression_start() {
                self.parse_expression()
            } else {
                None
            })
        } else if self.eat(TokenKind::Return) {
            CoalescingAlternative::Return(if self.at_expression_start() {
                self.parse_expression()
            } else {
                None
            })
        } else {
            CoalescingAlternative::Expression(self.parse_coalesce()?)
        };
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(
            span,
            NodeKind::Coalesce(CoalesceExpr { value, alternative }),
        ))
    }

    /// Precedence-climbing binary expression parser (left associative).
    fn parse_binary(&mut self, minimum_precedence: u8) -> Option<NodeId> {
        let start_token = self.position;
        let mut left = self.parse_unary()?;

        loop {
            if self.at(TokenKind::Is) {
                let precedence = 9;
                if precedence < minimum_precedence {
                    break;
                }
                self.bump();
                let target = self.parse_is_target()?;
                let span = self.span_between(start_token, self.position);
                left = self.ast.push(
                    span,
                    NodeKind::Is(IsExpr {
                        value: left,
                        target,
                    }),
                );
                continue;
            }

            if self.at(TokenKind::As) {
                let precedence = 9;
                if precedence < minimum_precedence {
                    break;
                }
                self.bump();
                let type_ = self.parse_type()?;
                let span = self.span_between(start_token, self.position);
                left = self
                    .ast
                    .push(span, NodeKind::Cast(CastExpr { type_, value: left }));
                continue;
            }

            let Some(operator) = self.peek_binary_operator() else {
                break;
            };
            let precedence = operator.precedence();
            if precedence < minimum_precedence {
                break;
            }
            self.bump();
            let right = self.parse_binary(precedence + 1)?;
            let span = self.span_between(start_token, self.position);
            left = self.ast.push(
                span,
                NodeKind::Binary(BinaryExpr {
                    op: operator,
                    left,
                    right,
                }),
            );
        }

        Some(left)
    }

    fn peek_binary_operator(&self) -> Option<BinaryOp> {
        let operator = match self.peek_kind() {
            TokenKind::Star => BinaryOp::Multiply,
            TokenKind::Slash => BinaryOp::Divide,
            TokenKind::Percent => BinaryOp::Remainder,
            TokenKind::Plus => BinaryOp::Add,
            TokenKind::Minus => BinaryOp::Subtract,
            TokenKind::ShiftLeft => BinaryOp::ShiftLeft,
            TokenKind::ShiftRight => BinaryOp::ShiftRight,
            TokenKind::CircularShiftLeft => BinaryOp::CircularShiftLeft,
            TokenKind::CircularShiftRight => BinaryOp::CircularShiftRight,
            TokenKind::Less => BinaryOp::Less,
            TokenKind::Greater => BinaryOp::Greater,
            TokenKind::LessEquals => BinaryOp::LessEqual,
            TokenKind::GreaterEquals => BinaryOp::GreaterEqual,
            TokenKind::EqualsEquals => BinaryOp::Equal,
            TokenKind::NotEquals => BinaryOp::NotEqual,
            TokenKind::Ampersand => BinaryOp::BitwiseAnd,
            TokenKind::Caret => BinaryOp::BitwiseXor,
            TokenKind::Pipe => BinaryOp::BitwiseOr,
            TokenKind::AmpersandAmpersand => BinaryOp::LogicalAnd,
            TokenKind::PipePipe => BinaryOp::LogicalOr,
            _ => return None,
        };
        Some(operator)
    }

    fn parse_unary(&mut self) -> Option<NodeId> {
        let start_token = self.position;
        let operator = match self.peek_kind() {
            TokenKind::Minus => UnaryOp::Negate,
            TokenKind::Bang => UnaryOp::LogicalNot,
            TokenKind::Caret => UnaryOp::BitwiseNot,
            TokenKind::Ampersand => UnaryOp::Reference,
            TokenKind::Star => UnaryOp::Dereference,
            _ => return self.parse_postfix(),
        };
        self.bump();
        let operand = self.parse_unary()?;
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(
            span,
            NodeKind::Unary(UnaryExpr {
                op: operator,
                operand,
            }),
        ))
    }

    fn parse_postfix(&mut self) -> Option<NodeId> {
        let start_token = self.position;
        let mut expression = self.parse_primary()?;

        loop {
            match self.peek_kind() {
                TokenKind::Dot => {
                    self.bump();
                    let (name, span) = self.parse_member_name()?;
                    let full = Span::new(self.file, self.ast.span(expression).start, span.end);
                    expression = self.ast.push(
                        full,
                        NodeKind::MemberAccess(MemberAccess {
                            base: expression,
                            name,
                        }),
                    );
                }
                TokenKind::Arrow => {
                    self.bump();
                    let (name, span) = self.parse_member_name()?;
                    let full = Span::new(self.file, self.ast.span(expression).start, span.end);
                    expression = self.ast.push(
                        full,
                        NodeKind::PointerMemberAccess(MemberAccess {
                            base: expression,
                            name,
                        }),
                    );
                }
                TokenKind::LeftBracket => {
                    self.bump();
                    let index = self.parse_expression()?;
                    self.expect(TokenKind::RightBracket, "']' after index");
                    let span = self.span_between(start_token, self.position);
                    expression = self.ast.push(
                        span,
                        NodeKind::Index(IndexExpr {
                            base: expression,
                            index,
                        }),
                    );
                }
                TokenKind::LeftParen => {
                    let arguments = self.parse_arguments()?;
                    let span = self.span_between(start_token, self.position);
                    expression = self.ast.push(
                        span,
                        NodeKind::Call(CallExpr {
                            callee: expression,
                            arguments,
                        }),
                    );
                }
                TokenKind::Less => {
                    let Some(arguments) = self.try_parse(Self::parse_monomorphization_arguments)
                    else {
                        break;
                    };
                    if !self.is_identifier_like(expression) {
                        self.error_here(
                            "the base of a monomorphization expression must be an identifier (§10.2)",
                        );
                    }
                    let span = self.span_between(start_token, self.position);
                    expression = self.ast.push(
                        span,
                        NodeKind::Monomorphization(MonomorphizationExpr {
                            base: expression,
                            arguments,
                        }),
                    );
                }
                _ => break,
            }
        }

        Some(expression)
    }

    /// Parses the name after `.` or `->`; any keyword may be a member name.
    pub(crate) fn parse_member_name(&mut self) -> Option<(String, Span)> {
        self.parse_name()
    }

    /// Whether an expression is a (possibly module-qualified) identifier, the
    /// only allowed base of a monomorphization expression (§10.2).
    fn is_identifier_like(&self, node: NodeId) -> bool {
        match &self.ast.node(node).kind {
            NodeKind::Name(_) => true,
            NodeKind::MemberAccess(access) => self.is_identifier_like(access.base),
            _ => false,
        }
    }

    fn parse_primary(&mut self) -> Option<NodeId> {
        // `@(Expression)` is a no-optimize expression (§14.3.1).
        if self.at(TokenKind::At) && self.peek_at(1) == TokenKind::LeftParen {
            let start_token = self.position;
            self.require_at_adjacent("'('");
            self.bump();
            self.expect(TokenKind::LeftParen, "'(' after '@'")?;
            let inner = self.parse_expression()?;
            self.expect(TokenKind::RightParen, "')' after no-optimize expression");
            let span = self.span_between(start_token, self.position);
            return Some(self.ast.push(span, NodeKind::VerbatimExpression(inner)));
        }
        let start_token = self.position;
        let token = self.peek();
        let span = token.span;

        match token.kind {
            TokenKind::IntegerLiteral => {
                self.bump();
                Some(self.ast.push(
                    span,
                    NodeKind::IntegerLiteral(IntLiteral {
                        raw: token.text(self.source).to_string(),
                    }),
                ))
            }
            TokenKind::FloatLiteral => {
                self.bump();
                Some(self.ast.push(
                    span,
                    NodeKind::FloatLiteral(FloatLiteral {
                        raw: token.text(self.source).to_string(),
                    }),
                ))
            }
            TokenKind::CharacterLiteral => {
                self.bump();
                Some(self.ast.push(
                    span,
                    NodeKind::CharacterLiteral(CharacterLiteral {
                        raw: token.text(self.source).to_string(),
                    }),
                ))
            }
            TokenKind::StringLiteral => {
                self.bump();
                Some(self.ast.push(
                    span,
                    NodeKind::StringLiteral(StringLiteral {
                        raw: token.text(self.source).to_string(),
                    }),
                ))
            }
            TokenKind::VerbatimStringLiteral => {
                self.bump();
                Some(self.ast.push(
                    span,
                    NodeKind::VerbatimStringLiteral(StringLiteral {
                        raw: token.text(self.source).to_string(),
                    }),
                ))
            }
            TokenKind::True => {
                self.bump();
                Some(self.ast.push(span, NodeKind::BooleanLiteral(true)))
            }
            TokenKind::False => {
                self.bump();
                Some(self.ast.push(span, NodeKind::BooleanLiteral(false)))
            }
            TokenKind::Infinite => {
                self.bump();
                Some(self.ast.push(span, NodeKind::InfiniteLiteral))
            }
            TokenKind::Illegal => {
                self.bump();
                Some(self.ast.push(span, NodeKind::IllegalLiteral))
            }
            TokenKind::Uninitialized => {
                self.bump();
                Some(self.ast.push(span, NodeKind::UninitializedLiteral))
            }
            TokenKind::This => {
                self.bump();
                Some(self.ast.push(span, NodeKind::This))
            }
            TokenKind::Owner => {
                self.bump();
                Some(self.ast.push(span, NodeKind::Owner))
            }
            TokenKind::Base => {
                self.bump();
                Some(self.ast.push(span, NodeKind::Base))
            }
            TokenKind::Hash => self.parse_compile_time_expression(),
            TokenKind::LeftBrace => self.parse_array_literal(),
            TokenKind::LeftParen => self.parse_parenthesis_or_function_literal(),
            _ => {
                let (name, name_span) = self.parse_name()?;
                let _ = start_token;
                Some(self.ast.push(name_span, NodeKind::Name(NameExpr { name })))
            }
        }
    }

    fn parse_compile_time_expression(&mut self) -> Option<NodeId> {
        let start_token = self.position;
        self.bump(); // '#'
        self.expect(TokenKind::LeftParen, "'(' after '#'");
        let inner = self.parse_expression()?;
        self.expect(TokenKind::RightParen, "')' after compile-time expression");
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(span, NodeKind::CompileTimeExpr(inner)))
    }

    fn parse_array_literal(&mut self) -> Option<NodeId> {
        let start_token = self.position;
        self.expect(TokenKind::LeftBrace, "'{'")?;
        let mut elements = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at_end() {
            if self.at(TokenKind::LeftBracket) {
                self.bump();
                let first = self.parse_expression()?;
                if self.eat(TokenKind::Colon) {
                    let count = self.parse_expression()?;
                    self.expect(TokenKind::RightBracket, "']' after repeat element");
                    elements.push(ArrayElement::Repeat {
                        value: first,
                        count,
                    });
                } else {
                    let range = self.parse_range_after_start(first)?;
                    self.expect(TokenKind::RightBracket, "']' after range element");
                    elements.push(ArrayElement::Range(range));
                }
            } else {
                elements.push(ArrayElement::Value(self.parse_expression()?));
            }
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RightBrace, "'}' after array literal");
        let span = self.span_between(start_token, self.position);
        Some(
            self.ast
                .push(span, NodeKind::ArrayLiteral(ArrayLiteral { elements })),
        )
    }

    fn parse_parenthesis_or_function_literal(&mut self) -> Option<NodeId> {
        let start_token = self.position;
        let header = self.try_parse(Self::parse_function_literal_header);
        if let Some((parameters, calling_convention, return_type)) = header {
            // Draft 1.153 §2.7.9 requires a Block or VerbatimBlock body.
            self.push_shadow_scope();
            for parameter in &parameters {
                self.declare_shadow_name(&parameter.name);
            }
            let body = if self.at(TokenKind::At) && self.peek_at(1) == TokenKind::LeftBrace {
                self.parse_verbatim_block()
            } else if self.at(TokenKind::LeftBrace) {
                self.parse_block()
            } else {
                self.error_here("expected '{' after '=>' in a function literal (§2.7.9)");
                None
            };
            self.pop_shadow_scope();
            let body = body?;
            let span = self.span_between(start_token, self.position);
            return Some(self.ast.push(
                span,
                NodeKind::FunctionLiteral(FunctionLiteral {
                    parameters,
                    calling_convention,
                    return_type,
                    body,
                }),
            ));
        }

        self.expect(TokenKind::LeftParen, "'('")?;
        let inner = self.parse_expression()?;
        self.expect(TokenKind::RightParen, "')' after parenthesized expression");
        Some(inner)
    }

    fn parse_function_literal_header(
        &mut self,
    ) -> Option<(Vec<LiteralParameter>, Option<NodeId>, Option<NodeId>)> {
        self.expect(TokenKind::LeftParen, "'('")?;
        let parameters = self.parse_literal_parameters()?;
        let calling_convention = if self.eat(TokenKind::Semicolon) {
            self.parse_expression()
        } else {
            None
        };
        self.expect(
            TokenKind::RightParen,
            "')' after function literal parameters",
        )?;
        let return_type = if self.eat(TokenKind::Colon)
            || (self.at_type_start() && !self.at(TokenKind::FatArrow))
        {
            self.parse_type()
        } else {
            None
        };
        if !self.eat(TokenKind::FatArrow) {
            return None;
        }
        Some((parameters, calling_convention, return_type))
    }

    fn parse_literal_parameters(&mut self) -> Option<Vec<LiteralParameter>> {
        let mut parameters = Vec::new();
        if self.at(TokenKind::RightParen) || self.at(TokenKind::Semicolon) {
            return Some(parameters);
        }
        loop {
            let span = self.peek().span;
            let compile_time = self.eat(TokenKind::Hash);
            // `array(Name: Type) T` parameter expansion (§3.6.1): the expanded
            // pointer parameter has no name in the source; the length binding is
            // named by the expansion itself.
            if self.at(TokenKind::Array) && self.peek_at(1) == TokenKind::LeftParen {
                let ty = self.parse_type();
                parameters.push(LiteralParameter {
                    compile_time,
                    name: format!("message{}", parameters.len()),
                    by_ref: false,
                    ty,
                    span,
                });
                if !self.eat(TokenKind::Comma) {
                    break;
                }
                if self.at(TokenKind::RightParen) || self.at(TokenKind::Semicolon) {
                    self.error_here(
                        "a trailing comma is not allowed in a function literal parameter list",
                    );
                    break;
                }
                continue;
            }
            let (name, _) = self.parse_name()?;
            let mut by_ref = false;
            let mut ty = None;
            if self.eat(TokenKind::Colon) {
                by_ref = self.eat(TokenKind::Ampersand);
                ty = self.parse_type();
            }
            parameters.push(LiteralParameter {
                compile_time,
                name,
                by_ref,
                ty,
                span,
            });
            if !self.eat(TokenKind::Comma) {
                break;
            }
            if self.at(TokenKind::RightParen) || self.at(TokenKind::Semicolon) {
                self.error_here(
                    "a trailing comma is not allowed in a function literal parameter list",
                );
                break;
            }
        }
        Some(parameters)
    }

    fn parse_arguments(&mut self) -> Option<Vec<Argument>> {
        self.expect(TokenKind::LeftParen, "'('")?;
        let mut arguments = Vec::new();
        while !self.at(TokenKind::RightParen) && !self.at_end() {
            let span = self.peek().span;
            let name = if (self.at(TokenKind::Identifier) || self.peek_kind().is_keyword())
                && self.peek_at(1) == TokenKind::Colon
            {
                let (name, _) = self.parse_name()?;
                self.expect(TokenKind::Colon, "':' after argument name");
                Some(name)
            } else {
                None
            };
            let value = self.parse_expression()?;
            arguments.push(Argument { name, value, span });
            if !self.eat(TokenKind::Comma) {
                break;
            }
            if self.at(TokenKind::RightParen) {
                self.error_here("a trailing comma is not allowed in an argument list");
                break;
            }
        }
        self.expect(TokenKind::RightParen, "')' after arguments");
        Some(arguments)
    }
}
