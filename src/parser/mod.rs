//! The NovaSharp parser.
//!
//! A hand-written recursive-descent parser with precedence climbing for
//! expressions. It produces an arena [`Ast`] plus a list of [`ParseError`]s and
//! always recovers: after an error it skips to a synchronization point (`;`,
//! `}` or a statement keyword) and continues, so one run reports many problems.
//!
//! # Ambiguities resolved here
//!
//! - `{` starts an array literal in expression position and a block in
//!   statement position (§2.7.8).
//! - `(params) => ...` is a function literal; `(expr)` is a parenthesized
//!   expression. The parser speculates and backtracks.
//! - `name<...>` is either a generic parameter list (in declarations) or a
//!   monomorphization expression (in expressions/types). Speculative parsing
//!   with rollback decides; failed speculation discards its diagnostics.
//! - `1..2` is a range, never a float (enforced by the lexer §2.7.2).
//! - Keywords may be used as names when a name is expected (§2.4); `@` forces a
//!   keyword interpretation instead.

mod expr;

use crate::ast::*;
use crate::lexer::{Token, TokenKind};
use crate::source::{FileId, Span};

/// A parse error, ready to be rendered by [`crate::diagnostics`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParseError {
    /// Where the problem is.
    pub span: Span,
    /// What went wrong.
    pub message: String,
}

/// Result of parsing one file.
#[derive(Debug, Default)]
pub struct ParseResult {
    /// The parsed syntax tree.
    pub ast: Ast,
    /// All errors reported while parsing.
    pub errors: Vec<ParseError>,
}

/// Parses a whole file.
///
/// `tokens` must come from [`crate::lexer::tokenize`] for the same `source`.
#[must_use]
pub fn parse(file: FileId, source: &str, tokens: &[Token]) -> ParseResult {
    let mut parser = Parser::new(file, source, tokens);
    parser.parse_file();
    ParseResult {
        ast: parser.ast,
        errors: parser.errors,
    }
}

pub(crate) struct Parser<'a> {
    pub(crate) file: FileId,
    pub(crate) source: &'a str,
    pub(crate) tokens: &'a [Token],
    pub(crate) position: usize,
    pub(crate) ast: Ast,
    pub(crate) errors: Vec<ParseError>,
}

impl<'a> Parser<'a> {
    pub(crate) fn new(file: FileId, source: &'a str, tokens: &'a [Token]) -> Self {
        Self {
            file,
            source,
            tokens,
            position: 0,
            ast: Ast::new(file),
            errors: Vec::new(),
        }
    }

    // -- token access -------------------------------------------------------

    pub(crate) fn peek(&self) -> Token {
        self.tokens[self.position.min(self.tokens.len() - 1)]
    }

    pub(crate) fn peek_kind(&self) -> TokenKind {
        self.peek().kind
    }

    pub(crate) fn peek_at(&self, offset: usize) -> TokenKind {
        let index = (self.position + offset).min(self.tokens.len() - 1);
        self.tokens[index].kind
    }

    pub(crate) fn at(&self, kind: TokenKind) -> bool {
        self.peek_kind() == kind
    }

    pub(crate) fn at_end(&self) -> bool {
        self.at(TokenKind::Eof)
    }

    pub(crate) fn bump(&mut self) -> Token {
        let token = self.peek();
        if !self.at_end() {
            self.position += 1;
        }
        token
    }

