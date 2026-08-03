use std::collections::BTreeSet;

use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

use crate::{NirType, TypeId, VerifiedModule};

/// Byte order selected by an explicit target description.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Endianness {
    /// Least-significant byte first.
    Little,
    /// Most-significant byte first.
    Big,
}

/// Stable category for a target-layout construction or calculation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutErrorCode {
    /// Pointer width is not supported by the bootstrap contract.
    InvalidPointerWidth,
    /// An alignment is zero, not a power of two, or internally inconsistent.
    InvalidAlignment,
    /// A referenced type is absent from the verified module.
    UnknownType,
    /// A by-value aggregate contains itself recursively.
    RecursiveAggregate,
    /// Size or padding arithmetic exceeded the canonical `u64` range.
    SizeOverflow,
    /// A tagged union has no representable explicit discriminant.
    InvalidTaggedUnion,
}

impl LayoutErrorCode {
    /// Returns the stable layout diagnostic code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidPointerWidth => "E5500",
            Self::InvalidAlignment => "E5501",
            Self::UnknownType => "E5502",
            Self::RecursiveAggregate => "E5503",
            Self::SizeOverflow => "E5504",
            Self::InvalidTaggedUnion => "E5505",
        }
    }
}

/// One deterministic target-layout failure.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{}: {message}", code.as_str())]
pub struct LayoutError {
    code: LayoutErrorCode,
    message: String,
}

impl LayoutError {
    /// Returns the stable failure category.
    #[must_use]
    pub const fn code(&self) -> LayoutErrorCode {
        self.code
    }

    /// Returns deterministic failure context.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Checked physical size and alignment for one logical NIR type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueLayout {
    size: u64,
    alignment: u64,
}

/// Checked private layout details for one target-specific tagged union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaggedUnionLayout {
    tag: ValueLayout,
    payload: ValueLayout,
    payload_offset: u64,
    total: ValueLayout,
}

impl TaggedUnionLayout {
    /// Returns the explicit discriminant layout.
    #[must_use]
    pub const fn tag_layout(&self) -> ValueLayout {
        self.tag
    }

    /// Returns the maximum size and alignment reserved for variant payloads.
    #[must_use]
    pub const fn payload_layout(&self) -> ValueLayout {
        self.payload
    }

    /// Returns the checked byte offset where overlapping payload storage begins.
    #[must_use]
    pub const fn payload_offset(&self) -> u64 {
        self.payload_offset
    }

    /// Returns the complete tagged-union size and alignment.
    #[must_use]
    pub const fn total_layout(&self) -> ValueLayout {
        self.total
    }
}

impl ValueLayout {
    /// Creates an already validated size/alignment pair.
    #[must_use]
    pub const fn new(size: u64, alignment: u64) -> Self {
        Self { size, alignment }
    }

    /// Returns the physical size in bytes.
    #[must_use]
    pub const fn size(self) -> u64 {
        self.size
    }

    /// Returns the required byte alignment.
    #[must_use]
    pub const fn alignment(self) -> u64 {
        self.alignment
    }
}

/// Explicit target data layout used only after target-neutral NIR verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TargetLayout {
    pointer_width: u16,
    pointer_alignment: u64,
    aggregate_alignment: u64,
    stack_alignment: u64,
    endianness: Endianness,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TargetLayoutFields {
    pointer_width: u16,
    pointer_alignment: u64,
    aggregate_alignment: u64,
    stack_alignment: u64,
    endianness: Endianness,
}

impl<'de> Deserialize<'de> for TargetLayout {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = TargetLayoutFields::deserialize(deserializer)?;
        Self::new(
            fields.pointer_width,
            fields.pointer_alignment,
            fields.aggregate_alignment,
            fields.stack_alignment,
            fields.endianness,
        )
        .map_err(serde::de::Error::custom)
    }
}

