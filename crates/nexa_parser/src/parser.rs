use nexa_diagnostics::{Diagnostic, DiagnosticCode, Label};
use nexa_span::{FileId, SourceSpan, TextRange};
use nexa_syntax::{SyntaxKind, Token};
use rowan::{GreenNode, GreenNodeBuilder};

const PARSE_ERROR: DiagnosticCode = DiagnosticCode::new("E1001");
const TOP_LEVEL_RECOVERY: &[SyntaxKind] = &[SyntaxKind::FunctionKw, SyntaxKind::TypeKw];
const STATEMENT_RECOVERY: &[SyntaxKind] = &[
    SyntaxKind::Semicolon,
    SyntaxKind::RBrace,
    SyntaxKind::ElseKw,
    SyntaxKind::FunctionKw,
    SyntaxKind::ConstKw,
    SyntaxKind::LetKw,
    SyntaxKind::IfKw,
    SyntaxKind::WhileKw,
    SyntaxKind::BreakKw,
    SyntaxKind::ContinueKw,
    SyntaxKind::ReturnKw,
    SyntaxKind::TypeKw,
];
const ARRAY_RECOVERY: &[SyntaxKind] = &[
    SyntaxKind::Comma,
    SyntaxKind::RBracket,
    SyntaxKind::RParen,
    SyntaxKind::Semicolon,
    SyntaxKind::RBrace,
    SyntaxKind::ElseKw,
    SyntaxKind::FunctionKw,
    SyntaxKind::ConstKw,
    SyntaxKind::LetKw,
    SyntaxKind::IfKw,
    SyntaxKind::WhileKw,
    SyntaxKind::BreakKw,
    SyntaxKind::ContinueKw,
    SyntaxKind::ReturnKw,
    SyntaxKind::TypeKw,
];
const RECORD_BODY_RECOVERY: &[SyntaxKind] = &[
    SyntaxKind::Ident,
    SyntaxKind::Semicolon,
    SyntaxKind::RBrace,
    SyntaxKind::FunctionKw,
    SyntaxKind::TypeKw,
];
const RECORD_EXPRESSION_RECOVERY: &[SyntaxKind] = &[
    SyntaxKind::Comma,
    SyntaxKind::RBrace,
    SyntaxKind::RBracket,
    SyntaxKind::RParen,
    SyntaxKind::Semicolon,
    SyntaxKind::FunctionKw,
    SyntaxKind::ConstKw,
    SyntaxKind::LetKw,
    SyntaxKind::IfKw,
    SyntaxKind::WhileKw,
    SyntaxKind::BreakKw,
    SyntaxKind::ContinueKw,
    SyntaxKind::ReturnKw,
    SyntaxKind::TypeKw,
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
            } else if self.at(SyntaxKind::TypeKw) {
                self.parse_record_declaration();
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

    fn parse_record_declaration(&mut self) {
        self.builder
            .start_node(SyntaxKind::RecordDeclaration.into());
        self.expect(SyntaxKind::TypeKw, "expected `type`");
        self.expect(SyntaxKind::Ident, "expected record name");
        self.expect(SyntaxKind::Eq, "expected `=` after record name");
        self.parse_record_body();
        if !self.expect(
            SyntaxKind::Semicolon,
            "expected `;` after record declaration",
        ) {
            self.recover_to(TOP_LEVEL_RECOVERY);
        }
        self.builder.finish_node();
    }

    fn parse_record_body(&mut self) {
        self.builder.start_node(SyntaxKind::RecordBody.into());
        if !self.expect(SyntaxKind::LBrace, "expected `{` before record fields") {
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
                Some(SyntaxKind::Ident) => self.parse_record_field_declaration(),
                Some(SyntaxKind::FunctionKw | SyntaxKind::TypeKw) => {
                    self.error_at_current("expected `}` after record fields");
                    break;
                }
                Some(_) => {
                    self.error_at_current("expected record field declaration");
                    self.recover_record_body();
                }
                None => {
                    self.error_at_current("expected `}` after record fields");
                    break;
                }
            }
        }

        self.builder.finish_node();
    }

    fn parse_record_field_declaration(&mut self) {
        self.builder
            .start_node(SyntaxKind::RecordFieldDeclaration.into());
        self.expect(SyntaxKind::Ident, "expected record field name");
        self.expect(SyntaxKind::Colon, "expected `:` after record field name");
        self.parse_type();
        if !self.expect(
            SyntaxKind::Semicolon,
            "expected `;` after record field declaration",
        ) {
            self.recover_record_body();
        }
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
        self.skip_trivia();
        let checkpoint = self.builder.checkpoint();
        self.builder.start_node(SyntaxKind::Type.into());

        let parsed = if matches!(
            self.current(),
            Some(
                SyntaxKind::IntKw
                    | SyntaxKind::BoolKw
                    | SyntaxKind::StringKw
                    | SyntaxKind::UnitKw
                    | SyntaxKind::Ident
            )
        ) {
            self.bump();
            true
        } else {
            self.error_at_current("expected type `Int`, `Bool`, `String`, `Unit`, or a named type");
            false
        };

        self.builder.finish_node();

        while parsed && self.nth_non_trivia(0) == Some(SyntaxKind::LBracket) {
            self.builder
                .start_node_at(checkpoint, SyntaxKind::ArrayType.into());
            self.expect(SyntaxKind::LBracket, "expected `[` in array type");
            self.expect(SyntaxKind::RBracket, "expected `]` after array type");
            self.builder.finish_node();
        }
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
                Some(SyntaxKind::LetKw) => self.parse_let_declaration(),
                Some(SyntaxKind::IfKw) => self.parse_if_statement(),
                Some(SyntaxKind::WhileKw) => self.parse_while_statement(),
                Some(SyntaxKind::BreakKw) => self.parse_break_statement(),
                Some(SyntaxKind::ContinueKw) => self.parse_continue_statement(),
                Some(SyntaxKind::ReturnKw) => self.parse_return_statement(),
                Some(SyntaxKind::ElseKw) => {
                    self.error_at_current("unexpected `else`");
                    self.bump();
                }
                Some(SyntaxKind::FunctionKw | SyntaxKind::TypeKw) => {
                    self.error_at_current("expected `}` to close block");
                    break;
                }
                Some(SyntaxKind::Ident) if self.at_assignment_statement() => {
                    self.parse_assignment_statement();
                }
                Some(_) => {
                    let statement_position = self.position;
                    self.parse_expression_statement();

                    // A recovery boundary is valid only when another parser branch
                    // can consume it. Do not let a new grammar token stall this loop.
                    if self.position == statement_position {
                        self.error_at_current("unable to recover from statement");
                        self.bump();
                    }
                }
                None => {
                    self.error_at_current("expected `}` to close block");
                    break;
                }
            }
        }

        self.builder.finish_node();
    }

    fn parse_const_declaration(&mut self) {
        self.parse_binding_declaration(
            SyntaxKind::ConstDeclaration,
            SyntaxKind::ConstKw,
            "expected `const`",
        );
    }

    fn parse_let_declaration(&mut self) {
        self.parse_binding_declaration(
            SyntaxKind::LetDeclaration,
            SyntaxKind::LetKw,
            "expected `let`",
        );
    }

    fn parse_binding_declaration(
        &mut self,
        declaration_kind: SyntaxKind,
        keyword_kind: SyntaxKind,
        keyword_message: &'static str,
    ) {
        self.builder.start_node(declaration_kind.into());
        self.expect(keyword_kind, keyword_message);
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

    fn parse_assignment_statement(&mut self) {
        self.builder
            .start_node(SyntaxKind::AssignmentStatement.into());
        self.expect(SyntaxKind::Ident, "expected assignment target");
        self.expect(SyntaxKind::Eq, "expected `=` after assignment target");

        if !self.parse_expression() {
            self.recover_statement();
            self.builder.finish_node();
            return;
        }

        if !self.expect(SyntaxKind::Semicolon, "expected `;` after assignment") {
            self.recover_statement();
        }
        self.builder.finish_node();
    }

    fn parse_if_statement(&mut self) {
        self.builder.start_node(SyntaxKind::IfStatement.into());
        self.expect(SyntaxKind::IfKw, "expected `if`");
        let has_left_parenthesis = self.expect(SyntaxKind::LParen, "expected `(` after `if`");
        if has_left_parenthesis || !self.at(SyntaxKind::LBrace) {
            let _ = self.parse_expression();
        } else {
            self.error_at_current("expected expression");
        }
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

    fn parse_while_statement(&mut self) {
        self.builder.start_node(SyntaxKind::WhileStatement.into());
        self.expect(SyntaxKind::WhileKw, "expected `while`");
        let has_left_parenthesis = self.expect(SyntaxKind::LParen, "expected `(` after `while`");
        if has_left_parenthesis || !self.at(SyntaxKind::LBrace) {
            let _ = self.parse_expression();
        } else {
            self.error_at_current("expected expression");
        }
        self.expect(SyntaxKind::RParen, "expected `)` after condition");
        self.parse_block();
        self.builder.finish_node();
    }

    fn parse_break_statement(&mut self) {
        self.parse_terminated_keyword_statement(
            SyntaxKind::BreakStatement,
            SyntaxKind::BreakKw,
            "expected `break`",
            "expected `;` after `break`",
        );
    }

    fn parse_continue_statement(&mut self) {
        self.parse_terminated_keyword_statement(
            SyntaxKind::ContinueStatement,
            SyntaxKind::ContinueKw,
            "expected `continue`",
            "expected `;` after `continue`",
        );
    }

    fn parse_terminated_keyword_statement(
        &mut self,
        statement_kind: SyntaxKind,
        keyword_kind: SyntaxKind,
        keyword_message: &'static str,
        semicolon_message: &'static str,
    ) {
        self.builder.start_node(statement_kind.into());
        self.expect(keyword_kind, keyword_message);
        if !self.expect(SyntaxKind::Semicolon, semicolon_message) {
            self.recover_statement();
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
            self.parse_postfix_expression()
        }
    }

    fn parse_postfix_expression(&mut self) -> bool {
        self.skip_trivia();
        let checkpoint = self.builder.checkpoint();
        if !self.parse_primary_expression() {
            return false;
        }

        loop {
            self.skip_trivia();
            match self.current() {
                Some(SyntaxKind::LParen) => {
                    self.builder
                        .start_node_at(checkpoint, SyntaxKind::CallExpression.into());
                    self.bump();
                    self.parse_argument_list();
                    self.expect(SyntaxKind::RParen, "expected `)` after arguments");
                    self.builder.finish_node();
                }
                Some(SyntaxKind::LBracket) => {
                    self.builder
                        .start_node_at(checkpoint, SyntaxKind::IndexExpression.into());
                    self.bump();
                    let _ = self.parse_expression();
                    self.expect(SyntaxKind::RBracket, "expected `]` after index expression");
                    self.builder.finish_node();
                }
                Some(SyntaxKind::Dot) => {
                    self.builder
                        .start_node_at(checkpoint, SyntaxKind::MemberExpression.into());
                    self.bump();
                    self.expect(SyntaxKind::Ident, "expected member name after `.`");
                    self.builder.finish_node();
                }
                _ => break,
            }
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
            Some(SyntaxKind::String) => {
                self.builder.start_node(SyntaxKind::StringLiteral.into());
                self.bump();
                self.builder.finish_node();
                true
            }
            Some(SyntaxKind::LBracket) => {
                self.parse_array_expression();
                true
            }
            Some(SyntaxKind::LBrace) => {
                self.parse_record_expression();
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

    fn parse_array_expression(&mut self) {
        self.builder.start_node(SyntaxKind::ArrayExpression.into());
        self.expect(SyntaxKind::LBracket, "expected `[` before array elements");
        self.skip_trivia();

        if self.at(SyntaxKind::RBracket) {
            self.bump();
            self.builder.finish_node();
            return;
        }

        loop {
            if !self.parse_expression() {
                self.recover_to(ARRAY_RECOVERY);
            }

            self.skip_trivia();
            if self.at(SyntaxKind::Comma) {
                self.bump();
                self.skip_trivia();
                if self.at(SyntaxKind::RBracket) {
                    self.error_at_current("expected expression after `,`");
                    break;
                }
                continue;
            }
            if self.at(SyntaxKind::RBracket) {
                break;
            }

            self.error_at_current("expected `,` or `]` after array element");
            self.recover_to(ARRAY_RECOVERY);
            if self.at(SyntaxKind::Comma) {
                self.bump();
                self.skip_trivia();
                if self.at(SyntaxKind::RBracket) {
                    self.error_at_current("expected expression after `,`");
                    break;
                }
                continue;
            }
            break;
        }

        self.expect(SyntaxKind::RBracket, "expected `]` after array elements");
        self.builder.finish_node();
    }

    fn parse_record_expression(&mut self) {
        self.builder.start_node(SyntaxKind::RecordExpression.into());
        self.expect(SyntaxKind::LBrace, "expected `{` before record fields");
        self.skip_trivia();

        if self.at(SyntaxKind::RBrace) {
            self.bump();
            self.builder.finish_node();
            return;
        }

        loop {
            self.parse_record_field_initializer();
            self.skip_trivia();

            if self.at(SyntaxKind::Comma) {
                self.bump();
                self.skip_trivia();
                if self.at(SyntaxKind::RBrace) {
                    self.error_at_current("expected record field after `,`");
                    break;
                }
                continue;
            }
            if self.at(SyntaxKind::RBrace) {
                break;
            }
            if self.at(SyntaxKind::Ident) {
                self.error_at_current("expected `,` or `}` after record field");
                continue;
            }

            self.error_at_current("expected `,` or `}` after record field");
            self.recover_to(RECORD_EXPRESSION_RECOVERY);
            if self.at(SyntaxKind::Comma) {
                self.bump();
                self.skip_trivia();
                if self.at(SyntaxKind::RBrace) {
                    self.error_at_current("expected record field after `,`");
                    break;
                }
                continue;
            }
            break;
        }

        self.expect(SyntaxKind::RBrace, "expected `}` after record fields");
        self.builder.finish_node();
    }

    fn parse_record_field_initializer(&mut self) {
        self.builder
            .start_node(SyntaxKind::RecordFieldInitializer.into());
        self.expect(SyntaxKind::Ident, "expected record field name");
        self.expect(SyntaxKind::Colon, "expected `:` after record field name");
        if !self.parse_expression() {
            self.recover_to(RECORD_EXPRESSION_RECOVERY);
        }
        self.builder.finish_node();
    }

    fn binary_precedence(&self) -> Option<u8> {
        match self.current() {
            Some(SyntaxKind::PipePipe) => Some(1),
            Some(SyntaxKind::AmpAmp) => Some(2),
            Some(SyntaxKind::EqEqEq) => Some(3),
            Some(SyntaxKind::Lt | SyntaxKind::LtEq | SyntaxKind::Gt | SyntaxKind::GtEq) => Some(4),
            Some(SyntaxKind::Plus | SyntaxKind::Minus) => Some(5),
            Some(SyntaxKind::Star | SyntaxKind::Slash) => Some(6),
            _ => None,
        }
    }

    fn parse_unexpected_top_level_item(&mut self) {
        self.builder.start_node(SyntaxKind::Error.into());

        if !self.at(SyntaxKind::Unknown) {
            self.error_at_current("expected `function` declaration");
        }

        while let Some(kind) = self.current() {
            if matches!(kind, SyntaxKind::FunctionKw | SyntaxKind::TypeKw) {
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

    fn recover_record_body(&mut self) {
        self.recover_to(RECORD_BODY_RECOVERY);
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

    fn at_assignment_statement(&self) -> bool {
        self.nth_non_trivia(0) == Some(SyntaxKind::Ident)
            && self.nth_non_trivia(1) == Some(SyntaxKind::Eq)
    }

    fn nth_non_trivia(&self, offset: usize) -> Option<SyntaxKind> {
        self.tokens[self.position..]
            .iter()
            .filter(|token| !token.kind().is_trivia())
            .nth(offset)
            .map(Token::kind)
    }

    fn bump(&mut self) {
        let token = &self.tokens[self.position];
        self.builder.token(token.kind().into(), token.text());
        self.position += 1;
    }
}
