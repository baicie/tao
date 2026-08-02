#![forbid(unsafe_code)]
//! Parser entry points.

mod parser;

use nexa_diagnostics::{Diagnostic, DiagnosticCode, Label, Severity};
use nexa_span::{FileId, SourceSpan};
use nexa_syntax::{tokenize, SyntaxNode, Token};
use rowan::{GreenNode, NodeOrToken, WalkEvent};

use crate::parser::parse_tokens;

const LEXICAL_ERROR: DiagnosticCode = DiagnosticCode::new("E1001");

/// Result of lexing one source file without running parser recovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexedSource {
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
}

impl LexedSource {
    /// Returns the ordered lossless token stream.
    #[must_use]
    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    /// Returns only diagnostics produced by lexical classification.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    fn into_parts(self) -> (Vec<Token>, Vec<Diagnostic>) {
        (self.tokens, self.diagnostics)
    }
}

/// Result of parsing one source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parse {
    tokens: Vec<Token>,
    green: GreenNode,
    diagnostics: Vec<Diagnostic>,
    parser_diagnostics: Vec<Diagnostic>,
}

impl Parse {
    /// Returns the lossless token stream.
    #[must_use]
    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    /// Returns the lossless concrete syntax tree.
    #[must_use]
    pub fn syntax(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }

    /// Returns a deterministic, line-oriented dump of the concrete syntax tree.
    #[must_use]
    pub fn debug_tree(&self) -> String {
        let mut output = String::new();
        let mut depth = 0;

        for event in self.syntax().preorder_with_tokens() {
            match event {
                WalkEvent::Enter(element) => {
                    if !output.is_empty() {
                        output.push('\n');
                    }
                    for _ in 0..depth {
                        output.push_str("  ");
                    }

                    let range = element.text_range();
                    let start = u32::from(range.start());
                    let end = u32::from(range.end());
                    let line = match element {
                        NodeOrToken::Node(node) => {
                            format!("{:?}@{start}..{end}", node.kind())
                        }
                        NodeOrToken::Token(token) => {
                            format!("{:?}@{start}..{end} {:?}", token.kind(), token.text())
                        }
                    };
                    output.push_str(&line);
                    depth += 1;
                }
                WalkEvent::Leave(_) => depth -= 1,
            }
        }

        output
    }

    /// Returns parser and lexer diagnostics.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Returns only diagnostics produced by parser grammar and recovery logic.
    #[must_use]
    pub fn parser_diagnostics(&self) -> &[Diagnostic] {
        &self.parser_diagnostics
    }

    /// Returns true when no error diagnostics were produced.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        !self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity() == Severity::Error)
    }
}

/// Parses one source file.
///
/// The parser always returns a lossless concrete syntax tree, including for
/// malformed source. Syntax errors are accumulated in [`Parse::diagnostics`].
///
/// # Panics
///
/// Panics when `source` is larger than Rowan's four-gibibyte tree limit.
#[must_use]
pub fn parse_source(file: FileId, source: &str) -> Parse {
    let (tokens, lexical_diagnostics) = lex_source(file, source).into_parts();
    let (green, parser_diagnostics) = parse_tokens(file, source.len(), &tokens);
    let mut diagnostics = lexical_diagnostics;
    diagnostics.extend(parser_diagnostics.iter().cloned());
    diagnostics.sort_by_key(|diagnostic| {
        diagnostic
            .labels()
            .first()
            .map_or((usize::MAX, usize::MAX), |label| {
                (label.span().range().start(), label.span().range().end())
            })
    });

    Parse {
        tokens,
        green,
        diagnostics,
        parser_diagnostics,
    }
}

/// Lexes one source file without running parser recovery.
///
/// The returned tokens cover the complete UTF-8 input losslessly. Every
/// unrecognized token produces one source-spanned `E1001` diagnostic.
#[must_use]
pub fn lex_source(file: FileId, source: &str) -> LexedSource {
    let tokens = tokenize(source);
    let diagnostics = tokens
        .iter()
        .filter(|token| token.kind() == nexa_syntax::SyntaxKind::Unknown)
        .map(|token| {
            Diagnostic::error(LEXICAL_ERROR, "unknown token").with_label(Label::new(
                SourceSpan::new(file, token.range()),
                format!("unexpected `{}`", token.text()),
            ))
        })
        .collect();

    LexedSource {
        tokens,
        diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_source;
    use nexa_span::FileId;

    #[test]
    fn reports_unknown_tokens() {
        let parse = parse_source(FileId::new(0), "@");

        assert_eq!(
            (
                parse.is_ok(),
                parse.diagnostics()[0].message(),
                parse.syntax().to_string(),
            ),
            (false, "unknown token", "@".to_owned())
        );
    }
}
