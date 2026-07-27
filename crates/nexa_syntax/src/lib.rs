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
    /// The `function` keyword.
    FunctionKw = 12,
    /// The `const` keyword.
    ConstKw = 13,
    /// The `let` keyword.
    LetKw = 14,
    /// The `if` keyword.
    IfKw = 15,
    /// The `else` keyword.
    ElseKw = 16,
    /// The `while` keyword.
    WhileKw = 17,
    /// The `break` keyword.
    BreakKw = 18,
    /// The `continue` keyword.
    ContinueKw = 19,
    /// The `return` keyword.
    ReturnKw = 20,
    /// The `true` keyword.
    TrueKw = 21,
    /// The `false` keyword.
    FalseKw = 22,
    /// The `Int` type keyword.
    IntKw = 23,
    /// The `Bool` type keyword.
    BoolKw = 24,
    /// The `Unit` type keyword.
    UnitKw = 25,
    /// `:`
    Colon = 26,
    /// `+`
    Plus = 27,
    /// `-`
    Minus = 28,
    /// `*`
    Star = 29,
    /// `/`
    Slash = 30,
    /// `!`
    Bang = 31,
    /// `&&`
    AmpAmp = 32,
    /// `||`
    PipePipe = 33,
    /// `===`
    EqEqEq = 34,
    /// `<`
    Lt = 35,
    /// `<=`
    LtEq = 36,
    /// `>`
    Gt = 37,
    /// `>=`
    GtEq = 38,
    /// The root of a parsed source file.
    SourceFile = 39,
    /// A top-level function declaration.
    FunctionDeclaration = 40,
    /// A comma-separated parameter list.
    ParameterList = 41,
    /// One named and typed function parameter.
    Parameter = 42,
    /// A type reference.
    Type = 43,
    /// A braced statement block.
    Block = 44,
    /// An immutable local declaration.
    ConstDeclaration = 45,
    /// A mutable local declaration.
    LetDeclaration = 46,
    /// An assignment to a named local.
    AssignmentStatement = 47,
    /// A conditional statement.
    IfStatement = 48,
    /// The optional `else` part of a conditional.
    ElseClause = 49,
    /// A conditional loop.
    WhileStatement = 50,
    /// An exit from the nearest enclosing loop.
    BreakStatement = 51,
    /// A jump to the next iteration of the nearest enclosing loop.
    ContinueStatement = 52,
    /// An explicit function return.
    ReturnStatement = 53,
    /// An expression terminated by a semicolon.
    ExpressionStatement = 54,
    /// A binary operator expression.
    BinaryExpression = 55,
    /// A prefix unary operator expression.
    UnaryExpression = 56,
    /// A function call expression.
    CallExpression = 57,
    /// A reference to a named value.
    NameReference = 58,
    /// An integer literal expression.
    IntLiteral = 59,
    /// A boolean literal expression.
    BoolLiteral = 60,
    /// A parenthesized expression.
    ParenthesizedExpression = 61,
    /// A comma-separated call argument list.
    ArgumentList = 62,
    /// Tokens skipped during parser recovery.
    Error = 63,
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
            12 => Self::FunctionKw,
            13 => Self::ConstKw,
            14 => Self::LetKw,
            15 => Self::IfKw,
            16 => Self::ElseKw,
            17 => Self::WhileKw,
            18 => Self::BreakKw,
            19 => Self::ContinueKw,
            20 => Self::ReturnKw,
            21 => Self::TrueKw,
            22 => Self::FalseKw,
            23 => Self::IntKw,
            24 => Self::BoolKw,
            25 => Self::UnitKw,
            26 => Self::Colon,
            27 => Self::Plus,
            28 => Self::Minus,
            29 => Self::Star,
            30 => Self::Slash,
            31 => Self::Bang,
            32 => Self::AmpAmp,
            33 => Self::PipePipe,
            34 => Self::EqEqEq,
            35 => Self::Lt,
            36 => Self::LtEq,
            37 => Self::Gt,
            38 => Self::GtEq,
            39 => Self::SourceFile,
            40 => Self::FunctionDeclaration,
            41 => Self::ParameterList,
            42 => Self::Parameter,
            43 => Self::Type,
            44 => Self::Block,
            45 => Self::ConstDeclaration,
            46 => Self::LetDeclaration,
            47 => Self::AssignmentStatement,
            48 => Self::IfStatement,
            49 => Self::ElseClause,
            50 => Self::WhileStatement,
            51 => Self::BreakStatement,
            52 => Self::ContinueStatement,
            53 => Self::ReturnStatement,
            54 => Self::ExpressionStatement,
            55 => Self::BinaryExpression,
            56 => Self::UnaryExpression,
            57 => Self::CallExpression,
            58 => Self::NameReference,
            59 => Self::IntLiteral,
            60 => Self::BoolLiteral,
            61 => Self::ParenthesizedExpression,
            62 => Self::ArgumentList,
            63 => Self::Error,
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
            '&' => consume_required_pair(&mut chars, '&', SyntaxKind::AmpAmp),
            '|' => consume_required_pair(&mut chars, '|', SyntaxKind::PipePipe),
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
        "function" => SyntaxKind::FunctionKw,
        "const" => SyntaxKind::ConstKw,
        "let" => SyntaxKind::LetKw,
        "if" => SyntaxKind::IfKw,
        "else" => SyntaxKind::ElseKw,
        "while" => SyntaxKind::WhileKw,
        "break" => SyntaxKind::BreakKw,
        "continue" => SyntaxKind::ContinueKw,
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

fn consume_required_pair(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    expected: char,
    combined: SyntaxKind,
) -> SyntaxKind {
    if consume_if(chars, expected) {
        combined
    } else {
        SyntaxKind::Unknown
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
    fn tokenizes_stateful_control_flow_keywords_and_logical_operators() {
        let tokens = tokenize("let while break continue && ||");
        let kinds: Vec<_> = tokens.iter().map(|token| token.kind()).collect();

        assert_eq!(
            kinds,
            [
                SyntaxKind::LetKw,
                SyntaxKind::Whitespace,
                SyntaxKind::WhileKw,
                SyntaxKind::Whitespace,
                SyntaxKind::BreakKw,
                SyntaxKind::Whitespace,
                SyntaxKind::ContinueKw,
                SyntaxKind::Whitespace,
                SyntaxKind::AmpAmp,
                SyntaxKind::Whitespace,
                SyntaxKind::PipePipe,
            ]
        );
    }

    #[test]
    fn single_logical_operator_characters_are_unknown_tokens() {
        let tokens = tokenize("& |");

        assert_eq!(
            tokens
                .iter()
                .map(|token| (token.kind(), token.text()))
                .collect::<Vec<_>>(),
            [
                (SyntaxKind::Unknown, "&"),
                (SyntaxKind::Whitespace, " "),
                (SyntaxKind::Unknown, "|"),
            ]
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
            SyntaxKind::FunctionKw,
            SyntaxKind::ConstKw,
            SyntaxKind::LetKw,
            SyntaxKind::IfKw,
            SyntaxKind::ElseKw,
            SyntaxKind::WhileKw,
            SyntaxKind::BreakKw,
            SyntaxKind::ContinueKw,
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
            SyntaxKind::AmpAmp,
            SyntaxKind::PipePipe,
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
            SyntaxKind::LetDeclaration,
            SyntaxKind::AssignmentStatement,
            SyntaxKind::IfStatement,
            SyntaxKind::ElseClause,
            SyntaxKind::WhileStatement,
            SyntaxKind::BreakStatement,
            SyntaxKind::ContinueStatement,
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
