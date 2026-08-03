//! Explicit target layout acceptance and rejection coverage.

// Rust 1.80 predates stable `#[expect]`; invalid test setup should stop immediately.
#![allow(clippy::expect_used)]

use nexa_nir::{
    Endianness, LayoutErrorCode, ModuleBuilder, NirType, TargetLayout, TypeId, Verifier,
};

fn layout_module() -> nexa_nir::VerifiedModule {
    let mut module = ModuleBuilder::new("layout").expect("module identity is valid");
    module
        .define_type(TypeId::new(0), NirType::I8)
        .expect("type id is unique");
    module
        .define_type(TypeId::new(1), NirType::I64)
        .expect("type id is unique");
    module
        .define_type(
            TypeId::new(2),
            NirType::RawPtr {
                pointee: TypeId::new(0),
            },
        )
        .expect("type id is unique");
    module
        .define_type(
            TypeId::new(3),
            NirType::Struct {
                fields: vec![TypeId::new(0), TypeId::new(1)],
            },
        )
        .expect("type id is unique");
    Verifier::verify(module.finish()).expect("layout module should verify")
}

#[test]
fn target_layout_uses_explicit_pointer_width_instead_of_the_host() {
    let module = layout_module();
    let layout32 =
        TargetLayout::new(32, 4, 8, 16, Endianness::Little).expect("32-bit target layout is valid");
    let layout64 = TargetLayout::new(64, 8, 16, 16, Endianness::Little)
        .expect("64-bit target layout is valid");

    assert_eq!(
        (
            layout32.layout_of(&module, TypeId::new(2)),
            layout64.layout_of(&module, TypeId::new(2))
        ),
        (
            Ok(nexa_nir::ValueLayout::new(4, 4)),
            Ok(nexa_nir::ValueLayout::new(8, 8))
        )
    );
}

#[test]
fn aggregate_layout_inserts_checked_padding() {
    let module = layout_module();
    let target = TargetLayout::new(64, 8, 16, 16, Endianness::Little)
        .expect("64-bit target layout is valid");

    assert_eq!(
        target.layout_of(&module, TypeId::new(3)),
        Ok(nexa_nir::ValueLayout::new(16, 8))
    );
}

#[test]
fn target_layout_rejects_invalid_pointer_width() {
    let error = TargetLayout::new(24, 4, 8, 16, Endianness::Little)
        .expect_err("unsupported pointer widths must fail");

    assert_eq!(error.code(), LayoutErrorCode::InvalidPointerWidth);
}

#[test]
fn target_layout_rejects_non_power_of_two_alignment() {
    let error = TargetLayout::new(64, 3, 8, 16, Endianness::Little)
        .expect_err("invalid pointer alignment must fail");

    assert_eq!(error.code(), LayoutErrorCode::InvalidAlignment);
}

#[test]
fn target_layout_deserialization_rejects_invalid_alignment() {
    let result = serde_json::from_value::<TargetLayout>(serde_json::json!({
        "pointerWidth": 64,
        "pointerAlignment": 0,
        "aggregateAlignment": 8,
        "stackAlignment": 16,
        "endianness": "little"
    }));

    assert!(result.is_err());
}

#[test]
fn target_layout_serialization_round_trip_preserves_a_valid_target() {
    let expected = TargetLayout::new(64, 8, 16, 16, Endianness::Little)
        .expect("64-bit target layout is valid");
    let encoded = serde_json::to_value(expected).expect("target layout serializes");
    let decoded: TargetLayout =
        serde_json::from_value(encoded).expect("valid target layout deserializes");

    assert_eq!(decoded, expected);
}

