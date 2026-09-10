use std::{error::Error, fmt};

/// A binary decoding failure with the byte offset and optional field context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeError {
    offset: usize,
    message: String,
    contexts: Vec<String>,
}

impl DecodeError {
    /// Creates an error at `offset` with the supplied diagnostic `message`.
    pub fn new(offset: usize, message: impl Into<String>) -> Self {
        Self {
            offset,
            message: message.into(),
            contexts: Vec::new(),
        }
    }

    /// Adds an enclosing file, table, record, or field name and returns the error.
    pub fn context(mut self, context: impl Into<String>) -> Self {
        self.contexts.push(context.into());
        self
    }

    /// Returns the byte offset at which decoding failed.
    pub fn offset(&self) -> usize {
        self.offset
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for context in self.contexts.iter().rev() {
            write!(formatter, "{context}: ")?;
        }
        write!(formatter, "byte {}: {}", self.offset, self.message)
    }
}

impl Error for DecodeError {}
