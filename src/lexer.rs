//! Token kinds and the NovaSharp lexer.
//!
//! The lexer is hand-written and produces a flat token stream plus a list of
//! lexical errors (it always recovers, never panics). All literal *values* are
//! decoded later from the token's source span (see [`crate::literal`]); the
//! lexer only validates their surface syntax.
//!
//! Spec references are to `docs/grammar/Draft 1.1.md`:
//! - §2.2 whitespace and line terminators
//! - §2.3 comments
//! - §2.4 identifiers and the `@` keyword escape
//! - §2.6 keywords
//! - §2.7 literals (`:ascii`-style encoding tags included)
//! - §2.8 statement termination (handled by the parser)

use crate::source::{FileId, Span};

/// A single lexical token kind.
///
/// Keywords are kept as distinct kinds (rather than one `Keyword` variant) so
/// that parser code can match on them directly. Because NovaSharp allows
/// keywords to be used as names when the context expects a name, every keyword
/// token still carries its source text; see [`Token::text`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum TokenKind {
    // --- identifiers and literals -----------------------------------------
    /// A plain identifier (`Name` in the spec).
    Identifier,
    /// `42`, `0xFF`, `0b1010`, `1_000_000`
    IntegerLiteral,
    /// `3.14`, `1.5e10`, `2.5e-3`
    FloatLiteral,
    /// `'a'`, `'\n'`, `'中':utf8`
    CharacterLiteral,
    /// `"Hello"`
    StringLiteral,
    /// `@"C:\raw"`
    VerbatimStringLiteral,

    // --- identifier kinds (§2.6) ------------------------------------------
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
    /// `module`
    Module,
    /// `local`
    Local,
    /// `fixed`
    Fixed,
    /// `accessor`
    Accessor,
    /// `heap`
    Heap,
    /// `operator`
    Operator,
    /// `linkable`
    Linkable,

    // --- type constructors / basic types (§2.6) ---------------------------
    /// `array`
    Array,
    /// `function`
    Function,
    /// `pointer`
    Pointer,
    /// `integer`
    Integer,
    /// `cardinal`
    Cardinal,
    /// `float`
    Float,
    /// `boolean`
    Boolean,

    // --- modifiers (§2.6, §9) ---------------------------------------------
    /// `public`
    Public,
    /// `private`
    Private,
    /// `protected`
    Protected,
    /// `scoped`
    Scoped,
    /// `readonly`
    Readonly,
    /// `implicitable`
    Implicitable,
    /// `verbatim`
    Verbatim,
    /// `manual`
    Manual,
    /// `abstract`
    Abstract,
    /// `virtual`
    Virtual,
    /// `override`
    Override,
    /// `sealed`
    Sealed,
    /// `aligned`
    Aligned,
    /// `static`
    Static,

    // --- logical control flow (§2.6) --------------------------------------
    /// `if`
    If,
    /// `else`
    Else,
    /// `while`
    While,
    /// `do`
    Do,
    /// `for`
    For,
    /// `in`
    In,
    /// `match`
    Match,
    /// `case`
    Case,
    /// `break`
    Break,
    /// `continue`
    Continue,
    /// `when`
    When,

    // --- control flow (§2.6) ----------------------------------------------
    /// `return`
    Return,
    /// `throw`
    Throw,
    /// `trap`
    Trap,
    /// `catch`
    Catch,
    /// `finally`
    Finally,

    // --- misc keywords (§2.6) ---------------------------------------------
    /// `dispose`
    Dispose,
    /// `this`
    This,
    /// `owner`
    Owner,
    /// `base`
    Base,
    /// `ascii`
    Ascii,
    /// `utf8`
    Utf8,
    /// `utf16`
    Utf16,
    /// `utf32`
    Utf32,
    /// `ucs2`
    Ucs2,
    /// `ucs4`
    Ucs4,
    /// `wildcard`
    Wildcard,
    /// `true`
    True,
    /// `false`
    False,
    /// `illegal`
    Illegal,
    /// `infinite`
    Infinite,
    /// `uninitialized`
    Uninitialized,
    /// `use`
    Use,
    /// `is`
    Is,
    /// `extension`
    Extension,
    /// `Cast`
    Cast,
    /// `basic`
    Basic,

    // --- punctuation and operators ----------------------------------------
    /// `@`
    At,
    /// `#`
    Hash,
    /// `(`
    LeftParen,
    /// `)`
    RightParen,
    /// `{`
    LeftBrace,
    /// `}`
    RightBrace,
    /// `[`
    LeftBracket,
    /// `]`
    RightBracket,
    /// `,`
    Comma,
    /// `;`
    Semicolon,
    /// `:`
    Colon,
    /// `.`
    Dot,
    /// `..`
    DotDot,
    /// `..!`
    DotDotBang,
    /// `->`
    Arrow,
    /// `=>`
    FatArrow,
    /// `?`
    Question,
    /// `??`
    QuestionQuestion,
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `%`
    Percent,
    /// `&`
    Ampersand,
    /// `|`
    Pipe,
    /// `^`
    Caret,
    /// `!`
    Bang,
    /// `=`
    Equals,
    /// `==`
    EqualsEquals,
    /// `!=`
    NotEquals,
    /// `<`
    Less,
    /// `>`
    Greater,
    /// `<=`
    LessEquals,
    /// `>=`
    GreaterEquals,
    /// `&&`
    AmpersandAmpersand,
    /// `||`
    PipePipe,
    /// `<<`
    ShiftLeft,
    /// `>>`
    ShiftRight,
    /// `+=`
    PlusEquals,
    /// `-=`
    MinusEquals,
    /// `*=`
    StarEquals,
    /// `/=`
    SlashEquals,
    /// `%=`
    PercentEquals,
    /// `&=`
    AmpersandEquals,
    /// `|=`
    PipeEquals,
    /// `^=`
    CaretEquals,
    /// `<<=`
    ShiftLeftEquals,
    /// `>>=`
    ShiftRightEquals,

    /// End of input.
    Eof,
}