    pub(crate) fn eat(&mut self, kind: TokenKind) -> bool {
        if self.at(kind) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    /// Consumes `kind`, or `@ kind` (§2.4: `@` forces the keyword meaning).
    pub(crate) fn eat_forced(&mut self, kind: TokenKind) -> bool {
        if self.at(kind) {
            self.position += 1;
            return true;
        }
        if self.at(TokenKind::At) && self.peek_at(1) == kind {
            self.position += 2;
            return true;
        }
        false
    }

    pub(crate) fn expect(&mut self, kind: TokenKind, what: &str) -> Option<Token> {
        if self.at(kind) {
            Some(self.bump())
        } else {
            self.error_here(format!(
                "expected {what}, found {}",
                self.describe_current()
            ));
            None
        }
    }

    pub(crate) fn describe_current(&self) -> String {
        let token = self.peek();
        match token.kind {
            TokenKind::Eof => "end of file".to_string(),
            _ => format!("'{}'", token.text(self.source)),
        }
    }

    // -- spans and diagnostics ---------------------------------------------

    /// Span covering the tokens between two token positions.
    fn span_between(&self, start_token: usize, end_token: usize) -> Span {
        let start = self.tokens[start_token.min(self.tokens.len() - 1)]
            .span
            .start;
        let end = self.tokens[end_token.saturating_sub(1).min(self.tokens.len() - 1)]
            .span
            .end;
        Span::new(self.file, start, end.max(start))
    }

    fn span_of(&self, node: NodeId) -> Span {
        self.ast.span(node)
    }

    pub(crate) fn error_at(&mut self, span: Span, message: impl Into<String>) {
        self.errors.push(ParseError {
            span,
            message: message.into(),
        });
    }

    pub(crate) fn error_here(&mut self, message: impl Into<String>) {
        self.error_at(self.peek().span, message);
    }

    /// Runs `f`; if it returns `None` or reports errors, restores the position
    /// and discards the diagnostics. Used to resolve grammar ambiguities.
    fn try_parse<T>(&mut self, f: impl FnOnce(&mut Self) -> Option<T>) -> Option<T> {
        let position = self.position;
        let error_count = self.errors.len();
        match f(self) {
            Some(value) if self.errors.len() == error_count => Some(value),
            _ => {
                self.position = position;
                self.errors.truncate(error_count);
                None
            }
        }
    }

    // -- file ---------------------------------------------------------------

    fn parse_file(&mut self) {
        self.ast.file = self.file;
        while !self.at_end() {
            let before = self.position;
            if let Some(statement) = self.parse_statement() {
                self.ast.roots.push(statement);
            }
            self.expect_statement_terminator();
            if self.position == before {
                // No progress; consume a token to guarantee termination.
                self.bump();
            }
        }
    }

    // -- statement termination (§2.8) --------------------------------------

    /// Consumes a `;` when present. A `}` terminates a statement without a
    /// semicolon (§2.8); end of file is tolerated for recovery.
    pub(crate) fn expect_statement_terminator(&mut self) {
        if self.eat(TokenKind::Semicolon) {
            return;
        }
        if self.at(TokenKind::RightBrace) || self.at_end() {
            return;
        }
        // A `}` that ends its line also terminates the statement (§2.8).
        if self.previous_token_was_block_end_on_earlier_line() {
            return;
        }
        self.error_here(format!(
            "expected ';' after statement, found {}",
            self.describe_current()
        ));
        self.synchronize();
    }

    /// Whether the previously consumed token was a `}` followed by a line
    /// terminator before the current token (§2.8 statement termination).
    fn previous_token_was_block_end_on_earlier_line(&self) -> bool {
        let Some(previous_index) = self.position.checked_sub(1) else {
            return false;
        };
        let previous = self.tokens[previous_index];
        if previous.kind != TokenKind::RightBrace {
            return false;
        }
        let gap_start = previous.span.end as usize;
        let gap_end = (self.peek().span.start as usize).min(self.source.len());
        self.source
            .get(gap_start..gap_end)
            .is_some_and(|gap| gap.contains('\n') || gap.contains('\r'))
    }

    /// Skips tokens until something that can start a new statement.
    pub(crate) fn synchronize(&mut self) {
        while !self.at_end() {
            match self.peek_kind() {
                TokenKind::Semicolon => {
                    self.bump();
                    return;
                }
                TokenKind::RightBrace | TokenKind::Eof => return,
                TokenKind::Class
                | TokenKind::Structure
                | TokenKind::Interface
                | TokenKind::Enumeration
                | TokenKind::Variant
                | TokenKind::Union
                | TokenKind::Module
                | TokenKind::Local
                | TokenKind::Fixed
                | TokenKind::Heap
                | TokenKind::Linkable
                | TokenKind::Use
                | TokenKind::If
                | TokenKind::While
                | TokenKind::Do
                | TokenKind::For
                | TokenKind::Match
                | TokenKind::Break
                | TokenKind::Continue
                | TokenKind::Return
                | TokenKind::Throw
                | TokenKind::Trap
                | TokenKind::Dispose
                | TokenKind::Extension
                | TokenKind::Accessor
                | TokenKind::Operator => return,
                _ => {
                    self.bump();
                }
            }
        }
    }

    // -- names and modifiers ------------------------------------------------

    /// Parses a name. Keywords are accepted as names (§2.4: a name shadows a
    /// keyword); `@` is consumed when it forces a keyword into name position.
    pub(crate) fn parse_name(&mut self) -> Option<(String, Span)> {
        let token = self.peek();
        if token.kind == TokenKind::Identifier || token.kind.is_keyword() {
            self.bump();
            return Some((token.text(self.source).to_string(), token.span));
        }
        self.error_here(format!(
            "expected a name, found {}",
            self.describe_current()
        ));
        None
    }

    /// Parses `name` / `name.member` chains into MemberAccess nodes.
    pub(crate) fn parse_qualified_name(&mut self) -> Option<NodeId> {
        let (name, span) = self.parse_name()?;
        let mut node = self.ast.push(span, NodeKind::Name(NameExpr { name }));
        loop {
            if !self.at(TokenKind::Dot) {
                break;
            }
            self.bump();
            let (member, member_span) = match self.parse_name() {
                Some(parts) => parts,
                None => break,
            };
            let span = Span::new(self.file, self.span_of(node).start, member_span.end);
            node = self.ast.push(
                span,
                NodeKind::MemberAccess(MemberAccess {
                    base: node,
                    name: member,
                }),
            );
        }
        Some(node)
    }

    fn parse_modifiers(&mut self) -> Modifiers {
        let mut modifiers = Modifiers::default();
        loop {
            match self.peek_kind() {
                TokenKind::Public => {
                    self.bump();
                    modifiers.accessibility = Accessibility::Public;
                }
                TokenKind::Private => {
                    self.bump();
                    modifiers.accessibility = Accessibility::Private;
                }
                TokenKind::Protected => {
                    self.bump();
                    modifiers.accessibility = Accessibility::Protected;
                }
                TokenKind::Scoped => {
                    self.bump();
                    modifiers.scoped = self.parse_scoped_targets();
                }
                TokenKind::Readonly => {
                    self.bump();
                    modifiers.readonly = true;
                }
                TokenKind::Implicitable => {
                    self.bump();
                    modifiers.implicitable = true;
                }
                TokenKind::Verbatim => {
                    self.bump();
                    modifiers.verbatim = true;
                }
                TokenKind::Manual => {
                    self.bump();
                    modifiers.manual = true;
                }
                TokenKind::Abstract => {
                    self.bump();
                    modifiers.abstract_ = true;
                }
                TokenKind::Sealed => {
                    self.bump();
                    modifiers.sealed = true;
                }
                TokenKind::Virtual => {
                    self.bump();
                    modifiers.virtual_ = true;
                }
                TokenKind::Override => {
                    self.bump();
                    modifiers.override_ = true;
                }
                TokenKind::Static => {
                    self.bump();
                    modifiers.static_ = true;
                }
                TokenKind::Aligned => {
                    self.bump();
                    modifiers.aligned = self.parse_aligned();
                }
                _ => break,
            }
        }
        modifiers
    }

    fn parse_aligned(&mut self) -> Option<u32> {
        self.expect(TokenKind::LeftParen, "'(' after 'aligned'")?;
        let value = self.parse_integer_literal_value().unwrap_or(1);
        self.expect(TokenKind::RightParen, "')' after aligned value");
        Some(value)
    }

    fn parse_scoped_targets(&mut self) -> Vec<ScopedTarget> {
        let mut targets = Vec::new();
        if self
            .expect(TokenKind::LeftParen, "'(' after 'scoped'")
            .is_none()
        {
            return targets;
        }
        loop {
            if self.eat(TokenKind::Operator) {
                let name = if self.at(TokenKind::Identifier) || self.peek_kind().is_keyword() {
                    self.bump().text(self.source).to_string()
                } else {
                    String::new()
                };
                targets.push(ScopedTarget::Operator { name: Some(name) });
            } else {
                let sealed = self.eat(TokenKind::Sealed);
                if let Some((name, _)) = self.parse_qualified_name_text() {
                    targets.push(ScopedTarget::Named { sealed, name });
                } else {
                    break;
                }
            }
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RightParen, "')' after scoped targets");
        targets
    }

    /// Parses a dotted name and returns it as text (used by `scoped`).
    fn parse_qualified_name_text(&mut self) -> Option<(String, Span)> {
        let node = self.parse_qualified_name()?;
        let span = self.ast.span(node);
        let text = self.source[span.start as usize..span.end as usize]
            .split_whitespace()
            .collect::<String>();
        Some((text, span))
    }

    // -- integer literal helper --------------------------------------------

    fn parse_integer_literal_value(&mut self) -> Option<u32> {
        let token = self.peek();
        if token.kind != TokenKind::IntegerLiteral {
            self.error_here("expected an integer literal");
            return None;
        }
        self.bump();
        let raw = token.text(self.source);
        crate::literal::decode_integer(raw)
            .ok()
            .and_then(|value| u32::try_from(value).ok())
    }
}

// ==========================================================================
// Types (§3)
// ==========================================================================
impl<'a> Parser<'a> {
    /// Parses a type.
    pub(crate) fn parse_type(&mut self) -> Option<NodeId> {
        let start_token = self.position;

        if self.eat_forced(TokenKind::Array) {
            self.expect(TokenKind::LeftParen, "'(' after 'array'");
            let mut dimensions = Vec::new();
            if !self.at(TokenKind::RightParen) {
                loop {
                    dimensions.push(self.parse_array_dimension()?);
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                }
            }
            self.expect(TokenKind::RightParen, "')' after array dimensions");
            let element = self.parse_type().or_else(|| {
                self.error_here("expected an element type after the array dimensions");
                None
            })?;
            let span = self.span_between(start_token, self.position);
            return Some(self.ast.push(
                span,
                NodeKind::ArrayType(ArrayType {
                    dimensions,
                    element,
                }),
            ));
        }

        if self.eat_forced(TokenKind::Pointer) {
            let target = if self.at_type_start() {
                self.parse_type()
            } else {
                None
            };
            let span = self.span_between(start_token, self.position);
            return Some(self.ast.push(span, NodeKind::PointerType(target)));
        }

        if self.eat_forced(TokenKind::Function) {
            let mut parameters = Vec::new();
            let mut wildcard = false;
            let mut calling_convention = None;
            if self.eat(TokenKind::LeftParen) {
                (parameters, wildcard, calling_convention) = self.parse_parameter_list();
                self.expect(TokenKind::RightParen, "')' after function type parameters");
            }
            let return_type = if self.at_type_start() {
                self.parse_type()
            } else {
                None
            };
            let span = self.span_between(start_token, self.position);
            return Some(self.ast.push(
                span,
                NodeKind::FunctionType(FunctionType {
                    parameters,
                    wildcard,
                    calling_convention,
                    return_type,
                }),
            ));
        }

        if self.at(TokenKind::Integer)
            || self.at(TokenKind::Cardinal)
            || self.at(TokenKind::Float)
            || (self.at(TokenKind::At)
                && matches!(
                    self.peek_at(1),
                    TokenKind::Integer | TokenKind::Cardinal | TokenKind::Float
                ))
        {
            let kind = if self.eat_forced(TokenKind::Integer) {
                BasicKind::Integer
            } else if self.eat_forced(TokenKind::Cardinal) {
                BasicKind::Cardinal
            } else {
                self.eat_forced(TokenKind::Float);
                BasicKind::Float
            };
            self.expect(TokenKind::LeftParen, "'(' after basic type");
            let size_token = self.peek();
            let size = self.parse_integer_literal_value();
            let size_span = size_token.span;
            self.expect(TokenKind::RightParen, "')' after basic type size");
            let span = self.span_between(start_token, self.position);
            return Some(self.ast.push(
                span,
                NodeKind::BasicType(BasicType {
                    kind,
                    size,
                    size_span: Some(size_span),
                }),
            ));
        }

        if self.eat_forced(TokenKind::Boolean) {
            let span = self.span_between(start_token, self.position);
            return Some(self.ast.push(
                span,
                NodeKind::BasicType(BasicType {
                    kind: BasicKind::Boolean,
                    size: None,
                    size_span: None,
                }),
            ));
        }

        // Named type, possibly monomorphized.
        let name = self.parse_qualified_name()?;
        let arguments = self.try_parse(Self::parse_monomorphization_arguments);
        let span = self.span_between(start_token, self.position);
        Some(
            self.ast
                .push(span, NodeKind::NamedType(NamedType { name, arguments })),
        )
    }

