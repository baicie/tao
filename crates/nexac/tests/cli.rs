//! CLI integration tests for nexac.

use std::path::PathBuf;
use std::process::Command;

#[test]
fn nexac_reports_its_compiler_package_version() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("--version")
        .output()?;

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("nexac {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn nexac_check_accepts_a_language_core_program() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("check")
        .arg(fixture("accepted/language_core.nexa"))
        .output()?;

    assert!(
        output.status.success(),
        "nexac check failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ok\n");

    Ok(())
}

#[test]
fn nexac_check_accepts_stateful_control_flow() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("check")
        .arg(fixture("accepted/stateful_control_flow.nexa"))
        .output()?;

    assert!(
        output.status.success(),
        "nexac check failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ok\n");

    Ok(())
}

#[test]
fn nexac_check_rejects_invalid_syntax() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("check")
        .arg(fixture("rejected/missing_function_name.nexa"))
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(stderr.contains("E1001"), "stderr: {stderr}");
    assert!(
        stderr.contains("expected function name"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains(":1:10:"), "stderr: {stderr}");

    Ok(())
}

#[test]
fn nexac_check_rejects_invalid_semantics() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("check")
        .arg(fixture("rejected/non_boolean_condition.nexa"))
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(stderr.contains("E3002"), "stderr: {stderr}");
    assert!(
        stderr.contains("if condition must have type `Bool`"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains(":2:7:"), "stderr: {stderr}");

    Ok(())
}

#[test]
fn nexac_check_rejects_an_undefined_name() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects("rejected/undefined_name.nexa", "E2001")
}

#[test]
fn nexac_check_rejects_a_duplicate_binding() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects("rejected/duplicate_binding.nexa", "E2002")
}

#[test]
fn nexac_check_rejects_an_incorrect_call_arity() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects("rejected/incorrect_call_arity.nexa", "E2003")
}

#[test]
fn nexac_check_rejects_a_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects("rejected/type_mismatch.nexa", "E3001")
}

#[test]
fn nexac_check_rejects_an_invalid_return() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects("rejected/invalid_return.nexa", "E3003")
}

#[test]
fn nexac_check_rejects_immutable_assignments() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/immutable_assignment.nexa", "E2004", ":2:3:")
}

#[test]
fn nexac_check_rejects_break_outside_a_loop() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/break_outside_loop.nexa", "E3004", ":2:3:")
}

#[test]
fn nexac_check_rejects_continue_outside_a_loop() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/continue_outside_loop.nexa", "E3004", ":2:3:")
}

#[test]
fn nexac_check_rejects_an_assignment_type_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects("rejected/assignment_type_mismatch.nexa", "E3001")
}

#[test]
fn nexac_check_rejects_a_non_boolean_while_condition() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects("rejected/non_boolean_while.nexa", "E3002")
}

#[test]
fn nexac_check_rejects_an_untyped_empty_array() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects("rejected/empty_array_without_context.nexa", "E3001")
}

#[test]
fn nexac_check_rejects_mixed_array_elements() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects("rejected/mixed_array_elements.nexa", "E3001")
}

#[test]
fn nexac_check_rejects_an_unknown_array_member() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects("rejected/unknown_member.nexa", "E2005")
}

#[test]
fn nexac_check_rejects_an_invalid_string_escape() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects("rejected/invalid_string_escape.nexa", "E1001")
}

#[test]
fn nexac_check_rejects_array_element_assignment() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects("rejected/array_element_assignment.nexa", "E1001")
}

#[test]
fn nexac_check_rejects_a_non_integer_array_index() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects("rejected/non_integer_index.nexa", "E3001")
}

#[test]
fn nexac_check_rejects_array_equality() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects("rejected/array_equality.nexa", "E3001")
}

#[test]
fn nexac_check_rejects_an_invalid_main_argument_type() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects("rejected/invalid_main_arguments.nexa", "E3003")
}

#[test]
fn nexac_check_accepts_nominal_records() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("check")
        .arg(fixture("accepted/named_records.nexa"))
        .output()?;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ok\n");

    Ok(())
}

#[test]
fn nexac_check_accepts_recursive_tagged_unions_and_exhaustive_match(
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("check")
        .arg(fixture("accepted/tagged_unions.nexa"))
        .output()?;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ok\n");

    Ok(())
}