impl TargetLayout {
    /// Creates a checked target description without consulting Host state.
    ///
    /// # Errors
    ///
    /// Rejects unsupported pointer widths and invalid power-of-two alignments.
    pub fn new(
        pointer_width: u16,
        pointer_alignment: u64,
        aggregate_alignment: u64,
        stack_alignment: u64,
        endianness: Endianness,
    ) -> Result<Self, LayoutError> {
        if !matches!(pointer_width, 32 | 64) {
            return layout_error(
                LayoutErrorCode::InvalidPointerWidth,
                format!("pointer width {pointer_width} is not 32 or 64"),
            );
        }
        for (name, alignment) in [
            ("pointer", pointer_alignment),
            ("aggregate", aggregate_alignment),
            ("stack", stack_alignment),
        ] {
            if !alignment.is_power_of_two() {
                return layout_error(
                    LayoutErrorCode::InvalidAlignment,
                    format!("{name} alignment {alignment} is not a non-zero power of two"),
                );
            }
        }
        let pointer_size = u64::from(pointer_width / 8);
        if pointer_alignment > pointer_size || pointer_alignment > aggregate_alignment {
            return layout_error(
                LayoutErrorCode::InvalidAlignment,
                "pointer alignment exceeds the pointer size or aggregate ceiling".to_owned(),
            );
        }
        Ok(Self {
            pointer_width,
            pointer_alignment,
            aggregate_alignment,
            stack_alignment,
            endianness,
        })
    }

    /// Returns the target pointer width in bits.
    #[must_use]
    pub const fn pointer_width(self) -> u16 {
        self.pointer_width
    }

    /// Returns the target byte order.
    #[must_use]
    pub const fn endianness(self) -> Endianness {
        self.endianness
    }

    /// Returns the required stack alignment in bytes.
    #[must_use]
    pub const fn stack_alignment(self) -> u64 {
        self.stack_alignment
    }

    /// Computes a checked physical layout for one logical type.
    ///
    /// # Errors
    ///
    /// Rejects missing types, recursive by-value aggregates, and arithmetic overflow.
    pub fn layout_of(
        &self,
        module: &VerifiedModule,
        ty: TypeId,
    ) -> Result<ValueLayout, LayoutError> {
        self.layout_inner(module, ty, &mut BTreeSet::new())
    }

    /// Computes detailed private layout information when `ty` is a tagged union.
    ///
    /// Returns `Ok(None)` for another known NIR type.
    ///
    /// # Errors
    ///
    /// Rejects missing payload types, recursive by-value aggregates, invalid
    /// discriminant capacity, and arithmetic overflow.
    pub fn tagged_union_layout_of(
        &self,
        module: &VerifiedModule,
        ty: TypeId,
    ) -> Result<Option<TaggedUnionLayout>, LayoutError> {
        let nir_type = require_type(module, ty)?;
        match nir_type {
            NirType::TaggedUnion { variants } => self
                .tagged_union_layout(module, ty, variants, &mut BTreeSet::new())
                .map(Some),
            _ => Ok(None),
        }
    }

    fn layout_inner(
        &self,
        module: &VerifiedModule,
        id: TypeId,
        visiting: &mut BTreeSet<TypeId>,
    ) -> Result<ValueLayout, LayoutError> {
        let ty = require_type(module, id)?;
        let scalar = |size: u64| ValueLayout::new(size, size.min(self.aggregate_alignment).max(1));
        match ty {
            NirType::I1 | NirType::I8 | NirType::U8 => Ok(scalar(1)),
            NirType::I16 | NirType::U16 => Ok(scalar(2)),
            NirType::I32 | NirType::U32 | NirType::F32 | NirType::Char32 => Ok(scalar(4)),
            NirType::I64 | NirType::U64 | NirType::F64 => Ok(scalar(8)),
            NirType::Unit | NirType::Never => Ok(ValueLayout::new(0, 1)),
            NirType::RawPtr { .. }
            | NirType::OwnedPtr { .. }
            | NirType::BorrowPtr { .. }
            | NirType::MutBorrowPtr { .. }
            | NirType::Handle { .. }
            | NirType::FunctionRef => Ok(ValueLayout::new(
                u64::from(self.pointer_width / 8),
                self.pointer_alignment,
            )),
            NirType::Struct { fields } => self.aggregate_layout(module, id, fields, visiting),
            NirType::FixedArray { element, length } => {
                enter_aggregate(id, visiting)?;
                let element = self.layout_inner(module, *element, visiting)?;
                visiting.remove(&id);
                let stride = align_up(element.size, element.alignment)?;
                let size = stride.checked_mul(*length).ok_or_else(size_overflow)?;
                Ok(ValueLayout::new(size, element.alignment))
            }
            NirType::TaggedUnion { variants } => self
                .tagged_union_layout(module, id, variants, visiting)
                .map(|layout| layout.total_layout()),
        }
    }

