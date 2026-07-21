//! Explicit, loopback-only P5-07 Claude Code compatibility evidence.
//!
//! The test is ignored by default because it launches the locally installed Claude Code client.
//! It deliberately clears inherited environment state, binds the gateway only to `127.0.0.1`,
//! uses a synthetic Client Key, and scripts only fixed `printf` Tool Calls. It does not load a
//! real Provider credential or send a request beyond the test listener.

#![deny(unsafe_code)]

use std::{
    collections::VecDeque,
    env,
    error::Error,
    fs,
    io::{self, ErrorKind},
    net::{Ipv4Addr, TcpListener},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use actix_web::{App, HttpServer, dev::ServerHandle, web};
use gateway_auth::{InMemoryClientKey, InMemoryClientKeyAuthenticator};
use gateway_core::{
    CanonicalEvent, CanonicalRequest, ClientKeyId, ErrorScope, GatewayError, GatewayErrorCode,
    MessageContent, MessageEnd, MessageRole, MessageStart, RawExtensions, RawJson, ResponseEnd,
    ResponseId, ResponseStart, TextDelta, ToolCallArgumentsDelta, ToolCallEnd, ToolCallStart,
    Usage, UsageDelta,
};
use gateway_http_actix::{ResponsesHttpState, configure};
use gateway_router::{ResponsesEventSource, ResponsesExecutor, ResponsesFuture};
use gateway_stream::StreamCapacity;
use tokio::task::JoinHandle;

const CLAUDE_CODE_BIN_ENV: &str = "P5_07_CLAUDE_CODE_BIN";
const LOCAL_CLIENT_KEY: &str = "p5-07-loopback-client-key";
const LOCAL_MODEL: &str = "p5-07-local-model";
const CLI_TIMEOUT: Duration = Duration::from_secs(20);
const CHILD_PATH: &str = "/usr/bin:/bin:/usr/sbin:/sbin";

type TestResult = Result<(), Box<dyn Error>>;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SafeTrace {
    title_requests: usize,
    normal_requests: usize,
    single_tool_instruction_requests: usize,
    single_tool_result_requests: usize,
    parallel_tool_instruction_requests: usize,
    parallel_tool_result_requests: usize,
    plan_requests: usize,
    tool_results_received: usize,
}

#[derive(Clone, Default)]
struct ScriptedClaudeExecutor {
    trace: Arc<Mutex<SafeTrace>>,
}

impl ScriptedClaudeExecutor {
    fn response_for(
        &self,
        request: &CanonicalRequest,
    ) -> Result<Vec<CanonicalEvent>, GatewayError> {
        let mut trace = self.trace.lock().map_err(|_| protocol_error())?;
        if is_title_request(request) {
            trace.title_requests = trace
                .title_requests
                .checked_add(1)
                .ok_or_else(protocol_error)?;
            return text_response("title", r#"{"title":"P5-07 loopback"}"#);
        }

        let tool_result_count = tool_result_count(request);
        if request_contains_text(request, "P5_07_NORMAL") {
            if tool_result_count != 0 {
                return Err(protocol_error());
            }
            trace.normal_requests = trace
                .normal_requests
                .checked_add(1)
                .ok_or_else(protocol_error)?;
            return text_response("normal", "P5_07_NORMAL_OK");
        }

        if request_contains_text(request, "P5_07_SINGLE_TOOL") {
            if tool_result_count == 0 {
                trace.single_tool_instruction_requests = trace
                    .single_tool_instruction_requests
                    .checked_add(1)
                    .ok_or_else(protocol_error)?;
                return tool_response("single-tool", ToolMode::Single);
            }
            if tool_result_count != 1 {
                return Err(protocol_error());
            }
            trace.single_tool_result_requests = trace
                .single_tool_result_requests
                .checked_add(1)
                .ok_or_else(protocol_error)?;
            trace.tool_results_received = trace
                .tool_results_received
                .checked_add(tool_result_count)
                .ok_or_else(protocol_error)?;
            return text_response("single-tool-result", "P5_07_SINGLE_TOOL_OK");
        }

        if request_contains_text(request, "P5_07_PARALLEL_TOOL") {
            if tool_result_count == 0 {
                trace.parallel_tool_instruction_requests = trace
                    .parallel_tool_instruction_requests
                    .checked_add(1)
                    .ok_or_else(protocol_error)?;
                return tool_response("parallel-tool", ToolMode::Parallel);
            }
            if tool_result_count != 2 {
                return Err(protocol_error());
            }
            trace.parallel_tool_result_requests = trace
                .parallel_tool_result_requests
                .checked_add(1)
                .ok_or_else(protocol_error)?;
            trace.tool_results_received = trace
                .tool_results_received
                .checked_add(tool_result_count)
                .ok_or_else(protocol_error)?;
            return text_response("parallel-tool-result", "P5_07_PARALLEL_TOOL_OK");
        }

        if request_contains_text(request, "P5_07_PLAN") {
            if tool_result_count != 0 {
                return Err(protocol_error());
            }
            trace.plan_requests = trace
                .plan_requests
                .checked_add(1)
                .ok_or_else(protocol_error)?;
            return text_response("plan", "P5_07_PLAN_OK");
        }

        Err(protocol_error())
    }
}

impl ResponsesExecutor for ScriptedClaudeExecutor {
    fn execute(
        &self,
        _context: gateway_core::RequestContext,
        request: CanonicalRequest,
    ) -> ResponsesFuture<'_, Result<Box<dyn ResponsesEventSource>, GatewayError>> {
        let events = self.response_for(&request);
        Box::pin(async move {
            Ok(Box::new(ScriptedEventSource {
                events: events?.into(),
            }) as Box<dyn ResponsesEventSource>)
        })
    }
}

struct ScriptedEventSource {
    events: VecDeque<CanonicalEvent>,
}

impl ResponsesEventSource for ScriptedEventSource {
    fn next_event(&mut self) -> ResponsesFuture<'_, Result<Option<CanonicalEvent>, GatewayError>> {
        Box::pin(async move { Ok(self.events.pop_front()) })
    }
}