#[test]
fn nexac_check_and_run_execute_generic_records_unions_and_error_values(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = fixture("accepted/generics.nexa");
    let checked = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("check")
        .arg(&source)
        .output()?;

    assert!(
        checked.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&checked.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&checked.stdout), "ok\n");

    let run = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("run")
        .arg(source)
        .output()?;

    assert!(
        run.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "42\n");

    Ok(())
}

#[test]
fn nexac_check_and_run_execute_the_multi_file_generic_conformance_program(
) -> Result<(), Box<dyn std::error::Error>> {
    let entry = fixture("accepted/generic_modules/main.nexa");
    let checked = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("check")
        .arg(&entry)
        .output()?;

    assert!(
        checked.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&checked.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&checked.stdout), "ok\n");

    let run = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("run")
        .arg(entry)
        .output()?;

    assert!(
        run.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "42\nnexa\n2\n");

    Ok(())
}

#[test]
fn nexac_check_and_run_execute_the_v0_8_multi_file_conformance_program(
) -> Result<(), Box<dyn std::error::Error>> {
    let entry = fixture("accepted/practical_core/main.nexa");
    let checked = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("check")
        .arg(&entry)
        .output()?;

    assert!(
        checked.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&checked.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&checked.stdout), "ok\n");

    let run = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("run")
        .arg(entry)
        .arg("--")
        .arg("20")
        .output()?;

    assert!(
        run.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "42\n5\n");

    Ok(())
}

#[test]
fn nexac_check_and_run_preserve_one_generic_definition_across_a_diamond(
) -> Result<(), Box<dyn std::error::Error>> {
    let entry = fixture("accepted/generic_diamond/main.nexa");
    let checked = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("check")
        .arg(&entry)
        .output()?;

    assert!(
        checked.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&checked.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&checked.stdout), "ok\n");

    let run = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("run")
        .arg(entry)
        .output()?;

    assert!(
        run.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "42\n");

    Ok(())
}

#[test]
fn nexac_check_rejects_a_missing_generic_type_argument() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/generic_type_arity.nexa", "E2003", ":4:16:")
}

#[test]
fn nexac_check_rejects_an_unconstrained_type_parameter() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at(
        "rejected/unconstrained_type_parameter.nexa",
        "E3008",
        ":1:14:",
    )
}

#[test]
fn nexac_check_rejects_unresolved_generic_inference() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at(
        "rejected/unresolved_generic_inference.nexa",
        "E3009",
        ":4:17:",
    )
}

#[test]
fn nexac_check_rejects_expanding_generic_recursion() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at(
        "rejected/expanding_generic_recursion.nexa",
        "E3010",
        ":3:17:",
    )
}

#[test]
fn nexac_check_renders_both_labels_for_a_mutable_closure_capture(
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("check")
        .arg(fixture("rejected/v08_mutable_capture.nexa"))
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(stderr.contains("E3011"), "stderr: {stderr}");
    assert!(
        stderr.contains("v08_mutable_capture.nexa:3:38"),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains("v08_mutable_capture.nexa:2:7"),
        "stderr: {stderr}"
    );

    Ok(())
}

#[test]
fn nexac_check_renders_both_files_for_an_imported_generic_function_value(
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("check")
        .arg(fixture("rejected/v08_generic_value/main.nexa"))
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(stderr.contains("E3012"), "stderr: {stderr}");
    assert!(stderr.contains("main.nexa:4:42"), "stderr: {stderr}");
    assert!(stderr.contains("support.nexa:1:17"), "stderr: {stderr}");

    Ok(())
}

#[test]
fn nexac_check_renders_both_labels_for_a_local_generic_function_value(
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("check")
        .arg(fixture("rejected/v08_local_generic_value.nexa"))
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(stderr.contains("E3012"), "stderr: {stderr}");
    assert!(
        stderr.contains("v08_local_generic_value.nexa:6:42"),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains("v08_local_generic_value.nexa:1:10"),
        "stderr: {stderr}"
    );

    Ok(())
}

#[test]
fn nexac_check_rejects_a_function_signature_mismatch_at_the_function_value(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/v08_function_mismatch.nexa", "E3001", ":5:44:")
}

#[test]
fn nexac_check_rejects_a_non_array_for_of_iterable_at_the_iterable(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/v08_non_array_for.nexa", "E3001", ":2:23:")
}

