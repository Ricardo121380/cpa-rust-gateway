//! Synthetic standard Build SSE from an independent, bounded socket receiver.
use super::*;
pub(super) const EXEC_COMMAND: &str = "IFS= read -r n < seed.txt; printf 'seed=%s\\n' \"$n\"; IFS= read -r delta; printf '%s\\n' \"$((n + delta))\" > result.txt; cat result.txt";

pub(super) fn exec_arguments() -> Value {
    json!({"cmd":EXEC_COMMAND,"tty":true,"yield_time_ms":250,"max_output_tokens":1000})
}

pub(super) fn session_id(request: &Value) -> Result<i32, Box<dyn Error>> {
    let output = request["input"]
        .as_array()
        .ok_or("Codex input")?
        .iter()
        .find(|v| v["type"] == "function_call_output" && v["call_id"] == "call-62-0")
        .and_then(|v| v["output"].as_str())
        .ok_or("actual exec_command output")?;
    let ids: Vec<_> = output
        .lines()
        .filter_map(|line| line.strip_prefix("Process running with session ID "))
        .collect();
    assert_eq!(
        ids.len(),
        1,
        "only the actual CLI tool result supplies the session ID"
    );
    Ok(ids[0].parse()?)
}

pub(super) fn stdin_arguments(id: i32) -> Value {
    json!({"session_id":id,"chars":"1\n","yield_time_ms":1000,"max_output_tokens":1000})
}

fn standard_wire(round: usize, client: &str, request: &Value) -> Result<String, Box<dyn Error>> {
    let summary = json!({"id":format!("reason-62-{round}"),"type":"reasoning","status":"completed",
        "summary":[{"type":"summary_text","text":format!("Plan {round}")}]});
    let item = if client == "codex" && round < 2 {
        let (name, arguments) = if round == 0 {
            ("exec_command", exec_arguments())
        } else {
            ("write_stdin", stdin_arguments(session_id(request)?))
        };
        json!({"id":format!("function-62-{round}"),"type":"function_call","status":"completed",
            "call_id":format!("call-62-{round}"),"name":name,"arguments":arguments.to_string()})
    } else if client != "codex" && (round == 1 || round == 2) {
        let (name, arguments) = if round == 1 {
            ("read", json!({"path":"seed.txt"}))
        } else {
            (
                "bash",
                json!({"command":"n=$(cat seed.txt); echo $((n + 1)) > result.txt; cat result.txt"}),
            )
        };
        json!({"id":format!("function-62-{round}"),"type":"function_call","status":"completed",
            "call_id":format!("call-62-{round}"),"name":name,"arguments":arguments.to_string()})
    } else {
        json!({"id":format!("message-62-{round}"),"type":"message","status":"completed","role":"assistant",
            "content":[{"type":"output_text","text":if round==0 {"Ready to inspect seed.txt."} else {"The result is 42."},"annotations":[]}]})
    };
    let id = format!("response-62-{round}");
    let mut start = created();
    start["id"] = json!(id);
    let mut terminal = response();
    terminal["id"] = json!(id);
    terminal["output"] = json!([summary, item]);
    let mut wire = frame("response.created", &start);
    for (index, item) in [summary, item].iter().enumerate() {
        let mut added = item.clone();
        added["status"] = json!("in_progress");
        let tool = item["type"] == "function_call";
        if index == 0 {
            added["summary"] = json!([]);
        } else if tool {
            added["arguments"] = json!("");
        } else {
            added["content"] = json!([]);
        }
        wire += &native_event(
            "response.output_item.added",
            json!({"output_index":index,"item":added}),
        );
        let (kind, text) = if index == 0 {
            (
                "response.reasoning_summary_text.delta",
                format!("Plan {round}"),
            )
        } else if tool {
            (
                "response.function_call_arguments.delta",
                item["arguments"].as_str().unwrap_or_default().into(),
            )
        } else {
            (
                "response.output_text.delta",
                item["content"][0]["text"]
                    .as_str()
                    .unwrap_or_default()
                    .into(),
            )
        };
        for delta in text.as_bytes().chunks(3) {
            let mut payload = json!({"output_index":index,"item_id":item["id"],"delta":String::from_utf8_lossy(delta)});
            if index == 0 {
                payload["summary_index"] = json!(0);
            } else if !tool {
                payload["content_index"] = json!(0);
            }
            wire += &native_event(kind, payload);
        }
        wire += &native_event(
            "response.output_item.done",
            json!({"output_index":index,"item":item}),
        );
    }
    Ok(wire + &frame("response.completed", &terminal))
}

pub(super) fn scripted_peer(client: &'static str) -> Result<SocketPeer, Box<dyn Error>> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    listener.set_nonblocking(true)?;
    let listener = tokio::net::TcpListener::from_std(listener)?;
    let received = Arc::new(Mutex::new(Vec::<Value>::new()));
    let requests = received.clone();
    let receipts = Arc::new(Mutex::new(Vec::new()));
    let observations = receipts.clone();
    let task = tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await?;
            let (request, headers) = read_request_observed(&mut socket).await?;
            let round = {
                let mut rows = requests
                    .lock()
                    .map_err(|_| std::io::Error::other("receiver"))?;
                let round = rows.len();
                rows.push(request.clone());
                round
            };
            let body = standard_wire(round, client, &request)
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            observations.lock().map_err(|_| std::io::Error::other("receiver"))?.push(
                json!({"ordinal":round,"headers":headers,"response_id":format!("response-62-{round}")}));
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).as_bytes()).await?;
            for chunk in body.as_bytes().chunks(31) {
                socket.write_all(chunk).await?;
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        }
    });
    let policy = EgressPolicy::try_new(EgressPolicyInput {
        id: EgressPolicyId::try_new("build-62-owned-loopback")?,
        name: "Build CLI owned peer".into(),
        allowed_schemes: BTreeSet::from([EgressScheme::Http]),
        allowed_hosts: BTreeSet::from([EgressHost::try_new("127.0.0.1")?]),
        allowed_ports: BTreeSet::from([port]),
        allowed_cidrs: BTreeSet::from([EgressCidr::try_new("127.0.0.1".parse()?, 32)?]),
        redirect_policy: RedirectPolicy::Deny,
    })?;
    Ok(SocketPeer {
        target: policy.admit_url(
            &format!("http://127.0.0.1:{port}/v1/responses"),
            &SystemEgressDnsResolver,
        )?,
        received,
        receipts,
        closed: Arc::new(AtomicUsize::new(0)),
        task,
    })
}