    /// Parses one array dimension: a size expression or a named binding, e.g.
    /// `array(5)` or `array(length: cardinal(4))` (§3.6.1).
    fn parse_array_dimension(&mut self) -> Option<ArrayDimension> {
        if (self.at(TokenKind::Identifier) || self.peek_kind().is_keyword())
            && self.peek_at(1) == TokenKind::Colon
        {
            let (name, _) = self.parse_name()?;
            self.expect(TokenKind::Colon, "':' in array dimension binding");
            let ty = self.parse_type()?;
            return Some(ArrayDimension::Binding { name, ty });
        }
        Some(ArrayDimension::Size(self.parse_expression()?))
    }

    /// Parses an identifier-definition shorthand type (§2.5). The caller has
    /// already consumed the name and optional generic parameters.
    fn parse_shorthand_type(&mut self) -> Option<NodeId> {
        let start = self.position;
        if self.eat(TokenKind::LeftParen) {
            let (parameters, wildcard, calling_convention) = self.parse_parameter_list();
            self.expect(TokenKind::RightParen, "')' after parameter list");
            let return_type = if self.eat(TokenKind::Colon) {
                self.parse_type()
            } else {
                None
            };
            let span = self.span_between(start, self.position);
            return Some(self.ast.push(
                span,
                NodeKind::FunctionType(FunctionType {
                    parameters,
                    wildcard,
                    calling_convention,
                    return_type,
                }),
            ));
        }
        if self.eat(TokenKind::LeftBracket) {
            let mut dimensions = Vec::new();
            loop {
                dimensions.push(self.parse_array_dimension()?);
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::RightBracket, "']' after array dimensions");
            self.expect(TokenKind::Colon, "array shorthand requires ': Type'");
            let element = self.parse_type()?;
            let span = self.span_between(start, self.position);
            return Some(self.ast.push(
                span,
                NodeKind::ArrayType(ArrayType {
                    dimensions,
                    element,
                }),
            ));
        }
        self.error_here("expected '(' or '[' to start a definition shorthand");
        None
    }

    /// Whether the current token can start a type.
    pub(crate) fn at_type_start(&self) -> bool {
        matches!(
            self.peek_kind(),
            TokenKind::Identifier
                | TokenKind::At
                | TokenKind::Array
                | TokenKind::Function
                | TokenKind::Pointer
                | TokenKind::Integer
                | TokenKind::Cardinal
                | TokenKind::Float
                | TokenKind::Boolean
        )
    }

    /// Parses `(ParameterList)` contents, leaving `)` unconsumed.
    fn parse_parameter_list(&mut self) -> (Vec<Parameter>, bool, Option<NodeId>) {
        let mut parameters = Vec::new();
        let mut wildcard = false;
        let mut calling_convention = None;

        if self.at(TokenKind::Wildcard) {
            self.bump();
            wildcard = true;
            return (parameters, wildcard, calling_convention);
        }

        while !self.at(TokenKind::RightParen) && !self.at(TokenKind::Semicolon) && !self.at_end() {
            parameters.push(self.parse_parameter());
            if !self.eat(TokenKind::Comma) {
                break;
            }
            if self.at(TokenKind::Wildcard) {
                self.bump();
                wildcard = true;
                break;
            }
        }

        if self.eat(TokenKind::Semicolon) {
            calling_convention = self.parse_expression();
        }
        (parameters, wildcard, calling_convention)
    }

    /// Parses one function type parameter (§8.2). A parameter name may carry an
    /// identifier-definition shorthand, e.g. `empty[0]: integer(4)` (§2.5).
    fn parse_parameter(&mut self) -> Parameter {
        let span = self.peek().span;
        let compile_time = self.eat(TokenKind::Hash);

        let name_ahead = self.at(TokenKind::Identifier) || self.peek_kind().is_keyword();
        let shorthand_ahead = (self.peek_at(1) == TokenKind::LeftBracket
            && self.scan_brackets_then_colon(self.position + 1))
            || (self.peek_at(1) == TokenKind::LeftParen
                && self.scan_parens_then_colon(self.position + 1));
        let mut name = None;
        if name_ahead && (self.peek_at(1) == TokenKind::Colon || shorthand_ahead) {
            name = self.parse_name().map(|(parsed, _)| parsed);
        }

        let mut by_ref = false;
        let ty = if name.is_some()
            && matches!(
                self.peek_kind(),
                TokenKind::LeftParen | TokenKind::LeftBracket
            ) {
            self.parse_shorthand_type()
        } else {
            if name.is_some() {
                self.expect(TokenKind::Colon, "':' after parameter name");
            }
            by_ref = self.eat(TokenKind::Ampersand);
            if self.at_type_start() {
                self.parse_type()
            } else {
                self.error_here("expected a parameter type");
                None
            }
        };
        let default = if self.eat(TokenKind::Equals) {
            self.parse_expression()
        } else {
            None
        };

        let end = self
            .position
            .checked_sub(1)
            .map_or(span.end, |index| self.tokens[index].span.end);
        Parameter {
            compile_time,
            name,
            by_ref,
            ty,
            default,
            span: Span::new(self.file, span.start, end.max(span.start)),
        }
    }

    /// Parses `< [Type] { ";" [Type] } >`.
    fn parse_monomorphization_arguments(&mut self) -> Option<Vec<Option<NodeId>>> {
        self.expect(TokenKind::Less, "'<'")?;
        let mut arguments = Vec::new();
        if !self.at(TokenKind::Greater) {
            if self.at_type_start() {
                arguments.push(Some(self.parse_type()?));
            } else {
                arguments.push(None);
            }
            while self.eat(TokenKind::Semicolon) {
                if self.at_type_start() {
                    arguments.push(Some(self.parse_type()?));
                } else {
                    arguments.push(None);
                }
            }
        }
        self.expect(TokenKind::Greater, "'>'")?;
        Some(arguments)
    }

    /// Parses generic parameter definitions (§10.1).
    fn parse_generic_parameters(&mut self) -> Vec<GenericParameter> {
        let mut parameters = Vec::new();
        if self.expect(TokenKind::Less, "'<'").is_none() {
            return parameters;
        }
        loop {
            if self.at(TokenKind::Greater) {
                break;
            }
            let Some(parameter) = self.parse_single_generic_parameter() else {
                break;
            };
            parameters.push(parameter);
            if !self.eat(TokenKind::Semicolon) {
                break;
            }
        }
        self.expect(TokenKind::Greater, "'>' after generic parameters");
        parameters
    }

