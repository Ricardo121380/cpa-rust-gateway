//! Fixed independent Build peer for the actual Claude Read/Bash dialogue.
use super::*;

pub(super) const BASH: &str =
    "IFS= read -r n < seed.txt; printf '%s\\n' \"$((n + 1))\" > result.txt; cat result.txt";

pub(super) fn items(round: usize, directory: &std::path::Path) -> Value {
    let mut items = tool_rounds::output(round);
    items[1] = match round {
        0 | 2 => {
            let (name, arguments) = if round == 0 {
                (
                    "Read",
                    json!({"file_path": directory.join("work/seed.txt")}),
                )
            } else {
                (
                    "Bash",
                    json!({"command":BASH,"description":"Add one to the observed seed and write result.txt"}),
                )
            };
            json!({"id":format!("function-64-{round}"),"type":"function_call","status":"completed",
                "call_id":format!("call-64-{round}"),"name":name,"arguments":arguments.to_string()})
        }
        _ => {
            json!({"id":format!("message-64-{round}"),"type":"message","status":"completed","role":"assistant",
            "content":[{"type":"output_text","text":match round {
                1 => "The seed is 41.", 3 => "The result is 42.", _ => "result.txt contains 42, one more than the seed 41."
            },"annotations":[]}]})
        }
    };
    items
}

pub(super) fn start(directory: &std::path::Path) -> Result<SocketPeer, Box<dyn Error>> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    listener.set_nonblocking(true)?;
    let listener = tokio::net::TcpListener::from_std(listener)?;
    let received = Arc::new(Mutex::new(Vec::new()));
    let requests = received.clone();
    let receipts = Arc::new(Mutex::new(Vec::new()));
    let observations = receipts.clone();
    let directory = directory.to_owned();
    let task = tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await?;
            let (body, headers) = read_request_observed(&mut socket).await?;
            let round = {
                let mut rows = requests
                    .lock()
                    .map_err(|_| std::io::Error::other("receiver"))?;
                let round = rows.len();
                rows.push(body);
                round
            };
            if round >= 5 {
                return Err(std::io::Error::other("unexpected extra actual-client send"));
            }
            let output = items(round, &directory);
            let response_id = format!("response-64-{round}");
            let wire = tool_rounds::wire_output(round, true, &output)
                .map_err(|error| std::io::Error::other(error.to_string()))?
                .replace("\"response-61\"", &format!("\"{response_id}\""));
            observations.lock().map_err(|_| std::io::Error::other("receiver"))?.push(
                json!({"ordinal":round,"headers":headers,"output":output,"response_id":response_id}));
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",wire.len()).as_bytes()).await?;
            for chunk in wire.as_bytes().chunks(7) {
                socket.write_all(chunk).await?;
                tokio::task::yield_now().await;
            }
        }
    });
    let policy = EgressPolicy::try_new(EgressPolicyInput {
        id: EgressPolicyId::try_new("claude-d2-owned-peer")?,
        name: "Claude D2 controlled peer".into(),
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
