use std::error::Error;
use std::fmt;

/// A configuration failure identifying the table, row, field, or file involved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    context: String,
    message: String,
}

impl ConfigError {
    pub(crate) fn new(context: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            context: context.into(),
            message: message.into(),
        }
    }
    /// Returns the relevant table-row-field or file context.
    pub fn context(&self) -> &str {
        &self.context
    }
    /// Returns the actionable failure description.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.context, self.message)
    }
}
impl Error for ConfigError {}
