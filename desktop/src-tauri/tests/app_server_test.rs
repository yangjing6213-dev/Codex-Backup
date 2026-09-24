use rehome_desktop_lib::core::{
    app_server::{verify_with_transport, AppServerTransport, CodexAccessRequest, PROBE_MESSAGE},
    error::{ErrorCode, RehomeError},
};
use serde_json::{json, Value};
use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};
use uuid::Uuid;

const THREAD_A: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const THREAD_B: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const FORK: &str = "ephemeral-fork";

fn request(ids: &[&str], probe: &str) -> CodexAccessRequest {
    CodexAccessRequest {
        codex_home: PathBuf::from("C:/Synthetic/.codex"),
        required_thread_ids: ids.iter().map(|id| Uuid::parse_str(id).unwrap()).collect(),
        probe_thread_id: Uuid::parse_str(probe).unwrap(),
    }
}

fn safe_config() -> Value {
    json!({"config": {
        "features": {"hooks": false, "plugins": false, "apps": false,
            "browser_use": false, "browser_use_external": false, "computer_use": false,
            "shell_tool": false, "unified_exec": false, "multi_agent": false,
            "goals": false, "memories": false, "skill_mcp_dependency_install": false},
        "notify": [], "web_search": "disabled", "approval_policy": "never",
        "sandbox_mode": "read-only", "mcp_servers": {}
    }, "origins": {}, "layers": null})
}

fn safe_fork(parent: &str) -> Value {
    json!({"thread": {"id": FORK, "forkedFromId": parent, "ephemeral": true,
        "cwd": "$workspace", "environments": [], "canAcceptDirectInput": true},
        "cwd": "$workspace", "runtimeWorkspaceRoots": ["$workspace"],
        "approvalPolicy": "never", "sandbox": {"type": "readOnly", "networkAccess": false}})
}

fn agent(text: &str) -> Value {
    json!({"method": "item/completed", "params": {
        "threadId": FORK, "turnId": "probe-turn", "completedAtMs": 1,
        "item": {"id": "answer", "type": "agentMessage", "text": text}}})
}

fn completed(status: &str) -> Value {
    json!({"method": "turn/completed", "params": {"threadId": FORK,
        "turn": {"id": "probe-turn", "items": [], "status": status}}})
}

#[derive(Default)]
struct ScriptedTransport {
    responses: HashMap<String, VecDeque<Value>>,
    notifications: VecDeque<String>,
    incoming: VecDeque<String>,
    outbound: Vec<Value>,
    trace: Rc<RefCell<Vec<String>>>,
    early_events: bool,
    write_delay: Duration,
    first_receive_timeout: Option<Duration>,
}

impl ScriptedTransport {
    fn respond(mut self, method: &str, result: Value) -> Self {
        self.responses
            .entry(method.into())
            .or_default()
            .push_back(json!({"result": result}));
        self
    }

    fn notify(mut self, notification: Value) -> Self {
        self.notifications.push_back(notification.to_string());
        self
    }

    fn success() -> Self {
        Self::default()
            .respond("initialize", json!({"codexHome": "C:/Synthetic/.codex",
                "platformFamily": "windows", "platformOs": "windows", "userAgent": "synthetic-test"}))
            .respond("config/read", safe_config())
            .respond("account/read", json!({"account": {"type": "chatgpt", "email": null, "planType": "unknown"}, "requiresOpenaiAuth": true}))
            .respond("thread/list", json!({"data": [{"id": THREAD_A}], "nextCursor": "next-page"}))
            .respond("thread/list", json!({"data": [], "nextCursor": null}))
            .respond("thread/list", json!({"data": [{"id": THREAD_B}], "nextCursor": null}))
            .respond("thread/read", json!({"thread": {"id": THREAD_A}}))
            .respond("thread/read", json!({"thread": {"id": THREAD_B}}))
            .respond("thread/fork", safe_fork(THREAD_B))
            .respond("turn/start", json!({"turn": {"id": "probe-turn"}}))
            .notify(agent("ok"))
            .notify(completed("completed"))
    }

    fn replace(&mut self, method: &str, result: Value) {
        self.responses
            .insert(method.into(), VecDeque::from([json!({"result": result})]));
    }

    fn called(&self, method: &str) -> bool {
        self.outbound
            .iter()
            .any(|message| message["method"] == method)
    }

