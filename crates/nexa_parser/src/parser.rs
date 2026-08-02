use nexa_diagnostics::{Diagnostic, DiagnosticCode, Label};
use nexa_span::{FileId, SourceSpan, TextRange};
use nexa_syntax::{SyntaxKind, Token};
use rowan::{GreenNode, GreenNodeBuilder};

const PARSE_ERROR: DiagnosticCode = DiagnosticCode::new("E1001");
const TOP_LEVEL_RECOVERY: &[SyntaxKind] = &[
    SyntaxKind::ImportKw,
    SyntaxKind::ExportKw,
    SyntaxKind::FunctionKw,
    SyntaxKind::TypeKw,
];
const STATEMENT_RECOVERY: &[SyntaxKind] = &[
    SyntaxKind::Semicolon,
    SyntaxKind::RBrace,
    SyntaxKind::ElseKw,
    SyntaxKind::FunctionKw,
    SyntaxKind::ConstKw,
    SyntaxKind::LetKw,
    SyntaxKind::IfKw,
    SyntaxKind::WhileKw,
    SyntaxKind::ForKw,
    SyntaxKind::BreakKw,
    SyntaxKind::ContinueKw,
    SyntaxKind::ReturnKw,
    SyntaxKind::MatchKw,
    SyntaxKind::ImportKw,
    SyntaxKind::ExportKw,
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
    SyntaxKind::ForKw,
    SyntaxKind::BreakKw,
    SyntaxKind::ContinueKw,
    SyntaxKind::ReturnKw,
    SyntaxKind::MatchKw,
    SyntaxKind::CaseKw,
    SyntaxKind::DefaultKw,
    SyntaxKind::ImportKw,
    SyntaxKind::ExportKw,
    SyntaxKind::TypeKw,
];
const RECORD_BODY_RECOVERY: &[SyntaxKind] = &[
    SyntaxKind::Ident,
    SyntaxKind::Semicolon,
    SyntaxKind::RBrace,
    SyntaxKind::ImportKw,
    SyntaxKind::ExportKw,
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
    SyntaxKind::ForKw,
    SyntaxKind::BreakKw,
    SyntaxKind::ContinueKw,
    SyntaxKind::ReturnKw,
    SyntaxKind::MatchKw,
    SyntaxKind::CaseKw,
    SyntaxKind::DefaultKw,
    SyntaxKind::ImportKw,
    SyntaxKind::ExportKw,
    SyntaxKind::TypeKw,
];
const UNION_DECLARATION_RECOVERY: &[SyntaxKind] = &[
    SyntaxKind::Ident,
    SyntaxKind::Pipe,
    SyntaxKind::Semicolon,
    SyntaxKind::ImportKw,
    SyntaxKind::ExportKw,
    SyntaxKind::FunctionKw,
    SyntaxKind::TypeKw,
];
const VARIANT_PAYLOAD_RECOVERY: &[SyntaxKind] = &[
    SyntaxKind::Ident,
    SyntaxKind::Comma,
    SyntaxKind::RParen,
    SyntaxKind::Pipe,
    SyntaxKind::Semicolon,
    SyntaxKind::ImportKw,
    SyntaxKind::ExportKw,
    SyntaxKind::FunctionKw,
    SyntaxKind::TypeKw,
];
const MATCH_BODY_RECOVERY: &[SyntaxKind] = &[
    SyntaxKind::CaseKw,
    SyntaxKind::DefaultKw,
    SyntaxKind::RBrace,
    SyntaxKind::Semicolon,
    SyntaxKind::ConstKw,
    SyntaxKind::LetKw,
    SyntaxKind::IfKw,
    SyntaxKind::WhileKw,
    SyntaxKind::ForKw,
    SyntaxKind::BreakKw,
    SyntaxKind::ContinueKw,
    SyntaxKind::ReturnKw,
    SyntaxKind::MatchKw,
    SyntaxKind::ImportKw,
    SyntaxKind::ExportKw,
    SyntaxKind::FunctionKw,
    SyntaxKind::TypeKw,
];
const MATCH_ARM_RECOVERY: &[SyntaxKind] = &[
    SyntaxKind::Semicolon,
    SyntaxKind::CaseKw,
    SyntaxKind::DefaultKw,
    SyntaxKind::RBrace,
    SyntaxKind::ConstKw,
    SyntaxKind::LetKw,
    SyntaxKind::IfKw,
    SyntaxKind::WhileKw,
    SyntaxKind::ForKw,
    SyntaxKind::BreakKw,
    SyntaxKind::ContinueKw,
    SyntaxKind::ReturnKw,
    SyntaxKind::MatchKw,
    SyntaxKind::ImportKw,
    SyntaxKind::ExportKw,
    SyntaxKind::FunctionKw,
    SyntaxKind::TypeKw,
];
const PATTERN_BINDING_RECOVERY: &[SyntaxKind] = &[
    SyntaxKind::Ident,
    SyntaxKind::Comma,
    SyntaxKind::RParen,
    SyntaxKind::FatArrow,
    SyntaxKind::CaseKw,
    SyntaxKind::DefaultKw,
    SyntaxKind::RBrace,
];
const IMPORT_LIST_RECOVERY: &[SyntaxKind] = &[
    SyntaxKind::Ident,
    SyntaxKind::Comma,
    SyntaxKind::RBrace,
    SyntaxKind::FromKw,
    SyntaxKind::String,
    SyntaxKind::Semicolon,
    SyntaxKind::ImportKw,
    SyntaxKind::ExportKw,
    SyntaxKind::FunctionKw,
    SyntaxKind::TypeKw,
];
const TYPE_PARAMETER_RECOVERY: &[SyntaxKind] = &[
    SyntaxKind::Ident,
    SyntaxKind::Comma,
    SyntaxKind::Gt,
    SyntaxKind::GtEq,
    SyntaxKind::LParen,
    SyntaxKind::LBrace,
    SyntaxKind::RBrace,
    SyntaxKind::Colon,
    SyntaxKind::Eq,
    SyntaxKind::Semicolon,
    SyntaxKind::ImportKw,
    SyntaxKind::ExportKw,
    SyntaxKind::FunctionKw,
    SyntaxKind::TypeKw,
];
const TYPE_ARGUMENT_RECOVERY: &[SyntaxKind] = &[
    SyntaxKind::Ident,
    SyntaxKind::IntKw,
    SyntaxKind::BoolKw,
    SyntaxKind::StringKw,
    SyntaxKind::UnitKw,
    SyntaxKind::LParen,
    SyntaxKind::Comma,
    SyntaxKind::Gt,
    SyntaxKind::GtEq,
    SyntaxKind::RBracket,
    SyntaxKind::RParen,
    SyntaxKind::LBrace,
    SyntaxKind::RBrace,
    SyntaxKind::Colon,
    SyntaxKind::Eq,
    SyntaxKind::Semicolon,
    SyntaxKind::Pipe,
    SyntaxKind::FatArrow,
    SyntaxKind::ImportKw,
    SyntaxKind::ExportKw,
    SyntaxKind::FunctionKw,
    SyntaxKind::TypeKw,
];
const PARAMETER_LIST_RECOVERY: &[SyntaxKind] = &[
    SyntaxKind::Comma,
    SyntaxKind::RParen,
    SyntaxKind::Colon,
    SyntaxKind::FatArrow,
    SyntaxKind::LBrace,
    SyntaxKind::RBrace,
    SyntaxKind::Eq,
    SyntaxKind::Semicolon,
    SyntaxKind::ImportKw,
    SyntaxKind::ExportKw,
    SyntaxKind::FunctionKw,
    SyntaxKind::TypeKw,
];
const FOR_HEADER_RECOVERY: &[SyntaxKind] = &[
    SyntaxKind::RParen,
    SyntaxKind::LBrace,
    SyntaxKind::RBrace,
    SyntaxKind::Semicolon,
    SyntaxKind::ConstKw,
    SyntaxKind::LetKw,
    SyntaxKind::IfKw,
    SyntaxKind::WhileKw,
    SyntaxKind::ForKw,
    SyntaxKind::BreakKw,
    SyntaxKind::ContinueKw,
    SyntaxKind::ReturnKw,
];

