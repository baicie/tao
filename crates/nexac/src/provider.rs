use std::path::{Component, Path, PathBuf};

use nexa_source::{ProvidedSource, SourceKey, SourceLoadError, SourceProvider, SourceRequest};

/// Host-filesystem source access for the command-line compiler.
#[derive(Debug, Default, Clone, Copy)]
pub struct FileSystemSourceProvider;

impl SourceProvider for FileSystemSourceProvider {
    fn resolve(&mut self, request: SourceRequest<'_>) -> Result<SourceKey, SourceLoadError> {
        let (importer, requested) = match request {
            SourceRequest::Entry(path) => (None, path.to_path_buf()),
            SourceRequest::Import {
                importer,
                specifier,
            } => {
                let base = importer.as_path().parent().unwrap_or_else(|| Path::new(""));
                (Some(importer.clone()), base.join(specifier))
            }
        };
        let absolute = if requested.is_absolute() {
            requested.clone()
        } else {
            std::env::current_dir()
                .map_err(|error| SourceLoadError::Resolve {
                    importer,
                    specifier: requested.clone(),
                    message: error.to_string(),
                })?
                .join(&requested)
        };
        let lexical = normalize_path(&absolute);
        let canonical = std::fs::canonicalize(&lexical).unwrap_or(lexical);
        Ok(SourceKey::new(canonical))
    }

    fn load(&mut self, key: &SourceKey) -> Result<ProvidedSource, SourceLoadError> {
        match std::fs::read(key.as_path()) {
            Ok(bytes) => Ok(ProvidedSource::new(
                key.clone(),
                key.as_path().to_path_buf(),
                bytes,
            )),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Err(SourceLoadError::NotFound { key: key.clone() })
            }
            Err(error) => Err(SourceLoadError::Load {
                key: key.clone(),
                message: error.to_string(),
            }),
        }
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(
                    normalized.components().next_back(),
                    Some(Component::Normal(_))
                ) {
                    let _ = normalized.pop();
                } else if !normalized.has_root() {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use nexa_compiler::{CompilerSession, SessionBuildError};
    use nexa_source::{SourceLoadError, SourceProvider, SourceRequest};

    use super::FileSystemSourceProvider;

    static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn filesystem_provider_deduplicates_normalized_existing_and_missing_aliases(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = TestDirectory::new()?;
        let entry = directory.join("main.nexa");
        let shared = directory.join("shared.nexa");
        fs::create_dir(directory.join("nested"))?;
        fs::write(&entry, "")?;
        fs::write(&shared, "")?;

        let mut provider = FileSystemSourceProvider;
        let importer = provider.resolve(SourceRequest::Entry(&entry))?;
        let shared_direct = provider.resolve(SourceRequest::Import {
            importer: &importer,
            specifier: "./shared.nexa",
        })?;
        let shared_normalized = provider.resolve(SourceRequest::Import {
            importer: &importer,
            specifier: "./nested/../shared.nexa",
        })?;
        let missing_direct = provider.resolve(SourceRequest::Import {
            importer: &importer,
            specifier: "./missing.nexa",
        })?;
        let missing_normalized = provider.resolve(SourceRequest::Import {
            importer: &importer,
            specifier: "./nested/../missing.nexa",
        })?;

        assert_eq!(shared_direct, shared_normalized);
        assert_eq!(missing_direct, missing_normalized);
        assert_eq!(shared_direct.as_path(), fs::canonicalize(shared)?);

        Ok(())
    }

    #[test]
    fn filesystem_session_reports_non_utf8_import_bytes_at_the_import_literal(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = TestDirectory::new()?;
        let source = "import { Value } from \"./invalid.nexa\";";
        let entry = directory.join("main.nexa");
        fs::write(&entry, source)?;
        fs::write(directory.join("invalid.nexa"), [0xff])?;

        let session = CompilerSession::build(FileSystemSourceProvider, &entry)?;
        let diagnostic = session
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code().as_str() == "E4001")
            .ok_or_else(|| std::io::Error::other("expected E4001"))?;
        let label = diagnostic
            .labels()
            .first()
            .ok_or_else(|| std::io::Error::other("expected a primary label"))?;

        assert_eq!(session.modules().len(), 1);
        assert_eq!(
            &source[label.span().range().start()..label.span().range().end()],
            "\"./invalid.nexa\""
        );

        Ok(())
    }

    #[test]
    fn filesystem_session_reports_a_directory_import_as_a_load_failure(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = TestDirectory::new()?;
        let entry = directory.join("main.nexa");
        fs::write(&entry, "import { Value } from \"./directory.nexa\";")?;
        fs::create_dir(directory.join("directory.nexa"))?;

        let session = CompilerSession::build(FileSystemSourceProvider, &entry)?;

        assert_eq!(
            session
                .diagnostics()
                .iter()
                .filter(|diagnostic| diagnostic.code().as_str() == "E4001")
                .count(),
            1
        );

        Ok(())
    }

    #[test]
    fn filesystem_session_keeps_entry_load_failures_source_less(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = TestDirectory::new()?;
        let error = CompilerSession::build(FileSystemSourceProvider, directory.path()).err();

        assert!(matches!(
            error,
            Some(SessionBuildError::EntryLoad(SourceLoadError::Load { .. }))
        ));

        Ok(())
    }

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Result<Self, std::io::Error> {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "nexa-provider-test-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path)?;
            Ok(Self { path })
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn join(&self, path: impl AsRef<Path>) -> PathBuf {
            self.path.join(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
