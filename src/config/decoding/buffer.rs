use super::DecodeError;

/// Per-value bounds applied before allocating variable-length configuration data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeLimits {
    /// Maximum number of elements in one collection or rows in one table.
    pub max_collection_len: usize,
    /// Maximum UTF-8 byte length of one string.
    pub max_string_bytes: usize,
}

impl Default for DecodeLimits {
    fn default() -> Self {
        Self {
            max_collection_len: 1_000_000,
            max_string_bytes: 16 * 1024 * 1024,
        }
    }
}

/// An owned, bounds-checked cursor over one Luban binary data file.
#[derive(Debug)]
pub struct ByteBuf {
    bytes: Vec<u8>,
    offset: usize,
    limits: DecodeLimits,
}

impl ByteBuf {
    /// Takes ownership of `bytes` and returns a cursor using default limits.
    pub fn new(bytes: Vec<u8>) -> Self {
        Self::with_limits(bytes, DecodeLimits::default())
    }

    /// Takes ownership of `bytes` and returns a cursor using the supplied limits.
    pub fn with_limits(bytes: Vec<u8>, limits: DecodeLimits) -> Self {
        Self {
            bytes,
            offset: 0,
            limits,
        }
    }

    /// Returns the byte offset of the next unread byte.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the number of bytes that have not been read.
    pub fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    /// Reads a nonnegative collection size, returning an error above its limit.
    pub fn read_size(&mut self) -> Result<usize, DecodeError> {
        self.read_length(self.limits.max_collection_len)
    }

    /// Returns success only when the complete file has been consumed.
    pub fn finish(&self) -> Result<(), DecodeError> {
        if self.remaining() != 0 {
            return Err(DecodeError::new(
                self.offset,
                format!("{} trailing bytes remain", self.remaining()),
            ));
        }
        Ok(())
    }

    pub(super) fn read_string_size(&mut self) -> Result<usize, DecodeError> {
        self.read_length(self.limits.max_string_bytes)
    }

    fn read_length(&mut self, limit: usize) -> Result<usize, DecodeError> {
        let offset = self.offset;
        let length = self.read_int()?;
        let length =
            usize::try_from(length).map_err(|_| DecodeError::new(offset, "negative length"))?;
        if length > limit {
            return Err(DecodeError::new(
                offset,
                format!("length {length} exceeds limit {limit}"),
            ));
        }
        Ok(length)
    }

    pub(super) fn read_bytes(&mut self, count: usize) -> Result<&[u8], DecodeError> {
        let offset = self.offset;
        let end = offset
            .checked_add(count)
            .ok_or_else(|| DecodeError::new(offset, "requested byte count overflows the cursor"))?;
        let bytes = self.bytes.get(offset..end).ok_or_else(|| {
            DecodeError::new(
                offset,
                format!("need {count} bytes, only {} remain", self.remaining()),
            )
        })?;
        self.offset = end;
        Ok(bytes)
    }

    pub(super) fn read_array<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        let mut value = [0; N];
        value.copy_from_slice(self.read_bytes(N)?);
        Ok(value)
    }

    pub(super) fn read_byte(&mut self) -> Result<u8, DecodeError> {
        let [value] = self.read_array()?;
        Ok(value)
    }

    pub(super) fn read_int(&mut self) -> Result<i32, DecodeError> {
        let offset = self.offset;
        let head = self.read_byte()?;
        let (tail_len, mask) = match head {
            0x00..=0x7f => (0, 0x7f),
            0x80..=0xbf => (1, 0x3f),
            0xc0..=0xdf => (2, 0x1f),
            0xe0..=0xef => (3, 0x0f),
            0xf0 => (4, 0),
            _ => return Err(DecodeError::new(offset, "invalid int prefix")),
        };
        let mut value = u32::from(head & mask);
        for byte in self.read_bytes(tail_len)? {
            value = (value << 8) | u32::from(*byte);
        }
        Ok(value as i32)
    }

    pub(super) fn read_long(&mut self) -> Result<i64, DecodeError> {
        let head = self.read_byte()?;
        let (tail_len, mask) = match head {
            0x00..=0x7f => (0, 0x7f),
            0x80..=0xbf => (1, 0x3f),
            0xc0..=0xdf => (2, 0x1f),
            0xe0..=0xef => (3, 0x0f),
            0xf0..=0xf7 => (4, 0x07),
            0xf8..=0xfb => (5, 0x03),
            0xfc..=0xfd => (6, 0x01),
            0xfe => (7, 0),
            0xff => (8, 0),
        };
        let mut value = u64::from(head & mask);
        for byte in self.read_bytes(tail_len)? {
            value = (value << 8) | u64::from(*byte);
        }
        Ok(value as i64)
    }
}