#[test]
fn fixed_array_layout_rejects_size_overflow() {
    let mut module = ModuleBuilder::new("overflow").expect("module identity is valid");
    module
        .define_type(TypeId::new(0), NirType::I64)
        .expect("type id is unique");
    module
        .define_type(
            TypeId::new(1),
            NirType::FixedArray {
                element: TypeId::new(0),
                length: u64::MAX,
            },
        )
        .expect("type id is unique");
    let module = Verifier::verify(module.finish()).expect("logical array type should verify");
    let target = TargetLayout::new(64, 8, 16, 16, Endianness::Little)
        .expect("64-bit target layout is valid");

    let error = target
        .layout_of(&module, TypeId::new(1))
        .expect_err("physical size overflow must fail");

    assert_eq!(error.code(), LayoutErrorCode::SizeOverflow);
}

#[test]
fn aggregate_layout_rejects_direct_recursive_types() {
    let mut module = ModuleBuilder::new("recursive").expect("module identity is valid");
    module
        .define_type(
            TypeId::new(0),
            NirType::Struct {
                fields: vec![TypeId::new(0)],
            },
        )
        .expect("type id is unique");
    let module = Verifier::verify(module.finish()).expect("logical references are resolved");
    let target = TargetLayout::new(64, 8, 16, 16, Endianness::Little)
        .expect("64-bit target layout is valid");

    let error = target
        .layout_of(&module, TypeId::new(0))
        .expect_err("direct recursive aggregate has no finite layout");

    assert_eq!(error.code(), LayoutErrorCode::RecursiveAggregate);
}

#[test]
fn tagged_union_layout_exposes_target_specific_tag_and_payload_storage() {
    let mut module = ModuleBuilder::new("union-layout").expect("module identity is valid");
    module
        .define_type(TypeId::new(0), NirType::I8)
        .expect("type id is unique");
    module
        .define_type(TypeId::new(1), NirType::Unit)
        .expect("type id is unique");
    module
        .define_type(
            TypeId::new(2),
            NirType::RawPtr {
                pointee: TypeId::new(0),
            },
        )
        .expect("type id is unique");
    module
        .define_type(
            TypeId::new(3),
            NirType::TaggedUnion {
                variants: vec![TypeId::new(1), TypeId::new(0), TypeId::new(2)],
            },
        )
        .expect("type id is unique");
    let module = Verifier::verify(module.finish()).expect("union module should verify");
    let layout32 =
        TargetLayout::new(32, 4, 8, 16, Endianness::Little).expect("32-bit target is valid");
    let layout64 =
        TargetLayout::new(64, 8, 16, 16, Endianness::Little).expect("64-bit target is valid");

    let union32 = layout32
        .tagged_union_layout_of(&module, TypeId::new(3))
        .expect("32-bit union layout is valid")
        .expect("type is a tagged union");
    let union64 = layout64
        .tagged_union_layout_of(&module, TypeId::new(3))
        .expect("64-bit union layout is valid")
        .expect("type is a tagged union");

    assert_eq!(union32.tag_layout(), nexa_nir::ValueLayout::new(1, 1));
    assert_eq!(union32.payload_layout(), nexa_nir::ValueLayout::new(4, 4));
    assert_eq!(union32.payload_offset(), 4);
    assert_eq!(union32.total_layout(), nexa_nir::ValueLayout::new(8, 4));
    assert_eq!(
        layout32.layout_of(&module, TypeId::new(3)),
        Ok(union32.total_layout())
    );

    assert_eq!(union64.tag_layout(), nexa_nir::ValueLayout::new(1, 1));
    assert_eq!(union64.payload_layout(), nexa_nir::ValueLayout::new(8, 8));
    assert_eq!(union64.payload_offset(), 8);
    assert_eq!(union64.total_layout(), nexa_nir::ValueLayout::new(16, 8));
    assert_eq!(
        layout64.layout_of(&module, TypeId::new(3)),
        Ok(union64.total_layout())
    );
}