#[test]
fn nexac_check_renders_both_labels_for_assignment_to_a_for_of_binding(
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("check")
        .arg(fixture("rejected/v08_for_binding_assignment.nexa"))
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(stderr.contains("E2004"), "stderr: {stderr}");
    assert!(
        stderr.contains("v08_for_binding_assignment.nexa:3:5"),
        "stderr: {stderr}"
    );
    assert!(
        stderr.contains("v08_for_binding_assignment.nexa:2:14"),
        "stderr: {stderr}"
    );

    Ok(())
}

#[test]
fn nexac_check_rejects_array_intrinsic_extraction_at_the_member_name(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/v08_intrinsic_extraction.nexa", "E2005", ":3:25:")
}

#[test]
fn nexac_check_rejects_array_intrinsic_arity_at_the_complete_call(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/v08_intrinsic_arity.nexa", "E2003", ":3:3:")
}

#[test]
fn nexac_check_rejects_array_intrinsic_type_mismatch_at_the_argument(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at(
        "rejected/v08_intrinsic_type_mismatch.nexa",
        "E3001",
        ":3:18:",
    )
}

#[test]
fn nexac_check_rejects_a_missing_record_field() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/missing_record_field.nexa", "E2006", ":3:22:")
}

#[test]
fn nexac_check_rejects_recursive_records() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/recursive_record.nexa", "E3005", ":1:25:")
}

#[test]
fn nexac_check_rejects_an_unknown_record_field() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/unknown_record_field.nexa", "E2005", ":3:37:")
}

#[test]
fn nexac_check_rejects_record_field_assignment() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/record_field_assignment.nexa", "E1001", ":4:13:")
}

#[test]
fn nexac_check_rejects_type_as_a_v0_3_identifier() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/reserved_type_identifier.nexa", "E1001", ":1:10:")
}

#[test]
fn nexac_check_rejects_an_unknown_union_variant() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/unknown_union_variant.nexa", "E2005", ":3:24:")
}

#[test]
fn nexac_check_rejects_an_unknown_union_qualifier() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/unknown_union_qualifier.nexa", "E2001", ":2:17:")
}

#[test]
fn nexac_check_rejects_an_unknown_pattern_variant() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/unknown_pattern_variant.nexa", "E2005", ":4:17:")
}

#[test]
fn nexac_check_rejects_an_incorrect_constructor_arity() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at(
        "rejected/incorrect_constructor_arity.nexa",
        "E2003",
        ":3:21:",
    )
}

#[test]
fn nexac_check_rejects_an_incorrect_pattern_arity() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/incorrect_pattern_arity.nexa", "E2003", ":4:15:")
}

#[test]
fn nexac_check_rejects_a_non_exhaustive_match() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/non_exhaustive_match.nexa", "E3006", ":3:10:")
}

#[test]
fn nexac_check_rejects_a_duplicate_variant_case() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/duplicate_variant_case.nexa", "E2002", ":5:15:")
}

#[test]
fn nexac_check_rejects_a_default_after_complete_match_coverage(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/unreachable_match_default.nexa", "E3007", ":6:5:")
}

#[test]
fn nexac_check_rejects_a_case_after_a_match_default() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/unreachable_match_case.nexa", "E3007", ":5:5:")
}

#[test]
fn nexac_check_rejects_a_foreign_union_case() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/foreign_union_case.nexa", "E3001", ":5:10:")
}

#[test]
fn nexac_check_rejects_a_non_union_match_scrutinee() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/non_union_match.nexa", "E3001", ":2:25:")
}

#[test]
fn nexac_check_rejects_a_missing_match_arrow() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/malformed_match_arrow.nexa", "E1001", ":5:21:")
}

#[test]
fn nexac_check_rejects_match_as_an_identifier() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/reserved_match_identifier.nexa", "E1001", ":1:10:")
}

#[test]
fn nexac_check_rejects_case_as_an_identifier() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at("rejected/reserved_case_identifier.nexa", "E1001", ":1:10:")
}

#[test]
fn nexac_check_rejects_default_as_an_identifier() -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_at(
        "rejected/reserved_default_identifier.nexa",
        "E1001",
        ":1:10:",
    )
}

#[test]
fn nexac_run_executes_main_and_prints_its_output() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("run")
        .arg(fixture("accepted/language_core.nexa"))
        .output()?;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "42\n");

    Ok(())
}

