use std::collections::HashSet;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};
use nexa_compiler::{check_session, run_session_with_args, CompilerSession};
use nexa_source::{ProvidedSource, SourceKey, SourceLoadError, SourceProvider, SourceRequest};

const VERSION: &str = "1.0";
const COLUMN_COUNT: usize = 6;
const HEADER: &str = "name\tmode\tentry\targuments\texpectation\texpected";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Manifest {
    version: String,
    cases: Vec<Case>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Case {
    name: String,
    mode: Mode,
    entry: PathBuf,
    arguments: Vec<String>,
    expectation: Expectation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Check,
    Run,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Expectation {
    Stdout(String),
    Diagnostics(Vec<String>),
    Runtime { message: String, stdout: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ManifestError {
    line: usize,
    kind: ManifestErrorKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ManifestErrorKind {
    MissingVersion,
    InvalidVersionRow,
    UnsupportedVersion(String),
    MissingHeader,
    InvalidHeader,
    InvalidColumnCount { expected: usize, found: usize },
    InvalidEscape { field: &'static str, escape: String },
    InvalidName(String),
    InvalidMode(String),
    InvalidPath(String),
    InvalidExpectation(String),
    InvalidCombination(String),
    InvalidDiagnosticCode(String),
    DuplicateCase(String),
    EmptyCorpus,
}

impl Display for ManifestError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "manifest line {}: ", self.line)?;
        match &self.kind {
            ManifestErrorKind::MissingVersion => formatter.write_str("missing version row"),
            ManifestErrorKind::InvalidVersionRow => {
                formatter.write_str("expected `version<TAB>1.0`")
            }
            ManifestErrorKind::UnsupportedVersion(version) => {
                write!(formatter, "unsupported conformance version `{version}`")
            }
            ManifestErrorKind::MissingHeader => formatter.write_str("missing column header"),
            ManifestErrorKind::InvalidHeader => write!(formatter, "expected header `{HEADER}`"),
            ManifestErrorKind::InvalidColumnCount { expected, found } => write!(
                formatter,
                "expected {expected} tab-separated columns, found {found}"
            ),
            ManifestErrorKind::InvalidEscape { field, escape } => {
                write!(formatter, "invalid escape `{escape}` in {field} field")
            }
            ManifestErrorKind::InvalidName(name) => write!(formatter, "invalid case name `{name}`"),
            ManifestErrorKind::InvalidMode(mode) => write!(formatter, "unknown mode `{mode}`"),
            ManifestErrorKind::InvalidPath(path) => {
                write!(
                    formatter,
                    "entry path `{path}` is not a bounded relative .nexa path"
                )
            }
            ManifestErrorKind::InvalidExpectation(expectation) => {
                write!(formatter, "unknown expectation `{expectation}`")
            }
            ManifestErrorKind::InvalidCombination(message) => formatter.write_str(message),
            ManifestErrorKind::InvalidDiagnosticCode(code) => {
                write!(formatter, "invalid diagnostic code `{code}`")
            }
            ManifestErrorKind::DuplicateCase(name) => {
                write!(formatter, "duplicate case name `{name}`")
            }
            ManifestErrorKind::EmptyCorpus => {
                formatter.write_str("manifest contains no conformance cases")
            }
        }
    }
}

impl Error for ManifestError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Report {
    version: String,
    results: Vec<CaseResult>,
}

impl Report {
    fn is_ok(&self) -> bool {
        self.results
            .iter()
            .all(|result| matches!(result.outcome, CaseOutcome::Passed))
    }

    fn failure_count(&self) -> usize {
        self.results
            .iter()
            .filter(|result| matches!(result.outcome, CaseOutcome::Failed(_)))
            .count()
    }

    fn render(&self) -> String {
        let mut output = String::new();
        for result in &self.results {
            match &result.outcome {
                CaseOutcome::Passed => {
                    output.push_str("PASS\t");
                    output.push_str(&result.name);
                    output.push('\n');
                }
                CaseOutcome::Failed(failure) => {
                    output.push_str("FAIL\t");
                    output.push_str(&result.name);
                    output.push('\t');
                    output.push_str(&failure.to_string());
                    output.push('\n');
                }
            }
        }
        let failures = self.failure_count();
        let passed = self.results.len() - failures;
        output.push_str(&format!(
            "conformance {}: {passed} passed; {failures} failed\n",
            self.version
        ));
        output
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CaseResult {
    name: String,
    outcome: CaseOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CaseOutcome {
    Passed,
    Failed(CaseFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CaseFailure {
    Entry(String),
    Session(String),
    UnexpectedFailure {
        diagnostics: Vec<String>,
        runtime_error: Option<String>,
        stdout: String,
    },
    StdoutMismatch {
        expected: String,
        actual: String,
    },
    DiagnosticMismatch {
        expected: Vec<String>,
        actual: Vec<String>,
        check_succeeded: bool,
    },
    RuntimeMismatch {
        expected_message: String,
        actual_message: Option<String>,
        expected_stdout: String,
        actual_stdout: String,
        diagnostics: Vec<String>,
    },
}

impl Display for CaseFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Entry(message) => write!(formatter, "entry error: {message}"),
            Self::Session(message) => write!(formatter, "session error: {message}"),
            Self::UnexpectedFailure {
                diagnostics,
                runtime_error,
                stdout,
            } => write!(
                formatter,
                "expected success; diagnostics={diagnostics:?}, runtime={runtime_error:?}, stdout={stdout:?}"
            ),
            Self::StdoutMismatch { expected, actual } => {
                write!(formatter, "stdout mismatch: expected {expected:?}, found {actual:?}")
            }
            Self::DiagnosticMismatch {
                expected,
                actual,
                check_succeeded,
            } => write!(
                formatter,
                "diagnostic mismatch: expected={expected:?}, actual={actual:?}, check_succeeded={check_succeeded}"
            ),
            Self::RuntimeMismatch {
                expected_message,
                actual_message,
                expected_stdout,
                actual_stdout,
                diagnostics,
            } => write!(
                formatter,
                "runtime mismatch: expected_message={expected_message:?}, actual_message={actual_message:?}, expected_stdout={expected_stdout:?}, actual_stdout={actual_stdout:?}, diagnostics={diagnostics:?}"
            ),
        }
    }
}

pub(crate) fn default_manifest_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../conformance/1.0/manifest.tsv")
}

pub(crate) fn run(manifest_path: &Path) -> Result<()> {
    let report = load_and_run(manifest_path)?;
    print!("{}", report.render());
    if !report.is_ok() {
        bail!("{} conformance case(s) failed", report.failure_count());
    }
    Ok(())
}

fn load_and_run(manifest_path: &Path) -> Result<Report> {
    let text = std::fs::read_to_string(manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let manifest = parse_manifest(&text)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    let suite_root = manifest_path
        .parent()
        .context("conformance manifest has no parent directory")?;
    let suite_root = std::fs::canonicalize(suite_root)
        .with_context(|| format!("failed to resolve {}", suite_root.display()))?;

    Ok(execute_manifest(&manifest, &suite_root))
}

fn execute_manifest(manifest: &Manifest, suite_root: &Path) -> Report {
    let results = manifest
        .cases
        .iter()
        .map(|case| CaseResult {
            name: case.name.clone(),
            outcome: execute_case(case, suite_root),
        })
        .collect();

    Report {
        version: manifest.version.clone(),
        results,
    }
}

fn execute_case(case: &Case, suite_root: &Path) -> CaseOutcome {
    let entry = suite_root.join(&case.entry);
    let entry = match std::fs::canonicalize(&entry) {
        Ok(entry) if entry.starts_with(suite_root) => entry,
        Ok(_) => {
            return CaseOutcome::Failed(CaseFailure::Entry(
                "canonical entry escapes the suite directory".to_owned(),
            ));
        }
        Err(error) => {
            return CaseOutcome::Failed(CaseFailure::Entry(error.to_string()));
        }
    };
    if !entry.is_file() {
        return CaseOutcome::Failed(CaseFailure::Entry("entry is not a regular file".to_owned()));
    }

    let provider = BoundedSourceProvider::new(suite_root.to_path_buf());
    let session = match CompilerSession::build(provider, &entry) {
        Ok(session) => session,
        Err(error) => {
            return CaseOutcome::Failed(CaseFailure::Session(error.to_string()));
        }
    };

    match (&case.mode, &case.expectation) {
        (Mode::Check, Expectation::Stdout(expected)) => {
            let checked = check_session(&session);
            let diagnostics = diagnostic_codes(checked.diagnostics());
            if !checked.is_ok() {
                CaseOutcome::Failed(CaseFailure::UnexpectedFailure {
                    diagnostics,
                    runtime_error: None,
                    stdout: String::new(),
                })
            } else {
                compare_stdout(expected, "ok\n")
            }
        }
        (Mode::Run, Expectation::Stdout(expected)) => {
            let executed = run_session_with_args(&session, &case.arguments);
            let diagnostics = diagnostic_codes(executed.diagnostics());
            let stdout = output_text(executed.output());
            if !executed.is_ok() {
                CaseOutcome::Failed(CaseFailure::UnexpectedFailure {
                    diagnostics,
                    runtime_error: executed.runtime_error().map(ToString::to_string),
                    stdout,
                })
            } else {
                compare_stdout(expected, &stdout)
            }
        }
        (Mode::Check, Expectation::Diagnostics(expected)) => {
            let checked = check_session(&session);
            let actual = diagnostic_codes(checked.diagnostics());
            if !checked.is_ok() && actual == *expected {
                CaseOutcome::Passed
            } else {
                CaseOutcome::Failed(CaseFailure::DiagnosticMismatch {
                    expected: expected.clone(),
                    actual,
                    check_succeeded: checked.is_ok(),
                })
            }
        }
        (
            Mode::Run,
            Expectation::Runtime {
                message,
                stdout: expected_stdout,
            },
        ) => {
            let executed = run_session_with_args(&session, &case.arguments);
            let diagnostics = diagnostic_codes(executed.diagnostics());
            let actual_stdout = output_text(executed.output());
            let actual_message = executed
                .runtime_error()
                .map(|error| error.message().to_owned());
            if !executed.is_ok()
                && diagnostics.is_empty()
                && actual_message.as_deref() == Some(message.as_str())
                && actual_stdout == *expected_stdout
            {
                CaseOutcome::Passed
            } else {
                CaseOutcome::Failed(CaseFailure::RuntimeMismatch {
                    expected_message: message.clone(),
                    actual_message,
                    expected_stdout: expected_stdout.clone(),
                    actual_stdout,
                    diagnostics,
                })
            }
        }
        (Mode::Check, Expectation::Runtime { .. }) => CaseOutcome::Failed(CaseFailure::Session(
            "check cases cannot expect a runtime failure".to_owned(),
        )),
        (Mode::Run, Expectation::Diagnostics(_)) => CaseOutcome::Failed(CaseFailure::Session(
            "run cases cannot expect static diagnostics".to_owned(),
        )),
    }
}

fn compare_stdout(expected: &str, actual: &str) -> CaseOutcome {
    if actual == expected {
        CaseOutcome::Passed
    } else {
        CaseOutcome::Failed(CaseFailure::StdoutMismatch {
            expected: expected.to_owned(),
            actual: actual.to_owned(),
        })
    }
}

fn diagnostic_codes(diagnostics: &[nexa_diagnostics::Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code().as_str().to_owned())
        .collect()
}

fn output_text(lines: &[String]) -> String {
    let mut output = String::new();
    for line in lines {
        output.push_str(line);
        output.push('\n');
    }
    output
}

fn parse_manifest(text: &str) -> std::result::Result<Manifest, ManifestError> {
    let mut rows = meaningful_rows(text);
    let Some((version_line, version_row)) = rows.next() else {
        return Err(ManifestError {
            line: 1,
            kind: ManifestErrorKind::MissingVersion,
        });
    };
    let version_columns = version_row.split('\t').collect::<Vec<_>>();
    if version_columns.len() != 2 || version_columns[0] != "version" {
        return Err(ManifestError {
            line: version_line,
            kind: ManifestErrorKind::InvalidVersionRow,
        });
    }
    if version_columns[1] != VERSION {
        return Err(ManifestError {
            line: version_line,
            kind: ManifestErrorKind::UnsupportedVersion(version_columns[1].to_owned()),
        });
    }

    let Some((header_line, header)) = rows.next() else {
        return Err(ManifestError {
            line: version_line + 1,
            kind: ManifestErrorKind::MissingHeader,
        });
    };
    if header != HEADER {
        return Err(ManifestError {
            line: header_line,
            kind: ManifestErrorKind::InvalidHeader,
        });
    }

    let mut cases = Vec::new();
    let mut names = HashSet::new();
    for (line, row) in rows {
        let columns = row.split('\t').collect::<Vec<_>>();
        if columns.len() != COLUMN_COUNT {
            return Err(ManifestError {
                line,
                kind: ManifestErrorKind::InvalidColumnCount {
                    expected: COLUMN_COUNT,
                    found: columns.len(),
                },
            });
        }
        let case = parse_case(&columns, line)?;
        if !names.insert(case.name.clone()) {
            return Err(ManifestError {
                line,
                kind: ManifestErrorKind::DuplicateCase(case.name),
            });
        }
        cases.push(case);
    }
    if cases.is_empty() {
        return Err(ManifestError {
            line: header_line,
            kind: ManifestErrorKind::EmptyCorpus,
        });
    }

    Ok(Manifest {
        version: VERSION.to_owned(),
        cases,
    })
}

fn meaningful_rows(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines().enumerate().filter_map(|(index, row)| {
        let row = row.strip_suffix('\r').unwrap_or(row);
        let trimmed = row.trim();
        (!trimmed.is_empty() && !trimmed.starts_with('#')).then_some((index + 1, row))
    })
}

fn parse_case(columns: &[&str], line: usize) -> std::result::Result<Case, ManifestError> {
    let name = decode_field(columns[0], "name", line)?;
    if name.is_empty()
        || !name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character))
    {
        return Err(ManifestError {
            line,
            kind: ManifestErrorKind::InvalidName(name),
        });
    }

    let mode_text = decode_field(columns[1], "mode", line)?;
    let mode = match mode_text.as_str() {
        "check" => Mode::Check,
        "run" => Mode::Run,
        _ => {
            return Err(ManifestError {
                line,
                kind: ManifestErrorKind::InvalidMode(mode_text),
            });
        }
    };

    let entry_text = decode_field(columns[2], "entry", line)?;
    let entry = validate_entry_path(&entry_text).ok_or_else(|| ManifestError {
        line,
        kind: ManifestErrorKind::InvalidPath(entry_text.clone()),
    })?;
    let arguments = decode_arguments(columns[3], line)?;
    if mode == Mode::Check && !arguments.is_empty() {
        return Err(ManifestError {
            line,
            kind: ManifestErrorKind::InvalidCombination(
                "check cases cannot declare arguments".to_owned(),
            ),
        });
    }

    let expectation_text = decode_field(columns[4], "expectation", line)?;
    let expected = decode_field(columns[5], "expected", line)?;
    let expectation = match expectation_text.as_str() {
        "stdout" => Expectation::Stdout(expected),
        "diagnostics" => {
            if mode != Mode::Check {
                return Err(ManifestError {
                    line,
                    kind: ManifestErrorKind::InvalidCombination(
                        "diagnostic expectations require check mode".to_owned(),
                    ),
                });
            }
            let codes = expected.split(',').map(str::to_owned).collect::<Vec<_>>();
            if codes.is_empty() || codes.iter().any(|code| !valid_diagnostic_code(code)) {
                return Err(ManifestError {
                    line,
                    kind: ManifestErrorKind::InvalidDiagnosticCode(expected),
                });
            }
            Expectation::Diagnostics(codes)
        }
        "runtime" => {
            if mode != Mode::Run {
                return Err(ManifestError {
                    line,
                    kind: ManifestErrorKind::InvalidCombination(
                        "runtime expectations require run mode".to_owned(),
                    ),
                });
            }
            let Some((message, stdout)) = expected.split_once('\t') else {
                return Err(ManifestError {
                    line,
                    kind: ManifestErrorKind::InvalidCombination(
                        "runtime expected value requires `message\\tstdout`".to_owned(),
                    ),
                });
            };
            if message.is_empty() {
                return Err(ManifestError {
                    line,
                    kind: ManifestErrorKind::InvalidCombination(
                        "runtime message cannot be empty".to_owned(),
                    ),
                });
            }
            Expectation::Runtime {
                message: message.to_owned(),
                stdout: stdout.to_owned(),
            }
        }
        _ => {
            return Err(ManifestError {
                line,
                kind: ManifestErrorKind::InvalidExpectation(expectation_text),
            });
        }
    };

    Ok(Case {
        name,
        mode,
        entry,
        arguments,
        expectation,
    })
}

fn decode_field(
    raw: &str,
    field: &'static str,
    line: usize,
) -> std::result::Result<String, ManifestError> {
    let mut value = String::new();
    let mut characters = raw.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            value.push(character);
            continue;
        }
        let escaped = characters.next().ok_or_else(|| ManifestError {
            line,
            kind: ManifestErrorKind::InvalidEscape {
                field,
                escape: "trailing backslash".to_owned(),
            },
        })?;
        value.push(decode_escape(escaped).ok_or_else(|| ManifestError {
            line,
            kind: ManifestErrorKind::InvalidEscape {
                field,
                escape: format!("\\{escaped}"),
            },
        })?);
    }
    Ok(value)
}

fn decode_arguments(raw: &str, line: usize) -> std::result::Result<Vec<String>, ManifestError> {
    if raw == "-" {
        return Ok(Vec::new());
    }

    let mut arguments = vec![String::new()];
    let mut characters = raw.chars();
    while let Some(character) = characters.next() {
        if character == ',' {
            arguments.push(String::new());
            continue;
        }
        let decoded = if character == '\\' {
            let escaped = characters.next().ok_or_else(|| ManifestError {
                line,
                kind: ManifestErrorKind::InvalidEscape {
                    field: "arguments",
                    escape: "trailing backslash".to_owned(),
                },
            })?;
            if escaped == '-' {
                '-'
            } else {
                decode_escape(escaped).ok_or_else(|| ManifestError {
                    line,
                    kind: ManifestErrorKind::InvalidEscape {
                        field: "arguments",
                        escape: format!("\\{escaped}"),
                    },
                })?
            }
        } else {
            character
        };
        if let Some(argument) = arguments.last_mut() {
            argument.push(decoded);
        }
    }
    Ok(arguments)
}

const fn decode_escape(character: char) -> Option<char> {
    match character {
        '\\' => Some('\\'),
        't' => Some('\t'),
        'n' => Some('\n'),
        'r' => Some('\r'),
        ',' => Some(','),
        _ => None,
    }
}

fn validate_entry_path(value: &str) -> Option<PathBuf> {
    if value.is_empty()
        || value.starts_with('/')
        || value.contains('\\')
        || value.contains(':')
        || value.chars().any(char::is_control)
    {
        return None;
    }
    let mut parts = value.split('/').peekable();
    if parts
        .clone()
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return None;
    }
    let last = parts.next_back()?;
    if !last.ends_with(".nexa") || last == ".nexa" {
        return None;
    }
    Some(PathBuf::from(value))
}

