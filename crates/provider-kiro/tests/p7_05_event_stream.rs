//! P7-05 Kiro AWS `EventStream` framing, CRC, chunk, and recovery fixtures.

use std::error::Error;

use provider_kiro::event_stream::{
    KiroEventStreamDecoder, KiroEventStreamError, KiroEventStreamFrame, KiroEventStreamHeaderValue,
};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn every_binary_split_and_one_byte_chunks_produce_the_same_frames() -> TestResult {
    let first = wire_frame(
        &[
            string_header(":message-type", "event"),
            string_header(":event-type", "assistantResponseEvent"),
        ],
        b"fixture-first",
    );
    let second = wire_frame(
        &[
            string_header(":message-type", "event"),
            string_header(":event-type", "contextUsageEvent"),
        ],
        b"fixture-second",
    );
    let wire = [first, second].concat();
    let baseline = decode_chunks(&wire, &[wire.len()])?;

    for split in 0..=wire.len() {
        assert_eq!(
            decode_chunks(&wire, &[split, wire.len() - split])?,
            baseline
        );
    }
    assert_eq!(decode_chunks(&wire, &vec![1; wire.len()])?, baseline);
    Ok(())
}

#[test]
fn full_aws_header_types_and_payload_are_retained_without_debug_leaks() -> TestResult {
    let frame = wire_frame(
        &[
            typed_header("true", 0, &[]),
            typed_header("false", 1, &[]),
            typed_header("byte", 2, &[0xfe]),
            typed_header("int16", 3, &(-2_i16).to_be_bytes()),
            typed_header("int32", 4, &(-3_i32).to_be_bytes()),
            typed_header("int64", 5, &(-4_i64).to_be_bytes()),
            typed_header("bytes", 6, &length_prefixed(b"raw-value")),
            string_header("text", "do-not-debug-header"),
            typed_header("time", 8, &123_i64.to_be_bytes()),
            typed_header("uuid", 9, &[7; 16]),
        ],
        b"do-not-debug-payload",
    );
    let mut decoder = KiroEventStreamDecoder::new();
    decoder.feed(&frame)?;
    let frame = decoder
        .next_frame()?
        .ok_or("fixture frame was not decoded")?;

    assert_eq!(
        frame.headers().get("true"),
        Some(&KiroEventStreamHeaderValue::BoolTrue)
    );
    assert_eq!(
        frame.headers().get("false"),
        Some(&KiroEventStreamHeaderValue::BoolFalse)
    );
    assert_eq!(
        frame.headers().get("byte"),
        Some(&KiroEventStreamHeaderValue::Byte(-2))
    );
    assert_eq!(
        frame.headers().get("int16"),
        Some(&KiroEventStreamHeaderValue::Int16(-2))
    );
    assert_eq!(
        frame.headers().get("int32"),
        Some(&KiroEventStreamHeaderValue::Int32(-3))
    );
    assert_eq!(
        frame.headers().get("int64"),
        Some(&KiroEventStreamHeaderValue::Int64(-4))
    );
    assert_eq!(
        frame.headers().get("bytes"),
        Some(&KiroEventStreamHeaderValue::ByteArray(
            b"raw-value".to_vec()
        ))
    );
    assert_eq!(
        frame.headers().get_string("text"),
        Some("do-not-debug-header")
    );
    assert_eq!(
        frame.headers().get("time"),
        Some(&KiroEventStreamHeaderValue::Timestamp(123))
    );
    assert_eq!(
        frame.headers().get("uuid"),
        Some(&KiroEventStreamHeaderValue::Uuid([7; 16]))
    );
    assert_eq!(frame.payload(), b"do-not-debug-payload");
    let diagnostic = format!(
        "{frame:?}{:?}{:?}",
        frame.headers(),
        frame.headers().get("text")
    );
    assert!(!diagnostic.contains("do-not-debug-header"));
    assert!(!diagnostic.contains("do-not-debug-payload"));
    decoder.finish()?;
    Ok(())
}

#[test]
fn crc_and_header_failures_are_reported_then_a_following_valid_frame_recovers() -> TestResult {
    let valid = wire_frame(
        &[
            string_header(":message-type", "event"),
            string_header(":event-type", "recovered"),
        ],
        b"good",
    );

    let mut message_crc_bad = wire_frame(&[string_header(":message-type", "event")], b"bad");
    let payload_offset = 12 + header_bytes(&[string_header(":message-type", "event")]).len();
    message_crc_bad[payload_offset] ^= 1;
    assert_recovery(
        &[message_crc_bad, valid.clone()].concat(),
        KiroEventStreamError::MessageCrcMismatch,
        &valid,
    )?;

    let mut prelude_crc_bad = wire_frame(&[string_header(":message-type", "event")], b"bad");
    prelude_crc_bad[8] ^= 1;
    assert_recovery(
        &[prelude_crc_bad, valid.clone()].concat(),
        KiroEventStreamError::PreludeCrcMismatch,
        &valid,
    )?;

    let bad_header = wire_frame(&[typed_header("bad", 255, &[])], b"bad");
    assert_recovery(
        &[bad_header, valid.clone()].concat(),
        KiroEventStreamError::InvalidHeaderType,
        &valid,
    )?;

    let empty_header_name = wire_frame(&[vec![0]], b"bad");
    assert_recovery(
        &[empty_header_name, valid.clone()].concat(),
        KiroEventStreamError::InvalidHeaderName,
        &valid,
    )?;

    let duplicate_header = wire_frame(
        &[string_header("same", "one"), string_header("same", "two")],
        b"bad",
    );
    assert_recovery(
        &[duplicate_header, valid].concat(),
        KiroEventStreamError::DuplicateHeader,
        &wire_frame(
            &[
                string_header(":message-type", "event"),
                string_header(":event-type", "recovered"),
            ],
            b"good",
        ),
    )?;
    Ok(())
}