    fn tagged_union_layout(
        &self,
        module: &VerifiedModule,
        id: TypeId,
        variants: &[TypeId],
        visiting: &mut BTreeSet<TypeId>,
    ) -> Result<TaggedUnionLayout, LayoutError> {
        enter_aggregate(id, visiting)?;
        let result = (|| {
            let tag = self.tag_layout(variants.len())?;
            let mut payload = ValueLayout::new(0, 1);
            for variant in variants {
                let variant = self.layout_inner(module, *variant, visiting)?;
                payload.size = payload.size.max(variant.size);
                payload.alignment = payload.alignment.max(variant.alignment);
            }
            let payload_offset = align_up(tag.size, payload.alignment)?;
            let payload_end = payload_offset
                .checked_add(payload.size)
                .ok_or_else(size_overflow)?;
            let alignment = tag.alignment.max(payload.alignment);
            let total = ValueLayout::new(align_up(payload_end, alignment)?, alignment);
            Ok(TaggedUnionLayout {
                tag,
                payload,
                payload_offset,
                total,
            })
        })();
        visiting.remove(&id);
        result
    }

    fn tag_layout(&self, variant_count: usize) -> Result<ValueLayout, LayoutError> {
        let max_discriminant = u64::try_from(variant_count)
            .ok()
            .and_then(|count| count.checked_sub(1))
            .ok_or_else(|| LayoutError {
                code: LayoutErrorCode::InvalidTaggedUnion,
                message: "tagged union variant count has no representable discriminant".to_owned(),
            })?;
        let size = if max_discriminant <= u64::from(u8::MAX) {
            1
        } else if max_discriminant <= u64::from(u16::MAX) {
            2
        } else if max_discriminant <= u64::from(u32::MAX) {
            4
        } else {
            8
        };
        Ok(ValueLayout::new(size, size.min(self.aggregate_alignment)))
    }

    fn aggregate_layout(
        &self,
        module: &VerifiedModule,
        id: TypeId,
        fields: &[TypeId],
        visiting: &mut BTreeSet<TypeId>,
    ) -> Result<ValueLayout, LayoutError> {
        enter_aggregate(id, visiting)?;
        let result = (|| {
            let mut size = 0_u64;
            let mut alignment = 1_u64;
            for field in fields {
                let field = self.layout_inner(module, *field, visiting)?;
                let field_alignment = field.alignment.min(self.aggregate_alignment);
                size = align_up(size, field_alignment)?;
                size = size.checked_add(field.size).ok_or_else(size_overflow)?;
                alignment = alignment.max(field_alignment);
            }
            Ok(ValueLayout::new(align_up(size, alignment)?, alignment))
        })();
        visiting.remove(&id);
        result
    }
}

fn require_type(module: &VerifiedModule, id: TypeId) -> Result<&NirType, LayoutError> {
    module
        .module()
        .types
        .iter()
        .find(|definition| definition.id == id)
        .map(|definition| &definition.ty)
        .ok_or_else(|| LayoutError {
            code: LayoutErrorCode::UnknownType,
            message: format!("type {} is not defined", id.index()),
        })
}

fn enter_aggregate(id: TypeId, visiting: &mut BTreeSet<TypeId>) -> Result<(), LayoutError> {
    if !visiting.insert(id) {
        return layout_error(
            LayoutErrorCode::RecursiveAggregate,
            format!("type {} recursively contains itself by value", id.index()),
        );
    }
    Ok(())
}

fn align_up(value: u64, alignment: u64) -> Result<u64, LayoutError> {
    let mask = alignment - 1;
    value
        .checked_add(mask)
        .map(|padded| padded & !mask)
        .ok_or_else(size_overflow)
}

fn size_overflow() -> LayoutError {
    LayoutError {
        code: LayoutErrorCode::SizeOverflow,
        message: "target layout size arithmetic overflowed u64".to_owned(),
    }
}

fn layout_error<T>(code: LayoutErrorCode, message: String) -> Result<T, LayoutError> {
    Err(LayoutError { code, message })
}