#[test]
fn nexac_run_executes_stateful_control_flow() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("run")
        .arg(fixture("accepted/stateful_control_flow.nexa"))
        .output()?;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "12\n");

    Ok(())
}

#[test]
fn nexac_run_executes_immutable_data_with_cli_arguments() -> Result<(), Box<dyn std::error::Error>>
{
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("run")
        .arg(fixture("accepted/immutable_data.nexa"))
        .arg("--")
        .arg("Nexa")
        .output()?;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "Nexa\n42\n");

    Ok(())
}

#[test]
fn nexac_run_executes_nominal_records() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("run")
        .arg(fixture("accepted/named_records.nexa"))
        .output()?;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "Ada\n42\n");

    Ok(())
}

#[test]
fn nexac_run_executes_recursive_tagged_unions_and_exhaustive_match(
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("run")
        .arg(fixture("accepted/tagged_unions.nexa"))
        .output()?;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "42\n");

    Ok(())
}

#[test]
fn nexac_run_preserves_multiple_cli_arguments_after_the_separator(
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("run")
        .arg(fixture("accepted/cli_arguments.nexa"))
        .arg("--")
        .arg("first")
        .arg("two words")
        .arg("--literal")
        .output()?;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "3\nfirst\ntwo words\n--literal\n"
    );

    Ok(())
}

#[test]
fn nexac_run_passes_an_empty_argument_array_to_main() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("run")
        .arg(fixture("accepted/cli_arguments.nexa"))
        .output()?;

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "0\n");

    Ok(())
}

#[test]
fn nexac_run_rejects_arguments_for_a_parameterless_main() -> Result<(), Box<dyn std::error::Error>>
{
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("run")
        .arg(fixture("accepted/language_core.nexa"))
        .arg("--")
        .arg("unexpected")
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(
        stderr.contains("parameterless `main` does not accept command-line arguments"),
        "stderr: {stderr}"
    );

    Ok(())
}

#[test]
fn nexac_run_reports_runtime_failures_with_a_source_location(
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("run")
        .arg(fixture("rejected/division_by_zero.nexa"))
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(
        stderr.contains("runtime error: division by zero"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains(":2:9"), "stderr: {stderr}");

    Ok(())
}

#[test]
fn nexac_run_keeps_output_emitted_before_a_runtime_failure(
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("run")
        .arg(fixture("rejected/output_before_runtime_failure.nexa"))
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "1\n");
    assert!(
        stderr.contains("runtime error: division by zero"),
        "stderr: {stderr}"
    );

    Ok(())
}

#[test]
fn nexac_run_reports_the_execution_step_limit_with_a_source_location(
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("run")
        .arg(fixture("rejected/execution_step_limit.nexa"))
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "1\n");
    assert!(
        stderr.contains("runtime error: execution step limit of 100000 exceeded"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains(":3:15"), "stderr: {stderr}");

    Ok(())
}

#[test]
fn nexac_run_reports_array_bounds_and_preserves_output() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("run")
        .arg(fixture("rejected/array_out_of_bounds.nexa"))
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "before\n");
    assert!(
        stderr.contains("runtime error: array index 1 out of bounds for length 1"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains(":4:9"), "stderr: {stderr}");

    Ok(())
}

#[test]
fn nexac_run_reports_malformed_parse_int_at_the_complete_call_and_preserves_output(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_run_rejects_at(
        "rejected/v08_parse_int_malformed.nexa",
        "7\n",
        "parseInt expected a complete ASCII decimal integer",
        "v08_parse_int_malformed.nexa:3:17",
    )
}

#[test]
fn nexac_run_reports_parse_int_overflow_at_the_complete_call_and_preserves_output(
) -> Result<(), Box<dyn std::error::Error>> {
    assert_run_rejects_at(
        "rejected/v08_parse_int_overflow.nexa",
        "7\n",
        "parseInt result is outside the Int range",
        "v08_parse_int_overflow.nexa:3:17",
    )
}

#[test]
fn nexac_parse_prints_the_concrete_syntax_tree() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("parse")
        .arg(fixture("accepted/language_core.nexa"))
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.starts_with("SourceFile@0.."), "stdout: {stdout}");
    assert!(
        stdout.contains("FunctionDeclaration@0.."),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("ConstDeclaration@"), "stdout: {stdout}");
    assert!(stdout.contains("CallExpression@"), "stdout: {stdout}");

    Ok(())
}