impl TokenKind {
    /// Maps a keyword's spelling to its token kind.
    #[must_use]
    pub fn keyword(text: &str) -> Option<TokenKind> {
        use TokenKind::{
            Abstract, Accessor, Aligned, Array, Ascii, Base, Basic, Boolean, Break, Cardinal, Case,
            Cast, Catch, Class, Continue, Dispose, Do, Else, Enumeration, Extension, False,
            Finally, Fixed, Float, For, Function, Heap, If, Illegal, Implicitable, In, Infinite,
            Integer, Interface, Is, Linkable, Local, Manual, Match, Module, Operator, Override,
            Owner, Pointer, Private, Protected, Public, Readonly, Return, Scoped, Sealed, Static,
            Structure, This, Throw, Trap, True, Ucs2, Ucs4, Uninitialized, Union, Use, Utf16,
            Utf32, Utf8, Variant, Verbatim, Virtual, When, While, Wildcard,
        };
        Some(match text {
            "class" => Class,
            "structure" => Structure,
            "interface" => Interface,
            "enumeration" => Enumeration,
            "variant" => Variant,
            "union" => Union,
            "module" => Module,
            "local" => Local,
            "fixed" => Fixed,
            "accessor" => Accessor,
            "heap" => Heap,
            "operator" => Operator,
            "linkable" => Linkable,
            "array" => Array,
            "function" => Function,
            "pointer" => Pointer,
            "integer" => Integer,
            "cardinal" => Cardinal,
            "float" => Float,
            "boolean" => Boolean,
            "public" => Public,
            "private" => Private,
            "protected" => Protected,
            "scoped" => Scoped,
            "readonly" => Readonly,
            "implicitable" => Implicitable,
            "verbatim" => Verbatim,
            "manual" => Manual,
            "abstract" => Abstract,
            "virtual" => Virtual,
            "override" => Override,
            "sealed" => Sealed,
            "aligned" => Aligned,
            "static" => Static,
            "if" => If,
            "else" => Else,
            "while" => While,
            "do" => Do,
            "for" => For,
            "in" => In,
            "match" => Match,
            "case" => Case,
            "break" => Break,
            "continue" => Continue,
            "when" => When,
            "return" => Return,
            "throw" => Throw,
            "trap" => Trap,
            "catch" => Catch,
            "finally" => Finally,
            "dispose" => Dispose,
            "this" => This,
            "owner" => Owner,
            "base" => Base,
            "ascii" => Ascii,
            "utf8" => Utf8,
            "utf16" => Utf16,
            "utf32" => Utf32,
            "ucs2" => Ucs2,
            "ucs4" => Ucs4,
            "wildcard" => Wildcard,
            "true" => True,
            "false" => False,
            "illegal" => Illegal,
            "infinite" => Infinite,
            "uninitialized" => Uninitialized,
            "use" => Use,
            "is" => Is,
            "extension" => Extension,
            "Cast" => Cast,
            "basic" => Basic,
            _ => return None,
        })
    }

    /// Returns whether this token is one of NovaSharp's reserved words.
    #[must_use]
    pub const fn is_keyword(self) -> bool {
        use TokenKind::*;
        matches!(
            self,
            Class
                | Structure
                | Interface
                | Enumeration
                | Variant
                | Union
                | Module
                | Local
                | Fixed
                | Accessor
                | Heap
                | Operator
                | Linkable
                | Array
                | Function
                | Pointer
                | Integer
                | Cardinal
                | Float
                | Boolean
                | Public
                | Private
                | Protected
                | Scoped
                | Readonly
                | Implicitable
                | Verbatim
                | Manual
                | Abstract
                | Virtual
                | Override
                | Sealed
                | Aligned
                | Static
                | If
                | Else
                | While
                | Do
                | For
                | In
                | Match
                | Case
                | Break
                | Continue
                | When
                | Return
                | Throw
                | Trap
                | Catch
                | Finally
                | Dispose
                | This
                | Owner
                | Base
                | Ascii
                | Utf8
                | Utf16
                | Utf32
                | Ucs2
                | Ucs4
                | Wildcard
                | True
                | False
                | Illegal
                | Infinite
                | Uninitialized
                | Use
                | Is
                | Extension
                | Cast
                | Basic
        )
    }