    /// Parses one `GenericParameterDefinition` (§10.1).
    fn parse_single_generic_parameter(&mut self) -> Option<GenericParameter> {
        let (name, span) = self.parse_name()?;
        let mut constraints = Vec::new();
        if self.eat(TokenKind::Colon) {
            loop {
                match self.peek_kind() {
                    TokenKind::Class
                    | TokenKind::Structure
                    | TokenKind::Interface
                    | TokenKind::Enumeration
                    | TokenKind::Variant
                    | TokenKind::Union
                    | TokenKind::Basic => {
                        constraints.push(self.bump().text(self.source).to_string());
                    }
                    TokenKind::Identifier => {
                        constraints.push(self.bump().text(self.source).to_string());
                    }
                    _ => {
                        self.error_here("expected a generic constraint");
                        break;
                    }
                }
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
        }
        Some(GenericParameter {
            name,
            constraints,
            span,
        })
    }

    /// Parses a definition head: `Name [Generics] (shorthand | ":" Type)`.
    fn parse_definition_head(&mut self, allow_generics: bool) -> Option<DefinitionHead> {
        let (name, name_span) = self.parse_name()?;
        let generics = if allow_generics && self.at(TokenKind::Less) {
            self.parse_generic_parameters()
        } else {
            Vec::new()
        };

        let ty = if self.at(TokenKind::LeftParen) || self.at(TokenKind::LeftBracket) {
            self.parse_shorthand_type()
        } else if self.eat(TokenKind::Colon) {
            self.parse_type()
        } else {
            None
        };

        let _ = name_span;
        Some(DefinitionHead { name, generics, ty })
    }

    fn looks_like_field(&self) -> bool {
        if !(self.at(TokenKind::Identifier) || self.peek_kind().is_keyword()) {
            return false;
        }
        match self.peek_at(1) {
            TokenKind::Colon => true,
            TokenKind::LeftParen => self.scan_parens_then_colon(self.position + 1),
            TokenKind::LeftBracket => self.scan_brackets_then_colon(self.position + 1),
            _ => false,
        }
    }

    fn scan_parens_then_colon(&self, open_index: usize) -> bool {
        let close = self.scan_balanced(open_index, TokenKind::LeftParen, TokenKind::RightParen);
        match close {
            Some(index) => self.peek_index_kind(index) == TokenKind::Colon,
            None => false,
        }
    }

    fn scan_brackets_then_colon(&self, open_index: usize) -> bool {
        let close = self.scan_balanced(open_index, TokenKind::LeftBracket, TokenKind::RightBracket);
        match close {
            Some(index) => self.peek_index_kind(index) == TokenKind::Colon,
            None => false,
        }
    }

    fn peek_index_kind(&self, index: usize) -> TokenKind {
        self.tokens[index.min(self.tokens.len() - 1)].kind
    }

    fn scan_balanced(&self, open_index: usize, open: TokenKind, close: TokenKind) -> Option<usize> {
        let mut depth = 0usize;
        let mut index = open_index;
        while index < self.tokens.len() {
            let kind = self.tokens[index].kind;
            if kind == open {
                depth += 1;
            } else if kind == close {
                depth -= 1;
                if depth == 0 {
                    return Some(index + 1);
                }
            } else if kind == TokenKind::Eof {
                return None;
            }
            index += 1;
        }
        None
    }
}

/// The name/generics/type triple produced by [`Parser::parse_definition_head`].
pub(crate) struct DefinitionHead {
    pub(crate) name: String,
    pub(crate) generics: Vec<GenericParameter>,
    pub(crate) ty: Option<NodeId>,
}

// ==========================================================================
// Statements and declarations (§5, §3.7-3.8, §7)
// ==========================================================================
impl<'a> Parser<'a> {
    /// Parses one statement (or block-level declaration).
    pub(crate) fn parse_statement(&mut self) -> Option<NodeId> {
        let start_token = self.position;
        // `@class Name { }` forces the keyword meaning even when an identifier
        // of that name shadows it (§2.4).
        if self.at(TokenKind::At)
            && matches!(
                self.peek_at(1),
                TokenKind::Class
                    | TokenKind::Structure
                    | TokenKind::Interface
                    | TokenKind::Enumeration
                    | TokenKind::Variant
                    | TokenKind::Union
                    | TokenKind::Module
                    | TokenKind::Extension
                    | TokenKind::Accessor
                    | TokenKind::Operator
                    | TokenKind::Local
                    | TokenKind::Fixed
                    | TokenKind::Heap
                    | TokenKind::Linkable
            )
        {
            self.bump();
        }
        let modifiers = self.parse_modifiers();

        let kind = self.peek_kind();
        let has_modifiers = !modifiers_is_empty(&modifiers);

        if has_modifiers {
            return match kind {
                TokenKind::Class => self.parse_class_like(modifiers, ClassLikeKind::Class),
                TokenKind::Structure => self.parse_class_like(modifiers, ClassLikeKind::Structure),
                TokenKind::Interface => self.parse_class_like(modifiers, ClassLikeKind::Interface),
                TokenKind::Enumeration => self.parse_enumeration(modifiers),
                TokenKind::Variant => self.parse_variant(modifiers),
                TokenKind::Union => self.parse_union(modifiers),
                TokenKind::Module => self.parse_module(modifiers),
                TokenKind::Extension => self.parse_extension(modifiers),
                TokenKind::Accessor => self.parse_accessor(modifiers),
                TokenKind::Operator => self.parse_operator(modifiers),
                TokenKind::Local => self.parse_local(modifiers),
                TokenKind::Fixed => self.parse_fixed(modifiers),
                TokenKind::Heap => self.parse_heap(modifiers),
                TokenKind::Linkable => self.parse_linkable(modifiers),
                TokenKind::LeftBrace => self.parse_verbatim_block(),
                _ if self.looks_like_field() => self.parse_field(modifiers),
                _ => {
                    self.error_here(format!(
                        "expected a definition after modifiers, found {}",
                        self.describe_current()
                    ));
                    self.synchronize();
                    None
                }
            };
        }

        match kind {
            TokenKind::Class => self.parse_class_like(modifiers, ClassLikeKind::Class),
            TokenKind::Structure => self.parse_class_like(modifiers, ClassLikeKind::Structure),
            TokenKind::Interface => self.parse_class_like(modifiers, ClassLikeKind::Interface),
            TokenKind::Enumeration => self.parse_enumeration(modifiers),
            TokenKind::Variant => self.parse_variant(modifiers),
            TokenKind::Union => self.parse_union(modifiers),
            TokenKind::Module => self.parse_module(modifiers),
            TokenKind::Extension => self.parse_extension(modifiers),
            TokenKind::Accessor => self.parse_accessor(modifiers),
            TokenKind::Operator => self.parse_operator(modifiers),
            TokenKind::Local => self.parse_local(modifiers),
            TokenKind::Fixed => self.parse_fixed(modifiers),
            TokenKind::Heap => self.parse_heap(modifiers),
            TokenKind::Linkable => self.parse_linkable(modifiers),
            TokenKind::Use => self.parse_use(),
            TokenKind::If => self.parse_if(),
            TokenKind::While => self.parse_while(),
            TokenKind::Do => self.parse_do_while(),
            TokenKind::For => self.parse_for(),
            TokenKind::Match => self.parse_match(),
            TokenKind::Break => self.parse_break(),
            TokenKind::Continue => self.parse_continue(),
            TokenKind::Return => self.parse_return(),
            TokenKind::Throw => self.parse_throw(),
            TokenKind::Trap => self.parse_trap(),
            TokenKind::Dispose => self.parse_dispose(),
            TokenKind::Hash => self.parse_compile_time_statement(),
            TokenKind::LeftBrace => self.parse_block(),
            _ if self.looks_like_field() => self.parse_field(modifiers),
            _ => self.parse_expression_statement(start_token),
        }
    }

    fn parse_verbatim_block(&mut self) -> Option<NodeId> {
        let start_token = self.position;
        let block = self.parse_block()?;
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(span, NodeKind::VerbatimBlock(block)))
    }

    /// Parses `{ ... }` into a Block node.
    pub(crate) fn parse_block(&mut self) -> Option<NodeId> {
        let start_token = self.position;
        if self.expect(TokenKind::LeftBrace, "'{'").is_none() {
            self.synchronize();
            return None;
        }
        let statements = self.parse_block_statements_until_right_brace();
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(span, NodeKind::Block(statements)))
    }

    /// Parses statements until the matching `}` (consumed).
    fn parse_block_statements_until_right_brace(&mut self) -> Vec<NodeId> {
        let mut statements = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at_end() {
            let before = self.position;
            if let Some(statement) = self.parse_statement() {
                statements.push(statement);
            }
            self.expect_statement_terminator();
            if self.position == before {
                self.bump();
            }
        }
        self.expect(TokenKind::RightBrace, "'}'");
        statements
    }

    // -- declarations ------------------------------------------------------

