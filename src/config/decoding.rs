//! Bounds-checked decoding, error context, and primitive implementations.

#![forbid(unsafe_code)]

mod buffer;
mod error;
mod values;

pub use buffer::{ByteBuf, DecodeLimits};
pub use error::DecodeError;
pub use values::Decode;