#[test]
fn tagged_union_layout_handles_zst_and_nested_aggregate_payloads() {
    let mut module = ModuleBuilder::new("nested-union").expect("module identity is valid");
    for (id, ty) in [
        (TypeId::new(0), NirType::Unit),
        (TypeId::new(1), NirType::Never),
        (TypeId::new(2), NirType::I8),
        (TypeId::new(3), NirType::I64),
        (
            TypeId::new(4),
            NirType::Struct {
                fields: vec![TypeId::new(2), TypeId::new(3)],
            },
        ),
        (
            TypeId::new(5),
            NirType::TaggedUnion {
                variants: vec![TypeId::new(0), TypeId::new(1)],
            },
        ),
        (
            TypeId::new(6),
            NirType::TaggedUnion {
                variants: vec![TypeId::new(0), TypeId::new(4), TypeId::new(5)],
            },
        ),
    ] {
        module.define_type(id, ty).expect("type id is unique");
    }
    let module = Verifier::verify(module.finish()).expect("nested union module should verify");
    let target = TargetLayout::new(64, 8, 16, 16, Endianness::Little)
        .expect("64-bit target layout is valid");

    let zst = target
        .tagged_union_layout_of(&module, TypeId::new(5))
        .expect("ZST union layout is valid")
        .expect("type is a tagged union");
    let nested = target
        .tagged_union_layout_of(&module, TypeId::new(6))
        .expect("nested union layout is valid")
        .expect("type is a tagged union");

    assert_eq!(zst.payload_layout(), nexa_nir::ValueLayout::new(0, 1));
    assert_eq!(zst.payload_offset(), 1);
    assert_eq!(zst.total_layout(), nexa_nir::ValueLayout::new(1, 1));
    assert_eq!(nested.payload_layout(), nexa_nir::ValueLayout::new(16, 8));
    assert_eq!(nested.payload_offset(), 8);
    assert_eq!(nested.total_layout(), nexa_nir::ValueLayout::new(24, 8));
}

#[test]
fn tagged_union_tag_widens_at_the_checked_capacity_boundary() {
    let mut module = ModuleBuilder::new("union-tags").expect("module identity is valid");
    module
        .define_type(TypeId::new(0), NirType::Unit)
        .expect("type id is unique");
    module
        .define_type(
            TypeId::new(1),
            NirType::TaggedUnion {
                variants: vec![TypeId::new(0); 256],
            },
        )
        .expect("type id is unique");
    module
        .define_type(
            TypeId::new(2),
            NirType::TaggedUnion {
                variants: vec![TypeId::new(0); 257],
            },
        )
        .expect("type id is unique");
    let module = Verifier::verify(module.finish()).expect("union module should verify");
    let target = TargetLayout::new(64, 8, 16, 16, Endianness::Little)
        .expect("64-bit target layout is valid");

    let one_byte = target
        .tagged_union_layout_of(&module, TypeId::new(1))
        .expect("256-variant union layout is valid")
        .expect("type is a tagged union");
    let two_bytes = target
        .tagged_union_layout_of(&module, TypeId::new(2))
        .expect("257-variant union layout is valid")
        .expect("type is a tagged union");

    assert_eq!(one_byte.tag_layout(), nexa_nir::ValueLayout::new(1, 1));
    assert_eq!(one_byte.total_layout(), nexa_nir::ValueLayout::new(1, 1));
    assert_eq!(two_bytes.tag_layout(), nexa_nir::ValueLayout::new(2, 2));
    assert_eq!(two_bytes.total_layout(), nexa_nir::ValueLayout::new(2, 2));
}

#[test]
fn tagged_union_layout_rejects_direct_by_value_recursion() {
    let mut module =
        ModuleBuilder::new("direct-recursive-union").expect("module identity is valid");
    module
        .define_type(
            TypeId::new(0),
            NirType::TaggedUnion {
                variants: vec![TypeId::new(0)],
            },
        )
        .expect("type id is unique");
    let module = Verifier::verify(module.finish()).expect("logical references are resolved");
    let target = TargetLayout::new(64, 8, 16, 16, Endianness::Little)
        .expect("64-bit target layout is valid");

    let error = target
        .layout_of(&module, TypeId::new(0))
        .expect_err("direct recursive union has no finite layout");

    assert_eq!(error.code(), LayoutErrorCode::RecursiveAggregate);
}

