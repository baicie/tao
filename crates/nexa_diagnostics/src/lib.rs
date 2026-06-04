#![forbid(unsafe_code)]
//! Structured compiler diagnostics.

use nexa_span::SourceSpan;

/// Diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// A fatal or blocking error.
    Error,
    /// A non-blocking warning.
    Warning,
}

/// A source annotation attached to a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    span: SourceSpan,
    message: String,
}

impl Label {
    /// Creates a labeled source span.
    #[must_use]
    pub fn new(span: SourceSpan, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
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
    severity: Severity,
    message: String,
    labels: Vec<Label>,
}

impl Diagnostic {
    /// Creates an error diagnostic.
    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
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
