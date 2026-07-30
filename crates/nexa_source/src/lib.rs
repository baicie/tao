#![forbid(unsafe_code)]
//! Source file ownership and line/column lookup.

use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::path::{Component, Path, PathBuf};

use nexa_span::{FileId, SourceSpan};
use thiserror::Error;

/// A provider-defined canonical identity for one source.
///
/// The key is intentionally opaque to the compiler. Providers are responsible
/// for canonicalizing requests before constructing a key; this type does not
/// inspect the host file system or normalize path components.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceKey(PathBuf);

impl SourceKey {
    /// Creates an opaque canonical source identity.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    /// Returns the provider-defined path representation of this key.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl Display for SourceKey {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0.display())
    }
}

/// A request for an entry source or an import relative to a loaded source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceRequest<'a> {
    /// Loads a compiler entry point selected by the host.
    Entry(&'a Path),
    /// Loads a decoded, language-validated import specifier.
    Import {
        /// The canonical identity of the importing source.
        importer: &'a SourceKey,
        /// The decoded relative import specifier.
        specifier: &'a str,
    },
}

/// Source bytes and identities returned by a [`SourceProvider`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvidedSource {
    key: SourceKey,
    display_path: PathBuf,
    bytes: Vec<u8>,
}

impl ProvidedSource {
    /// Creates a provider result from its canonical key, diagnostic path, and bytes.
    #[must_use]
    pub fn new(
        key: SourceKey,
        display_path: impl Into<PathBuf>,
        bytes: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            key,
            display_path: display_path.into(),
            bytes: bytes.into(),
        }
    }

    /// Returns the canonical identity used for session deduplication.
    #[must_use]
    pub const fn key(&self) -> &SourceKey {
        &self.key
    }

    /// Returns the path shown in source diagnostics.
    #[must_use]
    pub fn display_path(&self) -> &Path {
        &self.display_path
    }

    /// Returns the unvalidated source bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Splits this result into its canonical key, diagnostic path, and bytes.
    #[must_use]
    pub fn into_parts(self) -> (SourceKey, PathBuf, Vec<u8>) {
        (self.key, self.display_path, self.bytes)
    }
}

/// A structured failure while resolving or loading a source request.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum SourceLoadError {
    /// The provider resolved a canonical key that has no source.
    #[error("source `{key}` was not found")]
    NotFound {
        /// The canonical identity that could not be loaded.
        key: SourceKey,
    },
    /// The provider could not resolve a request to a canonical key.
    #[error("source request `{specifier}` could not be resolved: {message}")]
    Resolve {
        /// The importing source, or `None` for an entry request.
        importer: Option<SourceKey>,
        /// The unresolved entry path or import specifier.
        specifier: PathBuf,
        /// Provider-specific resolution details.
        message: String,
    },
    /// A canonical source could not be read by the provider.
    #[error("source `{key}` could not be loaded: {message}")]
    Load {
        /// The canonical source identity.
        key: SourceKey,
        /// Provider-specific loading details.
        message: String,
    },
}

/// Resolves source requests and loads canonical sources for a compiler session.
pub trait SourceProvider {
    /// Resolves and canonicalizes one source request without loading its bytes.
    ///
    /// A successfully resolved key does not imply that the source exists or is
    /// readable. Keeping resolution separate lets a compiler session cache one
    /// load result for equivalent requests that share a canonical key.
    ///
    /// # Errors
    ///
    /// Returns [`SourceLoadError::Resolve`] when the request cannot be mapped to
    /// a canonical source identity.
    fn resolve(&mut self, request: SourceRequest<'_>) -> Result<SourceKey, SourceLoadError>;

    /// Loads raw bytes and a display path for one canonical source key.
    ///
    /// Returned bytes are intentionally not decoded; the compiler validates
    /// UTF-8 before registering text with the parser.
    ///
    /// # Errors
    ///
    /// Returns [`SourceLoadError`] when the canonical source does not exist or
    /// cannot be loaded.
    fn load(&mut self, key: &SourceKey) -> Result<ProvidedSource, SourceLoadError>;
}

/// A source file registered with the compiler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    id: FileId,
    key: Option<SourceKey>,
    path: PathBuf,
    text: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
    fn new(id: FileId, key: Option<SourceKey>, path: PathBuf, text: String) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(
            text.bytes()
                .enumerate()
                .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
        );

