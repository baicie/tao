//! Accepted, rejected, and recovery tests for module syntax.

use nexa_parser::parse_source;
use nexa_span::{FileId, TextRange};
use nexa_syntax::{SyntaxKind, SyntaxNode};

const TEST_FILE: FileId = FileId::new(6);

#[test]
fn parse_source_accepts_imports_at_every_top_level_position() {
    let source = r#"import { First } from "./first.nexa";
function before(): Unit {}
import { Second, Third } from "../shared.nexa";
type Value = { inner: Int; };
import { Last } from "./last.nexa";"#;
    let syntax = assert_parse_ok(source);

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::ImportDeclaration),
            count_nodes(&syntax, SyntaxKind::ImportList),
            count_nodes(&syntax, SyntaxKind::FunctionDeclaration),
            count_nodes(&syntax, SyntaxKind::RecordDeclaration),
        ),
        (3, 3, 1, 1)
    );
}

#[test]
fn import_declaration_keeps_names_and_path_in_distinct_cst_positions(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "import // declaration\n { First // first\n, Second } from // source\n \"./\u{8d44}\u{6599}.nexa\";";
    let syntax = assert_parse_ok(source);
    let import = first_node(&syntax, SyntaxKind::ImportDeclaration)?;
    let list = direct_child(&import, SyntaxKind::ImportList)?;

    assert_eq!(
        (
            direct_node_kinds(&import),
            direct_significant_token_kinds(&import),
            direct_significant_token_kinds(&list),
        ),
        (
            vec![SyntaxKind::ImportList],
            vec![
                SyntaxKind::ImportKw,
                SyntaxKind::LBrace,
                SyntaxKind::RBrace,
                SyntaxKind::FromKw,
                SyntaxKind::String,
                SyntaxKind::Semicolon,
            ],
            vec![SyntaxKind::Ident, SyntaxKind::Comma, SyntaxKind::Ident],
        )
    );

    Ok(())
}

#[test]
fn parse_source_accepts_exported_functions_records_and_unions(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"export function answer(): Int { return 42; }
export type Pair = { left: Int; right: Int; };
export type Choice = None() | Some(value: Int);"#;
    let syntax = assert_parse_ok(source);
    let exports = nodes(&syntax, SyntaxKind::ExportedDeclaration);

    assert_eq!(
        exports.iter().map(direct_node_kinds).collect::<Vec<_>>(),
        [
            vec![SyntaxKind::FunctionDeclaration],
            vec![SyntaxKind::RecordDeclaration],
            vec![SyntaxKind::UnionDeclaration],
        ]
    );

    Ok(())
}

#[test]
fn parser_leaves_import_names_paths_and_visibility_to_semantics() {
    assert_parse_ok(
        "import { Same, Same, Missing } from \"bare-or-unresolved\"; function main(): Unit {}",
    );
}

#[test]
fn module_keywords_are_rejected_where_identifiers_are_required(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_parse_rejected_at(
        "function import(): Unit {}",
        "expected function name",
        "import",
        "import".len(),
    )?;
    assert_parse_rejected_at(
        "function read(export: Int): Int { return export; }",
        "expected parameter name",
        "export: Int",
        "export".len(),
    )?;
    assert_parse_rejected_at(
        "function read(): Unit { const from = 1; }",
        "expected binding name",
        "from = 1",
        "from".len(),
    )?;

    Ok(())
}

#[test]
fn reserved_parameter_name_recovers_without_abandoning_the_function() {
    let source = "function read(export: Int): Unit {} function later(): Unit {}";
    let parse = parse_source(TEST_FILE, source);
    let syntax = parse.syntax();
    let messages = parse
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.message())
        .collect::<Vec<_>>();

    assert_eq!(
        (
            messages,
            count_nodes(&syntax, SyntaxKind::FunctionDeclaration)
        ),
        (vec!["expected parameter name"], 2)
    );
    assert_eq!(syntax.to_string(), source);
}

#[test]
fn parse_source_reports_an_empty_import_list_and_keeps_later_items(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "import {} from \"./empty.nexa\"; import { Value } from \"./value.nexa\"; function later(): Unit {}";
    let syntax = assert_parse_rejected_at(source, "expected imported name", "} from", "}".len())?;

    assert_recovered_top_level_counts(&syntax, 2, 1);
    Ok(())
}

#[test]
fn parse_source_reports_a_leading_import_comma_and_keeps_later_items(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "import { , Value } from \"./value.nexa\"; import { Next } from \"./next.nexa\"; function later(): Unit {}";
    let syntax = assert_parse_rejected_at(source, "expected imported name", ", Value", ",".len())?;

    assert_recovered_top_level_counts(&syntax, 2, 1);
    Ok(())
}

