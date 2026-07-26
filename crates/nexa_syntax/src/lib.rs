#![forbid(unsafe_code)]
//! Token and syntax primitives for the Nexa language.

use nexa_span::TextRange;

/// Lossless syntax kinds used by Nexa tokens and concrete syntax nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum SyntaxKind {
    /// An identifier.
    Ident = 0,
    /// An integer literal.
    Int = 1,
    /// Whitespace trivia.
    Whitespace = 2,
    /// A line comment.
    LineComment = 3,
    /// `(`
    LParen = 4,
    /// `)`
    RParen = 5,
    /// `{`
    LBrace = 6,
    /// `}`
    RBrace = 7,
    /// `,`
    Comma = 8,
    /// `;`
    Semicolon = 9,
    /// `=`
    Eq = 10,
    /// An unrecognized token.
    Unknown = 11,
    /// The retired bootstrap `let` keyword.
    LetKw = 12,
    /// The `function` keyword.
    FunctionKw = 13,
    /// The `const` keyword.
    ConstKw = 14,
    /// The `if` keyword.
    IfKw = 15,
    /// The `else` keyword.
    ElseKw = 16,
    /// The `return` keyword.
    ReturnKw = 17,
    /// The `true` keyword.
    TrueKw = 18,
    /// The `false` keyword.
    FalseKw = 19,
    /// The `Int` type keyword.
    IntKw = 20,
    /// The `Bool` type keyword.
    BoolKw = 21,
    /// The `Unit` type keyword.
    UnitKw = 22,
    /// `:`
    Colon = 23,
    /// `+`
    Plus = 24,
    /// `-`
    Minus = 25,
    /// `*`
    Star = 26,
    /// `/`
    Slash = 27,
    /// `!`
    Bang = 28,
    /// `===`
    EqEqEq = 29,
    /// `<`
    Lt = 30,
    /// `<=`
    LtEq = 31,
    /// `>`
    Gt = 32,
    /// `>=`
    GtEq = 33,
    /// The root of a parsed source file.
    SourceFile = 34,
    /// A top-level function declaration.
    FunctionDeclaration = 35,
    /// A comma-separated parameter list.
    ParameterList = 36,
    /// One named and typed function parameter.
    Parameter = 37,
    /// A type reference.
    Type = 38,
    /// A braced statement block.
    Block = 39,
    /// An immutable local declaration.
    ConstDeclaration = 40,
    /// A conditional statement.
    IfStatement = 41,
    /// The optional `else` part of a conditional.
    ElseClause = 42,
    /// An explicit function return.
    ReturnStatement = 43,
    /// An expression terminated by a semicolon.
    ExpressionStatement = 44,
    /// A binary operator expression.
    BinaryExpression = 45,
    /// A prefix unary operator expression.
    UnaryExpression = 46,
    /// A function call expression.
    CallExpression = 47,
    /// A reference to a named value.
    NameReference = 48,
    /// An integer literal expression.
    IntLiteral = 49,
    /// A boolean literal expression.
    BoolLiteral = 50,
    /// A parenthesized expression.
    ParenthesizedExpression = 51,
    /// A comma-separated call argument list.
    ArgumentList = 52,
    /// Tokens skipped during parser recovery.
    Error = 53,
}

impl SyntaxKind {
    /// Returns true for whitespace and comments.
    #[must_use]
    pub const fn is_trivia(self) -> bool {
        matches!(self, Self::Whitespace | Self::LineComment)
    }

    fn from_raw(raw: u16) -> Self {
        match raw {
            0 => Self::Ident,
            1 => Self::Int,
            2 => Self::Whitespace,
            3 => Self::LineComment,
            4 => Self::LParen,
            5 => Self::RParen,
            6 => Self::LBrace,
            7 => Self::RBrace,
            8 => Self::Comma,
            9 => Self::Semicolon,
            10 => Self::Eq,
            11 => Self::Unknown,
            12 => Self::LetKw,
            13 => Self::FunctionKw,
            14 => Self::ConstKw,
            15 => Self::IfKw,
            16 => Self::ElseKw,
            17 => Self::ReturnKw,
            18 => Self::TrueKw,
            19 => Self::FalseKw,
            20 => Self::IntKw,
            21 => Self::BoolKw,
            22 => Self::UnitKw,
            23 => Self::Colon,
            24 => Self::Plus,
            25 => Self::Minus,
            26 => Self::Star,
            27 => Self::Slash,
            28 => Self::Bang,
            29 => Self::EqEqEq,
            30 => Self::Lt,
            31 => Self::LtEq,
            32 => Self::Gt,
            33 => Self::GtEq,
            34 => Self::SourceFile,
            35 => Self::FunctionDeclaration,
            36 => Self::ParameterList,
            37 => Self::Parameter,
            38 => Self::Type,
            39 => Self::Block,
            40 => Self::ConstDeclaration,
            41 => Self::IfStatement,
            42 => Self::ElseClause,
            43 => Self::ReturnStatement,
            44 => Self::ExpressionStatement,
            45 => Self::BinaryExpression,
            46 => Self::UnaryExpression,
            47 => Self::CallExpression,
            48 => Self::NameReference,
            49 => Self::IntLiteral,
            50 => Self::BoolLiteral,
            51 => Self::ParenthesizedExpression,
            52 => Self::ArgumentList,
            53 => Self::Error,
            _ => unreachable!("invalid Nexa syntax kind: {raw}"),
        }
    }
}

