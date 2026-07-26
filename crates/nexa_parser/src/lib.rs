#![forbid(unsafe_code)]
//! Parser entry points.

mod parser;

use nexa_diagnostics::{Diagnostic, Severity};
use nexa_span::FileId;
use nexa_syntax::{tokenize, SyntaxNode, Token};
use rowan::{GreenNode, NodeOrToken, WalkEvent};

use crate::parser::parse_tokens;

/// Result of parsing one source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parse {
    tokens: Vec<Token>,
    green: GreenNode,
    diagnostics: Vec<Diagnostic>,
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
    let tokens = tokenize(source);
    let (green, diagnostics) = parse_tokens(file, source.len(), &tokens);

    Parse {
        tokens,
        green,
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