    fn params_for(&self, method: &str) -> Vec<&Value> {
        self.outbound
            .iter()
            .filter(|message| message["method"] == method)
            .map(|message| &message["params"])
            .collect()
    }

    fn verify(
        &mut self,
    ) -> Result<rehome_desktop_lib::core::models::CodexAccessVerification, RehomeError> {
        verify_with_transport(self, &request(&[THREAD_A, THREAD_B], THREAD_B), &mut || {})
    }
}

impl AppServerTransport for ScriptedTransport {
    fn send_line(&mut self, line: &str, timeout: Duration) -> Result<(), RehomeError> {
        assert!(timeout <= Duration::from_secs(10));
        std::thread::sleep(std::mem::take(&mut self.write_delay));
        let message: Value = serde_json::from_str(line).unwrap();
        let method = message["method"].as_str().unwrap();
        self.trace.borrow_mut().push(method.into());
        if let Some(id) = message.get("id") {
            let mut response = self
                .responses
                .get_mut(method)
                .unwrap_or_else(|| panic!("unexpected {method}"))
                .pop_front()
                .unwrap_or_else(|| panic!("no response for {method}"));
            // The fake only substitutes runtime values; safety expectations are literal fixtures.
            if method == "thread/fork" {
                let cwd = message["params"]["cwd"].as_str().unwrap();
                assert!(Path::new(cwd).is_absolute());
                assert_eq!(std::fs::read_dir(cwd).unwrap().count(), 0);
                let result = &mut response["result"];
                if result["cwd"] == "$workspace" {
                    result["cwd"] = json!(cwd);
                }
                if result["thread"]["cwd"] == "$workspace" {
                    result["thread"]["cwd"] = json!(cwd);
                }
                if result["runtimeWorkspaceRoots"] == json!(["$workspace"]) {
                    result["runtimeWorkspaceRoots"] = json!([cwd]);
                }
            }
            response["id"] = id.clone();
            if method == "turn/start" && self.early_events {
                self.incoming.append(&mut self.notifications);
            }
            self.incoming.push_back(response.to_string());
            if method == "turn/start" {
                self.incoming.append(&mut self.notifications);
            }
        }
        self.outbound.push(message);
        Ok(())
    }

    fn receive_line(&mut self, timeout: Duration) -> Result<String, RehomeError> {
        self.first_receive_timeout.get_or_insert(timeout);
        assert!(timeout <= Duration::from_secs(120));
        self.incoming
            .pop_front()
            .ok_or_else(|| RehomeError::new(ErrorCode::CodexVerificationFailed, "synthetic EOF"))
    }
}

fn assert_rejected(transport: &mut ScriptedTransport, forbidden: &str) {
    assert_eq!(
        transport.verify().unwrap_err().code,
        ErrorCode::CodexVerificationFailed
    );
    assert!(!transport.called(forbidden));
}

#[test]
fn recognizes_archived_threads_across_pages() {
    let mut transport = ScriptedTransport::success();
    let report = transport.verify().unwrap();
    assert_eq!(report.required_threads, 2);
    assert_eq!(report.recognized_threads, 2);
    assert!(report.threads_recognized && report.continuation_probe_valid && report.ephemeral_fork);
    assert_eq!(
        report.probe_thread_id,
        Some(Uuid::parse_str(THREAD_B).unwrap())
    );
    let pages = transport.params_for("thread/list");
    assert_eq!(pages.len(), 3);
    assert_eq!(pages[0]["archived"], false);
    assert_eq!(pages[1]["cursor"], "next-page");
    assert_eq!(pages[2]["archived"], true);
    assert!(pages[2]["cursor"].is_null());
    for params in pages {
        assert_eq!(params["useStateDbOnly"], true);
        assert_eq!(
            params["sourceKinds"],
            json!([
                "cli",
                "vscode",
                "exec",
                "appServer",
                "subAgent",
                "subAgentReview",
                "subAgentCompact",
                "subAgentThreadSpawn",
                "subAgentOther",
                "unknown"
            ])
        );
        assert_eq!(params["modelProviders"], json!([]));
    }
    let reads = transport.params_for("thread/read");
    assert_eq!(
        reads[0],
        &json!({"threadId": THREAD_A, "includeTurns": false})
    );
    assert_eq!(
        reads[1],
        &json!({"threadId": THREAD_B, "includeTurns": false})
    );
    assert_eq!(
        transport.outbound[0],
        json!({"id": 1, "method": "initialize", "params": {
        "clientInfo": {"name": "enhe-codex-backup", "version": env!("CARGO_PKG_VERSION")},
        "capabilities": {"experimentalApi": true}}})
    );
    assert_eq!(transport.outbound[1], json!({"method": "initialized"}));
    assert_eq!(
        transport.params_for("account/read")[0],
        &json!({"refreshToken": false})
    );
    assert!(!serde_json::to_string(&report).unwrap().contains("ok"));
}