#[test]
fn tagged_union_layout_rejects_indirect_by_value_recursion() {
    let mut module = ModuleBuilder::new("recursive-union").expect("module identity is valid");
    module
        .define_type(
            TypeId::new(0),
            NirType::TaggedUnion {
                variants: vec![TypeId::new(1)],
            },
        )
        .expect("type id is unique");
    module
        .define_type(
            TypeId::new(1),
            NirType::Struct {
                fields: vec![TypeId::new(0)],
            },
        )
        .expect("type id is unique");
    let module = Verifier::verify(module.finish()).expect("logical references are resolved");
    let target = TargetLayout::new(64, 8, 16, 16, Endianness::Little)
        .expect("64-bit target layout is valid");

    let error = target
        .layout_of(&module, TypeId::new(0))
        .expect_err("indirect recursive union has no finite layout");

    assert_eq!(error.code(), LayoutErrorCode::RecursiveAggregate);
}

#[test]
fn tagged_union_layout_rejects_payload_end_overflow() {
    let mut module = ModuleBuilder::new("union-overflow").expect("module identity is valid");
    module
        .define_type(TypeId::new(0), NirType::I8)
        .expect("type id is unique");
    module
        .define_type(
            TypeId::new(1),
            NirType::FixedArray {
                element: TypeId::new(0),
                length: u64::MAX,
            },
        )
        .expect("type id is unique");
    module
        .define_type(
            TypeId::new(2),
            NirType::TaggedUnion {
                variants: vec![TypeId::new(1)],
            },
        )
        .expect("type id is unique");
    let module = Verifier::verify(module.finish()).expect("logical types should verify");
    let target = TargetLayout::new(64, 8, 16, 16, Endianness::Little)
        .expect("64-bit target layout is valid");

    let error = target
        .layout_of(&module, TypeId::new(2))
        .expect_err("tag plus maximum payload must not wrap");

    assert_eq!(error.code(), LayoutErrorCode::SizeOverflow);
}

#[test]
fn tagged_union_layout_rejects_final_alignment_overflow() {
    let mut module = ModuleBuilder::new("union-tail-overflow").expect("module identity is valid");
    module
        .define_type(TypeId::new(0), NirType::I8)
        .expect("type id is unique");
    module
        .define_type(
            TypeId::new(1),
            NirType::FixedArray {
                element: TypeId::new(0),
                length: u64::MAX - 2,
            },
        )
        .expect("type id is unique");
    module
        .define_type(
            TypeId::new(2),
            NirType::TaggedUnion {
                variants: vec![TypeId::new(1); 257],
            },
        )
        .expect("type id is unique");
    let module = Verifier::verify(module.finish()).expect("logical types should verify");
    let target = TargetLayout::new(64, 8, 16, 16, Endianness::Little)
        .expect("64-bit target layout is valid");

    let error = target
        .layout_of(&module, TypeId::new(2))
        .expect_err("union tail padding must not wrap");

    assert_eq!(error.code(), LayoutErrorCode::SizeOverflow);
}

#[test]
fn tagged_union_type_serde_is_strict_and_preserves_variant_order() {
    let expected = NirType::TaggedUnion {
        variants: vec![TypeId::new(2), TypeId::new(0), TypeId::new(1)],
    };
    let encoded = serde_json::to_value(&expected).expect("tagged union type serializes");

    assert_eq!(
        encoded,
        serde_json::json!({
            "kind": "tagged-union",
            "variants": [2, 0, 1]
        })
    );
    let decoded =
        serde_json::from_value::<NirType>(encoded.clone()).expect("tagged union type deserializes");
    assert_eq!(decoded, expected);

    let mut with_unknown_field = encoded;
    with_unknown_field["niche"] = serde_json::json!(true);
    assert!(serde_json::from_value::<NirType>(with_unknown_field).is_err());
}
