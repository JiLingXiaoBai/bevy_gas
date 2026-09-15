//! Binary package loading and generated table decoding.

use super::decoding::ByteBuf;
use super::generated::{LubanError, Tables};
use super::{ConfigError, ConfigErrorKind, ConfigLocation, read_package};

mod files;

pub use files::load_tables;
