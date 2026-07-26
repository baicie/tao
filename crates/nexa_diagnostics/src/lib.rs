#![forbid(unsafe_code)]
//! Structured compiler diagnostics.

use nexa_span::SourceSpan;

/// A stable identifier for a class of compiler diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DiagnosticCode(&'static str);

impl DiagnosticCode {
    /// Creates a diagnostic code from a static compiler-owned string.
    #[must_use]
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    /// Returns the diagnostic code text.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl std::fmt::Display for DiagnosticCode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

/// Diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// A fatal or blocking error.
    Error,
    /// A non-blocking warning.
    Warning,
}

/// The role a source label plays in a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelStyle {
    /// The primary location of the problem.
    Primary,
    /// Additional context for the problem.
    Secondary,
}

/// A source annotation attached to a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    style: LabelStyle,
    span: SourceSpan,
    message: String,
}

impl Label {
    /// Creates a primary source label.
    #[must_use]
    pub fn new(span: SourceSpan, message: impl Into<String>) -> Self {
        Self::primary(span, message)
    }

    /// Creates a primary source label.
    #[must_use]
    pub fn primary(span: SourceSpan, message: impl Into<String>) -> Self {
        Self {
            style: LabelStyle::Primary,
            span,
            message: message.into(),
        }
    }

    /// Creates a secondary source label.
    #[must_use]
    pub fn secondary(span: SourceSpan, message: impl Into<String>) -> Self {
        Self {
            style: LabelStyle::Secondary,
            span,
            message: message.into(),
        }
    }

    /// Returns this label's diagnostic role.
    #[must_use]
    pub const fn style(&self) -> LabelStyle {
        self.style
    }

    /// Returns the labeled span.
    #[must_use]
    pub const fn span(&self) -> SourceSpan {
        self.span
    }

    /// Returns the label message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// A compiler diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    code: DiagnosticCode,
    severity: Severity,
    message: String,
    labels: Vec<Label>,
}

impl Diagnostic {
    /// Creates an error diagnostic.
    #[must_use]
    pub fn error(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message: message.into(),
            labels: Vec::new(),
        }
    }

    /// Creates a warning diagnostic.
    #[must_use]
    pub fn warning(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Warning,
            message: message.into(),
            labels: Vec::new(),
        }
    }

    /// Adds a source label to the diagnostic.
    #[must_use]
    pub fn with_label(mut self, label: Label) -> Self {
        self.labels.push(label);
        self
    }

    /// Returns the stable diagnostic code.
    #[must_use]
    pub const fn code(&self) -> DiagnosticCode {
        self.code
    }

    /// Returns the diagnostic severity.
    #[must_use]
    pub const fn severity(&self) -> Severity {
        self.severity
    }

    /// Returns the primary message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns all source labels.
    #[must_use]
    pub fn labels(&self) -> &[Label] {
        &self.labels
    }
}

#[cfg(test)]
mod tests {
    use super::{Diagnostic, DiagnosticCode, Label, LabelStyle, Severity};
    use nexa_span::{FileId, SourceSpan, TextRange};

    #[test]
    fn warning_preserves_its_code_and_secondary_label() {
        let span = SourceSpan::new(FileId::new(0), TextRange::new(1, 2));
        let diagnostic = Diagnostic::warning(DiagnosticCode::new("W0001"), "example warning")
            .with_label(Label::secondary(span, "context"));

        assert_eq!(diagnostic.code().as_str(), "W0001");
        assert_eq!(diagnostic.severity(), Severity::Warning);
        assert_eq!(diagnostic.labels()[0].style(), LabelStyle::Secondary);
    }
}
