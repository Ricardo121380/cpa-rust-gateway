//! Bounded uncompressed gRPC-Web quota projection. Unknown protobuf fields are skipped.
use super::{AccountQuotaWindow, Error, window};

pub(super) fn weekly(bytes: &[u8]) -> Result<AccountQuotaWindow, Error> {
    let mut rest = bytes;
    let mut message = None;
    let mut status = None;
    while !rest.is_empty() {
        let head = rest.get(..5).ok_or(Error::InvalidResponse)?;
        let len =
            u32::from_be_bytes(head[1..5].try_into().map_err(|_| Error::InvalidResponse)?) as usize;
        let payload = rest
            .get(5..5_usize.checked_add(len).ok_or(Error::InvalidResponse)?)
            .ok_or(Error::InvalidResponse)?;
        match head[0] {
            0 if message.is_none() => message = Some(payload),
            128 => {
                let text = std::str::from_utf8(payload).map_err(|_| Error::InvalidResponse)?;
                for line in text.lines() {
                    if let Some((key, value)) = line.split_once(':')
                        && key.eq_ignore_ascii_case("grpc-status")
                    {
                        if status.is_some() {
                            return Err(Error::InvalidResponse);
                        }
                        status = Some(value.trim());
                    }
                }
            }
            _ => return Err(Error::InvalidResponse),
        }
        rest = &rest[5 + len..];
    }
    if status != Some("0") {
        return Err(Error::InvalidResponse);
    }
    let envelope = fields(message.ok_or(Error::InvalidResponse)?)?;
    let config = unique_bytes(&envelope, 1)?.ok_or(Error::InvalidResponse)?;
    let config = fields(config)?;
    let mut percent = None;
    let mut seen = false;
    for (field, value) in &config {
        if *field == 1 {
            if seen {
                return Err(Error::InvalidResponse);
            }
            seen = true;
            let Wire::Fixed32(bits) = value else {
                return Err(Error::InvalidResponse);
            };
            let value = f64::from(f32::from_bits(*bits));
            if !value.is_finite() || value < 0.0 {
                return Err(Error::InvalidResponse);
            }
            percent = Some(value);
        }
    }
    let reset_at = unique_bytes(&config, 5)?.map(timestamp).transpose()?;
    let mut result = window("web.credits", None, None, percent, "percent");
    result.reset_at_ms = reset_at;
    // Product-specific sub-pools are not collapsed into this aggregate.
    Ok(result)
}
#[derive(Clone, Copy)]
enum Wire<'a> {
    Varint(u64),
    Fixed32(u32),
    Bytes(&'a [u8]),
    Fixed64,
}
fn fields(mut bytes: &[u8]) -> Result<Vec<(u64, Wire<'_>)>, Error> {
    let mut result = Vec::new();
    while !bytes.is_empty() {
        if result.len() >= 128 {
            return Err(Error::InvalidResponse);
        }
        let tag = varint(&mut bytes)?;
        if tag >> 3 == 0 {
            return Err(Error::InvalidResponse);
        }
        let value = match tag & 7 {
            0 => Wire::Varint(varint(&mut bytes)?),
            1 => {
                take(&mut bytes, 8)?;
                Wire::Fixed64
            }
            2 => {
                let len =
                    usize::try_from(varint(&mut bytes)?).map_err(|_| Error::InvalidResponse)?;
                Wire::Bytes(take(&mut bytes, len)?)
            }
            5 => Wire::Fixed32(u32::from_le_bytes(
                take(&mut bytes, 4)?
                    .try_into()
                    .map_err(|_| Error::InvalidResponse)?,
            )),
            _ => return Err(Error::InvalidResponse),
        };
        result.push((tag >> 3, value));
    }
    Ok(result)
}
fn take<'a>(bytes: &mut &'a [u8], len: usize) -> Result<&'a [u8], Error> {
    let value = bytes.get(..len).ok_or(Error::InvalidResponse)?;
    *bytes = &bytes[len..];
    Ok(value)
}
fn varint(bytes: &mut &[u8]) -> Result<u64, Error> {
    let mut value = 0;
    for shift in (0..70).step_by(7) {
        let byte = take(bytes, 1)?[0];
        if shift == 63 && byte > 1 {
            return Err(Error::InvalidResponse);
        }
        value |= u64::from(byte & 127) << shift;
        if byte & 128 == 0 {
            return Ok(value);
        }
    }
    Err(Error::InvalidResponse)
}
fn unique_bytes<'a>(fields: &[(u64, Wire<'a>)], number: u64) -> Result<Option<&'a [u8]>, Error> {
    let mut result = None;
    for (field, value) in fields {
        if *field == number {
            if result.is_some() {
                return Err(Error::InvalidResponse);
            }
            let Wire::Bytes(value) = value else {
                return Err(Error::InvalidResponse);
            };
            result = Some(*value);
        }
    }
    Ok(result)
}
fn timestamp(bytes: &[u8]) -> Result<i64, Error> {
    let fields = fields(bytes)?;
    let mut seconds = None;
    let mut nanos = None;
    for (field, value) in fields {
        if field == 1 || field == 2 {
            let Wire::Varint(value) = value else {
                return Err(Error::InvalidResponse);
            };
            let slot = if field == 1 { &mut seconds } else { &mut nanos };
            if slot.replace(value).is_some() {
                return Err(Error::InvalidResponse);
            }
        }
    }
    let nanos = nanos.unwrap_or(0);
    if nanos >= 1_000_000_000 {
        return Err(Error::InvalidResponse);
    }
    let millis = seconds
        .ok_or(Error::InvalidResponse)?
        .checked_mul(1000)
        .and_then(|v| v.checked_add(nanos / 1_000_000))
        .ok_or(Error::InvalidResponse)?;
    i64::try_from(millis).map_err(|_| Error::InvalidResponse)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn frame(config: &[u8], status: u8) -> Vec<u8> {
        let mut message = vec![10, u8::try_from(config.len()).unwrap_or(0)];
        message.extend(config);
        let mut result = vec![0, 0, 0, 0, u8::try_from(message.len()).unwrap_or(0)];
        result.extend(message);
        let mut trailer = b"grpc-status: 0\r\n".to_vec();
        trailer[13] = status;
        result.extend([128, 0, 0, 0, u8::try_from(trailer.len()).unwrap_or(0)]);
        result.extend(trailer);
        result
    }
    #[test]
    fn quota_frames_require_valid_status_and_preserve_absent_percentage() {
        let mut config = vec![13];
        config.extend(25_f32.to_le_bytes());
        config.extend([42, 2, 8, 10]);
        let value = weekly(&frame(&config, b'0')).unwrap_or_else(|_| unreachable!());
        assert_eq!(value.used_percent, Some(25.0));
        assert_eq!(value.reset_at_ms, Some(10_000));
        assert!(weekly(&frame(&config, b'7')).is_err());
        assert!(weekly(&[0, 0, 0, 0, 10]).is_err());
        let missing = weekly(&frame(&[42, 2, 8, 10], b'0')).unwrap_or_else(|_| unreachable!());
        assert_eq!(missing.used_percent, None);
    }
}
