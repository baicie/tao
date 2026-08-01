//! Explicit target layout acceptance and rejection coverage.

#![expect(
    clippy::expect_used,
    reason = "test setup failures should stop at the violated fixture assumption"
)]

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