#[test]
fn parse_source_reports_a_doubled_import_comma_and_keeps_later_items(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "import { First, , Second } from \"./value.nexa\"; import { Next } from \"./next.nexa\"; function later(): Unit {}";
    let syntax = assert_parse_rejected_at(source, "expected imported name", ", Second", ",".len())?;

    assert_recovered_top_level_counts(&syntax, 2, 1);
    Ok(())
}

#[test]
fn parse_source_reports_a_trailing_import_comma_and_keeps_later_items(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "import { Value, } from \"./value.nexa\"; import { Next } from \"./next.nexa\"; function later(): Unit {}";
    let syntax = assert_parse_rejected_at(
        source,
        "expected imported name after `,`",
        "} from",
        "}".len(),
    )?;

    assert_recovered_top_level_counts(&syntax, 2, 1);
    Ok(())
}

#[test]
fn parse_source_reports_a_missing_import_comma_and_keeps_both_names(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "import { First Second } from \"./value.nexa\"; function later(): Unit {}";
    let syntax = assert_parse_rejected_at(
        source,
        "expected `,` or `}` after imported name",
        "Second }",
        "Second".len(),
    )?;
    let list = first_node(&syntax, SyntaxKind::ImportList)?;

    assert_eq!(
        direct_significant_token_kinds(&list),
        [SyntaxKind::Ident, SyntaxKind::Ident]
    );
    Ok(())
}

#[test]
fn parse_source_reports_a_missing_import_opening_brace_and_keeps_later_items(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "import Value } from \"./value.nexa\"; import { Next } from \"./next.nexa\"; function later(): Unit {}";
    let syntax = assert_parse_rejected_at(
        source,
        "expected `{` after `import`",
        "Value }",
        "Value".len(),
    )?;

    assert_recovered_top_level_counts(&syntax, 2, 1);
    Ok(())
}

#[test]
fn parse_source_reports_a_missing_import_closing_brace_and_keeps_later_items(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "import { Value from \"./value.nexa\"; import { Next } from \"./next.nexa\"; function later(): Unit {}";
    let syntax = assert_parse_rejected_at(
        source,
        "expected `}` after imported names",
        "from \"./value",
        "from".len(),
    )?;

    assert_recovered_top_level_counts(&syntax, 2, 1);
    Ok(())
}

#[test]
fn parse_source_reports_a_missing_from_keyword_and_keeps_later_items(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "import { Value } \"./value.nexa\"; import { Next } from \"./next.nexa\"; function later(): Unit {}";
    let syntax = assert_parse_rejected_at(
        source,
        "expected `from` after imported names",
        "\"./value.nexa\"",
        "\"./value.nexa\"".len(),
    )?;

    assert_recovered_top_level_counts(&syntax, 2, 1);
    Ok(())
}

#[test]
fn parse_source_reports_a_missing_import_path_and_keeps_later_items(
) -> Result<(), Box<dyn std::error::Error>> {
    let source =
        "import { Value } from; import { Next } from \"./next.nexa\"; function later(): Unit {}";
    let syntax =
        assert_parse_rejected_at(source, "expected import path string", "; import", ";".len())?;

    assert_recovered_top_level_counts(&syntax, 2, 1);
    Ok(())
}

#[test]
fn parse_source_reports_a_missing_import_semicolon_and_keeps_later_items(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "import { Value } from \"./value.nexa\" import { Next } from \"./next.nexa\"; function later(): Unit {}";
    let syntax = assert_parse_rejected_at(
        source,
        "expected `;` after import declaration",
        "import { Next",
        "import".len(),
    )?;

    assert_recovered_top_level_counts(&syntax, 2, 1);
    Ok(())
}

#[test]
fn parse_source_rejects_export_import_and_recovers_the_import(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "export import { Value } from \"./value.nexa\"; function later(): Unit {}";
    let syntax = assert_parse_rejected_at(
        source,
        "expected `function` or `type` after `export`",
        "import { Value",
        "import".len(),
    )?;

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::ExportedDeclaration),
            count_nodes(&syntax, SyntaxKind::ImportDeclaration),
            count_nodes(&syntax, SyntaxKind::FunctionDeclaration),
        ),
        (1, 1, 1)
    );
    Ok(())
}

#[test]
fn parse_source_rejects_repeated_export_and_keeps_the_declaration(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "export export function value(): Int { return 1; } function later(): Unit {}";
    let syntax = assert_parse_rejected_at(
        source,
        "expected `function` or `type` after `export`",
        "export function",
        "export".len(),
    )?;

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::ExportedDeclaration),
            count_nodes(&syntax, SyntaxKind::FunctionDeclaration),
        ),
        (2, 2)
    );
    Ok(())
}

#[test]
fn parse_source_rejects_a_local_declaration_after_export_and_keeps_later_items(
) -> Result<(), Box<dyn std::error::Error>> {
    let source =
        "export const value = 1; import { Next } from \"./next.nexa\"; function later(): Unit {}";
    let syntax = assert_parse_rejected_at(
        source,
        "expected `function` or `type` after `export`",
        "const value",
        "const".len(),
    )?;

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::ImportDeclaration),
            count_nodes(&syntax, SyntaxKind::FunctionDeclaration),
        ),
        (1, 1)
    );
    Ok(())
}