#[test]
fn invalid_length_and_eof_are_not_silently_ignored() -> TestResult {
    let valid = wire_frame(&[string_header(":message-type", "event")], b"good");
    let mut invalid_length = vec![0; 12];
    invalid_length[..4].copy_from_slice(&15_u32.to_be_bytes());
    invalid_length[4..8].copy_from_slice(&0_u32.to_be_bytes());
    let prelude_crc = crc32(&invalid_length[..8]);
    invalid_length[8..12].copy_from_slice(&prelude_crc.to_be_bytes());
    assert_recovery(
        &[invalid_length, valid.clone()].concat(),
        KiroEventStreamError::InvalidFrameLength,
        &valid,
    )?;

    let mut decoder = KiroEventStreamDecoder::new();
    decoder.feed(&valid[..valid.len() - 1])?;
    assert_eq!(decoder.next_frame()?, None);
    assert_eq!(decoder.finish(), Err(KiroEventStreamError::TruncatedFrame));
    assert_eq!(decoder.feed(&[0]), Err(KiroEventStreamError::Stopped));
    Ok(())
}

#[test]
fn repeated_corruption_reaches_a_terminal_recovery_limit() -> TestResult {
    let invalid = invalid_length_prelude();
    let mut decoder = KiroEventStreamDecoder::new();
    for attempt in 1..=5 {
        decoder.feed(&invalid)?;
        let actual = decoder.next_frame();
        if attempt == 5 {
            assert_eq!(actual, Err(KiroEventStreamError::TooManyErrors));
        } else {
            assert!(matches!(
                actual,
                Err(KiroEventStreamError::InvalidFrameLength
                    | KiroEventStreamError::PreludeCrcMismatch)
            ));
        }
    }
    assert_eq!(decoder.next_frame(), Err(KiroEventStreamError::Stopped));
    Ok(())
}

fn assert_recovery(
    wire: &[u8],
    expected_error: KiroEventStreamError,
    expected_valid_wire: &[u8],
) -> TestResult {
    let mut decoder = KiroEventStreamDecoder::new();
    decoder.feed(wire)?;
    assert_eq!(decoder.next_frame(), Err(expected_error));
    let expected = decode_chunks(expected_valid_wire, &[expected_valid_wire.len()])?;
    assert_eq!(decoder.next_frame()?, expected.into_iter().next());
    decoder.finish()?;
    Ok(())
}

fn decode_chunks(
    wire: &[u8],
    chunks: &[usize],
) -> Result<Vec<KiroEventStreamFrame>, KiroEventStreamError> {
    let mut decoder = KiroEventStreamDecoder::new();
    let mut offset = 0;
    let mut frames = Vec::new();
    for chunk_length in chunks {
        let end = offset + chunk_length;
        decoder.feed(&wire[offset..end])?;
        drain(&mut decoder, &mut frames)?;
        offset = end;
    }
    assert_eq!(offset, wire.len());
    decoder.finish()?;
    Ok(frames)
}

fn drain(
    decoder: &mut KiroEventStreamDecoder,
    frames: &mut Vec<KiroEventStreamFrame>,
) -> Result<(), KiroEventStreamError> {
    while let Some(frame) = decoder.next_frame()? {
        frames.push(frame);
    }
    Ok(())
}

fn wire_frame(headers: &[Vec<u8>], payload: &[u8]) -> Vec<u8> {
    let headers = header_bytes(headers);
    let total_length = 12 + headers.len() + payload.len() + 4;
    let mut wire = Vec::with_capacity(total_length);
    wire.extend_from_slice(
        &u32::try_from(total_length)
            .unwrap_or_default()
            .to_be_bytes(),
    );
    wire.extend_from_slice(
        &u32::try_from(headers.len())
            .unwrap_or_default()
            .to_be_bytes(),
    );
    wire.extend_from_slice(&crc32(&wire).to_be_bytes());
    wire.extend_from_slice(&headers);
    wire.extend_from_slice(payload);
    wire.extend_from_slice(&crc32(&wire).to_be_bytes());
    wire
}

fn header_bytes(headers: &[Vec<u8>]) -> Vec<u8> {
    headers.concat()
}

fn string_header(name: &str, value: &str) -> Vec<u8> {
    typed_header(name, 7, &length_prefixed(value.as_bytes()))
}

fn typed_header(name: &str, value_type: u8, value: &[u8]) -> Vec<u8> {
    let mut header = Vec::with_capacity(2 + name.len() + value.len());
    header.push(u8::try_from(name.len()).unwrap_or_default());
    header.extend_from_slice(name.as_bytes());
    header.push(value_type);
    header.extend_from_slice(value);
    header
}

fn length_prefixed(value: &[u8]) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(value.len() + 2);
    encoded.extend_from_slice(&u16::try_from(value.len()).unwrap_or_default().to_be_bytes());
    encoded.extend_from_slice(value);
    encoded
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

fn invalid_length_prelude() -> Vec<u8> {
    let mut invalid = vec![0; 12];
    invalid[..4].copy_from_slice(&15_u32.to_be_bytes());
    invalid[4..8].copy_from_slice(&0_u32.to_be_bytes());
    let prelude_crc = crc32(&invalid[..8]);
    invalid[8..12].copy_from_slice(&prelude_crc.to_be_bytes());
    invalid
}