pub(super) fn parse_tokens(
    file: FileId,
    source_len: usize,
    tokens: &[Token],
    lexical_diagnostics: Vec<Diagnostic>,
) -> (GreenNode, Vec<Diagnostic>) {
    Parser {
        file,
        source_len,
        tokens,
        position: 0,
        pending_split_eq: false,
        builder: GreenNodeBuilder::new(),
        diagnostics: lexical_diagnostics,
    }
    .parse()
}

struct Parser<'tokens> {
    file: FileId,
    source_len: usize,
    tokens: &'tokens [Token],
    position: usize,
    pending_split_eq: bool,
    builder: GreenNodeBuilder<'static>,
    diagnostics: Vec<Diagnostic>,
}

impl Parser<'_> {
    fn parse(mut self) -> (GreenNode, Vec<Diagnostic>) {
        self.builder.start_node(SyntaxKind::SourceFile.into());

        while self.current().is_some() {
            if self.current().is_some_and(SyntaxKind::is_trivia) {
                self.bump();
            } else if self.at(SyntaxKind::ImportKw) {
                self.parse_import_declaration();
            } else if self.at(SyntaxKind::ExportKw) {
                self.parse_exported_declaration();
            } else if self.at(SyntaxKind::FunctionKw) {
                self.parse_function_declaration();
            } else if self.at(SyntaxKind::TypeKw) {
                self.parse_type_declaration();
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

    fn parse_type_declaration(&mut self) {
        if self.type_declaration_is_union() {
            self.parse_union_declaration();
        } else {
            self.parse_record_declaration();
        }
    }

    fn parse_import_declaration(&mut self) {
        self.builder
            .start_node(SyntaxKind::ImportDeclaration.into());
        self.expect(SyntaxKind::ImportKw, "expected `import`");
        self.expect(SyntaxKind::LBrace, "expected `{` after `import`");
        self.parse_import_list();
        self.expect(SyntaxKind::RBrace, "expected `}` after imported names");
        self.expect(SyntaxKind::FromKw, "expected `from` after imported names");
        self.expect(SyntaxKind::String, "expected import path string");
        if !self.expect(
            SyntaxKind::Semicolon,
            "expected `;` after import declaration",
        ) {
            self.recover_to(TOP_LEVEL_RECOVERY);
        }
        self.builder.finish_node();
    }

    fn parse_import_list(&mut self) {
        self.builder.start_node(SyntaxKind::ImportList.into());
        self.skip_trivia();

        if self.import_list_is_finished() {
            self.error_at_current("expected imported name");
            self.builder.finish_node();
            return;
        }

        loop {
            if self.at(SyntaxKind::Ident) {
                self.bump();
            } else {
                self.error_at_current("expected imported name");
                self.recover_to(IMPORT_LIST_RECOVERY);
                if self.at(SyntaxKind::Comma) {
                    self.bump();
                    self.skip_trivia();
                }
                if self.import_list_is_finished() {
                    break;
                }
                if !self.at(SyntaxKind::Ident) {
                    continue;
                }
                self.bump();
            }

            self.skip_trivia();
            if self.at(SyntaxKind::Comma) {
                self.bump();
                self.skip_trivia();
                if self.import_list_is_finished() {
                    self.error_at_current("expected imported name after `,`");
                    break;
                }
                continue;
            }
            if self.at(SyntaxKind::Ident) {
                self.error_at_current("expected `,` or `}` after imported name");
                continue;
            }
            break;
        }

        self.builder.finish_node();
    }

    fn parse_exported_declaration(&mut self) {
        self.builder
            .start_node(SyntaxKind::ExportedDeclaration.into());
        self.expect(SyntaxKind::ExportKw, "expected `export`");
        self.skip_trivia();

        match self.current() {
            Some(SyntaxKind::FunctionKw) => self.parse_function_declaration(),
            Some(SyntaxKind::TypeKw) => self.parse_type_declaration(),
            _ => {
                self.error_at_current("expected `function` or `type` after `export`");
                self.recover_to(TOP_LEVEL_RECOVERY);
            }
        }

        self.builder.finish_node();
    }

    fn parse_function_declaration(&mut self) {
        self.builder
            .start_node(SyntaxKind::FunctionDeclaration.into());
        self.expect(SyntaxKind::FunctionKw, "expected `function`");
        self.expect(SyntaxKind::Ident, "expected function name");
        self.parse_type_parameter_list();
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
        self.parse_type_parameter_list();
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
                Some(
                    SyntaxKind::ImportKw
                    | SyntaxKind::ExportKw
                    | SyntaxKind::FunctionKw
                    | SyntaxKind::TypeKw,
                ) => {
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

    fn parse_union_declaration(&mut self) {
        self.builder.start_node(SyntaxKind::UnionDeclaration.into());
        self.expect(SyntaxKind::TypeKw, "expected `type`");
        self.expect(SyntaxKind::Ident, "expected union name");
        self.parse_type_parameter_list();
        self.expect(SyntaxKind::Eq, "expected `=` after union name");
        self.skip_trivia();

        if self.at(SyntaxKind::Pipe) {
            self.bump();
            self.skip_trivia();
        }

        let mut parsed_variant = false;
        loop {
            self.skip_trivia();
            match self.current() {
                Some(SyntaxKind::Ident) => {
                    self.parse_union_variant();
                    parsed_variant = true;
                }
                Some(
                    SyntaxKind::Semicolon
                    | SyntaxKind::ImportKw
                    | SyntaxKind::ExportKw
                    | SyntaxKind::FunctionKw
                    | SyntaxKind::TypeKw,
                )
                | None => {
                    if !parsed_variant {
                        self.error_at_current("expected union variant");
                    }
                    break;
                }
                Some(_) => {
                    self.error_at_current("expected union variant");
                    self.recover_to(UNION_DECLARATION_RECOVERY);
                    if self.at(SyntaxKind::Ident) {
                        continue;
                    }
                    if self.at(SyntaxKind::Pipe) {
                        self.bump();
                        continue;
                    }
                    break;
                }
            }

            self.skip_trivia();
            if self.at(SyntaxKind::Pipe) {
                self.bump();
                self.skip_trivia();
                if matches!(
                    self.current(),
                    Some(
                        SyntaxKind::Semicolon
                            | SyntaxKind::ImportKw
                            | SyntaxKind::ExportKw
                            | SyntaxKind::FunctionKw
                            | SyntaxKind::TypeKw
                    ) | None
                ) {
                    self.error_at_current("expected union variant after `|`");
                    break;
                }
                continue;
            }
            if self.at(SyntaxKind::Ident) {
                self.error_at_current("expected `|` between union variants");
                continue;
            }
            break;
        }

        if !self.expect(
            SyntaxKind::Semicolon,
            "expected `;` after union declaration",
        ) {
            self.recover_to(TOP_LEVEL_RECOVERY);
        }
        self.builder.finish_node();
    }

    fn parse_union_variant(&mut self) {
        self.builder.start_node(SyntaxKind::UnionVariant.into());
        self.expect(SyntaxKind::Ident, "expected union variant name");
        self.parse_variant_payload();
        self.builder.finish_node();
    }

    fn parse_variant_payload(&mut self) {
        self.builder.start_node(SyntaxKind::VariantPayload.into());
        if !self.expect(SyntaxKind::LParen, "expected `(` after union variant name") {
            self.builder.finish_node();
            return;
        }

        self.skip_trivia();
        if self.at(SyntaxKind::RParen) {
            self.bump();
            self.builder.finish_node();
            return;
        }

        loop {
            let field_position = self.position;
            self.expect(SyntaxKind::Ident, "expected variant field name");
            self.expect(SyntaxKind::Colon, "expected `:` after variant field name");
            self.parse_type();

            if self.position == field_position {
                self.recover_to(VARIANT_PAYLOAD_RECOVERY);
            }

            self.skip_trivia();
            if self.at(SyntaxKind::Comma) {
                self.bump();
                self.skip_trivia();
                if self.at(SyntaxKind::RParen) {
                    self.error_at_current("expected variant field after `,`");
                    break;
                }
                continue;
            }
            if self.at(SyntaxKind::RParen) {
                break;
            }
            if self.at(SyntaxKind::Ident) {
                self.error_at_current("expected `,` or `)` after variant field");
                continue;
            }

            self.error_at_current("expected `,` or `)` after variant field");
            self.recover_to(VARIANT_PAYLOAD_RECOVERY);
            if self.at(SyntaxKind::Comma) {
                self.bump();
                self.skip_trivia();
                if self.at(SyntaxKind::RParen) {
                    self.error_at_current("expected variant field after `,`");
                    break;
                }
                continue;
            }
            break;
        }

        self.expect(SyntaxKind::RParen, "expected `)` after variant payload");
        self.builder.finish_node();
    }

    fn parse_parameter_list(&mut self) {
        self.builder.start_node(SyntaxKind::ParameterList.into());
        self.skip_trivia();
        let mut can_recover_missing_name = true;

        while self.current().is_some() && !self.parameter_list_is_finished(can_recover_missing_name)
        {
            let position = self.position;
            self.parse_parameter();
            can_recover_missing_name = false;

            if self.position == position {
                self.recover_to(PARAMETER_LIST_RECOVERY);
            }

            self.skip_trivia();
            if self.at(SyntaxKind::Comma) {
                self.bump();
                self.skip_trivia();
                if self.at(SyntaxKind::RParen) {
                    self.error_at_current("expected parameter after `,`");
                    break;
                }
                can_recover_missing_name = true;
            } else if !self.parameter_list_is_finished(false) {
                self.error_at_current("expected `,` or `)` after parameter");
                self.recover_to(PARAMETER_LIST_RECOVERY);
                if self.at(SyntaxKind::Comma) {
                    self.bump();
                    self.skip_trivia();
                    can_recover_missing_name = true;
                }
            }
        }

        self.builder.finish_node();
    }

    fn parse_parameter(&mut self) {
        self.builder.start_node(SyntaxKind::Parameter.into());
        if !self.expect(SyntaxKind::Ident, "expected parameter name")
            && self.at_reserved_parameter_name()
        {
            self.bump();
        }
        self.expect(SyntaxKind::Colon, "expected `:` after parameter name");
        self.parse_type();
        self.builder.finish_node();
    }

    fn parse_type_parameter_list(&mut self) {
        self.skip_trivia();
        if !self.at(SyntaxKind::Lt) {
            return;
        }

        self.builder
            .start_node(SyntaxKind::TypeParameterList.into());
        self.bump();
        self.skip_trivia();

        if self.at_type_list_close() {
            self.error_at_current("expected type parameter");
            self.bump_type_list_close();
            self.builder.finish_node();
            return;
        }

        loop {
            let position = self.position;
            self.builder.start_node(SyntaxKind::TypeParameter.into());
            self.expect(SyntaxKind::Ident, "expected type parameter");
            self.builder.finish_node();

            if self.position == position {
                self.recover_to(TYPE_PARAMETER_RECOVERY);
            }

            self.skip_trivia();
            if self.at(SyntaxKind::Comma) {
                self.bump();
                self.skip_trivia();
                if self.at_type_list_close() {
                    self.error_at_current("expected type parameter after `,`");
                    break;
                }
                if self.type_parameter_list_is_finished() {
                    self.error_at_current("expected type parameter after `,`");
                    break;
                }
                continue;
            }
            if self.at_type_list_close() || self.type_parameter_list_is_finished() {
                break;
            }
            if self.at(SyntaxKind::Ident) {
                self.error_at_current("expected `,` or `>` after type parameter");
                continue;
            }

            self.error_at_current("expected `,` or `>` after type parameter");
            self.recover_to(TYPE_PARAMETER_RECOVERY);
            if self.at(SyntaxKind::Comma) {
                self.bump();
                self.skip_trivia();
                if self.at_type_list_close() {
                    self.error_at_current("expected type parameter after `,`");
                    break;
                }
                continue;
            }
            if self.at(SyntaxKind::Ident) {
                continue;
            }
            break;
        }

        self.expect_type_list_close("expected `>` after type parameters");
        self.builder.finish_node();
    }

    fn parse_type(&mut self) {
        self.skip_trivia();
        let checkpoint = self.builder.checkpoint();
        self.builder.start_node(SyntaxKind::Type.into());

        let current = self.current();
        let parsed = if matches!(
            current,
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
        } else if current == Some(SyntaxKind::LParen) {
            if self.nth_non_trivia(1) == Some(SyntaxKind::LParen) {
                self.parse_parenthesized_function_type();
            } else {
                self.parse_function_type();
            }
            true
        } else {
            self.error_at_current(
                "expected type `Int`, `Bool`, `String`, `Unit`, a named type, or a function type",
            );
            false
        };

        if parsed
            && current == Some(SyntaxKind::Ident)
            && self.nth_non_trivia(0) == Some(SyntaxKind::Lt)
        {
            self.parse_type_argument_list();
        }

        self.builder.finish_node();

        while parsed && self.nth_non_trivia(0) == Some(SyntaxKind::LBracket) {
            self.builder
                .start_node_at(checkpoint, SyntaxKind::ArrayType.into());
            self.expect(SyntaxKind::LBracket, "expected `[` in array type");
            self.expect(SyntaxKind::RBracket, "expected `]` after array type");
            self.builder.finish_node();
        }
    }

    fn parse_function_type(&mut self) {
        self.builder.start_node(SyntaxKind::FunctionType.into());
        self.expect(
            SyntaxKind::LParen,
            "expected `(` before function type parameters",
        );
        self.parse_parameter_list();
        self.expect(
            SyntaxKind::RParen,
            "expected `)` after function type parameters",
        );
        self.expect(SyntaxKind::FatArrow, "expected `=>` in function type");
        self.parse_type();
        self.builder.finish_node();
    }

    fn parse_parenthesized_function_type(&mut self) {
        self.expect(
            SyntaxKind::LParen,
            "expected `(` before parenthesized function type",
        );
        self.parse_function_type();
        self.expect(
            SyntaxKind::RParen,
            "expected `)` after parenthesized function type",
        );
    }

    fn parse_type_argument_list(&mut self) {
        self.skip_trivia();
        self.builder.start_node(SyntaxKind::TypeArgumentList.into());
        self.expect(SyntaxKind::Lt, "expected `<` before type arguments");
        self.skip_trivia();

        if self.at_type_list_close() {
            self.error_at_current("expected type argument");
            self.bump_type_list_close();
            self.builder.finish_node();
            return;
        }

        loop {
            let position = self.position;
            self.parse_type();
            if self.position == position {
                self.recover_to(TYPE_ARGUMENT_RECOVERY);
            }

            self.skip_trivia();
            if self.at(SyntaxKind::Comma) {
                self.bump();
                self.skip_trivia();
                if self.at_type_list_close() {
                    self.error_at_current("expected type argument after `,`");
                    break;
                }
                if self.type_argument_list_is_finished() {
                    self.error_at_current("expected type argument after `,`");
                    break;
                }
                continue;
            }
            if self.at_type_list_close() || self.type_argument_list_is_finished() {
                break;
            }
            if self.current().is_some_and(is_type_start) {
                self.error_at_current("expected `,` or `>` after type argument");
                continue;
            }

            self.error_at_current("expected `,` or `>` after type argument");
            self.recover_to(TYPE_ARGUMENT_RECOVERY);
            if self.at(SyntaxKind::Comma) {
                self.bump();
                self.skip_trivia();
                if self.at_type_list_close() {
                    self.error_at_current("expected type argument after `,`");
                    break;
                }
                continue;
            }
            if self.current().is_some_and(is_type_start) {
                continue;
            }
            break;
        }

        self.expect_type_list_close("expected `>` after type arguments");
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
                Some(SyntaxKind::LetKw) => self.parse_let_declaration(),
                Some(SyntaxKind::IfKw) => self.parse_if_statement(),
                Some(SyntaxKind::WhileKw) => self.parse_while_statement(),
                Some(SyntaxKind::ForKw) => self.parse_for_statement(),
                Some(SyntaxKind::BreakKw) => self.parse_break_statement(),
                Some(SyntaxKind::ContinueKw) => self.parse_continue_statement(),
                Some(SyntaxKind::ReturnKw) => self.parse_return_statement(),
                Some(SyntaxKind::ElseKw) => {
                    self.error_at_current("unexpected `else`");
                    self.bump();
                }
                Some(
                    SyntaxKind::ImportKw
                    | SyntaxKind::ExportKw
                    | SyntaxKind::FunctionKw
                    | SyntaxKind::TypeKw,
                ) => {
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

    fn parse_for_statement(&mut self) {
        self.builder.start_node(SyntaxKind::ForStatement.into());
        self.expect(SyntaxKind::ForKw, "expected `for`");
        self.expect(SyntaxKind::LParen, "expected `(` after `for`");

        self.builder.start_node(SyntaxKind::ForBinding.into());
        if !self.expect(
            SyntaxKind::ConstKw,
            "expected `const` in `for...of` binding",
        ) && self.at(SyntaxKind::LetKw)
        {
            self.bump();
        }
        self.expect(SyntaxKind::Ident, "expected binding name after `const`");
        self.builder.finish_node();

        self.expect_contextual_keyword("of", "expected `of` after `for` binding");
        if !self.parse_expression() {
            self.recover_to(FOR_HEADER_RECOVERY);
        }
        self.expect(SyntaxKind::RParen, "expected `)` after `for...of` header");
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
            Some(SyntaxKind::MatchKw) => {
                self.parse_match_expression();
                true
            }
            Some(SyntaxKind::LParen) => {
                if self.at_arrow_expression() {
                    self.parse_arrow_expression();
                } else {
                    self.builder
                        .start_node(SyntaxKind::ParenthesizedExpression.into());
                    self.bump();
                    let _ = self.parse_expression();
                    self.expect(SyntaxKind::RParen, "expected `)` after expression");
                    self.builder.finish_node();
                }
                true
            }
            _ => {
                self.error_at_current("expected expression");
                false
            }
        }
    }

    fn parse_arrow_expression(&mut self) {
        self.builder.start_node(SyntaxKind::ArrowExpression.into());
        self.expect(SyntaxKind::LParen, "expected `(` before arrow parameters");
        self.parse_parameter_list();
        self.expect(SyntaxKind::RParen, "expected `)` after arrow parameters");
        self.expect(SyntaxKind::Colon, "expected `:` before arrow return type");
        self.parse_type();
        self.expect(
            SyntaxKind::FatArrow,
            "expected `=>` after arrow return type",
        );
        self.skip_trivia();
        self.builder.start_node(SyntaxKind::ArrowBody.into());
        if self.at(SyntaxKind::LBrace) {
            self.parse_block();
        } else {
            let _ = self.parse_expression();
        }
        self.builder.finish_node();
        self.builder.finish_node();
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

    fn parse_match_expression(&mut self) {
        self.builder.start_node(SyntaxKind::MatchExpression.into());
        self.expect(SyntaxKind::MatchKw, "expected `match`");
        let has_left_parenthesis = self.expect(SyntaxKind::LParen, "expected `(` after `match`");
        if has_left_parenthesis || !self.at(SyntaxKind::LBrace) {
            let _ = self.parse_expression();
        } else {
            self.error_at_current("expected match scrutinee");
        }
        self.expect(SyntaxKind::RParen, "expected `)` after match scrutinee");
        if !self.expect(SyntaxKind::LBrace, "expected `{` before match arms") {
            self.builder.finish_node();
            return;
        }

        loop {
            self.skip_trivia();
            match self.current() {
                Some(SyntaxKind::CaseKw | SyntaxKind::DefaultKw) => self.parse_match_arm(),
                Some(SyntaxKind::RBrace) => {
                    self.bump();
                    break;
                }
                Some(
                    SyntaxKind::Semicolon
                    | SyntaxKind::ConstKw
                    | SyntaxKind::LetKw
                    | SyntaxKind::IfKw
                    | SyntaxKind::WhileKw
                    | SyntaxKind::ForKw
                    | SyntaxKind::BreakKw
                    | SyntaxKind::ContinueKw
                    | SyntaxKind::ReturnKw
                    | SyntaxKind::MatchKw
                    | SyntaxKind::ImportKw
                    | SyntaxKind::ExportKw
                    | SyntaxKind::FunctionKw
                    | SyntaxKind::TypeKw,
                ) => {
                    self.error_at_current("expected `}` after match arms");
                    break;
                }
                Some(_) => {
                    self.error_at_current("expected `case`, `default`, or `}` in match");
                    let position = self.position;
                    self.recover_to(MATCH_BODY_RECOVERY);
                    if self.position == position {
                        self.bump();
                    }
                }
                None => {
                    self.error_at_current("expected `}` after match arms");
                    break;
                }
            }
        }

        self.builder.finish_node();
    }

    fn parse_match_arm(&mut self) {
        self.builder.start_node(SyntaxKind::MatchArm.into());
        if self.at(SyntaxKind::CaseKw) {
            self.bump();
            self.parse_variant_pattern();
        } else {
            self.expect(SyntaxKind::DefaultKw, "expected `case` or `default`");
        }

        self.expect(SyntaxKind::FatArrow, "expected `=>` before match arm value");
        if !self.parse_expression() {
            self.recover_to(MATCH_ARM_RECOVERY);
        }
        if !self.expect(SyntaxKind::Semicolon, "expected `;` after match arm") {
            self.recover_to(MATCH_ARM_RECOVERY);
            if self.at(SyntaxKind::Semicolon) {
                self.bump();
            }
        }
        self.builder.finish_node();
    }

    fn parse_variant_pattern(&mut self) {
        self.builder.start_node(SyntaxKind::VariantPattern.into());
        self.expect(SyntaxKind::Ident, "expected union name in pattern");
        self.expect(SyntaxKind::Dot, "expected `.` in variant pattern");
        self.expect(SyntaxKind::Ident, "expected variant name in pattern");
        self.parse_pattern_binding_list();
        self.builder.finish_node();
    }

    fn parse_pattern_binding_list(&mut self) {
        self.builder
            .start_node(SyntaxKind::PatternBindingList.into());
        if !self.expect(SyntaxKind::LParen, "expected `(` after variant pattern") {
            self.builder.finish_node();
            return;
        }

        self.skip_trivia();
        if self.at(SyntaxKind::RParen) {
            self.bump();
            self.builder.finish_node();
            return;
        }

        loop {
            if !self.expect(SyntaxKind::Ident, "expected pattern binding") {
                self.recover_to(PATTERN_BINDING_RECOVERY);
            }

            self.skip_trivia();
            if self.at(SyntaxKind::Comma) {
                self.bump();
                self.skip_trivia();
                if self.at(SyntaxKind::RParen) {
                    self.error_at_current("expected pattern binding after `,`");
                    break;
                }
                continue;
            }
            if self.at(SyntaxKind::RParen) {
                break;
            }
            if self.at(SyntaxKind::Ident) {
                self.error_at_current("expected `,` or `)` after pattern binding");
                continue;
            }

            self.error_at_current("expected `,` or `)` after pattern binding");
            self.recover_to(PATTERN_BINDING_RECOVERY);
            if self.at(SyntaxKind::Comma) {
                self.bump();
                self.skip_trivia();
                if self.at(SyntaxKind::RParen) {
                    self.error_at_current("expected pattern binding after `,`");
                    break;
                }
                continue;
            }
            break;
        }

        self.expect(SyntaxKind::RParen, "expected `)` after pattern bindings");
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
            if TOP_LEVEL_RECOVERY.contains(&kind) {
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

    fn expect_contextual_keyword(&mut self, keyword: &str, message: &'static str) -> bool {
        self.skip_trivia();

        if self.at_contextual_keyword(keyword) {
            self.bump();
            true
        } else {
            self.error_at_current(message);
            false
        }
    }

    fn expect_type_list_close(&mut self, message: &'static str) -> bool {
        self.skip_trivia();
        if self.at_type_list_close() {
            self.bump_type_list_close();
            true
        } else {
            self.error_at_current(message);
            false
        }
    }

    fn at_type_list_close(&self) -> bool {
        matches!(self.current(), Some(SyntaxKind::Gt | SyntaxKind::GtEq))
    }

    fn bump_type_list_close(&mut self) {
        if self.at(SyntaxKind::GtEq) {
            self.builder.token(SyntaxKind::Gt.into(), ">");
            self.pending_split_eq = true;
        } else {
            self.bump();
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
        let range = self.current_range();
        self.diagnostics.push(
            Diagnostic::error(PARSE_ERROR, message)
                .with_label(Label::new(SourceSpan::new(self.file, range), message)),
        );
    }

    fn current(&self) -> Option<SyntaxKind> {
        if self.pending_split_eq {
            Some(SyntaxKind::Eq)
        } else {
            self.tokens.get(self.position).map(Token::kind)
        }
    }

    fn current_range(&self) -> TextRange {
        let Some(token) = self.tokens.get(self.position) else {
            return TextRange::new(self.source_len, self.source_len);
        };
        let range = token.range();
        if self.pending_split_eq {
            TextRange::new(range.start() + 1, range.end())
        } else {
            range
        }
    }

    fn at(&self, kind: SyntaxKind) -> bool {
        self.current() == Some(kind)
    }

    fn at_contextual_keyword(&self, keyword: &str) -> bool {
        !self.pending_split_eq
            && self
                .tokens
                .get(self.position)
                .is_some_and(|token| token.kind() == SyntaxKind::Ident && token.text() == keyword)
    }

    fn at_arrow_expression(&self) -> bool {
        if !self.at(SyntaxKind::LParen) {
            return false;
        }

        let mut depth = 0_usize;
        let mut offset = 0_usize;
        loop {
            match self.nth_non_trivia(offset) {
                Some(SyntaxKind::LParen) => depth += 1,
                Some(SyntaxKind::RParen) => {
                    if depth == 1 {
                        return matches!(
                            self.nth_non_trivia(offset + 1),
                            Some(SyntaxKind::Colon | SyntaxKind::FatArrow)
                        );
                    }
                    depth = depth.saturating_sub(1);
                }
                Some(_) => {}
                None => return false,
            }
            offset += 1;
        }
    }

    fn import_list_is_finished(&self) -> bool {
        matches!(
            self.current(),
            None | Some(
                SyntaxKind::RBrace
                    | SyntaxKind::FromKw
                    | SyntaxKind::String
                    | SyntaxKind::Semicolon
                    | SyntaxKind::ImportKw
                    | SyntaxKind::ExportKw
                    | SyntaxKind::FunctionKw
                    | SyntaxKind::TypeKw
            )
        )
    }

    fn type_parameter_list_is_finished(&self) -> bool {
        matches!(
            self.current(),
            None | Some(
                SyntaxKind::Gt
                    | SyntaxKind::GtEq
                    | SyntaxKind::LParen
                    | SyntaxKind::LBrace
                    | SyntaxKind::RBrace
                    | SyntaxKind::Colon
                    | SyntaxKind::Eq
                    | SyntaxKind::Semicolon
                    | SyntaxKind::ImportKw
                    | SyntaxKind::ExportKw
                    | SyntaxKind::FunctionKw
                    | SyntaxKind::TypeKw
            )
        )
    }

    fn parameter_list_is_finished(&self, can_recover_missing_name: bool) -> bool {
        if self.at_reserved_parameter_name() {
            return false;
        }
        if can_recover_missing_name && self.at(SyntaxKind::Colon) {
            return false;
        }

        matches!(
            self.current(),
            None | Some(
                SyntaxKind::RParen
                    | SyntaxKind::Colon
                    | SyntaxKind::FatArrow
                    | SyntaxKind::LBrace
                    | SyntaxKind::RBrace
                    | SyntaxKind::Eq
                    | SyntaxKind::Semicolon
                    | SyntaxKind::ImportKw
                    | SyntaxKind::ExportKw
                    | SyntaxKind::FunctionKw
                    | SyntaxKind::TypeKw
            )
        )
    }

    fn at_reserved_parameter_name(&self) -> bool {
        matches!(
            self.current(),
            Some(
                SyntaxKind::ImportKw
                    | SyntaxKind::ExportKw
                    | SyntaxKind::FunctionKw
                    | SyntaxKind::TypeKw
            )
        ) && self.nth_non_trivia(1) == Some(SyntaxKind::Colon)
    }

    fn type_argument_list_is_finished(&self) -> bool {
        matches!(
            self.current(),
            None | Some(
                SyntaxKind::Gt
                    | SyntaxKind::GtEq
                    | SyntaxKind::RBracket
                    | SyntaxKind::RParen
                    | SyntaxKind::LBrace
                    | SyntaxKind::RBrace
                    | SyntaxKind::Colon
                    | SyntaxKind::Eq
                    | SyntaxKind::Semicolon
                    | SyntaxKind::Pipe
                    | SyntaxKind::FatArrow
                    | SyntaxKind::ImportKw
                    | SyntaxKind::ExportKw
                    | SyntaxKind::FunctionKw
                    | SyntaxKind::TypeKw
            )
        )
    }

    fn at_assignment_statement(&self) -> bool {
        self.nth_non_trivia(0) == Some(SyntaxKind::Ident)
            && self.nth_non_trivia(1) == Some(SyntaxKind::Eq)
    }

    fn type_declaration_is_union(&self) -> bool {
        let mut offset = 1;
        let mut close_includes_equals = false;
        if self.nth_non_trivia(offset) == Some(SyntaxKind::Ident) {
            offset += 1;
        }
        if self.nth_non_trivia(offset) == Some(SyntaxKind::Lt) {
            offset += 1;
            while let Some(kind) = self.nth_non_trivia(offset) {
                match kind {
                    SyntaxKind::Gt => {
                        offset += 1;
                        break;
                    }
                    SyntaxKind::GtEq => {
                        offset += 1;
                        close_includes_equals = true;
                        break;
                    }
                    SyntaxKind::Eq
                    | SyntaxKind::Semicolon
                    | SyntaxKind::ImportKw
                    | SyntaxKind::ExportKw
                    | SyntaxKind::FunctionKw
                    | SyntaxKind::TypeKw => break,
                    _ => offset += 1,
                }
            }
        }
        if !close_includes_equals && self.nth_non_trivia(offset) == Some(SyntaxKind::Eq) {
            offset += 1;
        }

        matches!(
            self.nth_non_trivia(offset),
            Some(SyntaxKind::Ident | SyntaxKind::Pipe)
        )
    }

    fn nth_non_trivia(&self, offset: usize) -> Option<SyntaxKind> {
        if self.pending_split_eq {
            if offset == 0 {
                return Some(SyntaxKind::Eq);
            }
            return self.tokens[self.position + 1..]
                .iter()
                .filter(|token| !token.kind().is_trivia())
                .nth(offset - 1)
                .map(Token::kind);
        }
        self.tokens[self.position..]
            .iter()
            .filter(|token| !token.kind().is_trivia())
            .nth(offset)
            .map(Token::kind)
    }

    fn bump(&mut self) {
        if self.pending_split_eq {
            self.builder.token(SyntaxKind::Eq.into(), "=");
            self.pending_split_eq = false;
            self.position += 1;
            return;
        }
        let token = &self.tokens[self.position];
        self.builder.token(token.kind().into(), token.text());
        self.position += 1;
    }
}

const fn is_type_start(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::IntKw
            | SyntaxKind::BoolKw
            | SyntaxKind::StringKw
            | SyntaxKind::UnitKw
            | SyntaxKind::LParen
            | SyntaxKind::Ident
    )
}