    /// Human-readable name for diagnostics.
    #[must_use]
    pub fn describe(self) -> &'static str {
        use TokenKind::*;
        match self {
            Identifier => "identifier",
            IntegerLiteral => "integer literal",
            FloatLiteral => "float literal",
            CharacterLiteral => "character literal",
            StringLiteral => "string literal",
            VerbatimStringLiteral => "verbatim string literal",
            Eof => "end of file",
            At => "'@'",
            Hash => "'#'",
            LeftParen => "'('",
            RightParen => "')'",
            LeftBrace => "'{'",
            RightBrace => "'}'",
            LeftBracket => "'['",
            RightBracket => "']'",
            Comma => "','",
            Semicolon => "';'",
            Colon => "':'",
            Dot => "'.'",
            DotDot => "'..'",
            DotDotBang => "'..!'",
            Arrow => "'->'",
            FatArrow => "'=>'",
            Question => "'?'",
            QuestionQuestion => "'??'",
            Plus => "'+'",
            Minus => "'-'",
            Star => "'*'",
            Slash => "'/'",
            Percent => "'%'",
            Ampersand => "'&'",
            Pipe => "'|'",
            Caret => "'^'",
            Bang => "'!'",
            Equals => "'='",
            EqualsEquals => "'=='",
            NotEquals => "'!='",
            Less => "'<'",
            Greater => "'>'",
            LessEquals => "'<='",
            GreaterEquals => "'>='",
            AmpersandAmpersand => "'&&'",
            PipePipe => "'||'",
            ShiftLeft => "'<<'",
            ShiftRight => "'>>'",
            PlusEquals => "'+='",
            MinusEquals => "'-='",
            StarEquals => "'*='",
            SlashEquals => "'/='",
            PercentEquals => "'%='",
            AmpersandEquals => "'&='",
            PipeEquals => "'|='",
            CaretEquals => "'^='",
            ShiftLeftEquals => "'<<='",
            ShiftRightEquals => "'>>='",
            other => other.keyword_spelling().unwrap_or("token"),
        }
    }

    /// The spelling of a keyword token, if it is one.
    #[must_use]
    pub fn keyword_spelling(self) -> Option<&'static str> {
        use TokenKind::*;
        Some(match self {
            Class => "class",
            Structure => "structure",
            Interface => "interface",
            Enumeration => "enumeration",
            Variant => "variant",
            Union => "union",
            Module => "module",
            Local => "local",
            Fixed => "fixed",
            Accessor => "accessor",
            Heap => "heap",
            Operator => "operator",
            Linkable => "linkable",
            Array => "array",
            Function => "function",
            Pointer => "pointer",
            Integer => "integer",
            Cardinal => "cardinal",
            Float => "float",
            Boolean => "boolean",
            Public => "public",
            Private => "private",
            Protected => "protected",
            Scoped => "scoped",
            Readonly => "readonly",
            Implicitable => "implicitable",
            Verbatim => "verbatim",
            Manual => "manual",
            Abstract => "abstract",
            Virtual => "virtual",
            Override => "override",
            Sealed => "sealed",
            Aligned => "aligned",
            Static => "static",
            If => "if",
            Else => "else",
            While => "while",
            Do => "do",
            For => "for",
            In => "in",
            Match => "match",
            Case => "case",
            Break => "break",
            Continue => "continue",
            When => "when",
            Return => "return",
            Throw => "throw",
            Trap => "trap",
            Catch => "catch",
            Finally => "finally",
            Dispose => "dispose",
            This => "this",
            Owner => "owner",
            Base => "base",
            Ascii => "ascii",
            Utf8 => "utf8",
            Utf16 => "utf16",
            Utf32 => "utf32",
            Ucs2 => "ucs2",
            Ucs4 => "ucs4",
            Wildcard => "wildcard",
            True => "true",
            False => "false",
            Illegal => "illegal",
            Infinite => "infinite",
            Uninitialized => "uninitialized",
            Use => "use",
            Is => "is",
            Extension => "extension",
            Cast => "Cast",
            Basic => "basic",
            _ => return None,
        })
    }
}

/// A token: kind plus the source range it covers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Token {
    /// What kind of token this is.
    pub kind: TokenKind,
    /// Byte range in the source file.
    pub span: Span,
}

impl Token {
    /// Creates a token.
    #[must_use]
    pub const fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }

    /// The token's exact source text.
    #[must_use]
    pub fn text<'source>(&self, source: &'source str) -> &'source str {
        let start = self.span.start as usize;
        let end = (self.span.end as usize).min(source.len());
        source.get(start..end).unwrap_or("")
    }
}