    fn parse_class_like(&mut self, modifiers: Modifiers, kind: ClassLikeKind) -> Option<NodeId> {
        let start_token = self.position;
        self.bump(); // class / structure / interface
        let (name, _) = self.parse_name()?;
        let generics = if self.at(TokenKind::Less) {
            self.parse_generic_parameters()
        } else {
            Vec::new()
        };
        let mut bases = Vec::new();
        if self.eat(TokenKind::Colon) {
            loop {
                let base = self.parse_named_type_expression()?;
                bases.push(base);
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
        }
        let members = self.parse_member_block()?;
        let span = self.span_between(start_token, self.position);
        let declaration = ClassDecl {
            modifiers,
            name,
            generics,
            bases,
            members,
        };
        let node_kind = match kind {
            ClassLikeKind::Class => NodeKind::Class(declaration),
            ClassLikeKind::Structure => NodeKind::Structure(declaration),
            ClassLikeKind::Interface => NodeKind::Interface(declaration),
        };
        Some(self.ast.push(span, node_kind))
    }

    /// Parses a braced member list (type body), returning member node ids.
    fn parse_member_block(&mut self) -> Option<Vec<NodeId>> {
        self.expect(TokenKind::LeftBrace, "'{'")?;
        let mut members = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at_end() {
            let before = self.position;
            if let Some(member) = self.parse_statement() {
                members.push(member);
            }
            // Member lists use ';' as a separator, and a '}' always terminates.
            if !self.at(TokenKind::RightBrace) {
                self.expect_statement_terminator();
            }
            if self.position == before {
                self.bump();
            }
        }
        self.expect(TokenKind::RightBrace, "'}'");
        Some(members)
    }

    fn parse_named_type_expression(&mut self) -> Option<NodeId> {
        let start_token = self.position;
        let name = self.parse_qualified_name()?;
        let arguments = if self.at(TokenKind::Less) {
            self.try_parse(Self::parse_monomorphization_arguments)
        } else {
            None
        };
        let span = self.span_between(start_token, self.position);
        Some(
            self.ast
                .push(span, NodeKind::NamedType(NamedType { name, arguments })),
        )
    }

    fn parse_enumeration(&mut self, modifiers: Modifiers) -> Option<NodeId> {
        let start_token = self.position;
        self.bump();
        let (name, _) = self.parse_name()?;
        self.expect(TokenKind::Colon, "':' before the enumeration's basic type");
        let underlying = self.parse_type()?;
        self.expect(TokenKind::LeftBrace, "'{'");
        let mut members = Vec::new();
        let mut extensions = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at_end() {
            if self.at(TokenKind::Extension) {
                if let Some(extension) = self.parse_extension(Modifiers::default()) {
                    extensions.push(extension);
                }
            } else {
                let (member_name, member_span) = self.parse_name()?;
                let value = if self.eat(TokenKind::Equals) {
                    self.parse_expression()
                } else {
                    None
                };
                self.expect(TokenKind::Semicolon, "';' after enumeration member");
                members.push(EnumerationMember {
                    name: member_name,
                    value,
                    span: member_span,
                });
            }
        }
        self.expect(TokenKind::RightBrace, "'}'");
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(
            span,
            NodeKind::Enumeration(EnumerationDecl {
                modifiers,
                name,
                underlying,
                members,
                extensions,
            }),
        ))
    }

