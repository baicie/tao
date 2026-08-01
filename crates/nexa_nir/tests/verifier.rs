//! Accepted and rejected verifier contracts for target-neutral NIR.

// Rust 1.80 predates stable `#[expect]`; invalid test setup should stop immediately.
#![allow(clippy::expect_used)]

use nexa_nir::{
    BlockId, FunctionId, InstructionId, IntrinsicId, ModuleBuilder, NirSpan, NirType, Operation,
    Terminator, TypeId, TypedValue, ValueId, VerificationCode, Verifier,
};

const I1: TypeId = TypeId::new(0);
const I64: TypeId = TypeId::new(1);
const UNIT: TypeId = TypeId::new(2);
const OWNED_I64: TypeId = TypeId::new(3);
const OWNED_STRUCT: TypeId = TypeId::new(4);
const MAIN: FunctionId = FunctionId::new(0);
const ENTRY: BlockId = BlockId::new(0);
const SPAN: NirSpan = NirSpan::new(0, 0, 1);

fn minimal_module() -> ModuleBuilder {
    let mut module = ModuleBuilder::new("example").expect("module identity is valid");
    module
        .define_type(I1, NirType::I1)
        .expect("type id is unique");
    module
        .define_type(I64, NirType::I64)
        .expect("type id is unique");
    module
        .define_type(UNIT, NirType::Unit)
        .expect("type id is unique");
    module
        .define_type(OWNED_I64, NirType::OwnedPtr { pointee: I64 })
        .expect("type id is unique");
    module
        .define_type(
            OWNED_STRUCT,
            NirType::Struct {
                fields: vec![OWNED_I64],
            },
        )
        .expect("type id is unique");
    module
}

fn returning_integer() -> nexa_nir::UnverifiedModule {
    let mut module = minimal_module();
    module
        .define_function(MAIN, "main", [], I64, ENTRY)
        .expect("function definition is valid");
    module
        .define_block(MAIN, ENTRY, [])
        .expect("entry block is unique");
    module
        .append_instruction(
            MAIN,
            ENTRY,
            InstructionId::new(0),
            Some(TypedValue::new(ValueId::new(0), I64)),
            Operation::ConstI64 { value: 42 },
            SPAN,
        )
        .expect("instruction is valid");
    module
        .set_terminator(
            MAIN,
            ENTRY,
            Terminator::Return {
                value: Some(ValueId::new(0)),
            },
            SPAN,
        )
        .expect("terminator is unique");
    module.finish()
}

fn mutate(
    module: &nexa_nir::UnverifiedModule,
    mutation: impl FnOnce(&mut serde_json::Value),
) -> nexa_nir::UnverifiedModule {
    let mut value = serde_json::to_value(module).expect("module serializes");
    mutation(&mut value);
    serde_json::from_value(value).expect("mutation preserves the JSON shape")
}

#[test]
fn verifier_accepts_a_minimal_typed_module() {
    let verified = Verifier::verify(returning_integer()).expect("minimal NIR should verify");

    assert_eq!(verified.module_id(), "example");
}

#[test]
fn builder_rejects_duplicate_ids() {
    let mut module = minimal_module();

    let error = module
        .define_type(I64, NirType::I64)
        .expect_err("duplicate type ids must fail");

    assert_eq!(error.code(), "E5001");
}

#[test]
fn builder_rejects_invalid_handle_kinds() {
    let mut module = minimal_module();

    let error = module
        .define_type(
            TypeId::new(5),
            NirType::Handle {
                handle_kind: "../host-file".to_owned(),
                ownership: nexa_nir::ValueOwnership::Owned,
            },
        )
        .expect_err("handle registry identities must be portable");

    assert_eq!(error.code(), "E5000");
}

#[test]
fn verifier_rejects_invalid_module_identifiers() {
    let module = mutate(&returning_integer(), |value| {
        value["moduleId"] = serde_json::json!("../escape");
    });

    let error =
        Verifier::verify(module).expect_err("builder checks must be independently enforced");

    assert_eq!(error.code(), VerificationCode::InvalidIdentity);
}

#[test]
fn verifier_rejects_invalid_deserialized_handle_kinds() {
    let mut module = ModuleBuilder::new("handle-module").expect("module identity is valid");
    module
        .define_type(
            TypeId::new(0),
            NirType::Handle {
                handle_kind: "host-file".to_owned(),
                ownership: nexa_nir::ValueOwnership::Owned,
            },
        )
        .expect("handle kind is valid");
    let module = mutate(&module.finish(), |value| {
        value["types"][0]["ty"]["handle_kind"] = serde_json::json!("../host-file");
    });

    let error =
        Verifier::verify(module).expect_err("artifact handles must be independently checked");

    assert_eq!(error.code(), VerificationCode::InvalidIdentity);
}

