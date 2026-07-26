#![forbid(unsafe_code)]
//! Source identity and byte-range primitives.

/// A stable identifier for a source file known to the compiler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileId(u32);

impl FileId {
    /// Creates a file identifier from a raw numeric value.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the raw numeric value.
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// A half-open byte range in source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextRange {
    start: usize,
    end: usize,
}

impl TextRange {
    /// Creates a new half-open range.
    ///
    /// # Panics
    ///
    /// Panics when `start` is greater than `end`.
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        assert!(start <= end, "text range start must not exceed end");
        Self { start, end }
    }

    /// Returns the start byte offset.
    #[must_use]
    pub const fn start(self) -> usize {
        self.start
    }

    /// Returns the end byte offset.
    #[must_use]
    pub const fn end(self) -> usize {
        self.end
    }

    /// Returns true when the range is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

/// A source range tied to a specific file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceSpan {
    file: FileId,
    range: TextRange,
}

impl SourceSpan {
    /// Creates a source span.
    #[must_use]
    pub const fn new(file: FileId, range: TextRange) -> Self {
        Self { file, range }
    }

    /// Returns the file identifier.
    #[must_use]
    pub const fn file(self) -> FileId {
        self.file
    }

    /// Returns the text range.
    #[must_use]
    pub const fn range(self) -> TextRange {
        self.range
    }
}

#[cfg(test)]
mod tests {
    use super::{FileId, SourceSpan, TextRange};

    #[test]
    fn source_span_keeps_file_and_range_together() {
        let span = SourceSpan::new(FileId::new(7), TextRange::new(2, 5));

        assert_eq!(span.file().raw(), 7);
        assert_eq!(span.range().start(), 2);
        assert_eq!(span.range().end(), 5);
    }

    #[test]
    #[should_panic(expected = "text range start must not exceed end")]
    fn text_range_rejects_an_inverted_range() {
        let _ = TextRange::new(5, 2);
    }
}
