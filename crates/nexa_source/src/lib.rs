#![forbid(unsafe_code)]
//! Source file ownership and line/column lookup.

use std::path::{Path, PathBuf};

use nexa_span::{FileId, SourceSpan};
use thiserror::Error;

/// A source file registered with the compiler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    id: FileId,
    path: PathBuf,
    text: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
    fn new(id: FileId, path: PathBuf, text: String) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(
            text.bytes()
                .enumerate()
                .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
        );

        Self {
            id,
            path,
            text,
            line_starts,
        }
    }

    /// Returns this file's stable identifier.
    #[must_use]
    pub const fn id(&self) -> FileId {
        self.id
    }

    /// Returns the source file path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the complete source text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the one-based source location for a byte offset.
    #[must_use]
    pub fn location(&self, offset: usize) -> Option<SourceLocation> {
        if offset > self.text.len() || !self.text.is_char_boundary(offset) {
            return None;
        }

        let line_index = self.line_starts.partition_point(|start| *start <= offset) - 1;
        let line_start = self.line_starts[line_index];
        let column = self.text[line_start..offset].chars().count() + 1;

        Some(SourceLocation::new(line_index + 1, column))
    }
}

/// A one-based line and column location within a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceLocation {
    line: usize,
    column: usize,
}

impl SourceLocation {
    const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }

    /// Returns the one-based line number.
    #[must_use]
    pub const fn line(self) -> usize {
        self.line
    }

    /// Returns the one-based Unicode scalar column number.
    #[must_use]
    pub const fn column(self) -> usize {
        self.column
    }
}

/// An error raised while registering source files.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SourceMapError {
    /// The source collection cannot assign another `FileId`.
    #[error("source file limit exceeded")]
    TooManyFiles,
}

/// Owns compiler source files and resolves their stable identifiers.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    /// Registers source text under a path and returns its new file identifier.
    ///
    /// # Errors
    ///
    /// Returns [`SourceMapError::TooManyFiles`] when all `FileId` values have
    /// been assigned.
    pub fn add(
        &mut self,
        path: impl Into<PathBuf>,
        text: impl Into<String>,
    ) -> Result<FileId, SourceMapError> {
        let raw = u32::try_from(self.files.len()).map_err(|_| SourceMapError::TooManyFiles)?;
        let id = FileId::new(raw);
        self.files
            .push(SourceFile::new(id, path.into(), text.into()));
        Ok(id)
    }

    /// Returns a source file by its stable identifier.
    #[must_use]
    pub fn file(&self, id: FileId) -> Option<&SourceFile> {
        self.files.get(usize::try_from(id.raw()).ok()?)
    }

    /// Resolves the start of a source span to a file and one-based location.
    #[must_use]
    pub fn location(&self, span: SourceSpan) -> Option<(&SourceFile, SourceLocation)> {
        let file = self.file(span.file())?;
        let location = file.location(span.range().start())?;
        Some((file, location))
    }
}

#[cfg(test)]
mod tests {
    use super::{SourceMap, SourceMapError};
    use nexa_span::{SourceSpan, TextRange};

    #[test]
    fn source_map_assigns_file_ids_in_registration_order() -> Result<(), SourceMapError> {
        let mut sources = SourceMap::default();
        let first = sources.add("first.nexa", "")?;
        let second = sources.add("second.nexa", "")?;

        assert_eq!((first.raw(), second.raw()), (0, 1));
        assert_eq!(
            sources.file(second).map(|file| file.path()),
            Some("second.nexa".as_ref())
        );

        Ok(())
    }

    #[test]
    fn source_map_reports_one_based_unicode_locations() -> Result<(), SourceMapError> {
        let mut sources = SourceMap::default();
        let file = sources.add("example.nexa", "one\n\u{00e9}x")?;
        let span = SourceSpan::new(file, TextRange::new(6, 7));

        assert_eq!(
            sources
                .location(span)
                .map(|(_, location)| (location.line(), location.column())),
            Some((2, 2))
        );

        Ok(())
    }

    #[test]
    fn source_file_rejects_mid_code_point_offsets() -> Result<(), SourceMapError> {
        let mut sources = SourceMap::default();
        let file = sources.add("example.nexa", "\u{00e9}")?;

        assert_eq!(
            sources.file(file).and_then(|source| source.location(1)),
            None
        );

        Ok(())
    }
}