#[test]
fn parse_source_reports_a_missing_export_target_at_eof() -> Result<(), Box<dyn std::error::Error>> {
    let source = "export";
    let syntax = assert_parse_rejected_at(
        source,
        "expected `function` or `type` after `export`",
        "",
        0,
    )?;

    assert_eq!(count_nodes(&syntax, SyntaxKind::ExportedDeclaration), 1);
    Ok(())
}

#[test]
fn malformed_exported_declaration_keeps_the_next_import_and_declaration(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "export function (): Unit {} import { Value } from \"./value.nexa\"; function later(): Unit {}";
    let syntax = assert_parse_rejected_at(source, "expected function name", "()", "(".len())?;

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::ImportDeclaration),
            count_nodes(&syntax, SyntaxKind::FunctionDeclaration),
        ),
        (1, 2)
    );
    Ok(())
}

#[test]
fn missing_function_brace_recovers_into_a_following_import(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "function open(): Unit { const value = 1; import { Value } from \"./value.nexa\"; function later(): Unit {}";
    let syntax = assert_parse_rejected_at(
        source,
        "expected `}` to close block",
        "import { Value",
        "import".len(),
    )?;

    assert_eq!(
        (
            count_nodes(&syntax, SyntaxKind::ImportDeclaration),
            count_nodes(&syntax, SyntaxKind::FunctionDeclaration),
        ),
        (1, 2)
    );
    Ok(())
}

fn assert_parse_ok(source: &str) -> SyntaxNode {
    let parse = parse_source(TEST_FILE, source);
    assert!(
        parse.is_ok(),
        "diagnostics: {:?}",
        parse
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.message())
            .collect::<Vec<_>>()
    );
    let syntax = parse.syntax();
    assert_eq!(syntax.to_string(), source);
    syntax
}

fn assert_parse_rejected_at(
    source: &str,
    expected_message: &str,
    occurrence: &str,
    width: usize,
) -> Result<SyntaxNode, std::io::Error> {
    let parse = parse_source(TEST_FILE, source);
    let expected_start = if occurrence.is_empty() {
        source.len()
    } else {
        source.find(occurrence).ok_or_else(|| {
            std::io::Error::other(format!("expected `{occurrence}` in test source"))
        })?
    };
    let diagnostic = parse
        .diagnostics()
        .iter()
        .find(|diagnostic| diagnostic.message() == expected_message)
        .ok_or_else(|| {
            std::io::Error::other(format!(
                "expected diagnostic `{expected_message}`, got {:?}",
                parse
                    .diagnostics()
                    .iter()
                    .map(|diagnostic| diagnostic.message())
                    .collect::<Vec<_>>()
            ))
        })?;
    let label = diagnostic
        .labels()
        .first()
        .ok_or_else(|| std::io::Error::other("expected a primary label"))?;

    assert_eq!(diagnostic.code().as_str(), "E1001");
    assert_eq!(
        label.span().range(),
        TextRange::new(expected_start, expected_start + width)
    );
    let syntax = parse.syntax();
    assert_eq!(syntax.to_string(), source);

    Ok(syntax)
}

fn assert_recovered_top_level_counts(syntax: &SyntaxNode, imports: usize, functions: usize) {
    assert_eq!(
        (
            count_nodes(syntax, SyntaxKind::ImportDeclaration),
            count_nodes(syntax, SyntaxKind::FunctionDeclaration),
        ),
        (imports, functions)
    );
}

fn nodes(syntax: &SyntaxNode, kind: SyntaxKind) -> Vec<SyntaxNode> {
    syntax
        .descendants()
        .filter(|node| node.kind() == kind)
        .collect()
}

fn count_nodes(syntax: &SyntaxNode, kind: SyntaxKind) -> usize {
    nodes(syntax, kind).len()
}

fn first_node(syntax: &SyntaxNode, kind: SyntaxKind) -> Result<SyntaxNode, std::io::Error> {
    syntax
        .descendants()
        .find(|node| node.kind() == kind)
        .ok_or_else(|| std::io::Error::other(format!("expected {kind:?} node")))
}

fn direct_child(node: &SyntaxNode, kind: SyntaxKind) -> Result<SyntaxNode, std::io::Error> {
    node.children()
        .find(|child| child.kind() == kind)
        .ok_or_else(|| std::io::Error::other(format!("expected direct {kind:?} child")))
}

fn direct_node_kinds(node: &SyntaxNode) -> Vec<SyntaxKind> {
    node.children().map(|child| child.kind()).collect()
}

fn direct_significant_token_kinds(node: &SyntaxNode) -> Vec<SyntaxKind> {
    node.children_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .map(|token| token.kind())
        .collect()
}