fn valid_diagnostic_code(code: &str) -> bool {
    code.len() == 5 && code.starts_with('E') && code.as_bytes()[1..].iter().all(u8::is_ascii_digit)
}

#[derive(Debug, Clone)]
struct BoundedSourceProvider {
    root: PathBuf,
}

impl BoundedSourceProvider {
    const fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn bounded_key(
        &self,
        importer: Option<SourceKey>,
        requested: PathBuf,
    ) -> std::result::Result<SourceKey, SourceLoadError> {
        let absolute = if requested.is_absolute() {
            requested.clone()
        } else {
            self.root.join(&requested)
        };
        let lexical = normalize_path(&absolute);
        if !lexical.starts_with(&self.root) {
            return Err(SourceLoadError::Resolve {
                importer,
                specifier: requested,
                message: "source path escapes the conformance suite".to_owned(),
            });
        }
        let canonical = std::fs::canonicalize(&lexical).unwrap_or(lexical);
        if !canonical.starts_with(&self.root) {
            return Err(SourceLoadError::Resolve {
                importer,
                specifier: requested,
                message: "canonical source path escapes the conformance suite".to_owned(),
            });
        }
        Ok(SourceKey::new(canonical))
    }
}

impl SourceProvider for BoundedSourceProvider {
    fn resolve(
        &mut self,
        request: SourceRequest<'_>,
    ) -> std::result::Result<SourceKey, SourceLoadError> {
        match request {
            SourceRequest::Entry(path) => self.bounded_key(None, path.to_path_buf()),
            SourceRequest::Import {
                importer,
                specifier,
            } => {
                let base = importer.as_path().parent().unwrap_or_else(|| Path::new(""));
                self.bounded_key(Some(importer.clone()), base.join(specifier))
            }
        }
    }