#[test]
fn verifier_rejects_reversed_source_spans() {
    let module = mutate(&returning_integer(), |value| {
        value["functions"][0]["blocks"][0]["instructions"][0]["span"] = serde_json::json!({
            "source": 0,
            "start": 7,
            "end": 3
        });
    });

    let error = Verifier::verify(module).expect_err("source spans must be half-open ranges");

    assert_eq!(error.code(), VerificationCode::InvalidInstruction);
}

#[test]
fn verifier_rejects_entry_block_parameters() {
    let mut module = minimal_module();
    module
        .define_function(MAIN, "main", [], I64, ENTRY)
        .expect("function definition is valid");
    module
        .define_block(MAIN, ENTRY, [TypedValue::new(ValueId::new(0), I64)])
        .expect("entry block is unique");
    module
        .set_terminator(
            MAIN,
            ENTRY,
            Terminator::Return {
                value: Some(ValueId::new(0)),
            },
            SPAN,
        )
        .expect("terminator is unique");

    let error = Verifier::verify(module.finish())
        .expect_err("entry block parameters have no predecessor to define them");

    assert_eq!(error.code(), VerificationCode::InvalidIdentity);
}

#[test]
fn verifier_rejects_control_flow_back_to_the_entry_block() {
    let mut module = minimal_module();
    module
        .define_function(
            MAIN,
            "main",
            [TypedValue::new(ValueId::new(0), OWNED_I64)],
            UNIT,
            ENTRY,
        )
        .expect("function definition is valid");
    module
        .define_block(MAIN, ENTRY, [])
        .expect("entry block is unique");
    module
        .append_instruction(
            MAIN,
            ENTRY,
            InstructionId::new(0),
            None,
            Operation::Drop {
                value: ValueId::new(0),
            },
            SPAN,
        )
        .expect("instruction is valid");
    module
        .set_terminator(
            MAIN,
            ENTRY,
            Terminator::Goto {
                target: ENTRY,
                arguments: vec![],
            },
            SPAN,
        )
        .expect("terminator is unique");

    let error = Verifier::verify(module.finish())
        .expect_err("the function entry cannot reactivate consumed parameters");

    assert_eq!(error.code(), VerificationCode::InvalidIdentity);
}

#[test]
fn verifier_rejects_unknown_type_ids() {
    let module = mutate(&returning_integer(), |value| {
        value["functions"][0]["returnType"] = serde_json::json!(99);
    });

    let error = Verifier::verify(module).expect_err("unknown type ids must fail closed");

    assert_eq!(error.code(), VerificationCode::UnknownType);
}

#[test]
fn verifier_rejects_unknown_block_targets() {
    let mut module = minimal_module();
    module
        .define_function(MAIN, "main", [], UNIT, ENTRY)
        .expect("function definition is valid");
    module
        .define_block(MAIN, ENTRY, [])
        .expect("entry block is unique");
    module
        .set_terminator(
            MAIN,
            ENTRY,
            Terminator::Goto {
                target: BlockId::new(9),
                arguments: vec![],
            },
            SPAN,
        )
        .expect("terminator is unique");

    let error = Verifier::verify(module.finish()).expect_err("unknown targets must fail");

    assert_eq!(error.code(), VerificationCode::UnknownBlock);
}

#[test]
fn verifier_rejects_branch_conditions_that_are_not_i1() {
    let mut module = minimal_module();
    module
        .define_function(MAIN, "main", [], UNIT, ENTRY)
        .expect("function definition is valid");
    for block in [ENTRY, BlockId::new(1), BlockId::new(2)] {
        module
            .define_block(MAIN, block, [])
            .expect("block id is unique");
    }
    module
        .append_instruction(
            MAIN,
            ENTRY,
            InstructionId::new(0),
            Some(TypedValue::new(ValueId::new(0), I64)),
            Operation::ConstI64 { value: 1 },
            SPAN,
        )
        .expect("instruction is valid");
    module
        .set_terminator(
            MAIN,
            ENTRY,
            Terminator::Branch {
                condition: ValueId::new(0),
                then_target: BlockId::new(1),
                then_arguments: vec![],
                else_target: BlockId::new(2),
                else_arguments: vec![],
            },
            SPAN,
        )
        .expect("terminator is unique");
    for block in [BlockId::new(1), BlockId::new(2)] {
        module
            .set_terminator(MAIN, block, Terminator::Return { value: None }, SPAN)
            .expect("terminator is unique");
    }

    let error = Verifier::verify(module.finish()).expect_err("branch requires I1");

    assert_eq!(error.code(), VerificationCode::TypeMismatch);
}

#[test]
fn verifier_rejects_a_return_type_mismatch() {
    let module = mutate(&returning_integer(), |value| {
        value["functions"][0]["returnType"] = serde_json::json!(0);
    });

    let error = Verifier::verify(module).expect_err("return type mismatch must fail");

    assert_eq!(error.code(), VerificationCode::TypeMismatch);
}

#[test]
fn verifier_rejects_use_before_definition() {
    let module = mutate(&returning_integer(), |value| {
        value["functions"][0]["blocks"][0]["terminator"]["terminator"]["value"] =
            serde_json::json!(7);
    });

    let error = Verifier::verify(module).expect_err("unknown SSA values must fail");

    assert_eq!(error.code(), VerificationCode::UnknownValue);
}

