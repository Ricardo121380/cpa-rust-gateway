//! B-03 checks whole-output order through the same public path as B-02 confirmations.
use super::*;

fn reverse_terminal(frames: &mut [Value]) -> TestResult {
    frames.last_mut().ok_or("completed")?["response"]["output"]
        .as_array_mut()
        .ok_or("output")?
        .swap(0, 1);
    Ok(())
}

fn interleaved_frames(reasoning_finishes_first: bool) -> Result<Vec<Value>, Box<dyn Error>> {
    use std::collections::VecDeque;
    let mut original = native_frames();
    let completed = original.pop().ok_or("completed")?;
    let created = original.remove(0);
    let mut text = original
        .iter()
        .filter(|value| value["output_index"] == 0)
        .cloned()
        .collect::<VecDeque<_>>();
    let mut reason = original
        .into_iter()
        .filter(|value| value["output_index"] == 1)
        .collect::<VecDeque<_>>();
    let mut frames = vec![
        created,
        text.pop_front().ok_or("text start")?,
        reason.pop_front().ok_or("reason start")?,
    ];
    let reason_done = reason.pop_back().ok_or("reason done")?;
    while !text.is_empty() || !reason.is_empty() {
        if let Some(value) = reason.pop_front() {
            frames.push(value);
        }
        if reasoning_finishes_first && reason.is_empty() && !text.is_empty() {
            frames.push(reason_done.clone());
            frames.extend(text.drain(..));
        } else if let Some(value) = text.pop_front() {
            frames.push(value);
        }
    }
    if !reasoning_finishes_first {
        frames.push(reason_done);
    }
    frames.push(completed);
    for (sequence, frame) in frames.iter_mut().enumerate() {
        frame["sequence_number"] = json!(sequence);
    }
    Ok(frames)
}

#[actix_web::test]
async fn reversed_terminal_output_fails_publicly_without_storage_or_replay() -> TestResult {
    let mut frames = native_frames();
    reverse_terminal(&mut frames)?;
    eprintln!(
        "B-03 upstream terminal={}",
        frames.last().ok_or("completed")?
    );
    assert_public_failure(frames, "B-03 reversed final output").await
}

#[actix_web::test]
async fn terminal_order_rejection_cannot_be_repaired_by_later_completion() -> TestResult {
    for same_read in [true, false] {
        let mut frames = native_frames();
        let valid = frames.last().ok_or("completed")?.clone();
        reverse_terminal(&mut frames)?;
        let mut stream = chunks(&frames);
        let recovery = wire(&[valid]).into_bytes();
        if same_read {
            stream.last_mut().ok_or("tail")?.extend(recovery);
        } else {
            stream.push(recovery);
        }
        assert_public_failure_chunks(stream, "B-03 later completed").await?;
    }
    Ok(())
}

#[actix_web::test]
async fn terminal_item_identity_and_index_conflicts_fail_publicly() -> TestResult {
    for variant in [
        "duplicate-final",
        "missing-final",
        "unknown-final",
        "duplicate-index",
        "conflicting-done-index",
        "gap-index",
        "reversed-indices",
        "reversed-incomplete",
    ] {
        let mut frames = native_frames();
        let terminal = frames.last_mut().ok_or("completed")?;
        match variant {
            "duplicate-final" => {
                terminal["response"]["output"][1] = terminal["response"]["output"][0].clone();
            }
            "missing-final" => {
                terminal["response"]["output"]
                    .as_array_mut()
                    .ok_or("output")?
                    .pop();
            }
            "unknown-final" => terminal["response"]["output"][1]["id"] = json!("unknown-item"),
            "reversed-incomplete" => {
                terminal["type"] = json!("response.incomplete");
                terminal["response"]["status"] = json!("incomplete");
                terminal["response"]["incomplete_details"] = json!({"reason":"max_output_tokens"});
                reverse_terminal(&mut frames)?;
            }
            _ => {
                for frame in &mut frames {
                    if frame["output_index"] == 1 {
                        frame["output_index"] = json!(if variant == "gap-index" { 2 } else { 0 });
                        if variant == "duplicate-index"
                            && frame["type"] != "response.output_item.added"
                            || variant == "conflicting-done-index"
                                && frame["type"] != "response.output_item.done"
                        {
                            frame["output_index"] = json!(1);
                        }
                    } else if variant == "reversed-indices" && frame["output_index"] == 0 {
                        frame["output_index"] = json!(1);
                    }
                }
            }
        }
        if variant == "reversed-indices" {
            assert_bootstrap_failure(chunks(&frames)).await?;
        } else {
            assert_public_failure(frames, variant).await?;
        }
    }
    Ok(())
}