/// A lexical error. Lexing continues after an error so that later phases can
/// still run and report more problems.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LexError {
    /// Where the problem is.
    pub span: Span,
    /// What went wrong.
    pub message: String,
}

/// Result of lexing one file.
#[derive(Clone, Debug)]
pub struct LexResult {
    /// Tokens, always terminated by [`TokenKind::Eof`].
    pub tokens: Vec<Token>,
    /// Lexical errors in source order.
    pub errors: Vec<LexError>,
}

/// Lexes `source` (which must be registered with `file` in the source manager).
#[must_use]
pub fn tokenize(file: FileId, source: &str) -> LexResult {
    let mut lexer = Lexer::new(file, source);
    lexer.run();
    LexResult {
        tokens: lexer.tokens,
        errors: lexer.errors,
    }
}

struct Lexer<'source> {
    file: FileId,
    source: &'source str,
    position: usize,
    tokens: Vec<Token>,
    errors: Vec<LexError>,
}

impl<'source> Lexer<'source> {
    fn new(file: FileId, source: &'source str) -> Self {
        Self {
            file,
            source,
            position: 0,
            tokens: Vec::new(),
            errors: Vec::new(),
        }
    }

    // -- low level helpers --------------------------------------------------

    fn peek_byte(&self) -> Option<u8> {
        self.source.as_bytes().get(self.position).copied()
    }

    fn peek_byte_at(&self, offset: usize) -> Option<u8> {
        self.source.as_bytes().get(self.position + offset).copied()
    }

    fn at_end(&self) -> bool {
        self.position >= self.source.len()
    }

    /// Consumes one full UTF-8 scalar value.
    fn bump_char(&mut self) -> Option<char> {
        let character = self.source[self.position..].chars().next()?;
        self.position += character.len_utf8();
        Some(character)
    }

    fn span_from(&self, start: usize) -> Span {
        Span::new(self.file, start as u32, self.position as u32)
    }

    fn error(&mut self, start: usize, message: impl Into<String>) {
        let span = self.span_from(start);
        self.errors.push(LexError {
            span,
            message: message.into(),
        });
    }

    fn push(&mut self, kind: TokenKind, start: usize) {
        let span = self.span_from(start);
        self.tokens.push(Token::new(kind, span));
    }

    fn starts_with(&self, text: &str) -> bool {
        self.source[self.position..].starts_with(text)
    }

    fn is_identifier_start(byte: u8) -> bool {
        byte.is_ascii_alphabetic() || byte == b'_'
    }

    fn is_identifier_continue(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || byte == b'_'
    }

    fn is_line_terminator(byte: u8) -> bool {
        byte == b'\n' || byte == b'\r'
    }

    fn is_whitespace(byte: u8) -> bool {
        matches!(byte, b' ' | b'\t' | b'\n' | b'\r')
    }

    // -- driver -------------------------------------------------------------

    fn run(&mut self) {
        loop {
            self.skip_whitespace_and_comments();
            if self.at_end() {
                let start = self.position;
                self.push(TokenKind::Eof, start);
                break;
            }
            self.lex_token();
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek_byte() {
                Some(byte) if Self::is_whitespace(byte) => {
                    self.position += 1;
                }
                Some(b'/') if self.peek_byte_at(1) == Some(b'/') => {
                    self.position += 2;
                    while let Some(byte) = self.peek_byte() {
                        if Self::is_line_terminator(byte) {
                            break;
                        }
                        self.position += 1;
                    }
                }
                Some(b'/') if self.peek_byte_at(1) == Some(b'*') => {
                    let start = self.position;
                    self.position += 2;
                    loop {
                        if self.at_end() {
                            self.error(start, "unterminated block comment");
                            break;
                        }
                        if self.peek_byte() == Some(b'*') && self.peek_byte_at(1) == Some(b'/') {
                            self.position += 2;
                            break;
                        }
                        self.position += 1;
                    }
                }
                _ => break,
            }
        }
    }