#[test]
fn verifier_rejects_use_after_move() {
    let mut module = minimal_module();
    module
        .define_function(
            MAIN,
            "main",
            [TypedValue::new(ValueId::new(0), OWNED_I64)],
            UNIT,
            ENTRY,
        )
        .expect("function definition is valid");
    module
        .define_block(MAIN, ENTRY, [])
        .expect("entry block is unique");
    module
        .append_instruction(
            MAIN,
            ENTRY,
            InstructionId::new(0),
            Some(TypedValue::new(ValueId::new(1), OWNED_I64)),
            Operation::Move {
                value: ValueId::new(0),
            },
            SPAN,
        )
        .expect("instruction is valid");
    module
        .append_instruction(
            MAIN,
            ENTRY,
            InstructionId::new(1),
            None,
            Operation::Drop {
                value: ValueId::new(0),
            },
            SPAN,
        )
        .expect("instruction is valid");
    module
        .append_instruction(
            MAIN,
            ENTRY,
            InstructionId::new(2),
            None,
            Operation::Drop {
                value: ValueId::new(1),
            },
            SPAN,
        )
        .expect("instruction is valid");
    module
        .set_terminator(MAIN, ENTRY, Terminator::Return { value: None }, SPAN)
        .expect("terminator is unique");

    let error = Verifier::verify(module.finish()).expect_err("moved values cannot be reused");

    assert_eq!(error.code(), VerificationCode::UseAfterConsume);
}

#[test]
fn verifier_rejects_owned_values_without_move_or_drop() {
    let mut module = minimal_module();
    module
        .define_function(
            MAIN,
            "main",
            [TypedValue::new(ValueId::new(0), OWNED_I64)],
            UNIT,
            ENTRY,
        )
        .expect("function definition is valid");
    module
        .define_block(MAIN, ENTRY, [])
        .expect("entry block is unique");
    module
        .set_terminator(MAIN, ENTRY, Terminator::Return { value: None }, SPAN)
        .expect("terminator is unique");

    let error = Verifier::verify(module.finish()).expect_err("owned values require cleanup");

    assert_eq!(error.code(), VerificationCode::OwnedValueNotConsumed);
}

#[test]
fn verifier_accepts_an_owned_value_dropped_once() {
    let mut module = minimal_module();
    module
        .define_function(
            MAIN,
            "main",
            [TypedValue::new(ValueId::new(0), OWNED_I64)],
            UNIT,
            ENTRY,
        )
        .expect("function definition is valid");
    module
        .define_block(MAIN, ENTRY, [])
        .expect("entry block is unique");
    module
        .append_instruction(
            MAIN,
            ENTRY,
            InstructionId::new(0),
            None,
            Operation::Drop {
                value: ValueId::new(0),
            },
            SPAN,
        )
        .expect("instruction is valid");
    module
        .set_terminator(MAIN, ENTRY, Terminator::Return { value: None }, SPAN)
        .expect("terminator is unique");

    Verifier::verify(module.finish()).expect("one drop consumes the owned value");
}

#[test]
fn verifier_requires_cleanup_for_aggregates_with_owned_fields() {
    let mut module = minimal_module();
    module
        .define_function(
            MAIN,
            "main",
            [TypedValue::new(ValueId::new(0), OWNED_STRUCT)],
            UNIT,
            ENTRY,
        )
        .expect("function definition is valid");
    module
        .define_block(MAIN, ENTRY, [])
        .expect("entry block is unique");
    module
        .set_terminator(MAIN, ENTRY, Terminator::Return { value: None }, SPAN)
        .expect("terminator is unique");

    let error = Verifier::verify(module.finish()).expect_err("owned fields require aggregate drop");

    assert_eq!(error.code(), VerificationCode::OwnedValueNotConsumed);
}

#[test]
fn verifier_accepts_the_typed_print_intrinsic_registry_entry() {
    let mut module = minimal_module();
    module
        .define_function(MAIN, "main", [], UNIT, ENTRY)
        .expect("function definition is valid");
    module
        .define_block(MAIN, ENTRY, [])
        .expect("entry block is unique");
    module
        .append_instruction(
            MAIN,
            ENTRY,
            InstructionId::new(0),
            Some(TypedValue::new(ValueId::new(0), I64)),
            Operation::ConstI64 { value: 42 },
            SPAN,
        )
        .expect("instruction is valid");
    module
        .append_instruction(
            MAIN,
            ENTRY,
            InstructionId::new(1),
            None,
            Operation::CallIntrinsic {
                intrinsic: IntrinsicId::PrintI64,
                arguments: vec![ValueId::new(0)],
            },
            SPAN,
        )
        .expect("instruction is valid");
    module
        .set_terminator(MAIN, ENTRY, Terminator::Return { value: None }, SPAN)
        .expect("terminator is unique");

    Verifier::verify(module.finish()).expect("registered intrinsic signature should verify");
}
