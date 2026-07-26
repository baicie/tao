use nexa_diagnostics::{Diagnostic, DiagnosticCode, Label};
use nexa_span::{FileId, SourceSpan, TextRange};
use nexa_syntax::{SyntaxKind, Token};
use rowan::{GreenNode, GreenNodeBuilder};

const PARSE_ERROR: DiagnosticCode = DiagnosticCode::new("E1001");

pub(super) fn parse_tokens(
    file: FileId,
    source_len: usize,
    tokens: &[Token],
) -> (GreenNode, Vec<Diagnostic>) {
    Parser {
        file,
        source_len,
        tokens,
        position: 0,
        builder: GreenNodeBuilder::new(),
        diagnostics: lexical_diagnostics(file, tokens),
    }
    .parse()
}

fn lexical_diagnostics(file: FileId, tokens: &[Token]) -> Vec<Diagnostic> {
    tokens
        .iter()
        .filter(|token| token.kind() == SyntaxKind::Unknown)
        .map(|token| {
            Diagnostic::error(PARSE_ERROR, "unknown token").with_label(Label::new(
                SourceSpan::new(file, token.range()),
                format!("unexpected `{}`", token.text()),
            ))
        })
        .collect()
}

struct Parser<'tokens> {
    file: FileId,
    source_len: usize,
    tokens: &'tokens [Token],
    position: usize,
    builder: GreenNodeBuilder<'static>,
    diagnostics: Vec<Diagnostic>,
}

impl Parser<'_> {
    fn parse(mut self) -> (GreenNode, Vec<Diagnostic>) {
        self.builder.start_node(SyntaxKind::SourceFile.into());

        while self.current().is_some() {
            if self.current().is_some_and(SyntaxKind::is_trivia) {
                self.bump();
            } else if self.at(SyntaxKind::LetKw) {
                self.parse_let_statement();
            } else {
                self.parse_unexpected_item();
            }
        }

        self.builder.finish_node();
        self.diagnostics.sort_by_key(|diagnostic| {
            diagnostic
                .labels()
                .first()
                .map_or((usize::MAX, usize::MAX), |label| {
                    (label.span().range().start(), label.span().range().end())
                })
        });
        (self.builder.finish(), self.diagnostics)
    }

    fn parse_let_statement(&mut self) {
        self.builder.start_node(SyntaxKind::LetStatement.into());
        self.bump();

        if !self.expect(SyntaxKind::Ident, "expected binding name") {
            self.recover_to(&[SyntaxKind::Eq, SyntaxKind::Semicolon, SyntaxKind::LetKw]);
            if self.end_invalid_statement_at_boundary() {
                self.builder.finish_node();
                return;
            }
        }

        if !self.expect(SyntaxKind::Eq, "expected `=`") {
            self.recover_to(&[SyntaxKind::Int, SyntaxKind::Semicolon, SyntaxKind::LetKw]);
            if self.end_invalid_statement_at_boundary() {
                self.builder.finish_node();
                return;
            }
        }

        if !self.expect(SyntaxKind::Int, "expected integer literal") {
            self.recover_to(&[SyntaxKind::Semicolon, SyntaxKind::LetKw]);
            if self.end_invalid_statement_at_boundary() {
                self.builder.finish_node();
                return;
            }
        }

        if !self.expect(SyntaxKind::Semicolon, "expected `;`") {
            self.recover_to(&[SyntaxKind::Semicolon, SyntaxKind::LetKw]);
            if self.at(SyntaxKind::Semicolon) {
                self.bump();
            }
        }

        self.builder.finish_node();
    }

    fn parse_unexpected_item(&mut self) {
        self.builder.start_node(SyntaxKind::Error.into());

        if !self.at(SyntaxKind::Unknown) {
            self.error_at_current("expected `let` statement");
        }

        while let Some(kind) = self.current() {
            if kind == SyntaxKind::LetKw {
                break;
            }

            self.bump();
            if kind == SyntaxKind::Semicolon {
                break;
            }
        }

        self.builder.finish_node();
    }

    fn expect(&mut self, kind: SyntaxKind, message: &'static str) -> bool {
        self.skip_trivia();

        if self.at(kind) {
            self.bump();
            true
        } else {
            self.error_at_current(message);
            false
        }
    }

    fn recover_to(&mut self, recovery: &[SyntaxKind]) {
        let Some(kind) = self.current() else {
            return;
        };
        if recovery.contains(&kind) {
            return;
        }

        self.builder.start_node(SyntaxKind::Error.into());
        while self.current().is_some_and(|kind| !recovery.contains(&kind)) {
            self.bump();
        }
        self.builder.finish_node();
    }

    fn end_invalid_statement_at_boundary(&mut self) -> bool {
        if self.at(SyntaxKind::Semicolon) {
            self.bump();
            true
        } else {
            self.current().is_none() || self.at(SyntaxKind::LetKw)
        }
    }

    fn skip_trivia(&mut self) {
        while self.current().is_some_and(SyntaxKind::is_trivia) {
            self.bump();
        }
    }

    fn error_at_current(&mut self, message: &'static str) {
        let range = self.tokens.get(self.position).map_or_else(
            || TextRange::new(self.source_len, self.source_len),
            Token::range,
        );
        self.diagnostics.push(
            Diagnostic::error(PARSE_ERROR, message)
                .with_label(Label::new(SourceSpan::new(self.file, range), message)),
        );
    }

    fn current(&self) -> Option<SyntaxKind> {
        self.tokens.get(self.position).map(Token::kind)
    }

    fn at(&self, kind: SyntaxKind) -> bool {
        self.current() == Some(kind)
    }

    fn bump(&mut self) {
        let token = &self.tokens[self.position];
        self.builder.token(token.kind().into(), token.text());
        self.position += 1;
    }
}
