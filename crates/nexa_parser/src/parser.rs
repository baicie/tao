use nexa_diagnostics::{Diagnostic, DiagnosticCode, Label};
use nexa_span::{FileId, SourceSpan, TextRange};
use nexa_syntax::{SyntaxKind, Token};
use rowan::{GreenNode, GreenNodeBuilder};

const PARSE_ERROR: DiagnosticCode = DiagnosticCode::new("E1001");
const STATEMENT_RECOVERY: &[SyntaxKind] = &[
    SyntaxKind::Semicolon,
    SyntaxKind::RBrace,
    SyntaxKind::ElseKw,
    SyntaxKind::ConstKw,
    SyntaxKind::IfKw,
    SyntaxKind::ReturnKw,
    SyntaxKind::FunctionKw,
];

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
            } else if self.at(SyntaxKind::FunctionKw) {
                self.parse_function_declaration();
            } else {
                self.parse_unexpected_top_level_item();
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

    fn parse_function_declaration(&mut self) {
        self.builder
            .start_node(SyntaxKind::FunctionDeclaration.into());
        self.expect(SyntaxKind::FunctionKw, "expected `function`");
        self.expect(SyntaxKind::Ident, "expected function name");
        self.expect(SyntaxKind::LParen, "expected `(` after function name");
        self.parse_parameter_list();
        self.expect(SyntaxKind::RParen, "expected `)` after parameters");
        self.expect(SyntaxKind::Colon, "expected `:` before return type");
        self.parse_type();
        self.parse_block();
        self.builder.finish_node();
    }

    fn parse_parameter_list(&mut self) {
        self.builder.start_node(SyntaxKind::ParameterList.into());
        self.skip_trivia();

        while self.current().is_some() && !self.at(SyntaxKind::RParen) {
            let position = self.position;
            self.parse_parameter();

            if self.position == position {
                self.recover_to(&[SyntaxKind::Comma, SyntaxKind::RParen, SyntaxKind::LBrace]);
            }

            self.skip_trivia();
            if self.at(SyntaxKind::Comma) {
                self.bump();
                self.skip_trivia();
                if self.at(SyntaxKind::RParen) {
                    self.error_at_current("expected parameter after `,`");
                    break;
                }
            } else if !self.at(SyntaxKind::RParen) {
                self.error_at_current("expected `,` or `)` after parameter");
                self.recover_to(&[SyntaxKind::Comma, SyntaxKind::RParen, SyntaxKind::LBrace]);
                if self.at(SyntaxKind::Comma) {
                    self.bump();
                    self.skip_trivia();
                }
            }

            if self.at(SyntaxKind::LBrace) {
                break;
            }
        }

        self.builder.finish_node();
    }

    fn parse_parameter(&mut self) {
        self.builder.start_node(SyntaxKind::Parameter.into());
        self.expect(SyntaxKind::Ident, "expected parameter name");
        self.expect(SyntaxKind::Colon, "expected `:` after parameter name");
        self.parse_type();
        self.builder.finish_node();
    }

    fn parse_type(&mut self) {
        self.builder.start_node(SyntaxKind::Type.into());
        self.skip_trivia();

        if matches!(
            self.current(),
            Some(SyntaxKind::IntKw | SyntaxKind::BoolKw | SyntaxKind::UnitKw)
        ) {
            self.bump();
        } else {
            self.error_at_current("expected type `Int`, `Bool`, or `Unit`");
        }

        self.builder.finish_node();
    }

    fn parse_block(&mut self) {
        self.builder.start_node(SyntaxKind::Block.into());
        if !self.expect(SyntaxKind::LBrace, "expected `{`") {
            self.builder.finish_node();
            return;
        }

        loop {
            self.skip_trivia();

            match self.current() {
                Some(SyntaxKind::RBrace) => {
                    self.bump();
                    break;
                }
                Some(SyntaxKind::ConstKw) => self.parse_const_declaration(),
                Some(SyntaxKind::IfKw) => self.parse_if_statement(),
                Some(SyntaxKind::ReturnKw) => self.parse_return_statement(),
                Some(SyntaxKind::ElseKw) => {
                    self.error_at_current("unexpected `else`");
                    self.bump();
                }
                Some(_) => self.parse_expression_statement(),
                None => {
                    self.error_at_current("expected `}` to close block");
                    break;
                }
            }
        }

        self.builder.finish_node();
    }

    fn parse_const_declaration(&mut self) {
        self.builder.start_node(SyntaxKind::ConstDeclaration.into());
        self.expect(SyntaxKind::ConstKw, "expected `const`");
        self.expect(SyntaxKind::Ident, "expected binding name");
        self.skip_trivia();

        if self.at(SyntaxKind::Colon) {
            self.bump();
            self.parse_type();
        }

        self.expect(SyntaxKind::Eq, "expected `=` after binding name");
        if !self.parse_expression() {
            self.recover_statement();
            self.builder.finish_node();
            return;
        }

        if !self.expect(SyntaxKind::Semicolon, "expected `;` after declaration") {
            self.recover_statement();
        }
        self.builder.finish_node();
    }

    fn parse_if_statement(&mut self) {
        self.builder.start_node(SyntaxKind::IfStatement.into());
        self.expect(SyntaxKind::IfKw, "expected `if`");
        self.expect(SyntaxKind::LParen, "expected `(` after `if`");
        let _ = self.parse_expression();
        self.expect(SyntaxKind::RParen, "expected `)` after condition");
        self.parse_block();
        self.skip_trivia();

        if self.at(SyntaxKind::ElseKw) {
            self.builder.start_node(SyntaxKind::ElseClause.into());
            self.bump();
            self.parse_block();
            self.builder.finish_node();
        }

        self.builder.finish_node();
    }

    fn parse_return_statement(&mut self) {
        self.builder.start_node(SyntaxKind::ReturnStatement.into());
        self.expect(SyntaxKind::ReturnKw, "expected `return`");
        self.skip_trivia();

        if !self.at(SyntaxKind::Semicolon) && self.current().is_some() {
            let _ = self.parse_expression();
        }

        if !self.expect(SyntaxKind::Semicolon, "expected `;` after return") {
            self.recover_statement();
        }
        self.builder.finish_node();
    }

    fn parse_expression_statement(&mut self) {
        self.builder
            .start_node(SyntaxKind::ExpressionStatement.into());

        if !self.parse_expression() {
            self.recover_statement();
            self.builder.finish_node();
            return;
        }

        if !self.expect(SyntaxKind::Semicolon, "expected `;` after expression") {
            self.recover_statement();
        }
        self.builder.finish_node();
    }

    fn parse_expression(&mut self) -> bool {
        self.parse_binary_expression(1)
    }

    fn parse_binary_expression(&mut self, minimum_precedence: u8) -> bool {
        self.skip_trivia();
        let checkpoint = self.builder.checkpoint();
        if !self.parse_unary_expression() {
            return false;
        }

        loop {
            self.skip_trivia();
            let Some(precedence) = self.binary_precedence() else {
                break;
            };
            if precedence < minimum_precedence {
                break;
            }

            self.builder
                .start_node_at(checkpoint, SyntaxKind::BinaryExpression.into());
            self.bump();
            if !self.parse_binary_expression(precedence + 1) {
                self.error_at_current("expected expression after operator");
            }
            self.builder.finish_node();
        }

        true
    }

    fn parse_unary_expression(&mut self) -> bool {
        self.skip_trivia();
        if self.at(SyntaxKind::Bang) || self.at(SyntaxKind::Minus) {
            self.builder.start_node(SyntaxKind::UnaryExpression.into());
            self.bump();
            if !self.parse_unary_expression() {
                self.error_at_current("expected expression after unary operator");
            }
            self.builder.finish_node();
            true
        } else {
            self.parse_call_expression()
        }
    }

    fn parse_call_expression(&mut self) -> bool {
        self.skip_trivia();
        let checkpoint = self.builder.checkpoint();
        if !self.parse_primary_expression() {
            return false;
        }

        loop {
            self.skip_trivia();
            if !self.at(SyntaxKind::LParen) {
                break;
            }

            self.builder
                .start_node_at(checkpoint, SyntaxKind::CallExpression.into());
            self.bump();
            self.parse_argument_list();
            self.expect(SyntaxKind::RParen, "expected `)` after arguments");
            self.builder.finish_node();
        }

        true
    }

    fn parse_argument_list(&mut self) {
        self.skip_trivia();
        if self.at(SyntaxKind::RParen) {
            return;
        }

        self.builder.start_node(SyntaxKind::ArgumentList.into());
        loop {
            if !self.parse_expression() {
                self.recover_to(&[SyntaxKind::Comma, SyntaxKind::RParen]);
            }

            self.skip_trivia();
            if self.at(SyntaxKind::Comma) {
                self.bump();
                self.skip_trivia();
                continue;
            }
            break;
        }
        self.builder.finish_node();
    }

    fn parse_primary_expression(&mut self) -> bool {
        self.skip_trivia();

        match self.current() {
            Some(SyntaxKind::Ident) => {
                self.builder.start_node(SyntaxKind::NameReference.into());
                self.bump();
                self.builder.finish_node();
                true
            }
            Some(SyntaxKind::Int) => {
                self.builder.start_node(SyntaxKind::IntLiteral.into());
                self.bump();
                self.builder.finish_node();
                true
            }
            Some(SyntaxKind::TrueKw | SyntaxKind::FalseKw) => {
                self.builder.start_node(SyntaxKind::BoolLiteral.into());
                self.bump();
                self.builder.finish_node();
                true
            }
            Some(SyntaxKind::LParen) => {
                self.builder
                    .start_node(SyntaxKind::ParenthesizedExpression.into());
                self.bump();
                let _ = self.parse_expression();
                self.expect(SyntaxKind::RParen, "expected `)` after expression");
                self.builder.finish_node();
                true
            }
            _ => {
                self.error_at_current("expected expression");
                false
            }
        }
    }

    fn binary_precedence(&self) -> Option<u8> {
        match self.current() {
            Some(SyntaxKind::EqEqEq) => Some(1),
            Some(SyntaxKind::Lt | SyntaxKind::LtEq | SyntaxKind::Gt | SyntaxKind::GtEq) => Some(2),
            Some(SyntaxKind::Plus | SyntaxKind::Minus) => Some(3),
            Some(SyntaxKind::Star | SyntaxKind::Slash) => Some(4),
            _ => None,
        }
    }

    fn parse_unexpected_top_level_item(&mut self) {
        self.builder.start_node(SyntaxKind::Error.into());

        if !self.at(SyntaxKind::Unknown) {
            self.error_at_current("expected `function` declaration");
        }

        while let Some(kind) = self.current() {
            if kind == SyntaxKind::FunctionKw {
                break;
            }
            self.bump();
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

    fn recover_statement(&mut self) {
        self.recover_to(STATEMENT_RECOVERY);
        if self.at(SyntaxKind::Semicolon) {
            self.bump();
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
        while self
            .current()
            .is_some_and(|current| !recovery.contains(&current))
        {
            self.bump();
        }
        self.builder.finish_node();
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