    fn lex_token(&mut self) {
        let start = self.position;
        let Some(byte) = self.peek_byte() else {
            return;
        };

        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.lex_identifier(),
            b'0'..=b'9' => self.lex_number(),
            b'"' => self.lex_normal_string(),
            b'\'' => self.lex_character(),
            b'@' => {
                if self.peek_byte_at(1) == Some(b'"') {
                    self.position += 2;
                    self.lex_verbatim_string(start);
                } else {
                    self.position += 1;
                    self.push(TokenKind::At, start);
                }
            }
            _ => self.lex_symbol(start),
        }
    }

    // -- identifiers and keywords ------------------------------------------

    fn lex_identifier(&mut self) {
        let start = self.position;
        while let Some(byte) = self.peek_byte() {
            if Self::is_identifier_continue(byte) {
                self.position += 1;
            } else {
                break;
            }
        }
        let text = &self.source[start..self.position];
        let kind = TokenKind::keyword(text).unwrap_or(TokenKind::Identifier);
        self.push(kind, start);
    }

    // -- numbers ------------------------------------------------------------

    fn lex_number(&mut self) {
        let start = self.position;

        if self.starts_with("0x") {
            self.position += 2;
            let digits_start = self.position;
            while let Some(byte) = self.peek_byte() {
                if byte.is_ascii_hexdigit() || byte == b'_' {
                    self.position += 1;
                } else {
                    break;
                }
            }
            if self.position == digits_start {
                self.error(start, "hexadecimal literal requires at least one digit");
            }
            self.report_bad_numeric_suffix(start);
            self.push(TokenKind::IntegerLiteral, start);
            return;
        }

        if self.starts_with("0b") {
            self.position += 2;
            let digits_start = self.position;
            while let Some(byte) = self.peek_byte() {
                if matches!(byte, b'0' | b'1' | b'_') {
                    self.position += 1;
                } else {
                    break;
                }
            }
            if self.position == digits_start {
                self.error(start, "binary literal requires at least one digit");
            }
            self.report_bad_numeric_suffix(start);
            self.push(TokenKind::IntegerLiteral, start);
            return;
        }

        self.consume_decimal_digits();

        // A fraction only starts when the dot is followed by a digit. This is
        // what keeps `1..2` (a range) from being scanned as a float.
        if self.peek_byte() == Some(b'.')
            && self
                .peek_byte_at(1)
                .is_some_and(|byte| byte.is_ascii_digit())
        {
            self.position += 1;
            self.consume_decimal_digits();
            self.consume_exponent();
            self.push(TokenKind::FloatLiteral, start);
            return;
        }

        self.report_bad_numeric_suffix(start);
        self.push(TokenKind::IntegerLiteral, start);
    }

    fn consume_decimal_digits(&mut self) {
        while let Some(byte) = self.peek_byte() {
            if byte.is_ascii_digit() || byte == b'_' {
                self.position += 1;
            } else {
                break;
            }
        }
    }

    fn consume_exponent(&mut self) {
        let Some(marker) = self.peek_byte() else {
            return;
        };
        if !matches!(marker, b'e' | b'E') {
            return;
        }
        // Only consume the exponent when it actually has digits, mirroring the
        // spec's examples (`1.0e` stops before the `e`).
        let mut offset = 1;
        if matches!(self.peek_byte_at(offset), Some(b'+' | b'-')) {
            offset += 1;
        }
        if !self
            .peek_byte_at(offset)
            .is_some_and(|byte| byte.is_ascii_digit())
        {
            return;
        }
        self.position += offset;
        self.consume_decimal_digits();
    }

    /// Reports an invalid character immediately following a numeric literal.
    fn report_bad_numeric_suffix(&mut self, start: usize) {
        if let Some(byte) = self.peek_byte() {
            if Self::is_identifier_start(byte) {
                self.error(start, "unexpected character in numeric literal");
            }
        }
    }

    // -- characters ---------------------------------------------------------

    fn lex_character(&mut self) {
        let start = self.position;
        self.position += 1; // opening quote

        match self.peek_byte() {
            None => {
                self.error(start, "unterminated character literal");
                self.push(TokenKind::CharacterLiteral, start);
                return;
            }
            Some(byte) if Self::is_line_terminator(byte) => {
                self.error(
                    start,
                    "character literal must not contain a line terminator",
                );
                self.push(TokenKind::CharacterLiteral, start);
                return;
            }
            Some(b'\\') => self.consume_escape(start),
            Some(b'\'') => {
                self.error(
                    start,
                    "character literal must contain exactly one character",
                );
                self.position += 1;
                self.maybe_consume_encoding_tag();
                self.push(TokenKind::CharacterLiteral, start);
                return;
            }
            Some(_) => {
                self.bump_char();
            }
        }

        if self.peek_byte() == Some(b'\'') {
            self.position += 1;
        } else if self.peek_byte().is_some_and(Self::is_line_terminator) {
            self.error(
                start,
                "character literal must not contain a line terminator",
            );
            return;
        } else {
            self.error(
                start,
                "character literal must contain exactly one character",
            );
            // Recover: skip to the closing quote or the end of the line.
            while let Some(byte) = self.peek_byte() {
                if byte == b'\'' {
                    self.position += 1;
                    break;
                }
                if Self::is_line_terminator(byte) {
                    break;
                }
                self.bump_char();
            }
        }

        self.maybe_consume_encoding_tag();
        self.push(TokenKind::CharacterLiteral, start);
    }

    fn maybe_consume_encoding_tag(&mut self) {
        if self.peek_byte() != Some(b':') {
            return;
        }
        let tag_start = self.position + 1;
        let mut end = tag_start;
        while let Some(byte) = self.source.as_bytes().get(end) {
            if Self::is_identifier_continue(*byte) {
                end += 1;
            } else {
                break;
            }
        }
        let tag = &self.source[tag_start..end];
        if matches!(tag, "ascii" | "utf8" | "utf16" | "utf32" | "ucs2" | "ucs4") {
            self.position = end;
        }
    }

    // -- strings ------------------------------------------------------------

    fn lex_normal_string(&mut self) {
        let start = self.position;
        self.position += 1; // opening quote

        loop {
            match self.peek_byte() {
                None => {
                    self.error(start, "unterminated string literal");
                    break;
                }
                Some(b'"') => {
                    self.position += 1;
                    break;
                }
                Some(b'\\') => self.consume_escape(start),
                Some(byte) if Self::is_line_terminator(byte) => {
                    // A newline inside a normal string is not string content;
                    // the newline and any following whitespace are skipped.
                    while let Some(byte) = self.peek_byte() {
                        if Self::is_whitespace(byte) {
                            self.position += 1;
                        } else {
                            break;
                        }
                    }
                }
                Some(_) => {
                    self.bump_char();
                }
            }
        }

        self.maybe_consume_encoding_tag();
        self.push(TokenKind::StringLiteral, start);
    }

    fn lex_verbatim_string(&mut self, start: usize) {
        // The opening `@"` has already been consumed.
        loop {
            match self.peek_byte() {
                None => {
                    self.error(start, "unterminated verbatim string literal");
                    break;
                }
                Some(b'"') => {
                    if self.peek_byte_at(1) == Some(b'"') {
                        self.position += 2;
                    } else {
                        self.position += 1;
                        break;
                    }
                }
                Some(_) => {
                    self.bump_char();
                }
            }
        }
        self.maybe_consume_encoding_tag();
        self.push(TokenKind::VerbatimStringLiteral, start);
    }

    // -- escapes ------------------------------------------------------------

    fn consume_escape(&mut self, literal_start: usize) {
        self.position += 1; // backslash
        let Some(byte) = self.peek_byte() else {
            self.error(literal_start, "unterminated escape sequence");
            return;
        };
        match byte {
            b'a' | b'b' | b'f' | b'n' | b'r' | b't' | b'v' | b'\\' | b'"' | b'\'' | b'0' => {
                self.position += 1;
            }
            b'x' => {
                self.position += 1;
                self.require_hex_digits(literal_start, 2);
            }
            b'u' => {
                self.position += 1;
                self.require_hex_digits(literal_start, 4);
            }
            b'U' => {
                self.position += 1;
                self.require_hex_digits(literal_start, 8);
            }
            _ => {
                self.error(literal_start, "invalid escape sequence");
                self.position += 1;
            }
        }
    }

    fn require_hex_digits(&mut self, literal_start: usize, count: usize) {
        for _ in 0..count {
            match self.peek_byte() {
                Some(byte) if byte.is_ascii_hexdigit() => {
                    self.position += 1;
                }
                _ => {
                    self.error(
                        literal_start,
                        format!("escape sequence requires {count} hexadecimal digits"),
                    );
                    return;
                }
            }
        }
    }

    // -- symbols ------------------------------------------------------------

    fn lex_symbol(&mut self, start: usize) {
        use TokenKind::*;

        // Three-character tokens first.
        let three = match self.source.get(self.position..self.position + 3) {
            Some("..!") => Some(DotDotBang),
            Some("<<=") => Some(ShiftLeftEquals),
            Some(">>=") => Some(ShiftRightEquals),
            _ => None,
        };
        if let Some(kind) = three {
            self.position += 3;
            self.push(kind, start);
            return;
        }

        let two = match self.source.get(self.position..self.position + 2) {
            Some("..") => Some(DotDot),
            Some("->") => Some(Arrow),
            Some("=>") => Some(FatArrow),
            Some("??") => Some(QuestionQuestion),
            Some("==") => Some(EqualsEquals),
            Some("!=") => Some(NotEquals),
            Some("<=") => Some(LessEquals),
            Some(">=") => Some(GreaterEquals),
            Some("&&") => Some(AmpersandAmpersand),
            Some("||") => Some(PipePipe),
            Some("<<") => Some(ShiftLeft),
            Some(">>") => Some(ShiftRight),
            Some("+=") => Some(PlusEquals),
            Some("-=") => Some(MinusEquals),
            Some("*=") => Some(StarEquals),
            Some("/=") => Some(SlashEquals),
            Some("%=") => Some(PercentEquals),
            Some("&=") => Some(AmpersandEquals),
            Some("|=") => Some(PipeEquals),
            Some("^=") => Some(CaretEquals),
            _ => None,
        };
        if let Some(kind) = two {
            self.position += 2;
            self.push(kind, start);
            return;
        }

        let byte = self.peek_byte().expect("caller checked for end of input");
        let single = match byte {
            b'@' => Some(At),
            b'#' => Some(Hash),
            b'(' => Some(LeftParen),
            b')' => Some(RightParen),
            b'{' => Some(LeftBrace),
            b'}' => Some(RightBrace),
            b'[' => Some(LeftBracket),
            b']' => Some(RightBracket),
            b',' => Some(Comma),
            b';' => Some(Semicolon),
            b':' => Some(Colon),
            b'.' => Some(Dot),
            b'?' => Some(Question),
            b'+' => Some(Plus),
            b'-' => Some(Minus),
            b'*' => Some(Star),
            b'/' => Some(Slash),
            b'%' => Some(Percent),
            b'&' => Some(Ampersand),
            b'|' => Some(Pipe),
            b'^' => Some(Caret),
            b'!' => Some(Bang),
            b'=' => Some(Equals),
            b'<' => Some(Less),
            b'>' => Some(Greater),
            _ => None,
        };
        if let Some(kind) = single {
            self.position += 1;
            self.push(kind, start);
            return;
        }

        // Unknown character: report it and skip one scalar value.
        let character = self.source[self.position..].chars().next();
        self.position += character.map_or(1, char::len_utf8);
        self.error(start, "unexpected character");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::FileId;

    fn kinds(source: &str) -> Vec<TokenKind> {
        let result = tokenize(FileId::from_index(0), source);
        assert!(
            result.errors.is_empty(),
            "unexpected lex errors for {source:?}: {:?}",
            result.errors
        );
        result.tokens.into_iter().map(|token| token.kind).collect()
    }

    fn errors(source: &str) -> Vec<String> {
        tokenize(FileId::from_index(0), source)
            .errors
            .into_iter()
            .map(|error| error.message)
            .collect()
    }

    #[test]
    fn keywords_and_identifiers() {
        use TokenKind::*;
        assert_eq!(
            kinds("class structure interface enumeration variant union module local fixed accessor heap operator linkable"),
            vec![
                Class, Structure, Interface, Enumeration, Variant, Union, Module, Local, Fixed,
                Accessor, Heap, Operator, Linkable, Eof
            ]
        );
        assert_eq!(
            kinds("array function pointer integer cardinal float boolean"),
            vec![Array, Function, Pointer, Integer, Cardinal, Float, Boolean, Eof]
        );
        assert_eq!(
            kinds("public private protected scoped readonly implicitable verbatim manual abstract virtual override sealed aligned static"),
            vec![
                Public, Private, Protected, Scoped, Readonly, Implicitable, Verbatim, Manual,
                Abstract, Virtual, Override, Sealed, Aligned, Static, Eof
            ]
        );
        assert_eq!(
            kinds("if else while do for in match case break continue when return throw trap catch finally dispose"),
            vec![
                If, Else, While, Do, For, In, Match, Case, Break, Continue, When, Return, Throw,
                Trap, Catch, Finally, Dispose, Eof
            ]
        );
        assert_eq!(
            kinds("this owner base ascii utf8 utf16 utf32 ucs2 ucs4 wildcard true false illegal infinite uninitialized use is extension Cast basic"),
            vec![
                This, Owner, Base, Ascii, Utf8, Utf16, Utf32, Ucs2, Ucs4, Wildcard, True, False,
                Illegal, Infinite, Uninitialized, Use, Is, Extension, Cast, Basic, Eof
            ]
        );
        assert_eq!(
            kinds("_x foo9 name_ trailing_"),
            vec![Identifier, Identifier, Identifier, Identifier, Eof]
        );
    }

    #[test]
    fn operators_use_longest_match() {
        use TokenKind::*;
        assert_eq!(
            kinds(".. ..! ... -> => ?? ? . <<= >>= << >> <= >= == != && || &&= += -= *= /= %= &= |= ^= @ #"),
            vec![
                DotDot, DotDotBang, DotDot, Dot, Arrow, FatArrow, QuestionQuestion, Question, Dot,
                ShiftLeftEquals, ShiftRightEquals, ShiftLeft, ShiftRight, LessEquals, GreaterEquals,
                EqualsEquals, NotEquals, AmpersandAmpersand, PipePipe, AmpersandAmpersand, Equals,
                PlusEquals, MinusEquals, StarEquals, SlashEquals, PercentEquals, AmpersandEquals,
                PipeEquals, CaretEquals, At, Hash, Eof
            ]
        );
    }

    #[test]
    fn integer_literals() {
        use TokenKind::*;
        assert_eq!(
            kinds("0 42 1_000_000 0xFF 0xdead_beef 0b1010 0b1_0"),
            vec![
                IntegerLiteral,
                IntegerLiteral,
                IntegerLiteral,
                IntegerLiteral,
                IntegerLiteral,
                IntegerLiteral,
                IntegerLiteral,
                Eof
            ]
        );
        assert!(!errors("0x").is_empty());
        assert!(!errors("0b").is_empty());
        assert!(!errors("0xFG").is_empty());
        assert!(!errors("0b2010").is_empty());
    }

    #[test]
    fn float_literals_and_range_disambiguation() {
        use TokenKind::*;
        assert_eq!(
            kinds("3.14 1.5e10 2.5e-3 1.0E+5"),
            vec![FloatLiteral, FloatLiteral, FloatLiteral, FloatLiteral, Eof]
        );
        // `1..2` is a range, not a float.
        assert_eq!(
            kinds("1..2"),
            vec![IntegerLiteral, DotDot, IntegerLiteral, Eof]
        );
        assert_eq!(
            kinds("1..!2"),
            vec![IntegerLiteral, DotDotBang, IntegerLiteral, Eof]
        );
        // `1.` is an integer followed by a dot, so the parser reports it.
        assert_eq!(kinds("1."), vec![IntegerLiteral, Dot, Eof]);
        // `1.0e` stops before the `e`.
        assert_eq!(kinds("1.0e"), vec![FloatLiteral, Identifier, Eof]);
    }

    #[test]
    fn character_literals() {
        use TokenKind::*;
        assert_eq!(
            kinds(r"'a' '\n' '\x41' '\u4e2d' '\U0001F600' '\0' '\'' '\\'"),
            vec![
                CharacterLiteral,
                CharacterLiteral,
                CharacterLiteral,
                CharacterLiteral,
                CharacterLiteral,
                CharacterLiteral,
                CharacterLiteral,
                CharacterLiteral,
                Eof
            ]
        );
        assert_eq!(kinds("'中'"), vec![CharacterLiteral, Eof]);
        assert_eq!(
            kinds("'a':ascii"),
            vec![CharacterLiteral, Eof],
            "encoding tag belongs to the literal"
        );
        assert!(!errors("''").is_empty(), "empty character literal");
        assert!(!errors("'ab'").is_empty(), "multi-character literal");
        assert!(!errors("'a").is_empty(), "unterminated character literal");
        assert!(!errors(r"'\q'").is_empty(), "invalid escape");
    }

    #[test]
    fn string_literals() {
        use TokenKind::*;
        assert_eq!(kinds(r#""Hello""#), vec![StringLiteral, Eof]);
        assert_eq!(kinds(r#""foo\nbar""#), vec![StringLiteral, Eof]);
        assert_eq!(kinds(r#""x":ascii"#), vec![StringLiteral, Eof]);
        assert_eq!(kinds(r#""多字节":utf8"#), vec![StringLiteral, Eof]);

        // A line terminator and the whitespace after it are not content.
        let result = tokenize(FileId::from_index(0), "\"第一行，\n    还是第一行\"");
        assert!(result.errors.is_empty());
        assert_eq!(result.tokens[0].kind, StringLiteral);

        // Verbatim strings keep newlines and accept doubled quotes.
        assert_eq!(
            kinds("@\"C:\\Users\\test\""),
            vec![VerbatimStringLiteral, Eof]
        );
        assert_eq!(
            kinds("@\"He said \"\"Hello\"\"\""),
            vec![VerbatimStringLiteral, Eof]
        );
        assert!(!errors("\"unterminated").is_empty());
        assert!(!errors("@\"unterminated").is_empty());
    }

    #[test]
    fn comments_are_ignored() {
        use TokenKind::*;
        assert_eq!(
            kinds("local // line comment\n fixed /* block */ heap"),
            vec![Local, Fixed, Heap, Eof]
        );
        // `//` inside a string is string content.
        assert_eq!(kinds(r#""http://example.com""#), vec![StringLiteral, Eof]);
        // Block comments do not nest.
        assert_eq!(kinds("/* outer /* inner */ local"), vec![Local, Eof]);
        assert!(!errors("/* unterminated").is_empty());
    }

    #[test]
    fn at_sign_and_keyword_escape() {
        use TokenKind::*;
        assert_eq!(kinds("@class"), vec![At, Class, Eof]);
        assert_eq!(kinds("@integer"), vec![At, Integer, Eof]);
        // `@` followed by a string starts a verbatim string, not an `At` token.
        assert_eq!(kinds(r#"@"x""#), vec![VerbatimStringLiteral, Eof]);
    }

    #[test]
    fn hash_tokens_for_compile_time_syntax() {
        use TokenKind::*;
        assert_eq!(kinds("#if"), vec![Hash, If, Eof]);
        assert_eq!(
            kinds("#(1 + 2)"),
            vec![
                Hash,
                LeftParen,
                IntegerLiteral,
                Plus,
                IntegerLiteral,
                RightParen,
                Eof
            ]
        );
        assert_eq!(kinds("#{ }"), vec![Hash, LeftBrace, RightBrace, Eof]);
    }

    #[test]
    fn spans_and_token_text() {
        let source = "local answer = 42;";
        let result = tokenize(FileId::from_index(3), source);
        assert_eq!(result.tokens[1].text(source), "answer");
        assert_eq!(result.tokens[1].span.start, 6);
        assert_eq!(result.tokens[1].span.end, 12);
        assert_eq!(result.tokens[1].span.file, FileId::from_index(3));
    }

    #[test]
    fn always_ends_with_eof() {
        let result = tokenize(FileId::from_index(0), "");
        assert_eq!(result.tokens.len(), 1);
        assert_eq!(result.tokens[0].kind, TokenKind::Eof);
    }
}