#[test]
fn never_falls_back_to_original_thread() {
    let mut transport = ScriptedTransport::success();
    transport.responses.insert(
        "thread/fork".into(),
        VecDeque::from([json!({"error": {"code": -32601, "message": "SENSITIVE"}})]),
    );
    let error = transport.verify().unwrap_err();
    assert_eq!(error.code, ErrorCode::CodexVerificationFailed);
    assert!(!error.message.contains("SENSITIVE"));
    assert!(!transport.called("turn/start"));
}

#[test]
fn missing_required_account_is_rejected() {
    let mut transport = ScriptedTransport::success();
    transport.replace(
        "account/read",
        json!({"account": null, "requiresOpenaiAuth": true}),
    );
    assert_eq!(
        transport.verify().unwrap_err().code,
        ErrorCode::CodexAuthenticationRequired
    );
    assert!(!transport.called("thread/list"));
}

#[test]
fn provider_without_required_account_is_supported() {
    let mut transport = ScriptedTransport::success();
    transport.replace(
        "account/read",
        json!({"account": null, "requiresOpenaiAuth": false}),
    );
    transport.verify().unwrap();
}

#[test]
fn probe_fork_is_ephemeral_read_only_and_uses_empty_workspace() {
    let mut transport = ScriptedTransport::success();
    transport.verify().unwrap();
    let fork = transport.params_for("thread/fork")[0];
    let cwd = fork["cwd"].as_str().unwrap();
    assert!(!Path::new(cwd).exists());
    assert_eq!(fork["threadId"], THREAD_B);
    assert_eq!(fork["ephemeral"], true);
    assert_eq!(fork["excludeTurns"], true);
    assert_eq!(fork["deferGoalContinuation"], true);
    assert_eq!(fork["sandbox"], "read-only");
    assert_eq!(fork["approvalPolicy"], "never");
    assert_eq!(fork["runtimeWorkspaceRoots"], json!([cwd]));
    assert_eq!(fork["config"]["features.goals"], false);
    assert_eq!(fork["config"]["features.hooks"], false);
    assert_eq!(fork["config"]["notify"], json!([]));
    assert_eq!(
        transport.params_for("config/read")[0],
        &json!({"includeLayers": false, "cwd": cwd})
    );
    let turn = transport.params_for("turn/start")[0];
    assert_eq!(turn["threadId"], FORK);
    assert_eq!(turn["cwd"], cwd);
    assert_eq!(turn["runtimeWorkspaceRoots"], json!([cwd]));
    assert_eq!(turn["environments"], json!([]));
    assert_eq!(turn["approvalPolicy"], "never");
    assert_eq!(
        turn["sandboxPolicy"],
        json!({"type": "readOnly", "networkAccess": false})
    );
    assert_eq!(
        turn["input"],
        json!([{"type": "text", "text": PROBE_MESSAGE, "text_elements": []}])
    );
}

#[test]
fn non_ephemeral_or_wrong_parent_fork_is_rejected() {
    for (field, value) in [
        ("ephemeral", json!(false)),
        ("forkedFromId", json!(THREAD_A)),
        ("id", json!(THREAD_B)),
        ("id", json!("")),
    ] {
        let mut transport = ScriptedTransport::success();
        let mut fork = safe_fork(THREAD_B);
        fork["thread"][field] = value;
        transport.replace("thread/fork", fork);
        assert_rejected(&mut transport, "turn/start");
    }
}

#[test]
fn false_direct_input_capability_is_rejected_before_turn() {
    let mut transport = ScriptedTransport::success();
    let mut fork = safe_fork(THREAD_B);
    fork["thread"]["canAcceptDirectInput"] = json!(false);
    transport.replace("thread/fork", fork);
    assert_rejected(&mut transport, "turn/start");
}

