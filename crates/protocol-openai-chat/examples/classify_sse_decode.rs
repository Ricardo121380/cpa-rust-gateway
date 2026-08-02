//! Feed private Chat SSE bytes through the production decoder and print only value-free status.

use std::io::{self, Read};

use protocol_openai_chat::{ChatResponseMetadata, ChatSseEncoder, OpenAiChatSseDecoder};

fn next_frame(input: &[u8]) -> Option<(usize, usize)> {
    input
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|position| (position, 2))
        .or_else(|| {
            input
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .map(|position| (position, 4))
        })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = Vec::new();
    io::stdin().read_to_end(&mut input)?;
    let mut decoder = OpenAiChatSseDecoder::new();
    let mut encoder = ChatSseEncoder::new(ChatResponseMetadata::try_new("redacted", 0, true)?);
    let mut offset = 0;
    let mut frame_count = 0;
    let mut canonical_event_count = 0;

    while let Some((frame_end, delimiter_len)) = next_frame(&input[offset..]) {
        let end = offset + frame_end + delimiter_len;
        frame_count += 1;
        match decoder.push(&input[offset..end]) {
            Ok(events) => {
                for event in events {
                    canonical_event_count += 1;
                    if let Err(error) = encoder.encode_event(&event) {
                        println!(
                            "encoder=FAIL canonical_event_index={canonical_event_count} code={:?} scope={:?}",
                            error.code(),
                            error.scope()
                        );
                        return Ok(());
                    }
                }
            }
            Err(error) => {
                println!(
                    "decoder=FAIL frame_index={frame_count} code={:?} scope={:?}",
                    error.code(),
                    error.scope()
                );
                return Ok(());
            }
        }
        offset = end;
    }
    if offset < input.len() {
        match decoder.push(&input[offset..]) {
            Ok(events) => {
                for event in events {
                    canonical_event_count += 1;
                    if let Err(error) = encoder.encode_event(&event) {
                        println!(
                            "encoder=FAIL canonical_event_index={canonical_event_count} code={:?} scope={:?}",
                            error.code(),
                            error.scope()
                        );
                        return Ok(());
                    }
                }
            }
            Err(error) => {
                println!(
                    "decoder=FAIL frame_index={} code={:?} scope={:?}",
                    frame_count + 1,
                    error.code(),
                    error.scope()
                );
                return Ok(());
            }
        }
    }
    match decoder.finish() {
        Ok(events) => {
            for event in events {
                canonical_event_count += 1;
                if let Err(error) = encoder.encode_event(&event) {
                    println!(
                        "encoder=FAIL canonical_event_index={canonical_event_count} code={:?} scope={:?}",
                        error.code(),
                        error.scope()
                    );
                    return Ok(());
                }
            }
            println!(
                "pipeline=PASS frame_count={frame_count} canonical_event_count={canonical_event_count}"
            );
        }
        Err(error) => println!(
            "decoder=FAIL finish=true code={:?} scope={:?}",
            error.code(),
            error.scope()
        ),
    }
    Ok(())
}
