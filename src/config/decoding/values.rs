use std::str;

use super::{ByteBuf, DecodeError};

/// A value that can be decoded from the pinned Luban binary protocol.
pub trait Decode: Sized {
    /// Reads one value from `buf`, returning a contextual error on invalid input.
    fn decode(buf: &mut ByteBuf) -> Result<Self, DecodeError>;
}

impl Decode for bool {
    fn decode(buf: &mut ByteBuf) -> Result<Self, DecodeError> {
        let offset = buf.offset();
        match buf.read_byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(DecodeError::new(offset, "boolean must be 0 or 1")),
        }
    }
}

impl Decode for i32 {
    fn decode(buf: &mut ByteBuf) -> Result<Self, DecodeError> {
        buf.read_int()
    }
}

impl Decode for i64 {
    fn decode(buf: &mut ByteBuf) -> Result<Self, DecodeError> {
        buf.read_long()
    }
}

impl Decode for f32 {
    fn decode(buf: &mut ByteBuf) -> Result<Self, DecodeError> {
        Ok(Self::from_le_bytes(buf.read_array()?))
    }
}

impl Decode for f64 {
    fn decode(buf: &mut ByteBuf) -> Result<Self, DecodeError> {
        Ok(Self::from_le_bytes(buf.read_array()?))
    }
}

impl Decode for String {
    fn decode(buf: &mut ByteBuf) -> Result<Self, DecodeError> {
        let length = buf.read_string_size()?;
        let offset = buf.offset();
        let bytes = buf.read_bytes(length)?;
        let decoded = str::from_utf8(bytes).map_err(|error| {
            DecodeError::new(offset + error.valid_up_to(), "invalid UTF-8 string")
        })?;
        let mut value = String::new();
        value.try_reserve_exact(length).map_err(|error| {
            DecodeError::new(offset, format!("string allocation failed: {error}"))
        })?;
        value.push_str(decoded);
        Ok(value)
    }
}

impl<T: Decode> Decode for Option<T> {
    fn decode(buf: &mut ByteBuf) -> Result<Self, DecodeError> {
        if bool::decode(buf)? {
            Ok(Some(T::decode(buf)?))
        } else {
            Ok(None)
        }
    }
}

impl<T: Decode> Decode for Vec<T> {
    fn decode(buf: &mut ByteBuf) -> Result<Self, DecodeError> {
        let offset = buf.offset();
        let length = buf.read_size()?;
        let mut values = Vec::new();
        values.try_reserve_exact(length).map_err(|error| {
            DecodeError::new(offset, format!("collection allocation failed: {error}"))
        })?;
        for index in 0..length {
            values.push(T::decode(buf).map_err(|error| error.context(format!("element {index}")))?);
        }
        Ok(values)
    }
}
