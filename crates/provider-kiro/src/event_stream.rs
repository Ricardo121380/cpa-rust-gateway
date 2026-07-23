//! Bounded incremental AWS EventStream framing for Kiro response bodies.
//!
//! This module only validates and splits the binary envelope. It deliberately does not interpret
//! Kiro payload JSON, create Canonical events, classify upstream errors, or open a network
//! connection. Later P7 work owns those semantic steps.

use std::{collections::BTreeMap, error::Error, fmt};

const PRELUDE_BYTES: usize = 12;
const MESSAGE_CRC_BYTES: usize = 4;
const MIN_FRAME_BYTES: usize = PRELUDE_BYTES + MESSAGE_CRC_BYTES;
const MAX_FRAME_BYTES: usize = 16 * 1024 * 1024;
const MAX_BUFFER_BYTES: usize = MAX_FRAME_BYTES * 2;
const MAX_CONSECUTIVE_ERRORS: usize = 5;

/// One parsed AWS `EventStream` header value.
#[derive(Clone, Eq, PartialEq)]
pub enum KiroEventStreamHeaderValue {
    /// A true boolean header.
    BoolTrue,
    /// A false boolean header.
    BoolFalse,
    /// A signed byte header.
    Byte(i8),
    /// A big-endian signed 16-bit header.
    Int16(i16),
    /// A big-endian signed 32-bit header.
    Int32(i32),
    /// A big-endian signed 64-bit header.
    Int64(i64),
    /// A length-prefixed raw-byte header.
    ByteArray(Vec<u8>),
    /// A strictly UTF-8 length-prefixed header.
    String(String),
    /// An epoch-millis signed 64-bit header.
    Timestamp(i64),
    /// A 16-byte UUID header.
    Uuid([u8; 16]),
}

impl KiroEventStreamHeaderValue {
    /// Returns the contained string only when this header has AWS string type.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            Self::BoolTrue
            | Self::BoolFalse
            | Self::Byte(_)
            | Self::Int16(_)
            | Self::Int32(_)
            | Self::Int64(_)
            | Self::ByteArray(_)
            | Self::Timestamp(_)
            | Self::Uuid(_) => None,
        }
    }
}

impl fmt::Debug for KiroEventStreamHeaderValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::BoolTrue => "BoolTrue",
            Self::BoolFalse => "BoolFalse",
            Self::Byte(_) => "Byte",
            Self::Int16(_) => "Int16",
            Self::Int32(_) => "Int32",
            Self::Int64(_) => "Int64",
            Self::ByteArray(_) => "ByteArray",
            Self::String(_) => "String",
            Self::Timestamp(_) => "Timestamp",
            Self::Uuid(_) => "Uuid",
        };
        formatter.write_str(kind)
    }
}

/// One duplicate-free collection of AWS `EventStream` headers.
#[derive(Clone, Default, Eq, PartialEq)]
pub struct KiroEventStreamHeaders(BTreeMap<String, KiroEventStreamHeaderValue>);

impl KiroEventStreamHeaders {
    /// Returns one header by its exact AWS `EventStream` name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&KiroEventStreamHeaderValue> {
        self.0.get(name)
    }

    /// Returns the exact string header value, if present.
    #[must_use]
    pub fn get_string(&self, name: &str) -> Option<&str> {
        self.get(name).and_then(KiroEventStreamHeaderValue::as_str)
    }

    /// Returns the standard AWS `:message-type` header only when it is a string.
    #[must_use]
    pub fn message_type(&self) -> Option<&str> {
        self.get_string(":message-type")
    }

    /// Returns the standard AWS `:event-type` header only when it is a string.
    #[must_use]
    pub fn event_type(&self) -> Option<&str> {
        self.get_string(":event-type")
    }

    /// Returns the standard AWS `:exception-type` header only when it is a string.
    #[must_use]
    pub fn exception_type(&self) -> Option<&str> {
        self.get_string(":exception-type")
    }

    /// Returns the number of decoded headers without exposing their values in diagnostics.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns whether this frame carried no headers.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for KiroEventStreamHeaders {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KiroEventStreamHeaders")
            .field("count", &self.len())
            .finish()
    }
}

/// One CRC-verified Kiro AWS `EventStream` frame.
#[derive(Clone, Eq, PartialEq)]
pub struct KiroEventStreamFrame {
    headers: KiroEventStreamHeaders,
    payload: Vec<u8>,
}

impl KiroEventStreamFrame {
    /// Returns the validated frame headers.
    #[must_use]
    pub fn headers(&self) -> &KiroEventStreamHeaders {
        &self.headers
    }