impl From<SyntaxKind> for rowan::SyntaxKind {
    fn from(kind: SyntaxKind) -> Self {
        Self(kind as u16)
    }
}

/// Rowan language marker for Nexa concrete syntax trees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NexaLanguage {}

impl rowan::Language for NexaLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        SyntaxKind::from_raw(raw.0)
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into()
    }
}

/// A typed Nexa concrete syntax node.
pub type SyntaxNode = rowan::SyntaxNode<NexaLanguage>;

/// A typed Nexa concrete syntax token.
pub type SyntaxToken = rowan::SyntaxToken<NexaLanguage>;

/// A node or token in a Nexa concrete syntax tree.
pub type SyntaxElement = rowan::SyntaxElement<NexaLanguage>;

/// A lossless token with a source range and original text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    kind: SyntaxKind,
    range: TextRange,
    text: String,
}

impl Token {
    /// Creates a token.
    #[must_use]
    pub fn new(kind: SyntaxKind, range: TextRange, text: impl Into<String>) -> Self {
        Self {
            kind,
            range,
            text: text.into(),
        }
    }

    /// Returns the token kind.
    #[must_use]
    pub const fn kind(&self) -> SyntaxKind {
        self.kind
    }

    /// Returns the token source range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }

    /// Returns the token text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// Tokenizes source text into a lossless Nexa token stream.
#[must_use]
pub fn tokenize(source: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = source.char_indices().peekable();

    while let Some((start, ch)) = chars.next() {
        let kind = match ch {
            character if character.is_ascii_whitespace() => {
                consume_while(&mut chars, |next| next.is_ascii_whitespace());
                SyntaxKind::Whitespace
            }
            character if character.is_ascii_alphabetic() || character == '_' => {
                consume_while(&mut chars, |next| {
                    next.is_ascii_alphanumeric() || next == '_'
                });
                SyntaxKind::Ident
            }
            character if character.is_ascii_digit() => {
                consume_while(&mut chars, |next| next.is_ascii_digit());
                SyntaxKind::Int
            }
            '/' if matches!(chars.peek(), Some((_, '/'))) => {
                let _ = chars.next();
                consume_while(&mut chars, |next| next != '\n');
                SyntaxKind::LineComment
            }
            '=' => consume_equals(&mut chars),
            '!' => consume_bang(&mut chars),
            '<' => consume_optional_equals(&mut chars, SyntaxKind::Lt, SyntaxKind::LtEq),
            '>' => consume_optional_equals(&mut chars, SyntaxKind::Gt, SyntaxKind::GtEq),
            '(' => SyntaxKind::LParen,
            ')' => SyntaxKind::RParen,
            '{' => SyntaxKind::LBrace,
            '}' => SyntaxKind::RBrace,
            ',' => SyntaxKind::Comma,
            ';' => SyntaxKind::Semicolon,
            ':' => SyntaxKind::Colon,
            '+' => SyntaxKind::Plus,
            '-' => SyntaxKind::Minus,
            '*' => SyntaxKind::Star,
            '/' => SyntaxKind::Slash,
            _ => SyntaxKind::Unknown,
        };

        let end = chars.peek().map_or(source.len(), |(index, _)| *index);
        let text = &source[start..end];
        let kind = keyword_kind(kind, text);
        tokens.push(Token::new(kind, TextRange::new(start, end), text));
    }

    tokens
}

fn keyword_kind(kind: SyntaxKind, text: &str) -> SyntaxKind {
    if kind != SyntaxKind::Ident {
        return kind;
    }

    match text {
        "let" => SyntaxKind::LetKw,
        "function" => SyntaxKind::FunctionKw,
        "const" => SyntaxKind::ConstKw,
        "if" => SyntaxKind::IfKw,
        "else" => SyntaxKind::ElseKw,
        "return" => SyntaxKind::ReturnKw,
        "true" => SyntaxKind::TrueKw,
        "false" => SyntaxKind::FalseKw,
        "Int" => SyntaxKind::IntKw,
        "Bool" => SyntaxKind::BoolKw,
        "Unit" => SyntaxKind::UnitKw,
        _ => SyntaxKind::Ident,
    }
}

fn consume_equals(chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>) -> SyntaxKind {
    if !consume_if(chars, '=') {
        return SyntaxKind::Eq;
    }

    if consume_if(chars, '=') {
        SyntaxKind::EqEqEq
    } else {
        SyntaxKind::Unknown
    }
}

fn consume_bang(chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>) -> SyntaxKind {
    if consume_if(chars, '=') {
        let _ = consume_if(chars, '=');
        SyntaxKind::Unknown
    } else {
        SyntaxKind::Bang
    }
}

fn consume_optional_equals(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    single: SyntaxKind,
    combined: SyntaxKind,
) -> SyntaxKind {
    if consume_if(chars, '=') {
        combined
    } else {
        single
    }
}

fn consume_if(chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>, expected: char) -> bool {
    if matches!(chars.peek(), Some((_, character)) if *character == expected) {
        let _ = chars.next();
        true
    } else {
        false
    }
}

fn consume_while(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    predicate: impl Fn(char) -> bool,
) {
    while matches!(chars.peek(), Some((_, character)) if predicate(*character)) {
        let _ = chars.next();
    }
}

#[cfg(test)]
mod tests {
    use super::{tokenize, SyntaxKind};