#[derive(Clone, Copy)]
enum ToolMode {
    Single,
    Parallel,
}

fn is_title_request(request: &CanonicalRequest) -> bool {
    request
        .extensions
        .get("anthropic.messages.output_config")
        .is_some_and(|value| value.get().contains("\"json_schema\""))
}

fn request_contains_text(request: &CanonicalRequest, marker: &str) -> bool {
    request.messages.iter().any(|message| {
        message.content.iter().any(
            |content| matches!(content, MessageContent::Text(text) if text.text.contains(marker)),
        )
    })
}

fn tool_result_count(request: &CanonicalRequest) -> usize {
    request
        .messages
        .iter()
        .flat_map(|message| &message.content)
        .filter(|content| matches!(content, MessageContent::ToolResult(_)))
        .count()
}

fn text_response(label: &str, text: &str) -> Result<Vec<CanonicalEvent>, GatewayError> {
    Ok(vec![
        response_start(label)?,
        interim_usage(),
        message_start(),
        CanonicalEvent::TextDelta(TextDelta {
            text: text.to_owned(),
            extensions: RawExtensions::default(),
        }),
        CanonicalEvent::MessageEnd(MessageEnd::default()),
        final_usage(),
        response_end("end_turn"),
    ])
}

fn tool_response(label: &str, mode: ToolMode) -> Result<Vec<CanonicalEvent>, GatewayError> {
    let mut events = vec![response_start(label)?, interim_usage(), message_start()];
    let [first_start, first_delta, first_end] = tool_events("p5-07-first", "P5_07_TOOL_FIRST")?;
    match mode {
        ToolMode::Single => events.extend([first_start, first_delta, first_end]),
        ToolMode::Parallel => {
            let [second_start, second_delta, second_end] =
                tool_events("p5-07-second", "P5_07_TOOL_SECOND")?;
            // Start both calls before either one completes. The Anthropic encoder still serializes
            // legal non-overlapping SSE blocks, while the Canonical sequence remains the P5-03
            // parallel-Tool shape that the client receives as one `tool_use` response.
            events.extend([
                first_start,
                second_start,
                first_delta,
                second_delta,
                first_end,
                second_end,
            ]);
        }
    }
    events.extend([
        CanonicalEvent::MessageEnd(MessageEnd::default()),
        final_usage(),
        response_end("tool_use"),
    ]);
    Ok(events)
}

