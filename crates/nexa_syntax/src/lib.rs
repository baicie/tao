#![forbid(unsafe_code)]
//! Token and syntax primitives.

use nexa_span::TextRange;

/// Lossless syntax kind used by tokens and CST nodes.
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
    /// An unrecognized character.
    Unknown = 11,
    /// The `let` keyword.
    LetKw = 12,
    /// The root of a parsed source file.
    SourceFile = 13,
    /// A `let` binding statement.
    LetStatement = 14,
    /// Tokens skipped during parser recovery.
    Error = 15,
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
            13 => Self::SourceFile,
            14 => Self::LetStatement,
            15 => Self::Error,
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

/// A lossless token with source range and original text.
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

/// Tokenizes source text into a lossless token stream.
#[must_use]
pub fn tokenize(source: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = source.char_indices().peekable();

    while let Some((start, ch)) = chars.next() {
        let kind = match ch {
            c if c.is_ascii_whitespace() => {
                consume_while(&mut chars, |c| c.is_ascii_whitespace());
                SyntaxKind::Whitespace
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                consume_while(&mut chars, |c| c.is_ascii_alphanumeric() || c == '_');
                SyntaxKind::Ident
            }
            c if c.is_ascii_digit() => {
                consume_while(&mut chars, |c| c.is_ascii_digit());
                SyntaxKind::Int
            }
            '/' if matches!(chars.peek(), Some((_, '/'))) => {
                let _ = chars.next();
                consume_while(&mut chars, |c| c != '\n');
                SyntaxKind::LineComment
            }
            '(' => SyntaxKind::LParen,
            ')' => SyntaxKind::RParen,
            '{' => SyntaxKind::LBrace,
            '}' => SyntaxKind::RBrace,
            ',' => SyntaxKind::Comma,
            ';' => SyntaxKind::Semicolon,
            '=' => SyntaxKind::Eq,
            _ => SyntaxKind::Unknown,
        };

        let end = chars.peek().map_or(source.len(), |(index, _)| *index);
        let text = &source[start..end];
        let kind = if kind == SyntaxKind::Ident && text == "let" {
            SyntaxKind::LetKw
        } else {
            kind
        };
        tokens.push(Token::new(kind, TextRange::new(start, end), text));
    }

    tokens
}

fn consume_while(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    predicate: impl Fn(char) -> bool,
) {
    while matches!(chars.peek(), Some((_, ch)) if predicate(*ch)) {
        let _ = chars.next();
    }
}

#[cfg(test)]
mod tests {
    use super::{tokenize, SyntaxKind};

    #[test]
    fn tokenizes_identifiers_and_punctuation() {
        let kinds: Vec<_> = tokenize("let answer = 42;")
            .into_iter()
            .map(|token| token.kind())
            .collect();

        assert_eq!(
            kinds,
            [
                SyntaxKind::LetKw,
                SyntaxKind::Whitespace,
                SyntaxKind::Ident,
                SyntaxKind::Whitespace,
                SyntaxKind::Eq,
                SyntaxKind::Whitespace,
                SyntaxKind::Int,
                SyntaxKind::Semicolon
            ]
        );
    }

    #[test]
    fn tokenization_preserves_source_text() {
        let source = "// binding\r\nlet answer = 42;";
        let tokenized = tokenize(source)
            .iter()
            .map(|token| token.text())
            .collect::<String>();

        assert_eq!(tokenized, source);
    }

    #[test]
    fn identifiers_starting_with_let_are_not_keywords() {
        let tokens = tokenize("letter");

        assert_eq!(tokens[0].kind(), SyntaxKind::Ident);
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
            SyntaxKind::SourceFile,
            SyntaxKind::LetStatement,
            SyntaxKind::Error,
        ];

        for kind in kinds {
            let raw: rowan::SyntaxKind = kind.into();

            assert_eq!(SyntaxKind::from_raw(raw.0), kind);
        }
    }
}
