#![forbid(unsafe_code)]
//! Token and syntax primitives.

use nexa_span::TextRange;

/// Lossless syntax kind used by tokens and future CST nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum SyntaxKind {
    /// An identifier.
    Ident,
    /// An integer literal.
    Int,
    /// Whitespace trivia.
    Whitespace,
    /// A line comment.
    LineComment,
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `,`
    Comma,
    /// `;`
    Semicolon,
    /// `=`
    Eq,
    /// An unrecognized character.
    Unknown,
}

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
        tokens.push(Token::new(
            kind,
            TextRange::new(start, end),
            &source[start..end],
        ));
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
                SyntaxKind::Ident,
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
}