#[test]
fn null_direct_input_capability_is_rejected_before_turn() {
    let mut transport = ScriptedTransport::success();
    let mut fork = safe_fork(THREAD_B);
    fork["thread"]["canAcceptDirectInput"] = Value::Null;
    transport.replace("thread/fork", fork);
    assert_rejected(&mut transport, "turn/start");
}

#[test]
fn missing_direct_input_capability_is_rejected_before_turn() {
    let mut transport = ScriptedTransport::success();
    let mut fork = safe_fork(THREAD_B);
    fork["thread"]
        .as_object_mut()
        .unwrap()
        .remove("canAcceptDirectInput");
    transport.replace("thread/fork", fork);
    assert_rejected(&mut transport, "turn/start");
}

#[test]
fn unsafe_or_missing_fork_restrictions_are_rejected() {
    for (field, value) in [
        (
            "sandbox",
            json!({"type": "readOnly", "networkAccess": true}),
        ),
        ("sandbox", json!({"type": "dangerFullAccess"})),
        ("sandbox", Value::Null),
        ("cwd", json!("C:/Other")),
        ("runtimeWorkspaceRoots", json!(["C:/Other"])),
        ("runtimeWorkspaceRoots", Value::Null),
        ("approvalPolicy", json!("on-request")),
    ] {
        let mut transport = ScriptedTransport::success();
        let mut fork = safe_fork(THREAD_B);
        fork[field] = value;
        transport.replace("thread/fork", fork);
        assert_rejected(&mut transport, "turn/start");
    }
}

#[test]
fn enabled_mcp_is_rejected_but_explicitly_disabled_servers_are_allowed() {
    for server in [
        json!({"command": "SENSITIVE"}),
        json!({"enabled": true}),
        json!("unknown"),
    ] {
        let mut transport = ScriptedTransport::success();
        let mut config = safe_config();
        config["config"]["mcp_servers"] = json!({"synthetic": server});
        transport.replace("config/read", config);
        assert_rejected(&mut transport, "thread/fork");
    }
    let mut transport = ScriptedTransport::success();
    let mut config = safe_config();
    config["config"]["mcp_servers"] = json!({"synthetic": {"enabled": false}});
    transport.replace("config/read", config);
    transport.verify().unwrap();
}

#[test]
fn missing_or_unsafe_effective_settings_are_rejected() {
    for feature in [
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
    ] {
        for value in [Value::Null, json!(true)] {
            let mut transport = ScriptedTransport::success();
            let mut config = safe_config();
            config["config"]["features"][feature] = value;
            transport.replace("config/read", config);
            assert_rejected(&mut transport, "thread/fork");
        }
    }
    for (field, value) in [
        ("notify", json!(["SENSITIVE"])),
        ("notify", Value::Null),
        ("web_search", json!("live")),
        ("web_search", Value::Null),
        ("sandbox_mode", json!("workspace-write")),
        ("approval_policy", json!("on-request")),
        ("mcp_servers", json!("unknown")),
    ] {
        let mut transport = ScriptedTransport::success();
        let mut config = safe_config();
        config["config"][field] = value;
        transport.replace("config/read", config);
        assert_rejected(&mut transport, "thread/fork");
    }
}

#[test]
fn progress_callback_runs_once_after_reads_before_fork_even_if_probe_fails() {
    let mut transport = ScriptedTransport::success();
    transport.replace("thread/fork", safe_fork(THREAD_A));
    let trace = transport.trace.clone();
    let mut calls = 0;
    assert!(verify_with_transport(
        &mut transport,
        &request(&[THREAD_A, THREAD_B], THREAD_B),
        &mut || {
            calls += 1;
            trace.borrow_mut().push("recognized".into());
        }
    )
    .is_err());
    assert_eq!(calls, 1);
    let trace = trace.borrow();
    let index = trace
        .iter()
        .position(|entry| entry == "recognized")
        .unwrap();
    assert_eq!(trace[index - 1], "thread/read");
    assert_eq!(trace[index + 1], "thread/fork");
}

#[test]
fn missing_list_id_or_wrong_exact_read_prevents_progress_and_fork() {
    for wrong_read in [false, true] {
        let mut transport = ScriptedTransport::success();
        if wrong_read {
            transport.replace("thread/read", json!({"thread": {"id": THREAD_B}}));
        } else {
            transport.responses.get_mut("thread/list").unwrap()[0]["result"]["data"] = json!([]);
        }
        let mut calls = 0;
        assert!(verify_with_transport(
            &mut transport,
            &request(&[THREAD_A, THREAD_B], THREAD_B),
            &mut || calls += 1
        )
        .is_err());
        assert_eq!(calls, 0);
        assert!(!transport.called("thread/fork"));
    }
}

