//! P9-09 live Grok Web JSON-object decoder contract.

#![deny(unsafe_code)]

use std::error::Error;

use gateway_core::{CanonicalEvent, ErrorScope, GatewayErrorCode};
use provider_grok::GrokWebLiveStreamDecoder;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn concatenated_live_objects_are_chunk_invariant_and_complete_one_canonical_lifecycle() -> TestResult
{
    let fixture = successful_live_stream();
    let expected = decode_chunks(&fixture, fixture.len())?;
    for chunk_size in [1, 2, 7, 43, 127] {
        assert_eq!(decode_chunks(&fixture, chunk_size)?, expected);
    }
    let text = expected
        .iter()
        .filter_map(|event| match event {
            CanonicalEvent::TextDelta(delta) => Some(delta.text.as_str()),
            _ => None,
        })
        .collect::<String>();
    assert_eq!(text, "ready");
    assert!(matches!(
        expected.first(),
        Some(CanonicalEvent::ResponseStart(_))
    ));
    assert!(matches!(
        expected.last(),
        Some(CanonicalEvent::ResponseEnd(_))
    ));
    Ok(())
}

#[test]
fn reasoning_and_final_cumulative_text_are_projected_without_duplication() -> TestResult {
    let fixture = concat!(
        r#"{"result":{"conversation":{"conversationId":"conversation-live-reasoning"},"response":{"token":"think","isThinking":true}}}"#,
        r#"{"result":{"response":{"token":"re","isThinking":false}}}"#,
        r#"{"result":{"response":{"token":"ady","isThinking":false}}}"#,
        r#"{"result":{"response":{"modelResponse":{"message":"ready"}}}}"#,
    );
    let events = decode_chunks(fixture.as_bytes(), 3)?;
    let reasoning = events
        .iter()
        .filter_map(|event| match event {
            CanonicalEvent::ReasoningDelta(delta) => Some(delta.text.as_str()),
            _ => None,
        })
        .collect::<String>();
    assert_eq!(reasoning, "think");
    let text = events
        .iter()
        .filter_map(|event| match event {
            CanonicalEvent::TextDelta(delta) => Some(delta.text.as_str()),
            _ => None,
        })
        .collect::<String>();
    assert_eq!(text, "ready");
    Ok(())
}

#[test]
fn malformed_rewound_or_unfinished_live_objects_fail_closed() -> TestResult {
    let mut decoder = GrokWebLiveStreamDecoder::new();
    let error = decoder
        .push_bytes(br#"{"result":{"response":{"token":"before-conversation"}}}"#)
        .err()
        .ok_or("response before conversation was accepted")?;
    assert_eq!(error.code(), GatewayErrorCode::UpstreamProtocolError);
    assert_eq!(error.scope(), ErrorScope::Stream);

    let mut decoder = GrokWebLiveStreamDecoder::new();
    decoder.push_bytes(
        br#"{"result":{"conversation":{"conversationId":"conversation-live-rewind"},"response":{"token":"ready"}}}"#,
    )?;
    let error = decoder
        .push_bytes(br#"{"result":{"response":{"modelResponse":{"message":"rea"}}}}"#)
        .err()
        .ok_or("rewound final text was accepted")?;
    assert_eq!(error.code(), GatewayErrorCode::UpstreamProtocolError);

    let mut decoder = GrokWebLiveStreamDecoder::new();
    decoder.push_bytes(
        br#"{"result":{"conversation":{"conversationId":"conversation-live-truncated"},"response":{"token":"ready"}}}"#,
    )?;
    assert_eq!(
        decoder
            .finish()
            .err()
            .ok_or("missing final envelope was accepted")?
            .code(),
        GatewayErrorCode::StreamTruncated
    );
    Ok(())
}

#[test]
fn identity_change_and_duplicate_final_envelope_fail_closed() -> TestResult {
    let mut decoder = GrokWebLiveStreamDecoder::new();
    decoder.push_bytes(
        br#"{"result":{"conversation":{"conversationId":"conversation-live-first"},"response":{"token":"ready"}}}"#,
    )?;
    let error = decoder
        .push_bytes(br#"{"result":{"conversation":{"conversationId":"conversation-live-second"}}}"#)
        .err()
        .ok_or("changed conversation identity was accepted")?;
    assert_eq!(error.code(), GatewayErrorCode::UpstreamProtocolError);

    let mut decoder = GrokWebLiveStreamDecoder::new();
    decoder.push_bytes(
        br#"{"result":{"conversation":{"conversationId":"conversation-live-final"},"response":{"token":"ready"}}}{"result":{"response":{"modelResponse":{"message":"ready"}}}}"#,
    )?;
    let error = decoder
        .push_bytes(br#"{"result":{"response":{"modelResponse":{"message":"ready"}}}}"#)
        .err()
        .ok_or("duplicate final envelope was accepted")?;
    assert_eq!(error.code(), GatewayErrorCode::UpstreamProtocolError);
    Ok(())
}

#[test]
fn diagnostic_forms_never_render_live_identifiers_or_text() -> TestResult {
    let mut decoder = GrokWebLiveStreamDecoder::new();
    decoder.push_bytes(
        br#"{"result":{"conversation":{"conversationId":"conversation-secret"},"response":{"token":"private response"}}}"#,
    )?;
    let diagnostic = format!("{decoder:?}");
    assert!(!diagnostic.contains("conversation-secret"));
    assert!(!diagnostic.contains("private response"));
    assert!(diagnostic.contains("visible_text_bytes"));
    Ok(())
}

fn decode_chunks(
    fixture: &[u8],
    chunk_size: usize,
) -> Result<Vec<CanonicalEvent>, gateway_core::GatewayError> {
    let mut decoder = GrokWebLiveStreamDecoder::new();
    let mut events = Vec::new();
    for chunk in fixture.chunks(chunk_size) {
        events.extend(decoder.push_bytes(chunk)?);
    }
    events.extend(decoder.finish()?);
    Ok(events)
}

fn successful_live_stream() -> Vec<u8> {
    concat!(
        r#"{"result":{"conversation":{"conversationId":"conversation-live-ready"},"response":{"token":"rea","isThinking":false}}}"#,
        r#"{"result":{"response":{"token":"dy","isThinking":false}}}"#,
        r#"{"result":{"response":{"modelResponse":{"message":"ready"}}}}"#,
    )
    .as_bytes()
    .to_vec()
}
