//! Exact native Codex function identity, arguments, real process result and ordered replay.
use super::*;

pub(super) fn assert_codex(
    received: &[Value],
    directory: &std::path::Path,
) -> Result<Value, Box<dyn Error>> {
    assert_eq!(received.len(), 5);
    let id = peer::session_id(&received[1])?;
    let expected = [
        (
            "call-62-0",
            "exec_command",
            peer::exec_arguments(),
            "seed=41\r\n",
        ),
        (
            "call-62-1",
            "write_stdin",
            peer::stdin_arguments(id),
            "1\r\n42\r\n",
        ),
    ];
    let final_input = received[4]["input"].as_array().ok_or("resumed history")?;
    let mut exact_results = Vec::new();
    let mut last_position = None;
    for (index, (call_id, name, args, text)) in expected.iter().enumerate() {
        let calls: Vec<_> = final_input
            .iter()
            .enumerate()
            .filter(|(_, v)| v["type"] == "function_call" && v["call_id"] == *call_id)
            .collect();
        let outputs: Vec<_> = final_input
            .iter()
            .enumerate()
            .filter(|(_, v)| v["type"] == "function_call_output" && v["call_id"] == *call_id)
            .collect();
        assert_eq!((calls.len(), outputs.len()), (1, 1));
        let (call_position, call) = calls[0];
        let (result_position, result) = outputs[0];
        assert!(call_position < result_position);
        assert!(last_position.is_none_or(|last| last < call_position));
        last_position = Some(result_position);
        assert_eq!(call["name"], *name);
        assert!(
            call.get("namespace").is_none(),
            "native flat tool namespace stays absent"
        );
        assert_eq!(
            serde_json::from_str::<Value>(call["arguments"].as_str().ok_or("full args")?)?,
            *args
        );
        assert!(result.get("namespace").is_none());
        assert!(result.get("name").is_none());
        let output = result["output"]
            .as_str()
            .ok_or("actual native tool result text")?;
        assert_result(output, text, index, id)?;
        assert_replay(received, index, call_id, call, result)?;
        exact_results.push(json!({"call_id":call_id,"name":name,"namespace_present":false,
            "arguments":args,"result_id":result.get("id"),"result_namespace_present":false,
            "full_result":result["output"],"call_position":call_position,"result_position":result_position}));
    }
    assert_request_history(received)?;
    assert_eq!(std::fs::read(directory.join("work/seed.txt"))?, b"41\n");
    assert_eq!(std::fs::read(directory.join("work/result.txt"))?, b"42\n");
    let receipt: Value = serde_json::from_slice(&std::fs::read(directory.join("client.json"))?)?;
    assert_eq!(
        receipt["turn_outcomes"],
        json!(["completed", "completed", "completed"])
    );
    assert_eq!(receipt["process_exits"], json!([0, 0]));
    let pids = receipt["pids"]
        .as_array()
        .ok_or("actual native process identities")?;
    assert_eq!(pids.len(), 2);
    assert_ne!(
        pids[0], pids[1],
        "disk resume requires a new actual native process"
    );
    Ok(
        json!({"pass":true,"session_id_from_actual_exec_result":id,"ordered_tools":exact_results,
        "processes":pids,"disk_resume":true,"hosted_search_execution":"NOT_CALLED","namespaced_child_execution":"NOT_CALLED"}),
    )
}

fn assert_result(output: &str, text: &str, index: usize, id: i32) -> TestResult {
    let (header, body) = output
        .split_once("\nOutput:\n")
        .ok_or("native exec result envelope")?;
    assert_eq!(
        body, text,
        "exact actual tool output, including PTY line endings"
    );
    let mut kinds = BTreeSet::new();
    for line in header.lines() {
        let kind = if let Some(value) = line.strip_prefix("Chunk ID: ") {
            assert!(!value.is_empty() && value.bytes().all(|b| b.is_ascii_hexdigit()));
            "chunk"
        } else if let Some(value) = line
            .strip_prefix("Wall time: ")
            .and_then(|v| v.strip_suffix(" seconds"))
        {
            let seconds: f64 = value.parse()?;
            assert!(seconds.is_finite() && seconds >= 0.0);
            "wall"
        } else if let Some(value) = line.strip_prefix("Original token count: ") {
            let _: u64 = value.parse()?;
            "tokens"
        } else {
            assert_eq!(
                line,
                if index == 0 {
                    format!("Process running with session ID {id}")
                } else {
                    "Process exited with code 0".into()
                }
            );
            "process"
        };
        assert!(kinds.insert(kind));
    }
    assert!(kinds.contains("wall") && kinds.contains("process"));
    Ok(())
}

fn assert_replay(
    received: &[Value],
    index: usize,
    call_id: &str,
    call: &Value,
    result: &Value,
) -> TestResult {
    for (round, request) in received.iter().enumerate().skip(index + 1) {
        let history = request["input"].as_array().ok_or("explicit history")?;
        let replayed_calls: Vec<_> = history
            .iter()
            .filter(|v| v["type"] == "function_call" && v["call_id"] == call_id)
            .collect();
        let replayed_results: Vec<_> = history
            .iter()
            .filter(|v| v["type"] == "function_call_output" && v["call_id"] == call_id)
            .collect();
        assert_eq!(
            (replayed_calls.len(), replayed_results.len()),
            (1, 1),
            "round {round}"
        );
        assert_eq!(replayed_calls[0], call);
        assert_eq!(
            replayed_results[0], result,
            "complete result content and ID replay without rewriting"
        );
    }
    Ok(())
}

fn assert_request_history(received: &[Value]) -> TestResult {
    for (round, request) in received.iter().enumerate() {
        let input = request["input"].as_array().ok_or("input")?;
        assert_eq!(
            input
                .iter()
                .filter(|v| v["type"] == "function_call")
                .count(),
            round.min(2)
        );
        assert_eq!(
            input
                .iter()
                .filter(|v| v["type"] == "function_call_output")
                .count(),
            round.min(2)
        );
        let reasoning: Vec<_> = input.iter().filter(|v| v["type"] == "reasoning").collect();
        assert_eq!(reasoning.len(), round);
        for (index, item) in reasoning.iter().enumerate() {
            assert_eq!(
                item["summary"],
                json!([{"type":"summary_text","text":format!("Plan {index}")}])
            );
        }
    }
    Ok(())
}
