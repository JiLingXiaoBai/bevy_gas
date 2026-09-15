use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

/// Machine-readable category of a configuration or configured-ability failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigErrorKind {
    /// Reading or writing a configuration file failed.
    Io,
    /// A file, package, or decoder exceeded a supported capacity.
    Capacity,
    /// Binary data could not be decoded.
    Decode,
    /// A manifest, schema, hash, or generated table contract is invalid.
    Package,
    /// Optional authoring validation rejected a gameplay rule.
    Validation,
    /// A required ECS resource is missing.
    MissingResource,
    /// A name could not be registered consistently with existing registries.
    Registration,
    /// A referenced configuration definition is absent.
    Reference,
    /// A value cannot be represented by the runtime definition.
    InvalidValue,
    /// The requested catalog ability is absent.
    UnknownAbility,
    /// The requested grant or inspection level is unsupported.
    UnsupportedLevel,
    /// The owner already has a binding for this ability.
    AlreadyGranted,
    /// An active ability cannot be revoked.
    ActiveAbility,
    /// An inspection report could not be formatted.
    Report,
}

/// Structured source of a configuration diagnostic.
///
/// Row keys preserve authored IDs or names, while fields may identify a nested
/// field path. Binary offsets are present only for decoding failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigLocation {
    /// A file or a field inside that file.
    File {
        /// The input or output filesystem path.
        path: PathBuf,
        /// The optional field path inside the file.
        field: Option<String>,
        /// The optional byte offset of a binary decoding failure.
        byte_offset: Option<usize>,
    },
    /// A generated table, optionally narrowed to one row and field.
    Table {
        /// The authored table name.
        table: &'static str,
        /// The authored row ID or stable name.
        row: Option<String>,
        /// The optional authored field path.
        field: Option<String>,
    },
    /// A required or conflicting ECS registry.
    Resource(&'static str),
    /// A configuration operation without a single source row or file.
    Operation(&'static str),
}

impl ConfigLocation {
    pub(crate) fn file(path: impl AsRef<Path>) -> Self {
        Self::File {
            path: path.as_ref().to_owned(),
            field: None,
            byte_offset: None,
        }
    }

    pub(crate) fn table(table: &'static str) -> Self {
        Self::Table {
            table,
            row: None,
            field: None,
        }
    }

    pub(crate) fn row(mut self, value: impl ToString) -> Self {
        if let Self::Table { row, .. } = &mut self {
            *row = Some(value.to_string());
        }
        self
    }

    pub(crate) fn field(&self, value: impl Into<String>) -> Self {
        let mut location = self.clone();
        if let Self::File { field, .. } | Self::Table { field, .. } = &mut location {
            let value = value.into();
            *field = Some(match field.take() {
                Some(parent) => format!("{parent}.{value}"),
                None => value,
            });
        }
        location
    }

    pub(crate) fn at_byte(mut self, offset: usize) -> Self {
        if let Self::File { byte_offset, .. } = &mut self {
            *byte_offset = Some(offset);
        }
        self
    }
}

impl From<&ConfigLocation> for ConfigLocation {
    fn from(value: &ConfigLocation) -> Self {
        value.clone()
    }
}

impl fmt::Display for ConfigLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File {
                path,
                field,
                byte_offset,
            } => {
                write!(f, "{}", path.display())?;
                if let Some(field) = field {
                    write!(f, ".{field}")?;
                }
                if let Some(offset) = byte_offset {
                    write!(f, ": byte {offset}")?;
                }
                Ok(())
            }
            Self::Table { table, row, field } => {
                f.write_str(table)?;
                if let Some(row) = row {
                    write!(f, "[{row}]")?;
                }
                if let Some(field) = field {
                    write!(f, ".{field}")?;
                }
                Ok(())
            }
            Self::Resource(name) | Self::Operation(name) => f.write_str(name),
        }
    }
}

/// A classified failure with structured source location and a readable message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    kind: ConfigErrorKind,
    location: ConfigLocation,
    message: String,
}

impl ConfigError {
    pub(crate) fn new(
        kind: ConfigErrorKind,
        location: impl Into<ConfigLocation>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            location: location.into(),
            message: message.into(),
        }
    }

    /// Returns the stable error category for programmatic handling.
    pub fn kind(&self) -> ConfigErrorKind {
        self.kind
    }

    /// Returns the structured file, table-row-field, or ECS resource location.
    pub fn location(&self) -> &ConfigLocation {
        &self.location
    }

    /// Returns the readable explanation of the failure.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.location, self.message)
    }
}

impl Error for ConfigError {}