#[test]
fn nexac_parse_reports_invalid_syntax() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("parse")
        .arg(fixture("rejected/missing_function_name.nexa"))
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(
        stdout.contains("FunctionDeclaration@0.."),
        "stdout: {stdout}"
    );
    assert!(stderr.contains("E1001"), "stderr: {stderr}");
    assert!(
        stderr.contains("expected function name"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains(":1:10:"), "stderr: {stderr}");

    Ok(())
}

#[test]
fn nexac_check_and_run_execute_a_multi_file_module_graph() -> Result<(), Box<dyn std::error::Error>>
{
    let entry = fixture("accepted/modules/main.nexa");
    let checked = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("check")
        .arg(&entry)
        .output()?;
    assert!(
        checked.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&checked.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&checked.stdout), "ok\n");

    let run = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("run")
        .arg(entry)
        .output()?;
    assert!(
        run.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout), "42\n");

    Ok(())
}

#[test]
fn nexac_parse_does_not_load_a_syntactically_valid_import() -> Result<(), Box<dyn std::error::Error>>
{
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("parse")
        .arg(fixture("accepted/parse_missing_import.nexa"))
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("ImportDeclaration@"), "stdout: {stdout}");

    Ok(())
}

#[test]
fn nexac_check_reports_each_module_diagnostic_class() -> Result<(), Box<dyn std::error::Error>> {
    for (fixture_path, code) in [
        ("rejected/modules/missing_main.nexa", "E4001"),
        ("rejected/modules/cycle_left.nexa", "E4002"),
        ("rejected/modules/unknown_main.nexa", "E4003"),
        ("rejected/modules/private_main.nexa", "E4004"),
        ("rejected/modules/export_collision_main.nexa", "E4005"),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
            .arg("check")
            .arg(fixture(fixture_path))
            .output()?;
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(!output.status.success(), "fixture: {fixture_path}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), "");
        assert!(stderr.contains(code), "fixture: {fixture_path}; {stderr}");
    }

    Ok(())
}

#[test]
fn nexac_run_renders_dependency_runtime_locations_and_prior_output(
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("run")
        .arg(fixture("rejected/modules/runtime_main.nexa"))
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "7\n8\n");
    assert!(stderr.contains("division by zero"), "stderr: {stderr}");
    assert!(
        stderr.contains("runtime_library.nexa:3:19"),
        "stderr: {stderr}"
    );

    Ok(())
}

#[test]
fn nexac_run_does_not_select_an_imported_main_as_entry() -> Result<(), Box<dyn std::error::Error>> {
    let entry = fixture("rejected/modules/no_entry_main.nexa");
    let checked = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("check")
        .arg(&entry)
        .output()?;
    assert!(
        checked.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&checked.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&checked.stdout), "ok\n");

    let run = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("run")
        .arg(entry)
        .output()?;
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(!run.status.success());
    assert_eq!(String::from_utf8_lossy(&run.stdout), "");
    assert!(
        stderr.contains("program has no `main` entry point"),
        "stderr: {stderr}"
    );

    Ok(())
}

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(path)
}

fn assert_check_rejects(
    fixture_path: &str,
    diagnostic_code: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_with_location(fixture_path, diagnostic_code, None)
}

fn assert_check_rejects_at(
    fixture_path: &str,
    diagnostic_code: &str,
    source_location: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_check_rejects_with_location(fixture_path, diagnostic_code, Some(source_location))
}

fn assert_check_rejects_with_location(
    fixture_path: &str,
    diagnostic_code: &str,
    source_location: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("check")
        .arg(fixture(fixture_path))
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(stderr.contains(diagnostic_code), "stderr: {stderr}");
    if let Some(source_location) = source_location {
        assert!(stderr.contains(source_location), "stderr: {stderr}");
    }

    Ok(())
}

fn assert_run_rejects_at(
    fixture_path: &str,
    expected_stdout: &str,
    runtime_message: &str,
    source_location: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_nexac"))
        .arg("run")
        .arg(fixture(fixture_path))
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let rendered_message = format!("runtime error: {runtime_message}");

    assert!(!output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected_stdout);
    assert!(stderr.contains(&rendered_message), "stderr: {stderr}");
    assert!(stderr.contains(source_location), "stderr: {stderr}");

    Ok(())
}