    /// Returns the raw Kiro payload for a later semantic decoder.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

impl fmt::Debug for KiroEventStreamFrame {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KiroEventStreamFrame")
            .field("header_count", &self.headers.len())
            .field("payload_len", &self.payload.len())
            .finish()
    }
}

/// Safe, value-free AWS `EventStream` framing failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KiroEventStreamError {
    /// The prelude's total frame length is outside the fixed safe bounds.
    InvalidFrameLength,
    /// The declared header region cannot fit inside the decoded frame envelope.
    InvalidHeaderLength,
    /// The prelude checksum does not match its first eight bytes.
    PreludeCrcMismatch,
    /// The final message checksum does not match the preceding frame bytes.
    MessageCrcMismatch,
    /// A header name was empty, truncated, or not valid UTF-8.
    InvalidHeaderName,
    /// A header used an AWS type code outside the defined range.
    InvalidHeaderType,
    /// A header value was truncated or invalid UTF-8.
    InvalidHeaderValue,
    /// A frame repeated a header name and would otherwise overwrite a value.
    DuplicateHeader,
    /// Buffered bytes exceeded the fixed parser bound.
    BufferLimitExceeded,
    /// The input ended with a partial frame.
    TruncatedFrame,
    /// Consecutive malformed frames exceeded the safe recovery limit.
    TooManyErrors,
    /// The decoder is terminal after a fatal input condition.
    Stopped,
}

impl fmt::Display for KiroEventStreamError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidFrameLength => "Kiro EventStream frame length is invalid",
            Self::InvalidHeaderLength => "Kiro EventStream header length is invalid",
            Self::PreludeCrcMismatch => "Kiro EventStream prelude CRC does not match",
            Self::MessageCrcMismatch => "Kiro EventStream message CRC does not match",
            Self::InvalidHeaderName => "Kiro EventStream header name is invalid",
            Self::InvalidHeaderType => "Kiro EventStream header type is invalid",
            Self::InvalidHeaderValue => "Kiro EventStream header value is invalid",
            Self::DuplicateHeader => "Kiro EventStream duplicate header is invalid",
            Self::BufferLimitExceeded => "Kiro EventStream buffer limit exceeded",
            Self::TruncatedFrame => "Kiro EventStream ended with a partial frame",
            Self::TooManyErrors => "Kiro EventStream recovery limit exceeded",
            Self::Stopped => "Kiro EventStream decoder is stopped",
        })
    }
}

impl Error for KiroEventStreamError {}

/// Bounded incremental decoder for a Kiro AWS `EventStream` response body.
///
/// A framing error is returned to the caller but does not discard a later valid frame: malformed
/// preludes resynchronize at a later valid prelude and data-phase failures discard only their
/// already length- and prelude-validated frame. Five consecutive framing failures stop the
/// decoder, preventing unbounded adversarial scanning.
pub struct KiroEventStreamDecoder {
    buffer: Vec<u8>,
    consecutive_errors: usize,
    stopped: bool,
}