#[actix_web::test]
async fn same_read_terminal_order_conflict_fails_during_bootstrap_without_owned_result()
-> TestResult {
    let mut frames = native_frames();
    reverse_terminal(&mut frames)?;
    assert_bootstrap_failure(vec![wire(&frames).into_bytes()]).await
}

#[actix_web::test]
async fn interleaved_native_output_preserves_order_metadata_storage_and_continuation() -> TestResult
{
    let expected = native_frames().last().ok_or("completed")?["response"]["output"].clone();
    for variant in [
        "sequential",
        "text-finishes-first",
        "reasoning-finishes-first",
        "without-final-output",
        "legacy-indices",
        "snapshot-content",
    ] {
        let mut frames = if variant == "sequential" {
            native_frames()
        } else {
            interleaved_frames(true)?
        };
        if variant == "text-finishes-first" {
            frames = interleaved_frames(false)?;
        } else if variant == "without-final-output" {
            frames.last_mut().ok_or("completed")?["response"]
                .as_object_mut()
                .ok_or("response")?
                .remove("output");
        } else if variant == "legacy-indices" || variant == "snapshot-content" {
            frames.retain(|value| {
                matches!(
                    value["type"].as_str(),
                    Some(
                        "response.created"
                            | "response.output_item.added"
                            | "response.output_item.done"
                            | "response.completed"
                    )
                ) || variant == "legacy-indices"
                    && matches!(
                        value["type"].as_str(),
                        Some(
                            "response.output_text.delta"
                                | "response.reasoning_summary_text.delta"
                                | "response.reasoning_text.delta"
                        )
                    )
            });
            for frame in &mut frames {
                frame.as_object_mut().ok_or("frame")?.remove("output_index");
            }
        }
        assert_public_success(frames, &expected, variant).await?;
    }
    Ok(())
}

#[actix_web::test]
async fn terminal_order_conflict_has_no_websocket_root_and_valid_interleaving_can_continue()
-> TestResult {
    for invalid in [false, true] {
        let mut frames = interleaved_frames(true)?;
        let expected = frames.last().ok_or("completed")?["response"]["output"].clone();
        if invalid {
            reverse_terminal(&mut frames)?;
        }
        assert_websocket_result(chunks(&frames), invalid, Some(&expected)).await?;
    }
    Ok(())
}

#[actix_web::test]
async fn output_order_checks_are_read_boundary_invariant_and_permanent() -> TestResult {
    use protocol_openai_responses::OpenAiResponsesSseDecoder;
    let frames = interleaved_frames(true)?;
    let bytes = wire(&frames);
    let expected = OpenAiResponsesSseDecoder::new().push(bytes.as_bytes())?;
    for size in [1, 7, 31, bytes.len()] {
        let mut decoder = OpenAiResponsesSseDecoder::new();
        let mut events = Vec::new();
        for chunk in bytes.as_bytes().chunks(size) {
            events.extend(decoder.push(chunk)?);
        }
        events.extend(decoder.finish()?);
        assert_eq!(events, expected, "legal size {size}");
    }
    let mut conflict = frames.clone();
    reverse_terminal(&mut conflict)?;
    let prefix = wire(&conflict[..conflict.len() - 1]);
    let tail = wire(&conflict[conflict.len() - 1..]);
    for size in [1, 7, 31, tail.len()] {
        let mut decoder = OpenAiResponsesSseDecoder::new();
        let mut events = decoder.push(prefix.as_bytes())?;
        let mut rejected = false;
        for chunk in tail.as_bytes().chunks(size) {
            if let Ok(batch) = decoder.push(chunk) {
                events.extend(batch);
            } else {
                rejected = true;
                break;
            }
        }
        assert!(rejected, "conflict size {size}");
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, gateway_core::CanonicalEvent::ResponseEnd(_)))
        );
        assert!(
            decoder
                .push(wire(&frames[frames.len() - 1..]).as_bytes())
                .is_err()
        );
        assert!(decoder.finish().is_err());
        assert!(!decoder.is_finished());
    }
    Ok(())
}
