use crate::core::{
    bridge::{registration_cli_candidates, select_registration_cli},
    error::{ErrorCode, RehomeError},
    models::{CodexAccessVerification, SourceOs},
};
use serde_json::{json, Map, Value};
use std::{
    collections::{HashSet, VecDeque},
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tempfile::TempDir;
use uuid::Uuid;

pub const PROBE_MESSAGE: &str = "这是一次迁移连接验证。请简短回复，不要调用工具。";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const TURN_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_LINE_BYTES: usize = 1024 * 1024;
const MAX_NOTIFICATIONS: usize = 256;
const MAX_PAGES: usize = 1000;
const DISABLED_FEATURES: &[&str] = &[
    "hooks",
    "plugins",
    "apps",
    "browser_use",
    "browser_use_external",
    "computer_use",
    "shell_tool",
    "unified_exec",
    "multi_agent",
    "goals",
    "memories",
    "skill_mcp_dependency_install",
];
// Generated ThreadSourceKind, CLI 0.155.0-alpha.16. Empty/omitted means interactive only.
const SOURCE_KINDS: &[&str] = &[
    "cli",
    "vscode",
    "exec",
    "appServer",
    "subAgent",
    "subAgentReview",
    "subAgentCompact",
    "subAgentThreadSpawn",
    "subAgentOther",
    "unknown",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexAccessRequest {
    pub codex_home: PathBuf,
    pub required_thread_ids: Vec<Uuid>,
    pub probe_thread_id: Uuid,
}

/// Success and ordinary errors require confirmed owned-child termination. An inability
/// to confirm termination must return CodexCleanupUnconfirmed; callers must not write
/// checkpoints or roll back data while that possible writer remains.
pub trait CodexAccessVerifier: Send {
    fn preflight(&mut self) -> Result<(), RehomeError>;
    fn verify(
        &mut self,
        request: &CodexAccessRequest,
        on_threads_recognized: &mut dyn FnMut(),
    ) -> Result<CodexAccessVerification, RehomeError>;
}

#[derive(Default)]
pub struct SystemCodexAccessVerifier;

impl CodexAccessVerifier for SystemCodexAccessVerifier {
    fn preflight(&mut self) -> Result<(), RehomeError> {
        with_server(None, launch_server, |transport, _| {
            Client::new(transport).initialize()
        })
    }

    fn verify(
        &mut self,
        request: &CodexAccessRequest,
        on_threads_recognized: &mut dyn FnMut(),
    ) -> Result<CodexAccessVerification, RehomeError> {
        validate_request(request)?;
        with_server(
            Some(&request.codex_home),
            launch_server,
            |transport, workspace| {
                verify_in_workspace(transport, request, on_threads_recognized, workspace)
            },
        )
    }
}

/// A newline-delimited stdio boundary; implementations must bound both writes and reads.
pub trait AppServerTransport {
    fn send_line(&mut self, line: &str, timeout: Duration) -> Result<(), RehomeError>;
    fn receive_line(&mut self, timeout: Duration) -> Result<String, RehomeError>;
}

/// Protocol-only entry point for scripted transports. System verification also owns child cleanup.
pub fn verify_with_transport(
    transport: &mut impl AppServerTransport,
    request: &CodexAccessRequest,
    on_threads_recognized: &mut dyn FnMut(),
) -> Result<CodexAccessVerification, RehomeError> {
    validate_request(request)?;
    let workspace = temporary_directory("enhe-codex-probe-")?;
    verify_in_workspace(transport, request, on_threads_recognized, workspace.path())
}

fn failed(message: &'static str) -> RehomeError {
    RehomeError::new(ErrorCode::CodexVerificationFailed, message)
}

fn validate_request(request: &CodexAccessRequest) -> Result<(), RehomeError> {
    if !request.codex_home.is_absolute()
        || !request
            .required_thread_ids
            .contains(&request.probe_thread_id)
        || request
            .required_thread_ids
            .iter()
            .collect::<HashSet<_>>()
            .len()
            != request.required_thread_ids.len()
    {
        return Err(failed("invalid Codex verification selection"));
    }
    Ok(())
}

fn temporary_directory(prefix: &str) -> Result<TempDir, RehomeError> {
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir()
        .map_err(|_| failed("could not create the isolated Codex verification directory"))
}

// Dotted overrides are supported by both CLI -c and ThreadForkParams.config.
fn safe_overrides() -> Map<String, Value> {
    let mut config = Map::new();
    for feature in DISABLED_FEATURES {
        config.insert(format!("features.{feature}"), json!(false));
    }
    config.insert("notify".into(), json!([]));
    config.insert("web_search".into(), json!("disabled"));
    config.insert("approval_policy".into(), json!("never"));
    config.insert("sandbox_mode".into(), json!("read-only"));
    config
}

fn check_effective_config(response: &Value) -> Result<(), RehomeError> {
    let config = &response["config"];
    let safe = DISABLED_FEATURES
        .iter()
        .all(|feature| config["features"][*feature] == false)
        && config["notify"] == json!([])
        && config["web_search"] == "disabled"
        && config["approval_policy"] == "never"
        && config["sandbox_mode"] == "read-only";
    let no_enabled_mcp = match config.get("mcp_servers") {
        None => true,
        Some(Value::Object(servers)) => servers.values().all(|server| server["enabled"] == false),
        _ => false,
    };
    if !safe || !no_enabled_mcp {
        return Err(failed("Codex probe safety settings could not be confirmed; disable MCP integrations or use file-only restore"));
    }
    Ok(())
}

fn verify_in_workspace(
    transport: &mut impl AppServerTransport,
    request: &CodexAccessRequest,
    on_threads_recognized: &mut dyn FnMut(),
    workspace: &Path,
) -> Result<CodexAccessVerification, RehomeError> {
    let mut client = Client::new(transport);
    client.initialize()?;
    check_effective_config(&client.request(
        "config/read",
        json!({"includeLayers": false, "cwd": workspace}),
    )?)?;
    let account = client.request("account/read", json!({"refreshToken": false}))?;
    let requires_auth = account["requiresOpenaiAuth"]
        .as_bool()
        .ok_or_else(|| failed("Codex returned an invalid account response"))?;
    if requires_auth && account.get("account").is_none_or(Value::is_null) {
        return Err(RehomeError::new(
            ErrorCode::CodexAuthenticationRequired,
            "sign in to Codex before verifying continuation",
        ));
    }

    let required: HashSet<_> = request
        .required_thread_ids
        .iter()
        .map(Uuid::to_string)
        .collect();
    let mut recognized = HashSet::new();
    for archived in [false, true] {
        let mut cursor = Value::Null;
        let mut cursors = HashSet::new();
        for page in 0..MAX_PAGES {
            let response = client.request(
                "thread/list",
                json!({
                    "archived": archived, "cursor": cursor, "limit": 100,
                    "sourceKinds": SOURCE_KINDS, "modelProviders": [], "useStateDbOnly": true
                }),
            )?;
            let data = response["data"]
                .as_array()
                .ok_or_else(|| failed("Codex returned an invalid thread list"))?;
            for thread in data {
                if let Some(id) = thread["id"].as_str().filter(|id| required.contains(*id)) {
                    recognized.insert(id.to_owned());
                }
            }
            match response.get("nextCursor") {
                Some(Value::Null) => break,
                Some(Value::String(next))
                    if !next.is_empty() && cursors.insert(next.clone()) && page + 1 < MAX_PAGES =>
                {
                    cursor = json!(next);
                }
                _ => {
                    return Err(failed(
                        "Codex thread pagination exceeded its bounds or returned an invalid cursor",
                    ))
                }
            }
        }
    }
    if recognized != required {
        return Err(failed("Codex did not recognize every restored thread"));
    }
    for id in &request.required_thread_ids {
        let response = client.request(
            "thread/read",
            json!({"threadId": id, "includeTurns": false}),
        )?;
        if response["thread"]["id"] != id.to_string() {
            return Err(failed("Codex could not read the exact restored thread"));
        }
    }
    on_threads_recognized();

    let fork = client.request(
        "thread/fork",
        json!({
            "threadId": request.probe_thread_id, "ephemeral": true, "excludeTurns": true,
            "deferGoalContinuation": true, "approvalPolicy": "never", "sandbox": "read-only",
            "cwd": workspace, "runtimeWorkspaceRoots": [workspace], "config": safe_overrides()
        }),
    )?;
    let fork_id = fork["thread"]["id"]
        .as_str()
        .filter(|id| !id.is_empty() && !required.contains(*id))
        .ok_or_else(|| failed("Codex did not create a distinct ephemeral fork"))?;
    if fork["thread"]["ephemeral"] != true
        || fork["thread"]["canAcceptDirectInput"] != true
        || fork["thread"]["forkedFromId"] != request.probe_thread_id.to_string()
        || fork["cwd"] != json!(workspace)
        || fork["thread"]["cwd"] != json!(workspace)
        || fork["runtimeWorkspaceRoots"] != json!([workspace])
        || fork["approvalPolicy"] != "never"
        || fork["sandbox"] != json!({"type": "readOnly", "networkAccess": false})
    {
        return Err(failed(
            "Codex could not confirm an isolated read-only ephemeral fork",
        ));
    }
    // Only events received after this request can belong to the new probe.
    client.events.clear();
    let turn = client.request(
        "turn/start",
        json!({
            "threadId": fork_id, "approvalPolicy": "never", "cwd": workspace,
            "runtimeWorkspaceRoots": [workspace], "environments": [],
            "sandboxPolicy": {"type": "readOnly", "networkAccess": false},
            "input": [{"type": "text", "text": PROBE_MESSAGE, "text_elements": []}]
        }),
    )?;
    let turn_id = turn["turn"]["id"]
        .as_str()
        .filter(|id| !id.is_empty())
        .ok_or_else(|| failed("Codex returned an invalid probe turn"))?;
    client.wait_for_turn(fork_id, turn_id)?;
    Ok(CodexAccessVerification {
        required_threads: required.len() as u64,
        recognized_threads: recognized.len() as u64,
        probe_thread_id: Some(request.probe_thread_id),
        threads_recognized: true,
        continuation_probe_valid: true,
        ephemeral_fork: true,
    })
}

// Retain identifiers/booleans only, never conversation text from early notifications.
enum TurnEvent {
    Agent {
        thread: String,
        turn: String,
        nonempty: bool,
    },
    Completed {
        thread: String,
        turn: String,
        success: bool,
    },
}

struct Client<'a, T> {
    transport: &'a mut T,
    next_id: u64,
    events: VecDeque<TurnEvent>,
}

impl<'a, T: AppServerTransport> Client<'a, T> {
    fn new(transport: &'a mut T) -> Self {
        Self {
            transport,
            next_id: 1,
            events: VecDeque::new(),
        }
    }

    fn initialize(&mut self) -> Result<(), RehomeError> {
        self.request(
            "initialize",
            json!({
                "clientInfo": {"name": "enhe-codex-backup", "version": env!("CARGO_PKG_VERSION")},
                "capabilities": {"experimentalApi": true}
            }),
        )?;
        self.transport
            .send_line(r#"{"method":"initialized"}"#, REQUEST_TIMEOUT)
    }

    fn receive(&mut self, deadline: Instant) -> Result<Value, RehomeError> {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| failed("Codex App Server timed out"))?;
        let line = self.transport.receive_line(remaining)?;
        if line.len() > MAX_LINE_BYTES {
            return Err(failed("Codex App Server exceeded the line size limit"));
        }
        let message: Value = serde_json::from_str(&line)
            .map_err(|_| failed("Codex App Server returned malformed data"))?;
        if !message.is_object() {
            return Err(failed("Codex App Server returned malformed data"));
        }
        if message.get("method").is_some() && message.get("id").is_some() {
            return Err(failed(
                "Codex requested approval or an unsupported client action",
            ));
        }
        Ok(message)
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, RehomeError> {
        let id = self.next_id;
        self.next_id += 1;
        let deadline = Instant::now() + REQUEST_TIMEOUT;
        let line = json!({"id": id, "method": method, "params": params}).to_string();
        self.transport
            .send_line(&line, deadline.saturating_duration_since(Instant::now()))?;
        let mut notifications = 0;
        loop {
            let message = self.receive(deadline)?;
            if message.get("id").is_some() {
                if message["id"].as_u64() != Some(id) {
                    return Err(failed(
                        "Codex App Server returned an unexpected response ID",
                    ));
                }
                if message.get("error").is_some() {
                    return Err(failed("Codex App Server rejected the verification request"));
                }
                return message
                    .get("result")
                    .filter(|result| result.is_object())
                    .cloned()
                    .ok_or_else(|| failed("Codex App Server returned an invalid response"));
            }
            notifications += 1;
            if notifications > MAX_NOTIFICATIONS {
                return Err(failed("Codex App Server exceeded the notification limit"));
            }
            self.notification(&message)?;
        }
    }

    fn notification(&mut self, message: &Value) -> Result<(), RehomeError> {
        let method = message["method"]
            .as_str()
            .ok_or_else(|| failed("Codex App Server returned malformed data"))?;
        if method.contains("requestApproval") {
            return Err(failed("Codex requested approval during verification"));
        }
        let params = &message["params"];
        let event = match method {
            "item/completed" if params["item"]["type"] == "agentMessage" => {
                Some(TurnEvent::Agent {
                    thread: event_id(&params["threadId"])?,
                    turn: event_id(&params["turnId"])?,
                    nonempty: params["item"]["text"]
                        .as_str()
                        .is_some_and(|text| !text.trim().is_empty()),
                })
            }
            "turn/completed" => Some(TurnEvent::Completed {
                thread: event_id(&params["threadId"])?,
                turn: event_id(&params["turn"]["id"])?,
                success: params["turn"]["status"] == "completed",
            }),
            _ => None,
        };
        if let Some(event) = event {
            if self.events.len() >= MAX_NOTIFICATIONS {
                return Err(failed("Codex App Server exceeded the notification limit"));
            }
            self.events.push_back(event);
        }
        Ok(())
    }

    fn wait_for_turn(&mut self, thread_id: &str, turn_id: &str) -> Result<(), RehomeError> {
        let deadline = Instant::now() + TURN_TIMEOUT;
        let mut answered = false;
        loop {
            if let Some(event) = self.events.pop_front() {
                match event {
                    TurnEvent::Agent {
                        thread,
                        turn,
                        nonempty,
                    } if thread == thread_id && turn == turn_id => answered |= nonempty,
                    TurnEvent::Completed {
                        thread,
                        turn,
                        success,
                    } if thread == thread_id && turn == turn_id => {
                        return if success && answered {
                            Ok(())
                        } else {
                            Err(failed(
                                "Codex continuation did not complete with a non-empty answer",
                            ))
                        };
                    }
                    _ => (),
                }
            } else {
                let message = self.receive(deadline)?;
                if message.get("id").is_some() {
                    return Err(failed("Codex App Server returned an unexpected response"));
                }
                self.notification(&message)?;
            }
        }
    }
}

fn event_id(value: &Value) -> Result<String, RehomeError> {
    value
        .as_str()
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| failed("Codex App Server returned an invalid turn notification"))
}

fn server_command(cli: &Path, home: &Path, workspace: &Path) -> Command {
    let mut command = Command::new(cli);
    command.args(["app-server", "--stdio"]);
    for (key, value) in safe_overrides() {
        command.arg("-c").arg(format!("{key}={value}"));
    }
    command.env("CODEX_HOME", home).current_dir(workspace);
    command
}

fn launch_server(home: &Path, workspace: &Path) -> Result<StdioTransport, RehomeError> {
    let os = if cfg!(windows) {
        SourceOs::Windows
    } else {
        SourceOs::Macos
    };
    let cli = select_verifier_cli(os, registration_cli_candidates(os)).ok_or_else(|| {
        RehomeError::new(
            ErrorCode::CodexAppServerUnavailable,
            "a compatible Codex App Server is unavailable",
        )
    })?;
    StdioTransport::spawn(server_command(&cli, home, workspace))
}

fn select_verifier_cli(os: SourceOs, candidates: Vec<PathBuf>) -> Option<PathBuf> {
    select_registration_cli(candidates, |candidate| {
        os != SourceOs::Windows
            || candidate
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
    })
}

// An unconfirmed child exit retains its job directories for manual recovery.
fn with_server<T>(
    codex_home: Option<&Path>,
    launch: impl FnOnce(&Path, &Path) -> Result<StdioTransport, RehomeError>,
    operation: impl FnOnce(&mut StdioTransport, &Path) -> Result<T, RehomeError>,
) -> Result<T, RehomeError> {
    let temporary_home = if codex_home.is_none() {
        Some(temporary_directory("enhe-codex-preflight-")?)
    } else {
        None
    };
    let home = codex_home.unwrap_or_else(|| temporary_home.as_ref().unwrap().path());
    let workspace = temporary_directory("enhe-codex-probe-")?;
    let result = match launch(home, workspace.path()) {
        Ok(mut transport) => {
            let result = operation(&mut transport, workspace.path());
            finish_with_cleanup(result, transport.shutdown())
        }
        Err(error) => Err(error),
    };
    if matches!(&result, Err(error) if error.code == ErrorCode::CodexCleanupUnconfirmed) {
        if let Some(home) = temporary_home {
            let _ = home.keep();
        }
        let _ = workspace.keep();
    }
    result
}

fn finish_with_cleanup<T>(
    result: Result<T, RehomeError>,
    shutdown: Result<(), RehomeError>,
) -> Result<T, RehomeError> {
    match (result, shutdown) {
        (result, Ok(())) => result,
        (Ok(_), Err(cleanup)) => Err(cleanup),
        (Err(error), Err(cleanup)) => Err(RehomeError::new(
            if cleanup.code == ErrorCode::CodexCleanupUnconfirmed {
                cleanup.code
            } else {
                error.code
            },
            format!("{}; cleanup failed: {}", error.message, cleanup.message),
        )),
    }
}

struct StdioTransport {
    child: Child,
    writer_input: Option<SyncSender<String>>,
    write_results: Option<Receiver<Result<(), RehomeError>>>,
    writer: Option<JoinHandle<()>>,
    receiver: Option<Receiver<Result<String, RehomeError>>>,
    reader: Option<JoinHandle<()>>,
    overflow: Arc<AtomicBool>,
    shutdown_result: Option<Result<(), RehomeError>>,
}

fn finish_shutdown(
    mut has_exited: impl FnMut() -> std::io::Result<bool>,
    writer: &mut Option<JoinHandle<()>>,
    reader: &mut Option<JoinHandle<()>>,
    deadline: Instant,
) -> Result<(), RehomeError> {
    loop {
        match has_exited() {
            Ok(true) => break,
            Ok(false) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
            outcome => {
                return Err(RehomeError::new(
                    ErrorCode::CodexCleanupUnconfirmed,
                    format!(
                    "could not confirm Codex App Server termination: {}; manual recovery required",
                    match outcome {
                        Err(error) => error.to_string(),
                        _ => "exit confirmation timed out".into(),
                    }
                ),
                ))
            }
        }
    }
    while writer
        .iter()
        .chain(reader.iter())
        .any(|worker| !worker.is_finished())
    {
        if Instant::now() >= deadline {
            return Err(failed(
                "Codex App Server exited but pipe worker cleanup timed out",
            ));
        }
        thread::sleep(Duration::from_millis(5));
    }
    // Only completed workers are joined; unfinished pipe workers are detached on drop.
    let writer_joined = writer.take().map(|worker| worker.join()).transpose();
    let reader_joined = reader.take().map(|worker| worker.join()).transpose();
    if writer_joined.is_err() || reader_joined.is_err() {
        return Err(failed(
            "Codex App Server exited but pipe worker cleanup failed",
        ));
    }
    Ok(())
}

impl StdioTransport {
    fn spawn(mut command: Command) -> Result<Self, RehomeError> {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // Killing a cmd/bat shim would leave its server and stdout pipe alive.
            if !Path::new(command.get_program())
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
            {
                return Err(RehomeError::new(
                    ErrorCode::CodexAppServerUnavailable,
                    "verification requires a native Codex executable; command wrappers are unsupported",
                ));
            }
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        let child = command.spawn().map_err(|_| {
            RehomeError::new(
                ErrorCode::CodexAppServerUnavailable,
                "could not start Codex App Server",
            )
        })?;
        let (sender, receiver) = mpsc::sync_channel(64);
        let mut transport = Self {
            child,
            writer_input: None,
            write_results: None,
            writer: None,
            receiver: Some(receiver),
            reader: None,
            overflow: Arc::new(AtomicBool::new(false)),
            shutdown_result: None,
        };
        let setup = (|| {
            let mut stdin = transport
                .child
                .stdin
                .take()
                .ok_or_else(|| failed("Codex App Server stdin is unavailable"))?;
            let (writer_input, writes) = mpsc::sync_channel::<String>(1);
            let (written, write_results) = mpsc::sync_channel(1);
            transport.writer_input = Some(writer_input);
            transport.write_results = Some(write_results);
            transport.writer = Some(
                thread::Builder::new()
                    .name("codex-probe-stdin".into())
                    .spawn(move || {
                        while let Ok(line) = writes.recv() {
                            let result = writeln!(stdin, "{line}")
                                .and_then(|()| stdin.flush())
                                .map_err(|_| failed("could not send a Codex App Server request"));
                            let stop = result.is_err();
                            if written.try_send(result).is_err() || stop {
                                break;
                            }
                        }
                    })
                    .map_err(|_| failed("could not start the Codex App Server writer"))?,
            );
            let stdout = transport
                .child
                .stdout
                .take()
                .ok_or_else(|| failed("Codex App Server stdout is unavailable"))?;
            let overflow = Arc::clone(&transport.overflow);
            transport.reader = Some(
                thread::Builder::new()
                    .name("codex-probe-stdout".into())
                    .spawn(move || {
                        let mut reader = BufReader::new(stdout);
                        loop {
                            let mut line = Vec::new();
                            let result = match reader
                                .by_ref()
                                .take(MAX_LINE_BYTES as u64 + 1)
                                .read_until(b'\n', &mut line)
                            {
                                Ok(0) => Err(failed(
                                    "Codex App Server exited before verification completed",
                                )),
                                Ok(_) if line.len() > MAX_LINE_BYTES => {
                                    Err(failed("Codex App Server exceeded the line size limit"))
                                }
                                Ok(_) if !line.ends_with(b"\n") => {
                                    Err(failed("Codex App Server returned an incomplete line"))
                                }
                                Ok(_) => String::from_utf8(line).map_err(|_| {
                                    failed("Codex App Server returned malformed data")
                                }),
                                Err(_) => Err(failed("could not read Codex App Server output")),
                            };
                            let stop = result.is_err();
                            match sender.try_send(result) {
                                Ok(()) if !stop => (),
                                Err(mpsc::TrySendError::Full(_)) => {
                                    overflow.store(true, Ordering::Release);
                                    break;
                                }
                                _ => break,
                            }
                        }
                    })
                    .map_err(|_| failed("could not start the Codex App Server reader"))?,
            );
            Ok(())
        })();
        if let Err(error) = setup {
            return finish_with_cleanup(Err(error), transport.shutdown());
        }
        Ok(transport)
    }

    fn shutdown(&mut self) -> Result<(), RehomeError> {
        if let Some(result) = &self.shutdown_result {
            return result.clone();
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        self.writer_input.take();
        self.write_results.take();
        self.receiver.take();
        // The owned child handle avoids PID reuse; attempted kill is not exit proof.
        let _ = self.child.kill();
        let result = finish_shutdown(
            || self.child.try_wait().map(|status| status.is_some()),
            &mut self.writer,
            &mut self.reader,
            deadline,
        );
        self.shutdown_result = Some(result.clone());
        result
    }
}

impl AppServerTransport for StdioTransport {
    fn send_line(&mut self, line: &str, timeout: Duration) -> Result<(), RehomeError> {
        let deadline = Instant::now() + timeout;
        let result = (|| {
            if line.len() > MAX_LINE_BYTES {
                return Err(failed(
                    "Codex App Server request exceeded the line size limit",
                ));
            }
            self.writer_input
                .as_ref()
                .ok_or_else(|| failed("Codex App Server input is closed"))?
                .try_send(line.to_owned())
                .map_err(|_| failed("could not queue a Codex App Server request"))?;
            self.write_results
                .as_ref()
                .ok_or_else(|| failed("Codex App Server input is closed"))?
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .map_err(|error| match error {
                    mpsc::RecvTimeoutError::Timeout => failed("Codex App Server timed out"),
                    mpsc::RecvTimeoutError::Disconnected => {
                        failed("could not send a Codex App Server request")
                    }
                })?
        })();
        if result.is_err() {
            return finish_with_cleanup(result, self.shutdown());
        }
        result
    }

    fn receive_line(&mut self, timeout: Duration) -> Result<String, RehomeError> {
        if self.overflow.load(Ordering::Acquire) {
            return Err(failed("Codex App Server exceeded the output backlog limit"));
        }
        self.receiver
            .as_ref()
            .ok_or_else(|| failed("Codex App Server output is closed"))?
            .recv_timeout(timeout)
            .map_err(|error| match error {
                mpsc::RecvTimeoutError::Timeout => failed("Codex App Server timed out"),
                mpsc::RecvTimeoutError::Disconnected => {
                    failed("Codex App Server exited before verification completed")
                }
            })?
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

#[cfg(test)]
mod cleanup_tests {
    use super::*;

    #[test]
    fn cleanup_deadline_never_joins_a_blocked_worker() {
        for (exited, expected) in [
            (Ok(false), ErrorCode::CodexCleanupUnconfirmed),
            (
                Err(std::io::Error::other("synthetic wait failure")),
                ErrorCode::CodexCleanupUnconfirmed,
            ),
            (Ok(true), ErrorCode::CodexVerificationFailed),
        ] {
            let (release, blocked) = mpsc::channel();
            let mut writer = Some(thread::spawn(move || {
                let _ = blocked.recv();
            }));
            let mut reader = None;
            let mut exited = Some(exited);
            let started = Instant::now();
            let result =
                finish_shutdown(|| exited.take().unwrap(), &mut writer, &mut reader, started);
            let elapsed = started.elapsed();
            release.send(()).unwrap();
            writer.take().unwrap().join().unwrap();
            assert_eq!(result.unwrap_err().code, expected);
            assert!(elapsed < Duration::from_secs(1));
        }
    }

    #[test]
    fn confirmed_exit_joins_completed_workers() {
        let mut writer = Some(thread::spawn(|| {}));
        let mut reader = Some(thread::spawn(|| {}));
        finish_shutdown(
            || Ok(true),
            &mut writer,
            &mut reader,
            Instant::now() + Duration::from_secs(2),
        )
        .unwrap();
        assert!(writer.is_none());
        assert!(reader.is_none());
    }

    #[test]
    fn unconfirmed_launch_cleanup_retains_job_directories() {
        let mut paths = None;
        let error = with_server::<()>(
            None,
            |home, workspace| {
                paths = Some((home.to_path_buf(), workspace.to_path_buf()));
                Err(RehomeError::new(
                    ErrorCode::CodexCleanupUnconfirmed,
                    "synthetic setup and cleanup failure",
                ))
            },
            |_, _| panic!("failed launch cannot run protocol"),
        )
        .unwrap_err();
        let (home, workspace) = paths.unwrap();
        let retained = home.is_dir() && workspace.is_dir();
        // Empty, explicitly captured synthetic job directories only.
        if home.exists() {
            std::fs::remove_dir(&home).unwrap();
        }
        if workspace.exists() {
            std::fs::remove_dir(&workspace).unwrap();
        }
        assert_eq!(error.code, ErrorCode::CodexCleanupUnconfirmed);
        assert!(retained);
    }

    #[test]
    fn protocol_and_cleanup_failure_preserve_both_causes_and_unconfirmed_code() {
        let cleanup = finish_shutdown(
            || Err(std::io::Error::other("synthetic exit confirmation failure")),
            &mut None,
            &mut None,
            Instant::now(),
        );
        let error = finish_with_cleanup::<()>(Err(failed("synthetic protocol failure")), cleanup)
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::CodexCleanupUnconfirmed);
        assert!(error.message.contains("synthetic protocol failure"));
        assert!(error
            .message
            .contains("synthetic exit confirmation failure"));
    }

    #[test]
    fn protocol_and_confirmed_worker_failure_preserve_both_causes_without_claiming_live_child() {
        let error = finish_with_cleanup::<()>(
            Err(RehomeError::new(
                ErrorCode::CodexAuthenticationRequired,
                "synthetic authentication failure",
            )),
            Err(failed("synthetic worker join failure after child exit")),
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::CodexAuthenticationRequired);
        assert!(error.message.contains("synthetic authentication failure"));
        assert!(error
            .message
            .contains("synthetic worker join failure after child exit"));
    }

    #[test]
    fn confirmed_cleanup_preserves_protocol_result_and_cleanup_only_error_is_not_success() {
        assert_eq!(finish_with_cleanup(Ok(7), Ok(())).unwrap(), 7);
        let protocol = failed("synthetic protocol failure");
        assert_eq!(
            finish_with_cleanup::<()>(Err(protocol.clone()), Ok(())).unwrap_err(),
            protocol
        );
        let cleanup = RehomeError::new(
            ErrorCode::CodexCleanupUnconfirmed,
            "synthetic cleanup failure",
        );
        assert_eq!(
            finish_with_cleanup(Ok(7), Err(cleanup.clone())).unwrap_err(),
            cleanup
        );
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::{
        cell::RefCell,
        os::windows::io::{AsHandle, AsRawHandle, OwnedHandle},
    };

    // Only synthetic PowerShell children; no CLI lookup, profile or account access.
    fn fake_command(script: &str, home: &Path, cwd: &Path) -> Command {
        let mut command = Command::new("powershell.exe");
        command
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .env("CODEX_HOME", home)
            .current_dir(cwd);
        command
    }

    fn child_handle(transport: &StdioTransport) -> OwnedHandle {
        transport.child.as_handle().try_clone_to_owned().unwrap()
    }

    fn assert_exited(handle: &OwnedHandle) {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetExitCodeProcess(process: *mut std::ffi::c_void, code: *mut u32) -> i32;
        }
        let mut code = 259; // STILL_ACTIVE
        assert_ne!(
            unsafe { GetExitCodeProcess(handle.as_raw_handle(), &mut code) },
            0
        );
        assert_ne!(code, 259, "background child survived verification");
    }

    #[test]
    fn preflight_uses_job_owned_temporary_home() {
        for valid in [true, false] {
            let launched = RefCell::new(None);
            let result = with_server(
                None,
                |home, cwd| {
                    assert_ne!(home, Path::new("C:/Synthetic/restore-target"));
                    assert_ne!(home, cwd);
                    assert_eq!(std::fs::read_dir(home).unwrap().count(), 0);
                    assert_eq!(std::fs::read_dir(cwd).unwrap().count(), 0);
                    let script = if valid {
                        "$request = [Console]::ReadLine() | ConvertFrom-Json; @{id=$request.id; result=@{codexHome=$env:CODEX_HOME; platformFamily='windows'; platformOs='windows'; userAgent='synthetic'}} | ConvertTo-Json -Compress; [Console]::Out.Flush(); $null = [Console]::ReadLine(); Start-Sleep -Seconds 60"
                    } else {
                        "$null = [Console]::ReadLine(); [Console]::WriteLine('SENSITIVE malformed'); [Console]::Out.Flush(); Start-Sleep -Seconds 60"
                    };
                    let transport = StdioTransport::spawn(fake_command(script, home, cwd))?;
                    *launched.borrow_mut() = Some((
                        home.to_path_buf(),
                        cwd.to_path_buf(),
                        child_handle(&transport),
                    ));
                    Ok(transport)
                },
                |transport, _| Client::new(transport).initialize(),
            );
            assert_eq!(result.is_ok(), valid);
            if let Err(error) = result {
                assert!(!error.message.contains("SENSITIVE"));
            }
            let (home, workspace, handle) = launched.into_inner().unwrap();
            assert!(!home.exists());
            assert!(!workspace.exists());
            assert_exited(&handle);
        }
    }

    #[test]
    fn server_launch_passes_safe_overrides_and_job_paths() {
        let directory = tempfile::tempdir().unwrap();
        let command = server_command(
            Path::new("synthetic-codex.exe"),
            directory.path(),
            directory.path(),
        );
        let args: Vec<_> = command
            .get_args()
            .map(|arg| arg.to_str().unwrap())
            .collect();
        assert_eq!(&args[..2], &["app-server", "--stdio"]);
        for setting in [
            "features.hooks=false",
            "features.plugins=false",
            "features.apps=false",
            "features.browser_use=false",
            "features.browser_use_external=false",
            "features.computer_use=false",
            "features.shell_tool=false",
            "features.unified_exec=false",
            "features.multi_agent=false",
            "features.goals=false",
            "features.memories=false",
            "features.skill_mcp_dependency_install=false",
            "notify=[]",
            "web_search=\"disabled\"",
            "approval_policy=\"never\"",
            "sandbox_mode=\"read-only\"",
        ] {
            assert!(
                args.windows(2).any(|pair| pair == ["-c", setting]),
                "missing {setting}"
            );
        }
        assert!(!args.iter().any(|arg| arg.starts_with("mcp_servers=")));
        assert_eq!(command.get_current_dir(), Some(directory.path()));
        assert!(
            command
                .get_envs()
                .any(|(key, value)| key == "CODEX_HOME"
                    && value == Some(directory.path().as_os_str()))
        );
    }

    #[test]
    fn cli_discovery_skips_earlier_shim_and_missing_native() {
        let directory = tempfile::tempdir().unwrap();
        let shim = directory.path().join("early/codex.cmd");
        let native = directory.path().join("later/codex.exe");
        std::fs::create_dir_all(shim.parent().unwrap()).unwrap();
        std::fs::create_dir_all(native.parent().unwrap()).unwrap();
        std::fs::write(&shim, b"synthetic candidate only").unwrap();
        std::fs::write(&native, b"synthetic candidate only").unwrap();
        let candidates = vec![
            directory.path().join("early/codex.exe"),
            shim.clone(),
            native.clone(),
        ];
        assert_eq!(
            select_registration_cli(candidates.clone(), |_| true),
            Some(shim),
            "registration must keep its first-existing candidate semantics"
        );
        assert_eq!(
            select_verifier_cli(SourceOs::Windows, candidates),
            Some(native)
        );
    }

    #[test]
    fn cli_discovery_preserves_native_order_and_case_insensitive_extension() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first.EXE");
        let second = directory.path().join("second.exe");
        std::fs::write(&first, b"synthetic candidate only").unwrap();
        std::fs::write(&second, b"synthetic candidate only").unwrap();
        assert_eq!(
            select_verifier_cli(SourceOs::Windows, vec![first.clone(), second]),
            Some(first)
        );
    }

    #[test]
    fn cli_discovery_rejects_shim_only_and_non_files() {
        let directory = tempfile::tempdir().unwrap();
        let shim = directory.path().join("codex.cmd");
        let native_directory = directory.path().join("codex.exe");
        std::fs::write(&shim, b"synthetic candidate only").unwrap();
        std::fs::create_dir(&native_directory).unwrap();
        assert_eq!(
            select_verifier_cli(SourceOs::Windows, vec![native_directory, shim]),
            None
        );
    }

    #[test]
    fn cli_discovery_preserves_macos_extensionless_candidates() {
        let directory = tempfile::tempdir().unwrap();
        let native = directory.path().join("codex");
        std::fs::write(&native, b"synthetic candidate only").unwrap();
        assert_eq!(
            select_verifier_cli(SourceOs::Macos, vec![native.clone()]),
            Some(native)
        );
    }

    #[test]
    fn command_shims_are_rejected_before_spawn() {
        let directory = tempfile::tempdir().unwrap();
        for extension in ["cmd", "bat"] {
            let shim = directory
                .path()
                .join(format!("synthetic-codex.{extension}"));
            std::fs::write(&shim, "@echo off\r\nexit /b 0\r\n").unwrap();
            let mut command = Command::new(shim);
            command
                .current_dir(directory.path())
                .env("CODEX_HOME", directory.path());
            let result = StdioTransport::spawn(command);
            assert!(
                result.is_err(),
                "command shims cannot provide single-child ownership"
            );
            let error = result.err().unwrap();
            assert_eq!(error.code, ErrorCode::CodexAppServerUnavailable);
            assert!(error.message.contains("native Codex executable"));
        }
    }

    #[test]
    fn server_exit_is_sanitized() {
        let directory = tempfile::tempdir().unwrap();
        let mut transport = StdioTransport::spawn(fake_command(
            "[Console]::Error.WriteLine('SENSITIVE'); exit 7",
            directory.path(),
            directory.path(),
        ))
        .unwrap();
        let error = transport.receive_line(REQUEST_TIMEOUT).unwrap_err();
        assert_eq!(error.code, ErrorCode::CodexVerificationFailed);
        assert!(!error.message.contains("SENSITIVE"));
        transport.shutdown().unwrap();
        assert!(transport.child.try_wait().unwrap().is_some());
        assert!(transport.reader.is_none());
    }

    #[test]
    fn hung_server_times_out_and_is_terminated() {
        let directory = tempfile::tempdir().unwrap();
        let mut transport = StdioTransport::spawn(fake_command(
            "$null = [Console]::ReadLine(); Start-Sleep -Seconds 60",
            directory.path(),
            directory.path(),
        ))
        .unwrap();
        let handle = child_handle(&transport);
        let started = Instant::now();
        let error = Client::new(&mut transport).initialize().unwrap_err();
        assert_eq!(error.message, "Codex App Server timed out");
        assert!(started.elapsed() >= REQUEST_TIMEOUT);
        assert!(started.elapsed() < Duration::from_secs(20));
        transport.shutdown().unwrap();
        assert_exited(&handle);
        assert!(transport.child.try_wait().unwrap().is_some());
        assert!(transport.reader.is_none());
    }

    #[test]
    fn backpressured_non_reading_child_times_out_and_is_cleaned_up() {
        let launched = RefCell::new(None);
        let request_started = RefCell::new(None);
        let (result, reader_cleaned, writer_cleaned) = with_server(
            None,
            |home, cwd| {
                // The finite lifetime also bounds the RED run of the old blocking writer.
                let transport = StdioTransport::spawn(fake_command(
                "[Console]::WriteLine('ready'); [Console]::Out.Flush(); Start-Sleep -Seconds 15",
                home, cwd,
            ))?;
                *launched.borrow_mut() = Some((
                    home.to_path_buf(),
                    cwd.to_path_buf(),
                    child_handle(&transport),
                ));
                Ok(transport)
            },
            |transport, _| {
                assert_eq!(transport.receive_line(REQUEST_TIMEOUT)?.trim(), "ready");
                *request_started.borrow_mut() = Some(Instant::now());
                let result = Client::new(transport).request(
                    "synthetic/backpressure",
                    json!({"padding": "x".repeat(65536)}),
                );
                Ok((
                    result,
                    transport.reader.is_none(),
                    transport.writer.is_none(),
                ))
            },
        )
        .unwrap();
        let elapsed = request_started.into_inner().unwrap().elapsed();
        let (home, workspace, handle) = launched.into_inner().unwrap();
        assert_exited(&handle);
        assert!(!home.exists());
        assert!(!workspace.exists());
        eprintln!("backpressured request returned and child reaped after {elapsed:?}");
        assert!(
            elapsed < REQUEST_TIMEOUT + Duration::from_secs(2),
            "blocked stdin escaped the request deadline: {elapsed:?}"
        );
        assert_eq!(result.unwrap_err().message, "Codex App Server timed out");
        assert!(
            reader_cleaned,
            "write failure returned before reader cleanup"
        );
        assert!(
            writer_cleaned,
            "write failure returned before writer cleanup"
        );
    }

    #[test]
    fn drop_kills_and_reaps_child_after_error_or_unwind() {
        let directory = tempfile::tempdir().unwrap();
        let transport = StdioTransport::spawn(fake_command(
            "Start-Sleep -Seconds 60",
            directory.path(),
            directory.path(),
        ))
        .unwrap();
        let handle = child_handle(&transport);
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _guard = transport;
            panic!("synthetic caller panic");
        }));
        assert_exited(&handle);
    }

    #[test]
    fn spawn_failure_cleans_preflight_directories() {
        let paths = RefCell::new(None);
        let error = with_server(
            None,
            |home, cwd| {
                *paths.borrow_mut() = Some((home.to_path_buf(), cwd.to_path_buf()));
                StdioTransport::spawn(server_command(
                    &cwd.join("missing-synthetic.exe"),
                    home,
                    cwd,
                ))
            },
            |transport, _| Client::new(transport).initialize(),
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::CodexAppServerUnavailable);
        let (home, cwd) = paths.into_inner().unwrap();
        assert!(!home.exists());
        assert!(!cwd.exists());
        assert!(!error.message.contains("missing-synthetic"));
    }

    #[test]
    fn oversized_and_incomplete_child_output_is_bounded() {
        for script in [
            "[Console]::Write('x' * 1048577); [Console]::Out.Flush(); Start-Sleep -Seconds 60",
            "[Console]::Write('{'); exit 0",
        ] {
            let directory = tempfile::tempdir().unwrap();
            let mut transport =
                StdioTransport::spawn(fake_command(script, directory.path(), directory.path()))
                    .unwrap();
            let handle = child_handle(&transport);
            assert_eq!(
                transport.receive_line(REQUEST_TIMEOUT).unwrap_err().code,
                ErrorCode::CodexVerificationFailed
            );
            transport.shutdown().unwrap();
            assert_exited(&handle);
            assert!(transport.reader.is_none());
        }
    }

    #[test]
    fn overflowing_reader_queue_never_leaves_a_blocked_sender() {
        let directory = tempfile::tempdir().unwrap();
        let mut transport = StdioTransport::spawn(fake_command(
            "1..1000 | ForEach-Object { [Console]::WriteLine('{}') }; [Console]::Out.Flush(); Start-Sleep -Seconds 60",
            directory.path(), directory.path(),
        )).unwrap();
        let deadline = Instant::now() + REQUEST_TIMEOUT;
        while !transport.overflow.load(Ordering::Acquire) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            transport.receive_line(REQUEST_TIMEOUT).unwrap_err().message,
            "Codex App Server exceeded the output backlog limit"
        );
        transport.shutdown().unwrap();
        assert!(transport.reader.is_none());
        assert!(transport.child.try_wait().unwrap().is_some());
    }

    #[test]
    fn verification_keeps_target_home_and_workspace_until_child_exit() {
        let target = tempfile::tempdir().unwrap();
        for success in [true, false] {
            let launched = RefCell::new(None);
            let result = with_server(
                Some(target.path()),
                |home, cwd| {
                    assert_eq!(home, target.path());
                    let transport =
                        StdioTransport::spawn(fake_command("Start-Sleep -Seconds 60", home, cwd))?;
                    *launched.borrow_mut() = Some((cwd.to_path_buf(), child_handle(&transport)));
                    Ok(transport)
                },
                |_, cwd| {
                    assert!(cwd.is_dir());
                    assert!(target.path().is_dir());
                    if success {
                        Ok(())
                    } else {
                        Err(failed("synthetic verification failure"))
                    }
                },
            );
            assert_eq!(result.is_ok(), success);
            let (cwd, handle) = launched.into_inner().unwrap();
            assert_exited(&handle);
            assert!(!cwd.exists());
            assert!(target.path().is_dir());
        }
    }
}