    fn load(&mut self, key: &SourceKey) -> std::result::Result<ProvidedSource, SourceLoadError> {
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
    use super::{
        default_manifest_path, load_and_run, parse_manifest, Expectation, ManifestErrorKind, Mode,
    };

    const HEADER: &str = "version\t1.0\nname\tmode\tentry\targuments\texpectation\texpected\n";

    #[test]
    fn parser_preserves_manifest_order_and_decodes_fields() -> Result<(), Box<dyn std::error::Error>>
    {
        let manifest = parse_manifest(&format!(
            "{HEADER}first\trun\taccepted/main.nexa\t20,hello\\,world\tstdout\t42\\n\nsecond\tcheck\trejected/main.nexa\t-\tdiagnostics\tE2001,E3001\n"
        ))?;

        assert_eq!(manifest.cases[0].name, "first");
        assert_eq!(manifest.cases[0].mode, Mode::Run);
        assert_eq!(manifest.cases[0].arguments, ["20", "hello,world"]);
        assert_eq!(
            manifest.cases[0].expectation,
            Expectation::Stdout("42\n".to_owned())
        );
        assert_eq!(manifest.cases[1].name, "second");
        assert_eq!(
            manifest.cases[1].expectation,
            Expectation::Diagnostics(vec!["E2001".to_owned(), "E3001".to_owned()])
        );

        Ok(())
    }

    #[test]
    fn parser_rejects_an_unknown_column_count_with_its_line() {
        let error = parse_manifest(&format!(
            "{HEADER}broken\tcheck\taccepted/main.nexa\t-\tstdout\n"
        ))
        .err();

        assert!(matches!(
            error,
            Some(super::ManifestError {
                line: 3,
                kind: ManifestErrorKind::InvalidColumnCount {
                    expected: 6,
                    found: 5
                }
            })
        ));
    }

    #[test]
    fn parser_rejects_an_unknown_mode_with_its_line() {
        let error = parse_manifest(&format!(
            "{HEADER}broken\tcompile\taccepted/main.nexa\t-\tstdout\tok\\n\n"
        ))
        .err();

        assert!(matches!(
            error,
            Some(super::ManifestError {
                line: 3,
                kind: ManifestErrorKind::InvalidMode(mode)
            }) if mode == "compile"
        ));
    }

    #[test]
    fn parser_rejects_a_duplicate_case_with_its_line() {
        let row = "same\tcheck\taccepted/main.nexa\t-\tstdout\tok\\n\n";
        let error = parse_manifest(&format!("{HEADER}{row}{row}")).err();

        assert!(matches!(
            error,
            Some(super::ManifestError {
                line: 4,
                kind: ManifestErrorKind::DuplicateCase(name)
            }) if name == "same"
        ));
    }

    #[test]
    fn parser_rejects_an_out_of_bounds_entry_with_its_line() {
        let error = parse_manifest(&format!(
            "{HEADER}escape\tcheck\t../outside.nexa\t-\tstdout\tok\\n\n"
        ))
        .err();

        assert!(matches!(
            error,
            Some(super::ManifestError {
                line: 3,
                kind: ManifestErrorKind::InvalidPath(path)
            }) if path == "../outside.nexa"
        ));
    }

    #[test]
    fn parser_rejects_an_unknown_escape_with_its_line() {
        let error = parse_manifest(&format!(
            "{HEADER}escape\tcheck\taccepted/main.nexa\t-\tstdout\tbad\\q\n"
        ))
        .err();

        assert!(matches!(
            error,
            Some(super::ManifestError {
                line: 3,
                kind: ManifestErrorKind::InvalidEscape {
                    field: "expected",
                    escape
                }
            }) if escape == "\\q"
        ));
    }

    #[test]
    fn parser_splits_runtime_message_from_prior_stdout_after_decoding(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let manifest = parse_manifest(&format!(
            "{HEADER}runtime\trun\taccepted/main.nexa\t-\truntime\tdivision by zero\\t7\\n\n"
        ))?;

        assert_eq!(
            manifest.cases[0].expectation,
            Expectation::Runtime {
                message: "division by zero".to_owned(),
                stdout: "7\n".to_owned()
            }
        );

        Ok(())
    }

    #[test]
    fn checked_in_corpus_passes_in_declared_order() -> Result<(), Box<dyn std::error::Error>> {
        let report = load_and_run(&default_manifest_path())?;
        let names = report
            .results
            .iter()
            .map(|result| result.name.as_str())
            .collect::<Vec<_>>();

        assert!(report.is_ok(), "report: {report:?}");
        assert_eq!(names.len(), 31);
        assert_eq!(names.first(), Some(&"accepted-check"));
        assert_eq!(names.last(), Some(&"runtime-prior-output"));

        Ok(())
    }
}