#[test]
fn request_deadline_includes_time_spent_writing() {
    let mut transport = ScriptedTransport::success();
    transport.write_delay = Duration::from_millis(30);
    transport.verify().unwrap();
    assert!(
        transport.first_receive_timeout.unwrap() <= Duration::from_millis(9980),
        "receive timeout restarted after the request write"
    );
}

#[test]
fn invalid_probe_selection_is_rejected_before_requests() {
    let mut transport = ScriptedTransport::success();
    assert!(
        verify_with_transport(&mut transport, &request(&[THREAD_A], THREAD_B), &mut || {}).is_err()
    );
    assert!(transport.outbound.is_empty());
}

#[test]
fn cursor_loops_and_excessive_pages_are_rejected() {
    for repeated in [true, false] {
        let mut transport = ScriptedTransport::success();
        transport.responses.insert("thread/list".into(), (0..=1024).map(|n| json!({"result": {
            "data": [], "nextCursor": if repeated { "loop".into() } else { format!("page-{n}") }
        }})).collect());
        assert_rejected(&mut transport, "thread/read");
        assert!(transport.params_for("thread/list").len() <= 1000);
    }
}

#[test]
fn early_turn_events_are_matched_after_start_response() {
    let mut transport = ScriptedTransport::success();
    transport.early_events = true;
    transport.verify().unwrap();
}

#[test]
fn notifications_before_turn_start_cannot_validate_the_new_probe() {
    let mut transport = ScriptedTransport::success();
    transport.incoming.extend([
        agent("old response").to_string(),
        completed("completed").to_string(),
    ]);
    transport.notifications.clear();
    assert!(transport.verify().is_err());
}

#[test]
fn completed_turn_requires_non_empty_matching_agent_message_before_completion() {
    for events in [
        vec![completed("completed")],
        vec![agent("  \n"), completed("completed")],
        vec![completed("completed"), agent("ok")],
        vec![
            {
                let mut event = agent("ok");
                event["params"]["turnId"] = json!("other");
                event
            },
            completed("completed"),
        ],
        vec![
            {
                let mut event = agent("ok");
                event["params"]["threadId"] = json!(THREAD_B);
                event
            },
            completed("completed"),
        ],
        vec![agent("ok"), completed("failed")],
        vec![agent("ok"), completed("interrupted")],
    ] {
        let mut transport = ScriptedTransport::success();
        transport.notifications = events.iter().map(Value::to_string).collect();
        assert!(transport.verify().is_err());
    }
}

#[test]
fn approval_and_server_tool_requests_are_rejected() {
    for method in [
        "item/commandExecution/requestApproval",
        "item/fileChange/requestApproval",
        "item/tool/call",
        "unknown/request",
    ] {
        let mut transport = ScriptedTransport::success();
        transport.incoming.push_back(
            json!({"id": 999, "method": method, "params": {"secret": "SENSITIVE"}}).to_string(),
        );
        let error = transport.verify().unwrap_err();
        assert_eq!(error.code, ErrorCode::CodexVerificationFailed);
        assert!(!error.message.contains("SENSITIVE"));
        assert!(!transport.called("turn/start"));
    }
}

#[test]
fn malformed_json_and_unmatched_response_are_sanitized() {
    for line in ["SENSITIVE", "[]", "{\"id\":999,\"result\":\"SENSITIVE\"}"] {
        let mut transport = ScriptedTransport::success();
        transport.incoming.push_back(line.into());
        let error = transport.verify().unwrap_err();
        assert_eq!(error.code, ErrorCode::CodexVerificationFailed);
        assert!(!error.message.contains("SENSITIVE"));
    }
}

#[test]
fn oversized_lines_and_notification_backlogs_are_rejected() {
    let mut transport = ScriptedTransport::success();
    transport.incoming.push_back("x".repeat(1024 * 1024 + 1));
    assert_rejected(&mut transport, "thread/fork");
    let mut transport = ScriptedTransport::success();
    transport
        .incoming
        .extend((0..1024).map(|_| agent("SENSITIVE").to_string()));
    assert_rejected(&mut transport, "thread/fork");
}