impl Default for KiroEventStreamDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl KiroEventStreamDecoder {
    /// Creates an empty decoder with fixed frame, buffer, and recovery bounds.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            buffer: Vec::new(),
            consecutive_errors: 0,
            stopped: false,
        }
    }

    /// Adds one transport chunk without interpreting its payload.
    ///
    /// # Errors
    ///
    /// Returns a terminal safe error when a stopped decoder receives more data or the bounded
    /// buffer would overflow. A successfully accepted chunk may still contain a later framing
    /// error, which is reported by [`Self::next_frame`].
    pub fn feed(&mut self, bytes: &[u8]) -> Result<(), KiroEventStreamError> {
        if self.stopped {
            return Err(KiroEventStreamError::Stopped);
        }
        let Some(new_len) = self.buffer.len().checked_add(bytes.len()) else {
            return self.stop(KiroEventStreamError::BufferLimitExceeded);
        };
        if new_len > MAX_BUFFER_BYTES {
            return self.stop(KiroEventStreamError::BufferLimitExceeded);
        }
        self.buffer.extend_from_slice(bytes);
        Ok(())
    }

    /// Decodes one available frame, or returns `None` until more bytes arrive.
    ///
    /// A recoverable error consumes corrupted bytes. Callers may invoke this method again to
    /// receive a valid following frame without refeeding prior chunks.
    ///
    /// # Errors
    ///
    /// Returns the next value-free framing error after applying bounded recovery, or `Stopped`
    /// after a terminal buffer, truncation, or consecutive-error condition.
    pub fn next_frame(&mut self) -> Result<Option<KiroEventStreamFrame>, KiroEventStreamError> {
        if self.stopped {
            return Err(KiroEventStreamError::Stopped);
        }
        let Some(prelude) = self.try_prelude()? else {
            return Ok(None);
        };

        if self.buffer.len() < prelude.total_length {
            return Ok(None);
        }

        let message_without_crc = prelude.total_length - MESSAGE_CRC_BYTES;
        let expected_message_crc =
            read_u32(&self.buffer[message_without_crc..prelude.total_length]);
        if crc32(&self.buffer[..message_without_crc]) != expected_message_crc {
            self.discard(prelude.total_length);
            return self.recover(KiroEventStreamError::MessageCrcMismatch);
        }

        let headers_start = PRELUDE_BYTES;
        let headers_end = headers_start + prelude.headers_length;
        let headers = match parse_headers(&self.buffer[headers_start..headers_end]) {
            Ok(headers) => headers,
            Err(error) => {
                self.discard(prelude.total_length);
                return self.recover(error);
            }
        };
        let payload = self.buffer[headers_end..message_without_crc].to_vec();
        self.discard(prelude.total_length);
        self.consecutive_errors = 0;
        Ok(Some(KiroEventStreamFrame { headers, payload }))
    }

    /// Marks the byte source complete after callers have drained [`Self::next_frame`].
    ///
    /// # Errors
    ///
    /// Returns a terminal error when a partial frame remains. An empty fully drained decoder
    /// succeeds; a stopped decoder remains stopped.
    pub fn finish(&mut self) -> Result<(), KiroEventStreamError> {
        if self.stopped {
            return Err(KiroEventStreamError::Stopped);
        }
        if self.buffer.is_empty() {
            return Ok(());
        }
        self.stop(KiroEventStreamError::TruncatedFrame)
    }

    fn try_prelude(&mut self) -> Result<Option<Prelude>, KiroEventStreamError> {
        if self.buffer.len() < PRELUDE_BYTES {
            return Ok(None);
        }
        match parse_prelude(&self.buffer[..PRELUDE_BYTES]) {
            Ok(prelude) => Ok(Some(prelude)),
            Err(error) => {
                self.resynchronize_prelude();
                self.recover(error)
            }
        }
    }

    fn recover<T>(&mut self, error: KiroEventStreamError) -> Result<T, KiroEventStreamError> {
        self.consecutive_errors += 1;
        if self.consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
            return self.stop(KiroEventStreamError::TooManyErrors);
        }
        Err(error)
    }

    fn stop<T>(&mut self, error: KiroEventStreamError) -> Result<T, KiroEventStreamError> {
        self.buffer.clear();
        self.stopped = true;
        Err(error)
    }

    fn discard(&mut self, count: usize) {
        self.buffer.drain(..count);
    }

    fn resynchronize_prelude(&mut self) {
        let last_candidate = self.buffer.len().saturating_sub(PRELUDE_BYTES);
        let next_prelude = (1..=last_candidate)
            .find(|start| parse_prelude(&self.buffer[*start..*start + PRELUDE_BYTES]).is_ok());
        let discard = next_prelude.unwrap_or_else(|| self.buffer.len() - (PRELUDE_BYTES - 1));
        self.discard(discard);
    }
}

#[derive(Clone, Copy)]
struct Prelude {
    total_length: usize,
    headers_length: usize,
}

fn parse_prelude(bytes: &[u8]) -> Result<Prelude, KiroEventStreamError> {
    let total_length = read_u32(&bytes[..4]) as usize;
    let headers_length = read_u32(&bytes[4..8]) as usize;
    if !(MIN_FRAME_BYTES..=MAX_FRAME_BYTES).contains(&total_length) {
        return Err(KiroEventStreamError::InvalidFrameLength);
    }
    if headers_length > total_length - MIN_FRAME_BYTES {
        return Err(KiroEventStreamError::InvalidHeaderLength);
    }
    if crc32(&bytes[..8]) != read_u32(&bytes[8..PRELUDE_BYTES]) {
        return Err(KiroEventStreamError::PreludeCrcMismatch);
    }
    Ok(Prelude {
        total_length,
        headers_length,
    })
}

fn parse_headers(bytes: &[u8]) -> Result<KiroEventStreamHeaders, KiroEventStreamError> {
    let mut cursor = 0;
    let mut headers = BTreeMap::new();
    while cursor < bytes.len() {
        let name_length = read_byte(bytes, &mut cursor)? as usize;
        if name_length == 0 {
            return Err(KiroEventStreamError::InvalidHeaderName);
        }
        let name = read_utf8(
            bytes,
            &mut cursor,
            name_length,
            KiroEventStreamError::InvalidHeaderName,
        )?;
        let header_type = read_byte(bytes, &mut cursor)?;
        let value = parse_header_value(bytes, &mut cursor, header_type)?;
        if headers.insert(name, value).is_some() {
            return Err(KiroEventStreamError::DuplicateHeader);
        }
    }
    Ok(KiroEventStreamHeaders(headers))
}