    fn parse_variant(&mut self, modifiers: Modifiers) -> Option<NodeId> {
        let start_token = self.position;
        self.bump();
        let (name, _) = self.parse_name()?;
        let generics = if self.at(TokenKind::Less) {
            self.parse_generic_parameters()
        } else {
            Vec::new()
        };
        self.expect(TokenKind::LeftBrace, "'{'");
        let mut members = Vec::new();
        let mut extensions = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at_end() {
            if self.at(TokenKind::Extension) {
                if let Some(extension) = self.parse_extension(Modifiers::default()) {
                    extensions.push(extension);
                }
            } else {
                let (member_name, member_span) = self.parse_name()?;
                let payload = if self.eat(TokenKind::Colon) {
                    self.parse_type()
                } else {
                    None
                };
                self.expect(TokenKind::Semicolon, "';' after variant member");
                members.push(VariantMember {
                    name: member_name,
                    payload,
                    span: member_span,
                });
            }
        }
        self.expect(TokenKind::RightBrace, "'}'");
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(
            span,
            NodeKind::Variant(VariantDecl {
                modifiers,
                name,
                generics,
                members,
                extensions,
            }),
        ))
    }

    fn parse_union(&mut self, modifiers: Modifiers) -> Option<NodeId> {
        let start_token = self.position;
        self.bump();
        let (name, _) = self.parse_name()?;
        let generics = if self.at(TokenKind::Less) {
            self.parse_generic_parameters()
        } else {
            Vec::new()
        };
        self.expect(TokenKind::LeftBrace, "'{'");
        let mut members = Vec::new();
        let mut extensions = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at_end() {
            if self.at(TokenKind::Extension) {
                if let Some(extension) = self.parse_extension(Modifiers::default()) {
                    extensions.push(extension);
                }
            } else {
                let member_start = self.position;
                let member_modifiers = self.parse_modifiers();
                let head = self.parse_definition_head(true)?;
                let Some(ty) = head.ty else {
                    self.error_here("union member requires a type");
                    self.synchronize();
                    continue;
                };
                self.expect(TokenKind::Semicolon, "';' after union member");
                members.push(UnionMember {
                    modifiers: member_modifiers,
                    name: head.name,
                    ty,
                    span: self.span_between(member_start, self.position),
                });
            }
        }
        self.expect(TokenKind::RightBrace, "'}'");
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(
            span,
            NodeKind::Union(UnionDecl {
                modifiers,
                name,
                generics,
                members,
                extensions,
            }),
        ))
    }

    fn parse_module(&mut self, modifiers: Modifiers) -> Option<NodeId> {
        let start_token = self.position;
        self.bump();
        let (name, _) = self.parse_name()?;
        let members = self.parse_member_block()?;
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(
            span,
            NodeKind::Module(ModuleDecl {
                modifiers,
                name,
                members,
            }),
        ))
    }

    fn parse_extension(&mut self, _modifiers: Modifiers) -> Option<NodeId> {
        let start_token = self.position;
        self.bump();
        let target = if self.eat(TokenKind::For) {
            self.parse_extension_target()
        } else {
            None
        };
        let members = self.parse_member_block()?;
        let span = self.span_between(start_token, self.position);
        Some(
            self.ast
                .push(span, NodeKind::Extension(ExtensionDecl { target, members })),
        )
    }

    /// Parses an `extension for` target (§3.8.3).
    fn parse_extension_target(&mut self) -> Option<NodeId> {
        let start = self.position;
        if self.eat_forced(TokenKind::Array) {
            self.expect(TokenKind::LeftParen, "'(' after 'array'");
            let mut bindings = Vec::new();
            while !self.at(TokenKind::RightParen) && !self.at_end() {
                let (name, _) = self.parse_name()?;
                self.expect(TokenKind::Colon, "':' in array target binding");
                let ty = self.parse_type()?;
                bindings.push((name, ty));
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::RightParen, "')' in array target");
            let parameter = self
                .parse_single_generic_parameter()
                .unwrap_or(GenericParameter {
                    name: "T".to_string(),
                    constraints: Vec::new(),
                    span: self.peek().span,
                });
            let span = self.span_between(start, self.position);
            return Some(self.ast.push(
                span,
                NodeKind::ExtensionTarget(ExtensionTarget::Array {
                    bindings,
                    parameter,
                }),
            ));
        }

        if self.at_type_start() {
            if let Some(generic) = self.try_parse(Self::parse_generic_target) {
                return Some(generic);
            }
            if let Some(node) = self.parse_type() {
                let span = self.ast.span(node);
                return Some(
                    self.ast
                        .push(span, NodeKind::ExtensionTarget(ExtensionTarget::Type(node))),
                );
            }
        }
        self.error_here("expected an extension target");
        None
    }

    fn parse_generic_target(&mut self) -> Option<NodeId> {
        let start = self.position;
        let (name, _) = self.parse_name()?;
        if !self.at(TokenKind::Less) {
            return None;
        }
        let generics = self.parse_generic_parameters();
        if generics.is_empty() {
            return None;
        }
        let span = self.span_between(start, self.position);
        Some(self.ast.push(
            span,
            NodeKind::ExtensionTarget(ExtensionTarget::Generic { name, generics }),
        ))
    }

    fn parse_accessor(&mut self, modifiers: Modifiers) -> Option<NodeId> {
        let start_token = self.position;
        self.bump();
        let (name, _) = self.parse_name()?;
        self.expect(TokenKind::Colon, "':' in accessor definition");
        let ty = self.parse_type()?;
        let backing = if self.eat(TokenKind::FatArrow) {
            self.parse_qualified_name()
        } else {
            None
        };
        let mut members = Vec::new();
        if self.at(TokenKind::LeftBrace) {
            self.bump();
            while !self.at(TokenKind::RightBrace) && !self.at_end() {
                if let Some(member) = self.parse_accessor_member() {
                    members.push(member);
                } else {
                    self.synchronize();
                }
                if !self.at(TokenKind::RightBrace) {
                    self.expect_statement_terminator();
                }
            }
            self.expect(TokenKind::RightBrace, "'}'");
        }
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(
            span,
            NodeKind::Accessor(AccessorDecl {
                modifiers,
                name,
                ty,
                backing,
                members,
            }),
        ))
    }

    fn parse_accessor_member(&mut self) -> Option<AccessorMember> {
        let start_token = self.position;
        let modifiers = self.parse_modifiers();
        let (name, span) = self.parse_name()?;
        let is_getter = match name.as_str() {
            "get" => true,
            "set" => false,
            _ => {
                self.error_at(span, "expected 'get' or 'set' in accessor body");
                return None;
            }
        };

        let mut member = AccessorMember {
            modifiers,
            is_getter,
            ty: None,
            expression: None,
            this_type: None,
            value_type: None,
            body: None,
            span,
        };

        if self.eat(TokenKind::Colon) {
            member.ty = self.parse_type();
        }
        if self.eat(TokenKind::Equals) {
            member.expression = self.parse_expression();
            return Some(member);
        }
        if self.eat(TokenKind::LeftParen) {
            // [this : & Type] [, value [: Type]]
            while !self.at(TokenKind::RightParen) && !self.at_end() {
                let Some((parameter_name, _)) = self.parse_name() else {
                    break;
                };
                if parameter_name == "this" {
                    self.expect(TokenKind::Colon, "':' after 'this'");
                    self.eat(TokenKind::Ampersand);
                    member.this_type = self.parse_type();
                } else if parameter_name == "value" {
                    if self.eat(TokenKind::Colon) {
                        member.value_type = self.parse_type();
                    }
                } else {
                    self.error_here("expected 'this' or 'value' in accessor parameters");
                }
                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::RightParen, "')' in accessor member");
            member.body = self.parse_block();
        }
        let end = self.span_between(start_token, self.position);
        member.span = end;
        Some(member)
    }

    fn parse_operator(&mut self, modifiers: Modifiers) -> Option<NodeId> {
        let start_token = self.position;
        self.bump();
        let head_start = self.position;

        let head = if self.at(TokenKind::LeftBracket) {
            self.bump();
            let parameters = if self.at(TokenKind::RightBracket) {
                Vec::new()
            } else {
                let mut list = Vec::new();
                loop {
                    list.push(self.parse_parameter());
                    if !self.eat(TokenKind::Comma) {
                        break;
                    }
                }
                list
            };
            self.expect(TokenKind::RightBracket, "']' in indexer operator");
            let return_type = if self.eat(TokenKind::Colon) {
                self.parse_type()
            } else {
                None
            };
            OperatorHead::Indexer {
                parameters,
                return_type,
            }
        } else if let Some(symbol) = self.operator_symbol_text() {
            self.bump();
            if self.at(TokenKind::LeftParen) {
                self.bump();
                let parameters = if self.at(TokenKind::RightParen) {
                    Vec::new()
                } else {
                    let mut list = Vec::new();
                    loop {
                        list.push(self.parse_parameter());
                        if !self.eat(TokenKind::Comma) {
                            break;
                        }
                    }
                    list
                };
                self.expect(TokenKind::RightParen, "')' in operator shorthand");
                let return_type = if self.eat(TokenKind::Colon) {
                    self.parse_type()
                } else {
                    None
                };
                OperatorHead::Shorthand {
                    symbol,
                    parameters,
                    return_type,
                }
            } else {
                let return_type = if self.eat(TokenKind::Colon) {
                    self.parse_type()
                } else {
                    None
                };
                OperatorHead::Notation {
                    symbol,
                    return_type,
                }
            }
        } else {
            let type_ = self.parse_type()?;
            let mut parameters = Vec::new();
            if self.eat(TokenKind::LeftParen) {
                if !self.at(TokenKind::RightParen) {
                    loop {
                        parameters.push(self.parse_parameter());
                        if !self.eat(TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.expect(TokenKind::RightParen, "')' in conversion operator");
            }
            let return_type = if self.eat(TokenKind::Colon) {
                self.parse_type()
            } else {
                None
            };
            OperatorHead::Conversion {
                type_,
                parameters,
                return_type,
            }
        };

        let _ = head_start;
        let body = if self.eat(TokenKind::Equals) {
            match self.parse_expression() {
                Some(expression) => OperatorBody::Expression(expression),
                None => OperatorBody::None,
            }
        } else if self.at(TokenKind::LeftBrace) {
            // `{ get ... }` is an accessor member list; anything else is a block.
            let next_index = (self.position + 1).min(self.tokens.len() - 1);
            let next_text = self.tokens[next_index].text(self.source);
            let is_accessor_list = matches!(next_text, "get" | "set");
            if is_accessor_list {
                self.bump();
                let mut members = Vec::new();
                while !self.at(TokenKind::RightBrace) && !self.at_end() {
                    if let Some(member) = self.parse_accessor_member() {
                        members.push(member);
                    } else {
                        self.synchronize();
                    }
                    if !self.at(TokenKind::RightBrace) {
                        self.expect_statement_terminator();
                    }
                }
                self.expect(TokenKind::RightBrace, "'}'");
                OperatorBody::AccessorMembers(members)
            } else {
                match self.parse_block() {
                    Some(block) => OperatorBody::Block(block),
                    None => OperatorBody::None,
                }
            }
        } else {
            OperatorBody::None
        };

        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(
            span,
            NodeKind::Operator(OperatorDecl {
                modifiers,
                head,
                body,
            }),
        ))
    }

    fn operator_symbol_text(&self) -> Option<String> {
        let text = match self.peek_kind() {
            TokenKind::Plus => "+",
            TokenKind::Minus => "-",
            TokenKind::Star => "*",
            TokenKind::Slash => "/",
            TokenKind::Percent => "%",
            TokenKind::EqualsEquals => "==",
            TokenKind::Greater => ">",
            TokenKind::Less => "<",
            TokenKind::QuestionQuestion => "??",
            _ => return None,
        };
        Some(text.to_string())
    }

    fn parse_field(&mut self, modifiers: Modifiers) -> Option<NodeId> {
        let start_token = self.position;
        let head = self.parse_definition_head(true)?;
        let init = if self.eat(TokenKind::Equals) {
            self.parse_expression().map(Initializer::Expression)
        } else if self.at(TokenKind::LeftBrace) {
            self.parse_block().map(Initializer::Block)
        } else {
            None
        };
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(
            span,
            NodeKind::Field(FieldDecl {
                modifiers,
                name: head.name,
                ty: head.ty,
                init,
            }),
        ))
    }

    fn parse_local(&mut self, modifiers: Modifiers) -> Option<NodeId> {
        let start_token = self.position;
        self.bump();
        let mut items = Vec::new();
        loop {
            let item_start = self.position;
            let head = self.parse_definition_head(false)?;
            let init = if self.eat(TokenKind::Equals) {
                self.parse_expression()
            } else {
                None
            };
            items.push(LocalItem {
                name: head.name,
                ty: head.ty,
                init,
                span: self.span_between(item_start, self.position),
            });
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        let span = self.span_between(start_token, self.position);
        Some(
            self.ast
                .push(span, NodeKind::Local(LocalDecl { modifiers, items })),
        )
    }

    fn parse_heap(&mut self, modifiers: Modifiers) -> Option<NodeId> {
        let start_token = self.position;
        self.bump();
        let size = if self.eat(TokenKind::LeftParen) {
            let size = self.parse_expression();
            self.expect(TokenKind::RightParen, "')' after heap allocation size");
            size
        } else {
            None
        };
        let head = self.parse_definition_head(false)?;
        let init = if self.eat(TokenKind::Equals) {
            self.parse_expression()
        } else {
            None
        };
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(
            span,
            NodeKind::Heap(HeapDecl {
                modifiers,
                size,
                name: head.name,
                ty: head.ty,
                init,
            }),
        ))
    }

    fn parse_fixed(&mut self, modifiers: Modifiers) -> Option<NodeId> {
        let start_token = self.position;
        self.bump();
        let head = self.parse_definition_head(true)?;
        let init = if self.eat(TokenKind::Equals) {
            self.parse_expression().map(Initializer::Expression)
        } else if self.at(TokenKind::LeftBrace) {
            self.parse_block().map(Initializer::Block)
        } else {
            None
        };
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(
            span,
            NodeKind::Fixed(FixedDecl {
                modifiers,
                name: head.name,
                generics: head.generics,
                ty: head.ty,
                init,
            }),
        ))
    }

    fn parse_linkable(&mut self, modifiers: Modifiers) -> Option<NodeId> {
        let start_token = self.position;
        self.bump();
        let symbol = if self.eat(TokenKind::LeftParen) {
            let token = self.peek();
            let text = if token.kind == TokenKind::StringLiteral {
                self.bump();
                Some(token.text(self.source).to_string())
            } else {
                self.error_here("expected a string literal symbol name");
                None
            };
            self.expect(TokenKind::RightParen, "')' after symbol name");
            text
        } else {
            None
        };
        let head = self.parse_definition_head(true)?;
        let init = if self.eat(TokenKind::Equals) {
            self.parse_expression().map(Initializer::Expression)
        } else if self.at(TokenKind::LeftBrace) {
            self.parse_block().map(Initializer::Block)
        } else {
            None
        };
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(
            span,
            NodeKind::Linkable(LinkableDecl {
                modifiers,
                symbol,
                name: head.name,
                generics: head.generics,
                ty: head.ty,
                init,
            }),
        ))
    }

    // -- simple statements -------------------------------------------------

    fn parse_use(&mut self) -> Option<NodeId> {
        let start_token = self.position;
        self.bump();
        let mut targets = Vec::new();
        loop {
            targets.push(self.parse_qualified_name()?);
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(span, NodeKind::Use(UseStmt { targets })))
    }

    fn parse_if(&mut self) -> Option<NodeId> {
        let start_token = self.position;
        self.bump();
        let condition = self.parse_expression()?;
        let body = self.parse_block()?;
        let mut branches = vec![IfBranch {
            condition,
            body,
            span: self.span_between(start_token, self.position),
        }];
        let mut else_block = None;
        loop {
            if !self.eat(TokenKind::Else) {
                break;
            }
            if self.at(TokenKind::If) {
                let branch_start = self.position;
                self.bump();
                let condition = self.parse_expression()?;
                let body = self.parse_block()?;
                branches.push(IfBranch {
                    condition,
                    body,
                    span: self.span_between(branch_start, self.position),
                });
            } else {
                else_block = self.parse_block();
                break;
            }
        }
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(
            span,
            NodeKind::If(IfStmt {
                branches,
                else_block,
            }),
        ))
    }

    fn parse_while(&mut self) -> Option<NodeId> {
        let start_token = self.position;
        self.bump();
        let condition = self.parse_expression()?;
        let label = self.parse_optional_label();
        let body = self.parse_block()?;
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(
            span,
            NodeKind::While(LoopStmt {
                condition,
                label,
                body,
            }),
        ))
    }

    fn parse_do_while(&mut self) -> Option<NodeId> {
        let start_token = self.position;
        self.bump();
        let label = self.parse_optional_label();
        let body = self.parse_block()?;
        self.expect(TokenKind::While, "'while' after 'do' body");
        let condition = self.parse_expression()?;
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(
            span,
            NodeKind::DoWhile(DoWhileStmt {
                label,
                body,
                condition,
            }),
        ))
    }

    fn parse_for(&mut self) -> Option<NodeId> {
        let start_token = self.position;
        self.bump();
        let by_ref = self.eat(TokenKind::Ampersand);
        let head = self.parse_definition_head(false)?;
        let Some(ty) = head.ty else {
            self.error_here("the for variable requires a type");
            return None;
        };
        self.expect(TokenKind::In, "'in' in for statement");
        let first = self.parse_expression()?;
        let target = if self.at(TokenKind::DotDot) || self.at(TokenKind::DotDotBang) {
            let range = self.parse_range_after_start(first)?;
            ForTarget::Range(range)
        } else {
            ForTarget::Expression(first)
        };
        let label = self.parse_optional_label();
        let body = self.parse_block()?;
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(
            span,
            NodeKind::For(ForStmt {
                by_ref,
                name: head.name,
                ty,
                target,
                label,
                body,
            }),
        ))
    }

    /// Parses the rest of a range after its start expression.
    pub(crate) fn parse_range_after_start(&mut self, start: NodeId) -> Option<RangeExpr> {
        if self.eat(TokenKind::DotDot) {
            let second = self.parse_expression()?;
            if self.at(TokenKind::DotDot) || self.at(TokenKind::DotDotBang) {
                let inclusive = self.at(TokenKind::DotDot);
                self.bump();
                let end = self.parse_expression()?;
                Some(RangeExpr {
                    start,
                    step: Some(second),
                    end,
                    inclusive,
                })
            } else {
                Some(RangeExpr {
                    start,
                    step: None,
                    end: second,
                    inclusive: true,
                })
            }
        } else if self.eat(TokenKind::DotDotBang) {
            let end = self.parse_expression()?;
            Some(RangeExpr {
                start,
                step: None,
                end,
                inclusive: false,
            })
        } else {
            self.error_here("expected '..' or '..!' in range");
            None
        }
    }

    fn parse_optional_label(&mut self) -> Option<String> {
        if self.eat(TokenKind::Colon) {
            self.parse_name().map(|(name, _)| name)
        } else {
            None
        }
    }

    fn parse_match(&mut self) -> Option<NodeId> {
        let start_token = self.position;
        self.bump();
        let value = self.parse_expression()?;
        let label = self.parse_optional_label();
        self.expect(TokenKind::LeftBrace, "'{' in match statement");
        let mut cases = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at_end() {
            let case_start = self.position;
            if self.expect(TokenKind::Case, "'case'").is_none() {
                self.synchronize();
                // Recovery must make progress even when the current token is
                // one the synchronizer does not consume.
                if self.position == case_start {
                    self.bump();
                }
                continue;
            }
            let pattern = self.parse_match_pattern();
            let guard = if self.eat(TokenKind::When) {
                self.parse_expression()
            } else {
                None
            };
            // `MatchCase` requires a Block, but the Built-In Library's own
            // source writes `case is payload:T:` followed by a single
            // statement. Accept that form too (spec draft inconsistency,
            // recorded in docs/spec-notes/_inconsistencies.md).
            let body = if self.at(TokenKind::LeftBrace) {
                self.parse_block()?
            } else if self.eat(TokenKind::Colon) {
                let statement = self.parse_statement()?;
                self.expect_statement_terminator();
                statement
            } else {
                self.parse_block()?
            };
            cases.push(MatchCase {
                pattern,
                guard,
                body,
                span: self.span_between(case_start, self.position),
            });
            // Guarantee progress on malformed input: recovery that does not
            // consume a token must not spin forever.
            if self.position == case_start {
                self.bump();
            }
        }
        self.expect(TokenKind::RightBrace, "'}'");
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(
            span,
            NodeKind::Match(MatchStmt {
                value,
                label,
                cases,
            }),
        ))
    }

    fn parse_match_pattern(&mut self) -> Option<Pattern> {
        match self.peek_kind() {
            TokenKind::Wildcard => {
                self.bump();
                Some(Pattern::Wildcard)
            }
            TokenKind::Is => {
                self.bump();
                let name = if (self.at(TokenKind::Identifier) || self.peek_kind().is_keyword())
                    && self.peek_at(1) == TokenKind::Colon
                {
                    let (name, _) = self.parse_name()?;
                    self.expect(TokenKind::Colon, "':' after pattern binding");
                    Some(name)
                } else {
                    None
                };
                Some(Pattern::Is {
                    name,
                    target: self.parse_is_target()?,
                })
            }
            TokenKind::NotEquals
            | TokenKind::Greater
            | TokenKind::Less
            | TokenKind::GreaterEquals
            | TokenKind::LessEquals => {
                let operator_token = self.bump();
                let value = self.parse_expression()?;
                Some(Pattern::Comparison {
                    operator: operator_token.kind,
                    value,
                    span: operator_token.span,
                })
            }
            _ => Some(Pattern::Equal(self.parse_expression()?)),
        }
    }

    /// Parses an `is` target (§4.8).
    pub(crate) fn parse_is_target(&mut self) -> Option<IsTarget> {
        match self.peek_kind() {
            TokenKind::Illegal => {
                self.bump();
                Some(IsTarget::Illegal)
            }
            TokenKind::Infinite => {
                self.bump();
                Some(IsTarget::Infinite { negative: false })
            }
            TokenKind::Minus => {
                self.bump();
                self.expect(TokenKind::Infinite, "'infinite' after '-'");
                Some(IsTarget::Infinite { negative: true })
            }
            _ => {
                let start = self.position;
                let mut node = self.parse_qualified_name()?;
                loop {
                    if self.at(TokenKind::Less) {
                        let Some(arguments) =
                            self.try_parse(Self::parse_monomorphization_arguments)
                        else {
                            break;
                        };
                        let span = self.span_between(start, self.position);
                        node = self.ast.push(
                            span,
                            NodeKind::Monomorphization(MonomorphizationExpr {
                                base: node,
                                arguments,
                            }),
                        );
                        continue;
                    }
                    if self.eat(TokenKind::Dot) {
                        let (member, member_span) = self.parse_member_name()?;
                        let span = Span::new(self.file, self.ast.span(node).start, member_span.end);
                        node = self.ast.push(
                            span,
                            NodeKind::MemberAccess(MemberAccess {
                                base: node,
                                name: member,
                            }),
                        );
                        continue;
                    }
                    break;
                }
                let span = self.ast.span(node);
                let text = self.source[span.start as usize..span.end as usize]
                    .split_whitespace()
                    .collect::<String>();
                Some(IsTarget::Identifier { text, node })
            }
        }
    }

    fn parse_break(&mut self) -> Option<NodeId> {
        let start_token = self.position;
        self.bump();
        let label = if self.at(TokenKind::Identifier) || self.peek_kind().is_keyword() {
            self.parse_name().map(|(name, _)| name)
        } else {
            None
        };
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(span, NodeKind::Break(BreakStmt { label })))
    }

    fn parse_continue(&mut self) -> Option<NodeId> {
        let start_token = self.position;
        self.bump();
        let label = if self.at(TokenKind::Identifier) || self.peek_kind().is_keyword() {
            self.parse_name().map(|(name, _)| name)
        } else {
            None
        };
        let span = self.span_between(start_token, self.position);
        Some(
            self.ast
                .push(span, NodeKind::Continue(ContinueStmt { label })),
        )
    }

    fn parse_return(&mut self) -> Option<NodeId> {
        let start_token = self.position;
        self.bump();
        let value = if self.at_expression_start() {
            self.parse_expression()
        } else {
            None
        };
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(span, NodeKind::Return(ReturnStmt { value })))
    }

    fn parse_throw(&mut self) -> Option<NodeId> {
        let start_token = self.position;
        self.bump();
        let value = self.parse_expression()?;
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(span, NodeKind::Throw(ThrowStmt { value })))
    }

    fn parse_trap(&mut self) -> Option<NodeId> {
        let start_token = self.position;
        self.bump();
        let body = self.parse_block()?;
        let mut catches = Vec::new();
        let mut finally = None;
        loop {
            if self.eat(TokenKind::Catch) {
                let catch_start = self.position;
                let mut clause = CatchClause {
                    name: None,
                    ty: None,
                    wildcard: false,
                    body: 0,
                    span: self.peek().span,
                };
                if self.at(TokenKind::Wildcard) {
                    self.bump();
                    clause.wildcard = true;
                } else {
                    if (self.at(TokenKind::Identifier) || self.peek_kind().is_keyword())
                        && self.peek_at(1) == TokenKind::Colon
                    {
                        let (name, _) = self.parse_name()?;
                        self.expect(TokenKind::Colon, "':' in catch binding");
                        clause.name = Some(name);
                    }
                    clause.ty = self.parse_type();
                }
                clause.body = self.parse_block()?;
                clause.span = self.span_between(catch_start, self.position);
                catches.push(clause);
            } else if self.eat(TokenKind::Finally) {
                finally = self.parse_block();
                break;
            } else {
                break;
            }
        }
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(
            span,
            NodeKind::Trap(TrapStmt {
                body,
                catches,
                finally,
            }),
        ))
    }

    fn parse_dispose(&mut self) -> Option<NodeId> {
        let start_token = self.position;
        self.bump();
        let target = self.parse_expression()?;
        let span = self.span_between(start_token, self.position);
        Some(
            self.ast
                .push(span, NodeKind::Dispose(DisposeStmt { target })),
        )
    }

    fn parse_compile_time_statement(&mut self) -> Option<NodeId> {
        let start_token = self.position;
        self.bump(); // '#'
        if self.eat(TokenKind::If) {
            let condition = self.parse_expression()?;
            let then_block = self.parse_block()?;
            let else_block = if self.eat(TokenKind::Else) {
                self.parse_block()
            } else {
                None
            };
            let span = self.span_between(start_token, self.position);
            return Some(self.ast.push(
                span,
                NodeKind::CompileTimeIf(CompileTimeIfStmt {
                    condition,
                    then_block,
                    else_block,
                }),
            ));
        }
        let block = self.parse_block()?;
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(span, NodeKind::CompileTimeBlock(block)))
    }

    fn parse_expression_statement(&mut self, start_token: usize) -> Option<NodeId> {
        let expression = self.parse_expression()?;
        if self.at(TokenKind::Equals) {
            self.bump();
            let value = self.parse_expression()?;
            let span = self.span_between(start_token, self.position);
            return Some(self.ast.push(
                span,
                NodeKind::Set(SetStmt {
                    target: expression,
                    value,
                }),
            ));
        }
        if let Some(op) = self.compound_assignment_op() {
            self.bump();
            let value = self.parse_expression()?;
            let span = self.span_between(start_token, self.position);
            return Some(self.ast.push(
                span,
                NodeKind::CompoundSet(CompoundSetStmt {
                    target: expression,
                    value,
                    op,
                }),
            ));
        }
        let span = self.span_between(start_token, self.position);
        Some(self.ast.push(span, NodeKind::ExpressionStmt(expression)))
    }

    fn compound_assignment_op(&self) -> Option<BinaryOp> {
        let op = match self.peek_kind() {
            TokenKind::PlusEquals => BinaryOp::Add,
            TokenKind::MinusEquals => BinaryOp::Subtract,
            TokenKind::StarEquals => BinaryOp::Multiply,
            TokenKind::SlashEquals => BinaryOp::Divide,
            TokenKind::PercentEquals => BinaryOp::Remainder,
            TokenKind::AmpersandEquals => BinaryOp::BitwiseAnd,
            TokenKind::PipeEquals => BinaryOp::BitwiseOr,
            TokenKind::CaretEquals => BinaryOp::BitwiseXor,
            TokenKind::ShiftLeftEquals => BinaryOp::ShiftLeft,
            TokenKind::ShiftRightEquals => BinaryOp::ShiftRight,
            _ => return None,
        };
        Some(op)
    }

    /// Whether the current token can start an expression.
    pub(crate) fn at_expression_start(&self) -> bool {
        matches!(
            self.peek_kind(),
            TokenKind::Identifier
                | TokenKind::IntegerLiteral
                | TokenKind::FloatLiteral
                | TokenKind::CharacterLiteral
                | TokenKind::StringLiteral
                | TokenKind::VerbatimStringLiteral
                | TokenKind::True
                | TokenKind::False
                | TokenKind::Illegal
                | TokenKind::Infinite
                | TokenKind::Uninitialized
                | TokenKind::This
                | TokenKind::Owner
                | TokenKind::Base
                | TokenKind::LeftParen
                | TokenKind::LeftBrace
                | TokenKind::Minus
                | TokenKind::Bang
                | TokenKind::Caret
                | TokenKind::Ampersand
                | TokenKind::Star
                | TokenKind::Cast
                | TokenKind::Hash
        ) || (self.peek_kind().is_keyword() && !self.at(TokenKind::Eof))
    }
}

fn modifiers_is_empty(modifiers: &Modifiers) -> bool {
    modifiers.accessibility == Accessibility::Unspecified
        && modifiers.scoped.is_empty()
        && !modifiers.readonly
        && !modifiers.implicitable
        && !modifiers.verbatim
        && !modifiers.manual
        && !modifiers.abstract_
        && !modifiers.sealed
        && !modifiers.virtual_
        && !modifiers.override_
        && !modifiers.static_
        && modifiers.aligned.is_none()
}

enum ClassLikeKind {
    Class,
    Structure,
    Interface,
}