    #[test]
    fn tokenizes_language_core_keywords_and_operators() {
        let kinds: Vec<_> = tokenize("function main(): Unit { const answer: Int = 40 + 2; }")
            .into_iter()
            .map(|token| token.kind())
            .collect();

        assert_eq!(
            kinds,
            [
                SyntaxKind::FunctionKw,
                SyntaxKind::Whitespace,
                SyntaxKind::Ident,
                SyntaxKind::LParen,
                SyntaxKind::RParen,
                SyntaxKind::Colon,
                SyntaxKind::Whitespace,
                SyntaxKind::UnitKw,
                SyntaxKind::Whitespace,
                SyntaxKind::LBrace,
                SyntaxKind::Whitespace,
                SyntaxKind::ConstKw,
                SyntaxKind::Whitespace,
                SyntaxKind::Ident,
                SyntaxKind::Colon,
                SyntaxKind::Whitespace,
                SyntaxKind::IntKw,
                SyntaxKind::Whitespace,
                SyntaxKind::Eq,
                SyntaxKind::Whitespace,
                SyntaxKind::Int,
                SyntaxKind::Whitespace,
                SyntaxKind::Plus,
                SyntaxKind::Whitespace,
                SyntaxKind::Int,
                SyntaxKind::Semicolon,
                SyntaxKind::Whitespace,
                SyntaxKind::RBrace,
            ]
        );
    }

    #[test]
    fn tokenization_preserves_source_text() {
        let source = "// binding\r\nfunction main(): Unit { const \u{00e9} = 42; }";
        let tokenized = tokenize(source)
            .iter()
            .map(|token| token.text())
            .collect::<String>();

        assert_eq!(tokenized, source);
    }

    #[test]
    fn invalid_partial_equality_operator_is_one_unknown_token() {
        let tokens = tokenize("==");

        assert_eq!(
            (tokens.len(), tokens[0].kind(), tokens[0].text()),
            (1, SyntaxKind::Unknown, "==")
        );
    }

    #[test]
    fn syntax_kinds_round_trip_through_rowan() {
        let kinds = [
            SyntaxKind::Ident,
            SyntaxKind::Int,
            SyntaxKind::Whitespace,
            SyntaxKind::LineComment,
            SyntaxKind::LParen,
            SyntaxKind::RParen,
            SyntaxKind::LBrace,
            SyntaxKind::RBrace,
            SyntaxKind::Comma,
            SyntaxKind::Semicolon,
            SyntaxKind::Eq,
            SyntaxKind::Unknown,
            SyntaxKind::LetKw,
            SyntaxKind::FunctionKw,
            SyntaxKind::ConstKw,
            SyntaxKind::IfKw,
            SyntaxKind::ElseKw,
            SyntaxKind::ReturnKw,
            SyntaxKind::TrueKw,
            SyntaxKind::FalseKw,
            SyntaxKind::IntKw,
            SyntaxKind::BoolKw,
            SyntaxKind::UnitKw,
            SyntaxKind::Colon,
            SyntaxKind::Plus,
            SyntaxKind::Minus,
            SyntaxKind::Star,
            SyntaxKind::Slash,
            SyntaxKind::Bang,
            SyntaxKind::EqEqEq,
            SyntaxKind::Lt,
            SyntaxKind::LtEq,
            SyntaxKind::Gt,
            SyntaxKind::GtEq,
            SyntaxKind::SourceFile,
            SyntaxKind::FunctionDeclaration,
            SyntaxKind::ParameterList,
            SyntaxKind::Parameter,
            SyntaxKind::Type,
            SyntaxKind::Block,
            SyntaxKind::ConstDeclaration,
            SyntaxKind::IfStatement,
            SyntaxKind::ElseClause,
            SyntaxKind::ReturnStatement,
            SyntaxKind::ExpressionStatement,
            SyntaxKind::BinaryExpression,
            SyntaxKind::UnaryExpression,
            SyntaxKind::CallExpression,
            SyntaxKind::NameReference,
            SyntaxKind::IntLiteral,
            SyntaxKind::BoolLiteral,
            SyntaxKind::ParenthesizedExpression,
            SyntaxKind::ArgumentList,
            SyntaxKind::Error,
        ];

        for kind in kinds {
            let raw: rowan::SyntaxKind = kind.into();

            assert_eq!(SyntaxKind::from_raw(raw.0), kind);
        }
    }
}