fn parse_header_value(
    bytes: &[u8],
    cursor: &mut usize,
    header_type: u8,
) -> Result<KiroEventStreamHeaderValue, KiroEventStreamError> {
    match header_type {
        0 => Ok(KiroEventStreamHeaderValue::BoolTrue),
        1 => Ok(KiroEventStreamHeaderValue::BoolFalse),
        2 => Ok(KiroEventStreamHeaderValue::Byte(
            read_byte(bytes, cursor)?.cast_signed(),
        )),
        3 => Ok(KiroEventStreamHeaderValue::Int16(read_i16(bytes, cursor)?)),
        4 => Ok(KiroEventStreamHeaderValue::Int32(read_i32(bytes, cursor)?)),
        5 => Ok(KiroEventStreamHeaderValue::Int64(read_i64(bytes, cursor)?)),
        6 => Ok(KiroEventStreamHeaderValue::ByteArray(read_variable_bytes(
            bytes, cursor,
        )?)),
        7 => {
            let value = read_variable_bytes(bytes, cursor)?;
            let value =
                String::from_utf8(value).map_err(|_| KiroEventStreamError::InvalidHeaderValue)?;
            Ok(KiroEventStreamHeaderValue::String(value))
        }
        8 => Ok(KiroEventStreamHeaderValue::Timestamp(read_i64(
            bytes, cursor,
        )?)),
        9 => {
            let value = read_exact(bytes, cursor, 16)?;
            let mut uuid = [0; 16];
            uuid.copy_from_slice(value);
            Ok(KiroEventStreamHeaderValue::Uuid(uuid))
        }
        _ => Err(KiroEventStreamError::InvalidHeaderType),
    }
}

fn read_byte(bytes: &[u8], cursor: &mut usize) -> Result<u8, KiroEventStreamError> {
    Ok(read_exact(bytes, cursor, 1)?[0])
}

fn read_i16(bytes: &[u8], cursor: &mut usize) -> Result<i16, KiroEventStreamError> {
    Ok(i16::from_be_bytes(
        read_exact(bytes, cursor, 2)?
            .try_into()
            .map_err(|_| KiroEventStreamError::InvalidHeaderValue)?,
    ))
}

fn read_i32(bytes: &[u8], cursor: &mut usize) -> Result<i32, KiroEventStreamError> {
    Ok(i32::from_be_bytes(
        read_exact(bytes, cursor, 4)?
            .try_into()
            .map_err(|_| KiroEventStreamError::InvalidHeaderValue)?,
    ))
}

fn read_i64(bytes: &[u8], cursor: &mut usize) -> Result<i64, KiroEventStreamError> {
    Ok(i64::from_be_bytes(
        read_exact(bytes, cursor, 8)?
            .try_into()
            .map_err(|_| KiroEventStreamError::InvalidHeaderValue)?,
    ))
}

fn read_variable_bytes(bytes: &[u8], cursor: &mut usize) -> Result<Vec<u8>, KiroEventStreamError> {
    let length = u16::from_be_bytes(
        read_exact(bytes, cursor, 2)?
            .try_into()
            .map_err(|_| KiroEventStreamError::InvalidHeaderValue)?,
    ) as usize;
    Ok(read_exact(bytes, cursor, length)?.to_vec())
}

fn read_utf8(
    bytes: &[u8],
    cursor: &mut usize,
    length: usize,
    error: KiroEventStreamError,
) -> Result<String, KiroEventStreamError> {
    let Some(end) = cursor.checked_add(length) else {
        return Err(error);
    };
    let Some(value) = bytes.get(*cursor..end) else {
        return Err(error);
    };
    *cursor = end;
    std::str::from_utf8(value)
        .map(str::to_owned)
        .map_err(|_| error)
}

fn read_exact<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    length: usize,
) -> Result<&'a [u8], KiroEventStreamError> {
    let Some(end) = cursor.checked_add(length) else {
        return Err(KiroEventStreamError::InvalidHeaderValue);
    };
    let Some(value) = bytes.get(*cursor..end) else {
        return Err(KiroEventStreamError::InvalidHeaderValue);
    };
    *cursor = end;
    Ok(value)
}

fn read_u32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 0 {
                crc >> 1
            } else {
                (crc >> 1) ^ 0xedb8_8320
            };
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::crc32;

    #[test]
    fn crc32_uses_the_aws_eventstream_iso_hdlc_variant() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }
}