fn tool_events(call_id: &str, marker: &str) -> Result<[CanonicalEvent; 3], GatewayError> {
    let arguments = format!(
        r#"{{"command":"printf %s {marker}","description":"Print fixed P5-07 loopback marker"}}"#
    );
    let complete_arguments =
        RawJson::from_json_string(arguments.clone()).map_err(|_| protocol_error())?;
    Ok([
        CanonicalEvent::ToolCallStart(ToolCallStart {
            call_id: call_id.to_owned(),
            name: "Bash".to_owned(),
            extensions: RawExtensions::default(),
        }),
        CanonicalEvent::ToolCallArgumentsDelta(ToolCallArgumentsDelta {
            call_id: call_id.to_owned(),
            delta: arguments,
            extensions: RawExtensions::default(),
        }),
        CanonicalEvent::ToolCallEnd(ToolCallEnd {
            call_id: call_id.to_owned(),
            arguments: complete_arguments,
            extensions: RawExtensions::default(),
        }),
    ])
}

fn response_start(label: &str) -> Result<CanonicalEvent, GatewayError> {
    let response_id =
        ResponseId::try_new(format!("p5-07-{label}")).map_err(|_| protocol_error())?;
    Ok(CanonicalEvent::ResponseStart(ResponseStart {
        response_id,
        extensions: RawExtensions::default(),
    }))
}

fn interim_usage() -> CanonicalEvent {
    CanonicalEvent::UsageDelta(UsageDelta {
        usage: Usage {
            input_tokens: Some(1),
            ..Usage::default()
        },
        is_final: false,
        extensions: RawExtensions::default(),
    })
}

fn message_start() -> CanonicalEvent {
    CanonicalEvent::MessageStart(MessageStart {
        role: MessageRole("assistant".to_owned()),
        extensions: RawExtensions::default(),
    })
}

fn final_usage() -> CanonicalEvent {
    CanonicalEvent::UsageDelta(UsageDelta {
        usage: Usage {
            output_tokens: Some(1),
            ..Usage::default()
        },
        is_final: true,
        extensions: RawExtensions::default(),
    })
}

fn response_end(reason: &str) -> CanonicalEvent {
    CanonicalEvent::ResponseEnd(ResponseEnd {
        stop_reason: Some(reason.to_owned()),
        stop_sequence: None,
        extensions: RawExtensions::default(),
    })
}

struct LoopbackGateway {
    base_url: String,
    handle: ServerHandle,
    task: JoinHandle<io::Result<()>>,
}