        Self {
            id,
            key,
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

    /// Returns the provider-defined identity, when this file was provider-backed.
    #[must_use]
    pub const fn key(&self) -> Option<&SourceKey> {
        self.key.as_ref()
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
    files_by_key: HashMap<SourceKey, FileId>,
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
        self.insert_file(None, path.into(), text.into())
    }

    /// Registers provider-backed text once under its canonical source key.
    ///
    /// Re-registering an existing key returns its original `FileId` without
    /// replacing the diagnostic path or source text.
    ///
    /// # Errors
    ///
    /// Returns [`SourceMapError::TooManyFiles`] when all `FileId` values have
    /// been assigned.
    pub fn add_provided(
        &mut self,
        key: SourceKey,
        display_path: impl Into<PathBuf>,
        text: impl Into<String>,
    ) -> Result<FileId, SourceMapError> {
        if let Some(id) = self.id_for_key(&key) {
            return Ok(id);
        }

        let id = self.insert_file(Some(key.clone()), display_path.into(), text.into())?;
        let _ = self.files_by_key.insert(key, id);
        Ok(id)
    }

    /// Returns the stable file identifier assigned to a canonical source key.
    #[must_use]
    pub fn id_for_key(&self, key: &SourceKey) -> Option<FileId> {
        self.files_by_key.get(key).copied()
    }

    /// Returns a provider-backed source file by its canonical source key.
    #[must_use]
    pub fn file_by_key(&self, key: &SourceKey) -> Option<&SourceFile> {
        self.file(self.id_for_key(key)?)
    }

    fn insert_file(
        &mut self,
        key: Option<SourceKey>,
        path: PathBuf,
        text: String,
    ) -> Result<FileId, SourceMapError> {
        let raw = u32::try_from(self.files.len()).map_err(|_| SourceMapError::TooManyFiles)?;
        let id = FileId::new(raw);
        self.files.push(SourceFile::new(id, key, path, text));
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

/// A deterministic in-memory provider for compiler and integration tests.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MemorySourceProvider {
    sources: HashMap<SourceKey, MemorySource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MemorySource {
    display_path: PathBuf,
    bytes: Vec<u8>,
}

impl MemorySourceProvider {
    /// Inserts or replaces a virtual source and returns its canonical key.
    ///
    /// Paths are normalized lexically for this provider only. No host file
    /// system access or language-level import validation is performed.
    pub fn insert(&mut self, path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> SourceKey {
        let display_path = normalize_virtual_path(path.as_ref());
        let key = SourceKey::new(display_path.clone());
        let _ = self.sources.insert(
            key.clone(),
            MemorySource {
                display_path,
                bytes: bytes.as_ref().to_vec(),
            },
        );
        key
    }

    fn resolve_request(request: SourceRequest<'_>) -> SourceKey {
        match request {
            SourceRequest::Entry(path) => SourceKey::new(normalize_virtual_path(path)),
            SourceRequest::Import {
                importer,
                specifier,
            } => {
                let base = importer.as_path().parent().unwrap_or_else(|| Path::new(""));
                SourceKey::new(normalize_virtual_path(&base.join(specifier)))
            }
        }
    }
}

impl SourceProvider for MemorySourceProvider {
    fn resolve(&mut self, request: SourceRequest<'_>) -> Result<SourceKey, SourceLoadError> {
        Ok(Self::resolve_request(request))
    }

    fn load(&mut self, key: &SourceKey) -> Result<ProvidedSource, SourceLoadError> {
        let source = self
            .sources
            .get(key)
            .ok_or_else(|| SourceLoadError::NotFound { key: key.clone() })?;

        Ok(ProvidedSource::new(
            key.clone(),
            source.display_path.clone(),
            source.bytes.clone(),
        ))
    }
}

fn normalize_virtual_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                let last_is_normal = matches!(
                    normalized.components().next_back(),
                    Some(Component::Normal(_))
                );
                if last_is_normal {
                    let _ = normalized.pop();
                } else if !normalized.has_root() {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use nexa_span::{SourceSpan, TextRange};

    use super::{
        MemorySourceProvider, ProvidedSource, SourceKey, SourceLoadError, SourceMap,
        SourceMapError, SourceProvider, SourceRequest,
    };

    #[test]
    fn source_key_preserves_the_provider_defined_identity() {
        let key = SourceKey::new(PathBuf::from("source").join("..").join("main.nexa"));

        assert_eq!(
            key.as_path(),
            Path::new("source").join("..").join("main.nexa")
        );
        assert_ne!(key, SourceKey::new("main.nexa"));
    }

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
    fn source_map_add_allocates_distinct_ids_for_repeated_paths() -> Result<(), SourceMapError> {
        let mut sources = SourceMap::default();
        let first = sources.add("main.nexa", "first")?;
        let second = sources.add("main.nexa", "second")?;

        assert_ne!(first, second);
        assert_eq!(
            sources.file(second).map(|file| (file.key(), file.text())),
            Some((None, "second"))
        );

        Ok(())
    }

    #[test]
    fn source_map_add_provided_deduplicates_by_key_without_replacing_source(
    ) -> Result<(), SourceMapError> {
        let mut sources = SourceMap::default();
        let key = SourceKey::new("canonical/main.nexa");
        let first = sources.add_provided(key.clone(), "display/first.nexa", "first")?;
        let second = sources.add_provided(key.clone(), "display/second.nexa", "second")?;

        assert_eq!(first, second);
        assert_eq!(sources.id_for_key(&key), Some(first));
        assert_eq!(
            sources
                .file_by_key(&key)
                .map(|file| (file.key(), file.path(), file.text())),
            Some((Some(&key), Path::new("display/first.nexa"), "first"))
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

    #[test]
    fn provided_source_splits_into_its_original_parts() {
        let expected_key = SourceKey::new("canonical/main.nexa");
        let expected_path = PathBuf::from("display/main.nexa");
        let expected_bytes = vec![0xff, 0x00];
        let source = ProvidedSource::new(
            expected_key.clone(),
            expected_path.clone(),
            expected_bytes.clone(),
        );

        assert_eq!(
            source.into_parts(),
            (expected_key, expected_path, expected_bytes)
        );
    }

    #[test]
    fn memory_provider_loads_normalized_entries_as_raw_bytes() -> Result<(), SourceLoadError> {
        let mut provider = MemorySourceProvider::default();
        let key = provider.insert(Path::new("app").join(".").join("main.nexa"), [0xff, 0x00]);

        let resolved = provider.resolve(SourceRequest::Entry(
            &Path::new("app").join("nested").join("..").join("main.nexa"),
        ))?;
        let source = provider.load(&resolved)?;

        assert_eq!((resolved, source.key()), (key.clone(), &key));
        assert_eq!(source.display_path(), Path::new("app").join("main.nexa"));
        assert_eq!(source.bytes(), [0xff, 0x00]);

        Ok(())
    }

    #[test]
    fn memory_provider_resolves_imports_relative_to_the_importer() -> Result<(), SourceLoadError> {
        let mut provider = MemorySourceProvider::default();
        let importer =
            provider.insert(Path::new("project").join("src").join("main.nexa"), b"entry");
        let dependency = provider.insert(
            Path::new("project").join("lib").join("math.nexa"),
            b"dependency",
        );

        let resolved = provider.resolve(SourceRequest::Import {
            importer: &importer,
            specifier: "../lib/./math.nexa",
        })?;
        let source = provider.load(&resolved)?;

        assert_eq!((resolved, source.key()), (dependency.clone(), &dependency));
        assert_eq!(source.bytes(), b"dependency");

        Ok(())
    }

    #[test]
    fn memory_provider_resolves_missing_imports_before_load() -> Result<(), SourceLoadError> {
        let mut provider = MemorySourceProvider::default();
        let importer = SourceKey::new(Path::new("project").join("src").join("main.nexa"));
        let missing = SourceKey::new(Path::new("project").join("lib").join("missing.nexa"));
        let resolved = provider.resolve(SourceRequest::Import {
            importer: &importer,
            specifier: "../lib/./missing.nexa",
        })?;

        assert_eq!(resolved, missing);

        Ok(())
    }

    #[test]
    fn memory_provider_reports_not_found_when_loading_a_missing_key() {
        let mut provider = MemorySourceProvider::default();
        let missing = SourceKey::new(Path::new("project").join("lib").join("missing.nexa"));

        assert_eq!(
            provider.load(&missing),
            Err(SourceLoadError::NotFound { key: missing })
        );
    }
}
