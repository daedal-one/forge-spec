use std::fmt;
use std::path::PathBuf;

use colored::Colorize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Warning => write!(f, "warning"),
            Self::Error => write!(f, "error"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub file: PathBuf,
    pub line: Option<usize>,
    pub detail: Option<String>,
}

impl Diagnostic {
    pub fn error(code: &str, message: impl Into<String>, file: PathBuf) -> Self {
        Self {
            code: code.to_string(),
            severity: Severity::Error,
            message: message.into(),
            file,
            line: None,
            detail: None,
        }
    }

    pub fn warning(code: &str, message: impl Into<String>, file: PathBuf) -> Self {
        Self {
            code: code.to_string(),
            severity: Severity::Warning,
            message: message.into(),
            file,
            line: None,
            detail: None,
        }
    }

    pub fn info(code: &str, message: impl Into<String>, file: PathBuf) -> Self {
        Self {
            code: code.to_string(),
            severity: Severity::Info,
            message: message.into(),
            file,
            line: None,
            detail: None,
        }
    }

    pub fn at_line(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Downgrade error to warning (used for `status: draft`).
    pub fn downgrade(&mut self) {
        if self.severity == Severity::Error {
            self.severity = Severity::Warning;
        }
    }

    pub fn display_colored(&self) -> String {
        let sev_str = match self.severity {
            Severity::Error => format!("error[{}]", self.code).red().bold().to_string(),
            Severity::Warning => format!("warning[{}]", self.code)
                .yellow()
                .bold()
                .to_string(),
            Severity::Info => format!("info[{}]", self.code).blue().bold().to_string(),
        };

        let location = match self.line {
            Some(line) => format!("{}:{}", self.file.display(), line),
            None => format!("{}", self.file.display()),
        };

        let mut result = format!("{}: {}\n  --> {}", sev_str, self.message, location);
        if let Some(ref detail) = self.detail {
            result.push_str(&format!("\n  = {detail}"));
        }
        result
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.display_colored())
    }
}