impl LoopbackGateway {
    async fn start(executor: ScriptedClaudeExecutor) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let client_key = InMemoryClientKey::try_new(
            LOCAL_CLIENT_KEY,
            ClientKeyId::try_new("p5-07-loopback-client")
                .map_err(|_| io::Error::other("invalid test Client Key ID"))?,
            true,
        )
        .map_err(|_| io::Error::other("invalid local Client Key"))?;
        let authenticator = InMemoryClientKeyAuthenticator::try_new([client_key])
            .map_err(|_| io::Error::other("invalid local Client Key configuration"))?;
        let stream_capacity = StreamCapacity::try_new(8)
            .map_err(|_| io::Error::other("invalid local stream capacity"))?;
        let state =
            ResponsesHttpState::new(Arc::new(executor), Arc::new(authenticator), stream_capacity);
        let server_state = state.clone();
        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(server_state.clone()))
                .configure(configure)
        })
        .workers(1)
        .listen(listener)?
        .run();
        let handle = server.handle();
        let task = tokio::spawn(server);
        tokio::task::yield_now().await;

        Ok(Self {
            base_url: format!("http://127.0.0.1:{port}"),
            handle,
            task,
        })
    }

    async fn stop(self) -> io::Result<()> {
        self.handle.stop(true).await;
        self.task
            .await
            .map_err(|_| io::Error::other("loopback gateway task did not join"))??;
        Ok(())
    }
}

struct TemporaryDirectory {
    path: PathBuf,
}

impl TemporaryDirectory {
    fn create(label: &str) -> io::Result<Self> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| io::Error::other("system time precedes Unix epoch"))?
            .as_nanos();
        for serial in 0..32_u8 {
            let path =
                env::temp_dir().join(format!("{label}-{}-{nonce}-{serial}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            ErrorKind::AlreadyExists,
            "could not allocate an isolated P5-07 temporary directory",
        ))
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _cleanup = fs::remove_dir_all(&self.path);
    }
}

fn configured_claude_binary() -> io::Result<PathBuf> {
    let path = env::var_os(CLAUDE_CODE_BIN_ENV)
        .map(PathBuf::from)
        .ok_or_else(|| {
            io::Error::new(
                ErrorKind::NotFound,
                "P5-07 Claude Code binary is not configured",
            )
        })?;
    if path.is_file() {
        Ok(path)
    } else {
        Err(io::Error::new(
            ErrorKind::NotFound,
            "P5-07 Claude Code binary is not an executable file",
        ))
    }
}

async fn run_claude(
    binary: PathBuf,
    base_url: String,
    working_directory: PathBuf,
    prompt: &'static str,
    permission_mode: Option<&'static str>,
    execute_fixed_tools: bool,
) -> io::Result<Output> {
    tokio::task::spawn_blocking(move || {
        let mut command = Command::new(binary);
        command
            .current_dir(&working_directory)
            .env_clear()
            .env("PATH", CHILD_PATH)
            .env("HOME", &working_directory)
            .env("LANG", "C")
            .env("NO_PROXY", "127.0.0.1,localhost")
            .env("ANTHROPIC_BASE_URL", base_url)
            .env("ANTHROPIC_API_KEY", LOCAL_CLIENT_KEY)
            .env("ANTHROPIC_MODEL", LOCAL_MODEL)
            .env("ANTHROPIC_DEFAULT_SONNET_MODEL", LOCAL_MODEL)
            .env("CLAUDE_CODE_SKIP_MANTLE_AUTH", "1")
            .arg("--bare")
            .arg("--print")
            .arg("--no-session-persistence")
            .arg("--output-format=json");
        if let Some(permission_mode) = permission_mode {
            command.arg("--permission-mode").arg(permission_mode);
        }
        if execute_fixed_tools {
            command
                .arg("--allow-dangerously-skip-permissions")
                .arg("--dangerously-skip-permissions");
        }
        command
            .arg(prompt)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        wait_for_output(command)
    })
    .await
    .map_err(|_| io::Error::other("Claude Code runner task did not join"))?
}

fn wait_for_output(mut command: Command) -> io::Result<Output> {
    let mut child = command.spawn()?;
    let deadline = Instant::now()
        .checked_add(CLI_TIMEOUT)
        .ok_or_else(|| io::Error::other("Claude Code timeout overflowed"))?;
    loop {
        if child.try_wait()?.is_some() {
            return child.wait_with_output();
        }
        if Instant::now() >= deadline {
            let _kill = child.kill();
            let _wait = child.wait();
            return Err(io::Error::new(
                ErrorKind::TimedOut,
                "loopback-only Claude Code invocation exceeded its bounded timeout",
            ));
        }
        thread::sleep(Duration::from_millis(25));
    }
}

