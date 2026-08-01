use thiserror::Error;

/// A target-host value layout used by the safe reference storage kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValueLayout {
    size: u64,
    align: u32,
    stride: u64,
    needs_drop: bool,
}

impl ValueLayout {
    /// Validates a concrete size, alignment, and Drop requirement.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError`] when alignment is invalid, padded stride
    /// overflows, or the size cannot be represented by the current Host.
    pub fn new(size: u64, align: u32, needs_drop: bool) -> Result<Self, LayoutError> {
        if align == 0 || !align.is_power_of_two() {
            return Err(LayoutError::InvalidAlignment { align });
        }
        if usize::try_from(size).is_err() {
            return Err(LayoutError::HostSizeOverflow { size });
        }
        let stride = if size == 0 {
            0
        } else {
            let mask = u64::from(align) - 1;
            size.checked_add(mask)
                .map(|padded| padded & !mask)
                .ok_or(LayoutError::SizeOverflow)?
        };
        if usize::try_from(stride).is_err() {
            return Err(LayoutError::HostSizeOverflow { size: stride });
        }

        Ok(Self {
            size,
            align,
            stride,
            needs_drop,
        })
    }

    /// Derives the concrete layout of one Rust reference-kernel type.
    #[must_use]
    pub fn of<T>() -> Self {
        Self {
            size: std::mem::size_of::<T>() as u64,
            align: std::mem::align_of::<T>() as u32,
            stride: std::mem::size_of::<T>() as u64,
            needs_drop: std::mem::needs_drop::<T>(),
        }
    }

    /// Returns the value size in bytes, excluding allocator metadata.
    #[must_use]
    pub const fn size(self) -> u64 {
        self.size
    }

    /// Returns the non-zero power-of-two alignment in bytes.
    #[must_use]
    pub const fn align(self) -> u32 {
        self.align
    }

    /// Returns the padded byte distance between adjacent array elements.
    #[must_use]
    pub const fn stride(self) -> u64 {
        self.stride
    }

    /// Returns whether values require logical Drop processing.
    #[must_use]
    pub const fn needs_drop(self) -> bool {
        self.needs_drop
    }

    /// Returns whether the value occupies no backing bytes.
    #[must_use]
    pub const fn is_zero_sized(self) -> bool {
        self.size == 0
    }

    /// Computes backing bytes for a logical element count.
    ///
    /// # Errors
    ///
    /// Returns [`LayoutError::SizeOverflow`] when `count * stride` does not
    /// fit in `u64`, or [`LayoutError::HostSizeOverflow`] when it does not fit
    /// the current Host address space.
    pub fn checked_array_bytes(self, count: u64) -> Result<u64, LayoutError> {
        let bytes = count
            .checked_mul(self.stride)
            .ok_or(LayoutError::SizeOverflow)?;
        if usize::try_from(bytes).is_err() {
            return Err(LayoutError::HostSizeOverflow { size: bytes });
        }
        Ok(bytes)
    }
}

/// Rejection reason for an invalid concrete value layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum LayoutError {
    /// Alignment was zero or not a power of two.
    #[error("alignment {align} is not a non-zero power of two")]
    InvalidAlignment {
        /// Rejected alignment in bytes.
        align: u32,
    },
    /// Padding or an element-count multiplication overflowed `u64`.
    #[error("layout size calculation overflowed")]
    SizeOverflow,
    /// A byte count cannot be represented by the current Host.
    #[error("layout size {size} exceeds the Host address space")]
    HostSizeOverflow {
        /// Rejected size in bytes.
        size: u64,
    },
}
