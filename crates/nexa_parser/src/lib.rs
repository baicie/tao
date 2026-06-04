#![forbid(unsafe_code)]
//! Parser entry points.

use nexa_diagnostics::{Diagnostic, Label};
use nexa_span::{FileId, SourceSpan};
use nexa_syntax::{tokenize, SyntaxKind, Token};

/// Result of parsing one source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parse {
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
}

impl Parse {
    /// Returns the lossless token stream.
    #[must_use]
    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    /// Returns parser and lexer diagnostics.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Returns true when no diagnostics were produced.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

/// Parses one source file.
///
/// This first-stage parser keeps a lossless token stream and reports lexical
/// unknowns. Grammar production parsing will grow from this boundary.
#[must_use]
pub fn parse_source(file: FileId, source: &str) -> Parse {
    let tokens = tokenize(source);
    let diagnostics = tokens
        .iter()
        .filter(|token| token.kind() == SyntaxKind::Unknown)
        .map(|token| {
            Diagnostic::error("unknown token").with_label(Label::new(
                SourceSpan::new(file, token.range()),
                format!("unexpected `{}`", token.text()),
            ))
        })
        .collect();

    Parse {
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

        assert!(!parse.is_ok());
        assert_eq!(parse.diagnostics()[0].message(), "unknown token");
    }
}