fn assert_cli_success(output: &Output, marker: &str) -> TestResult {
    if !output.status.success() {
        return Err(io::Error::other(
            "Claude Code exited unsuccessfully against the loopback gateway",
        )
        .into());
    }
    let response: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    if response
        .get("is_error")
        .and_then(serde_json::Value::as_bool)
        != Some(false)
    {
        return Err(io::Error::other("Claude Code reported a loopback execution error").into());
    }
    if !response
        .get("result")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|result| result.contains(marker))
    {
        return Err(
            io::Error::other("Claude Code result omitted the expected loopback marker").into(),
        );
    }
    Ok(())
}

const fn protocol_error() -> GatewayError {
    GatewayError::new(GatewayErrorCode::UpstreamProtocolError, ErrorScope::Stream)
}

#[actix_web::test]
#[ignore = "requires P5_07_CLAUDE_CODE_BIN and launches a loopback-only local Claude Code --bare client"]
async fn local_claude_code_bare_covers_normal_tool_parallel_tool_and_plan_mode() -> TestResult {
    let binary = configured_claude_binary()?;
    let executor = ScriptedClaudeExecutor::default();
    let trace_handle = Arc::clone(&executor.trace);
    let gateway = LoopbackGateway::start(executor.clone()).await?;
    let working_directory = TemporaryDirectory::create("cpa-rust-gateway-p5-07")?;

    let exercise = async {
        let normal = run_claude(
            binary.clone(),
            gateway.base_url.clone(),
            working_directory.path().to_path_buf(),
            "P5_07_NORMAL: return the fixed loopback marker.",
            None,
            false,
        )
        .await?;
        assert_cli_success(&normal, "P5_07_NORMAL_OK")?;

        let single_tool = run_claude(
            binary.clone(),
            gateway.base_url.clone(),
            working_directory.path().to_path_buf(),
            "P5_07_SINGLE_TOOL: run the fixed local tool and return its marker.",
            Some("bypassPermissions"),
            true,
        )
        .await?;
        assert_cli_success(&single_tool, "P5_07_SINGLE_TOOL_OK")?;

        let parallel_tools = run_claude(
            binary.clone(),
            gateway.base_url.clone(),
            working_directory.path().to_path_buf(),
            "P5_07_PARALLEL_TOOL: run both fixed local tools and return their marker.",
            Some("bypassPermissions"),
            true,
        )
        .await?;
        assert_cli_success(&parallel_tools, "P5_07_PARALLEL_TOOL_OK")?;

        let plan = run_claude(
            binary,
            gateway.base_url.clone(),
            working_directory.path().to_path_buf(),
            "P5_07_PLAN: return the fixed plan-mode marker without a Tool Call.",
            Some("plan"),
            false,
        )
        .await?;
        assert_cli_success(&plan, "P5_07_PLAN_OK")?;

        Ok::<(), Box<dyn Error>>(())
    }
    .await;
    let shutdown = gateway.stop().await;
    shutdown?;
    exercise?;

    let trace = trace_handle
        .lock()
        .map(|trace| trace.clone())
        .map_err(|_| io::Error::other("loopback trace lock was poisoned"))?;
    assert!(trace.title_requests >= 1);
    assert_eq!(trace.normal_requests, 1);
    assert_eq!(trace.single_tool_instruction_requests, 1);
    assert_eq!(trace.single_tool_result_requests, 1);
    assert_eq!(trace.parallel_tool_instruction_requests, 1);
    assert_eq!(trace.parallel_tool_result_requests, 1);
    assert_eq!(trace.plan_requests, 1);
    assert_eq!(trace.tool_results_received, 3);
    Ok(())
}
