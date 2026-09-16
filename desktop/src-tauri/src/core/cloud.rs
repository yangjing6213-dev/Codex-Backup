use crate::core::error::{ErrorCode, RehomeError};
use serde::{Deserialize, Serialize};
use std::{
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::NamedTempFile;

/// Cloud is an opt-in capability.  Keeping this state explicit makes it
/// possible for callers to prove that the OFF path never starts rclone or
/// restic against a remote repository.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CloudState {
    Off,
    Configuring,
    On,
    NeedsAttention,
}

impl Default for CloudState {
    fn default() -> Self {
        Self::Off
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CloudConfig {
    pub enabled: bool,
    pub remote_name: Option<String>,
    pub remote_path: Option<String>,
    pub repository_path: Option<String>,
    pub config_file: Option<PathBuf>,
}

impl Default for CloudConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            remote_name: None,
            remote_path: None,
            repository_path: None,
            config_file: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CloudOperationResult {
    pub state: CloudState,
    pub message: String,
    pub remote_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CloudConfigStart {
    pub config_file: PathBuf,
    pub remote_name: String,
    pub provider: String,
    pub completed: bool,
    pub question: Option<CloudConfigQuestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CloudConfigQuestion {
    pub state: String,
    pub name: Option<String>,
    pub help: Option<String>,
    pub default_value: Option<String>,
    pub is_password: bool,
    pub examples: Vec<String>,
    pub error: Option<String>,
}

pub fn cloud_calls_allowed(state: CloudState) -> bool {
    !matches!(state, CloudState::Off)
}

/// Build an argument vector for the rclone configuration command.  Secrets
/// are intentionally not accepted here: rclone writes the OAuth token into
/// its own protected configuration after the user completes authorization.
pub fn build_rclone_config_create_args(config_file: &Path, remote_name: &str) -> Vec<OsString> {
    vec![
        OsString::from("--config"),
        config_file.as_os_str().to_owned(),
        OsString::from("config"),
        OsString::from("create"),
        OsString::from(remote_name),
        OsString::from("onedrive"),
        OsString::from("--non-interactive"),
    ]
}

/// Build the restic copy command used only after the cloud gate is enabled.
/// The source and destination passwords are separate temporary files and are
/// never placed into process arguments.
pub fn build_restic_copy_args(
    source_repo: &Path,
    target_repo: &Path,
    source_password_file: &Path,
    target_password_file: &Path,
    snapshot: &str,
    rclone_executable: &Path,
) -> Vec<OsString> {
    vec![
        OsString::from("--repo"),
        target_repo.as_os_str().to_owned(),
        OsString::from("--password-file"),
        target_password_file.as_os_str().to_owned(),
        OsString::from("-o"),
        OsString::from(format!("rclone.program={}", rclone_executable.display())),
        OsString::from("copy"),
        OsString::from("--from-repo"),
        source_repo.as_os_str().to_owned(),
        OsString::from("--from-password-file"),
        source_password_file.as_os_str().to_owned(),
        OsString::from(snapshot),
    ]
}

pub fn start_one_drive_configuration(
    config_file: &Path,
    remote_name: &str,
    rclone_executable: &Path,
) -> Result<CloudConfigStart, RehomeError> {
    validate_remote_name(remote_name)?;
    if config_file.as_os_str().is_empty() {
        return Err(cloud_configuration(
            "OneDrive configuration path cannot be empty",
        ));
    }
    fs::create_dir_all(
        config_file
            .parent()
            .ok_or_else(|| cloud_configuration("OneDrive configuration path has no parent"))?,
    )
    .map_err(|error| {
        cloud_configuration(format!(
            "create rclone configuration directory failed: {error}"
        ))
    })?;
    let args = build_rclone_config_create_args(config_file, remote_name);
    let output = Command::new(rclone_executable)
        .args(args)
        .output()
        .map_err(|error| cloud_unavailable(format!("rclone could not be started: {error}")))?;
    if !output.status.success() {
        return Err(cloud_unavailable(command_error(&output)));
    }
    let response = parse_config_response(&output.stdout);
    Ok(CloudConfigStart {
        config_file: config_file.to_path_buf(),
        remote_name: remote_name.to_owned(),
        provider: "onedrive".to_owned(),
        completed: response
            .as_ref()
            .map_or(true, |response| response.state.is_empty()),
        question: response.and_then(|response| response.question),
    })
}

pub fn build_rclone_config_continue_args(
    config_file: &Path,
    remote_name: &str,
    state: &str,
    result: &str,
) -> Vec<OsString> {
    vec![
        OsString::from("--config"),
        config_file.as_os_str().to_owned(),
        OsString::from("config"),
        OsString::from("update"),
        OsString::from(remote_name),
        OsString::from("--continue"),
        OsString::from("--state"),
        OsString::from(state),
        OsString::from("--result"),
        OsString::from(result),
        OsString::from("--non-interactive"),
    ]
}

pub fn continue_one_drive_configuration(
    config_file: &Path,
    remote_name: &str,
    state: &str,
    result: &str,
    rclone_executable: &Path,
) -> Result<CloudConfigStart, RehomeError> {
    validate_remote_name(remote_name)?;
    if config_file.as_os_str().is_empty() || config_file.parent().is_none() {
        return Err(cloud_configuration(
            "OneDrive configuration path cannot be resolved",
        ));
    }
    if state.trim().is_empty() {
        return Err(cloud_configuration(
            "rclone configuration state cannot be empty",
        ));
    }
    let args = build_rclone_config_continue_args(config_file, remote_name, state, result);
    let output = Command::new(rclone_executable)
        .args(args)
        .output()
        .map_err(|error| {
            cloud_unavailable(format!("rclone configuration could not continue: {error}"))
        })?;
    if !output.status.success() {
        return Err(cloud_unavailable(command_error(&output)));
    }
    let response = parse_config_response(&output.stdout);
    Ok(CloudConfigStart {
        config_file: config_file.to_path_buf(),
        remote_name: remote_name.to_owned(),
        provider: "onedrive".to_owned(),
        completed: response
            .as_ref()
            .map_or(true, |response| response.state.is_empty()),
        question: response.and_then(|response| response.question),
    })
}

pub fn test_one_drive_connection(
    config: &CloudConfig,
    rclone_executable: &Path,
) -> Result<CloudOperationResult, RehomeError> {
    validate_cloud_config(config)?;
    let config_file = config
        .config_file
        .as_deref()
        .ok_or_else(|| cloud_configuration("rclone configuration file is missing"))?;
    let remote = remote_root(config)?;
    let probe = format!(
        "{remote}/.enhe-connection-check-{}.txt",
        uuid::Uuid::new_v4()
    );
    let marker = tempfile_marker();
    let downloaded = marker.with_extension("downloaded");
    let expected = b"ENHE Codex Backup connection check\n";
    fs::write(&marker, expected).map_err(|error| {
        cloud_unavailable(format!("create connection-check marker failed: {error}"))
    })?;
    let cleanup_local = || {
        let _ = fs::remove_file(&marker);
        let _ = fs::remove_file(&downloaded);
    };
    let cleanup_remote =
        || run_rclone_deletefile(rclone_executable, config_file, OsStr::new(&probe));
    if let Err(error) = run_rclone_copyto(
        rclone_executable,
        config_file,
        marker.as_os_str(),
        OsStr::new(&probe),
    ) {
        let _ = cleanup_remote();
        cleanup_local();
        return Err(error);
    }
    if let Err(error) = run_rclone_copyto(
        rclone_executable,
        config_file,
        OsStr::new(&probe),
        downloaded.as_os_str(),
    ) {
        let _ = cleanup_remote();
        cleanup_local();
        return Err(error);
    }
    let downloaded_bytes = fs::read(&downloaded).map_err(|error| {
        let _ = cleanup_remote();
        cleanup_local();
        cloud_unavailable(format!(
            "read downloaded connection-check marker failed: {error}"
        ))
    })?;
    if downloaded_bytes != expected {
        let _ = cleanup_remote();
        cleanup_local();
        return Err(cloud_unavailable(
            "downloaded connection-check marker did not match the uploaded data",
        ));
    }
    let cleanup = cleanup_remote();
    cleanup_local();
    cleanup?;
    Ok(CloudOperationResult {
        state: CloudState::On,
        message: "OneDrive connection test completed".to_owned(),
        remote_path: Some(remote),
    })
}

fn run_rclone_copyto(
    executable: &Path,
    config_file: &Path,
    source: &OsStr,
    target: &OsStr,
) -> Result<(), RehomeError> {
    let output = Command::new(executable)
        .args([
            OsString::from("--config"),
            config_file.as_os_str().to_owned(),
            OsString::from("copyto"),
            source.to_owned(),
            target.to_owned(),
            OsString::from("--no-traverse"),
            OsString::from("--retries"),
            OsString::from("1"),
            OsString::from("--low-level-retries"),
            OsString::from("1"),
        ])
        .output()
        .map_err(|error| {
            cloud_unavailable(format!("rclone connection test could not start: {error}"))
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(cloud_unavailable(command_error(&output)))
    }
}

fn run_rclone_deletefile(
    executable: &Path,
    config_file: &Path,
    remote_file: &OsStr,
) -> Result<(), RehomeError> {
    let output = Command::new(executable)
        .args([
            OsString::from("--config"),
            config_file.as_os_str().to_owned(),
            OsString::from("deletefile"),
            remote_file.to_owned(),
        ])
        .output()
        .map_err(|error| {
            cloud_unavailable(format!(
                "rclone connection-check cleanup could not start: {error}"
            ))
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(cloud_unavailable(command_error(&output)))
    }
}

pub fn copy_local_snapshot_to_cloud(
    config: &CloudConfig,
    source_repository: &Path,
    source_password: &str,
    target_password: &str,
    snapshot: &str,
    restic_executable: &Path,
    rclone_executable: &Path,
) -> Result<CloudOperationResult, RehomeError> {
    validate_cloud_config(config)?;
    if source_password.is_empty() || target_password.is_empty() {
        return Err(RehomeError::new(
            ErrorCode::BackupPasswordRequired,
            "source and cloud repository passwords are required",
        ));
    }
    if !(8..=64).contains(&snapshot.len())
        || !snapshot
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(cloud_configuration(
            "snapshot id is not a valid restic identifier",
        ));
    }
    let source_password_file = NamedTempFile::new().map_err(|error| {
        cloud_unavailable(format!("create source password file failed: {error}"))
    })?;
    let target_password_file = NamedTempFile::new().map_err(|error| {
        cloud_unavailable(format!("create cloud password file failed: {error}"))
    })?;
    fs::write(source_password_file.path(), source_password.as_bytes()).map_err(|error| {
        cloud_unavailable(format!("write source password file failed: {error}"))
    })?;
    fs::write(target_password_file.path(), target_password.as_bytes())
        .map_err(|error| cloud_unavailable(format!("write cloud password file failed: {error}")))?;
    let target = remote_repository(config)?;
    let config_file = config
        .config_file
        .as_deref()
        .ok_or_else(|| cloud_configuration("rclone configuration file is missing"))?;
    let target_path = Path::new(&target);
    let probe_args = vec![
        OsString::from("--repo"),
        target_path.as_os_str().to_owned(),
        OsString::from("--password-file"),
        target_password_file.path().as_os_str().to_owned(),
        OsString::from("-o"),
        OsString::from(format!("rclone.program={}", rclone_executable.display())),
        OsString::from("cat"),
        OsString::from("config"),
    ];
    let probe = run_remote_restic(restic_executable, probe_args, config_file);
    if probe.is_err() {
        let init_args = vec![
            OsString::from("--repo"),
            target_path.as_os_str().to_owned(),
            OsString::from("--password-file"),
            target_password_file.path().as_os_str().to_owned(),
            OsString::from("-o"),
            OsString::from(format!("rclone.program={}", rclone_executable.display())),
            OsString::from("init"),
        ];
        run_remote_restic(restic_executable, init_args, config_file)?;
    }
    let args = build_restic_copy_args(
        source_repository,
        target_path,
        source_password_file.path(),
        target_password_file.path(),
        snapshot,
        rclone_executable,
    );
    run_remote_restic(restic_executable, args, config_file)?;
    Ok(CloudOperationResult {
        state: CloudState::On,
        message: "local snapshot copied to OneDrive".to_owned(),
        remote_path: Some(target),
    })
}

pub fn validate_cloud_config(config: &CloudConfig) -> Result<(), RehomeError> {
    if !config.enabled {
        return Err(RehomeError::new(
            ErrorCode::CloudDisabled,
            "cloud backup is disabled; no remote operation is allowed",
        ));
    }
    let remote_name = config.remote_name.as_deref().unwrap_or_default();
    let remote_path = config.remote_path.as_deref().unwrap_or_default();
    let repository_path = config.repository_path.as_deref().unwrap_or_default();
    validate_remote_name(remote_name)?;
    if !valid_remote_path(remote_path) || !valid_remote_path(repository_path) {
        return Err(cloud_configuration(
            "cloud paths must use non-empty safe segments without traversal",
        ));
    }
    if config
        .config_file
        .as_deref()
        .map_or(true, |path| !path.is_file())
    {
        return Err(cloud_configuration(
            "cloud backup requires an existing rclone configuration file",
        ));
    }
    Ok(())
}

pub fn remote_repository(config: &CloudConfig) -> Result<String, RehomeError> {
    validate_cloud_config(config)?;
    Ok(format!(
        "rclone:{}/{}",
        remote_root(config)?,
        config
            .repository_path
            .as_deref()
            .unwrap_or_default()
            .trim_matches('/')
    ))
}

fn run_remote_restic(
    executable: &Path,
    args: Vec<OsString>,
    config_file: &Path,
) -> Result<(), RehomeError> {
    let output = Command::new(executable)
        .args(args)
        .env("RCLONE_CONFIG", config_file)
        .output()
        .map_err(|error| {
            cloud_unavailable(format!("restic cloud operation could not start: {error}"))
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(cloud_unavailable(command_error(&output)))
    }
}

struct RcloneConfigResponse {
    state: String,
    question: Option<CloudConfigQuestion>,
}

fn parse_config_response(bytes: &[u8]) -> Option<RcloneConfigResponse> {
    let text = String::from_utf8_lossy(bytes);
    if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}')) {
        if start < end {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text[start..=end]) {
                if let Some(response) = response_from_value(&value) {
                    return Some(response);
                }
            }
        }
    }
    for line in text.lines().rev() {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(line.trim()) {
            if let Some(response) = response_from_value(&value) {
                return Some(response);
            }
        }
    }
    None
}

fn response_from_value(value: &serde_json::Value) -> Option<RcloneConfigResponse> {
    let state = value.get("State").and_then(serde_json::Value::as_str)?;
    let option = value.get("Option");
    let question = option.map(|option| CloudConfigQuestion {
        state: state.to_owned(),
        name: option
            .get("Name")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        help: option
            .get("Help")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        default_value: option.get("Default").and_then(value_to_string),
        is_password: option
            .get("IsPassword")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        examples: option
            .get("Examples")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|example| example.get("Value").and_then(value_to_string))
            .collect(),
        error: value
            .get("Error")
            .and_then(serde_json::Value::as_str)
            .filter(|error| !error.is_empty())
            .map(str::to_owned),
    });
    Some(RcloneConfigResponse {
        state: state.to_owned(),
        question,
    })
}

fn value_to_string(value: &serde_json::Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_owned)
        .or_else(|| value.as_bool().map(|value| value.to_string()))
        .or_else(|| value.as_i64().map(|value| value.to_string()))
}

fn remote_root(config: &CloudConfig) -> Result<String, RehomeError> {
    let name = config
        .remote_name
        .as_deref()
        .ok_or_else(|| cloud_configuration("OneDrive remote name is missing"))?;
    let path = config
        .remote_path
        .as_deref()
        .ok_or_else(|| cloud_configuration("OneDrive remote path is missing"))?
        .trim_matches('/');
    Ok(format!("{name}:{path}"))
}

fn validate_remote_name(name: &str) -> Result<(), RehomeError> {
    if name.trim().is_empty()
        || name
            .chars()
            .any(|character| character.is_control() || matches!(character, ':' | '/' | '\\'))
    {
        return Err(cloud_configuration(
            "OneDrive remote name must be a single safe name",
        ));
    }
    Ok(())
}

fn valid_remote_path(path: &str) -> bool {
    !path.trim().is_empty()
        && path.split('/').all(|segment| {
            !segment.is_empty()
                && segment != "."
                && segment != ".."
                && !segment
                    .chars()
                    .any(|character| character.is_control() || matches!(character, ':' | '\\'))
        })
}

fn tempfile_marker() -> PathBuf {
    std::env::temp_dir().join(format!("enhe-cloud-check-{}.txt", uuid::Uuid::new_v4()))
}

fn command_error(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if stderr.is_empty() {
        format!("command exited with {}", output.status)
    } else {
        stderr
    }
}

fn cloud_configuration(message: impl Into<String>) -> RehomeError {
    RehomeError::new(ErrorCode::CloudConfiguration, message)
}

fn cloud_unavailable(message: impl Into<String>) -> RehomeError {
    RehomeError::new(ErrorCode::CloudUnavailable, message)
}

#[cfg(test)]
mod tests {
    use super::parse_config_response;

    #[test]
    fn parses_rclone_multiline_question_protocol() {
        let response = parse_config_response(
            br#"{
                "State": "*oauth-islocal,teamdrive,,",
                "Option": {
                    "Name": "config_is_local",
                    "Help": "Use a browser",
                    "Default": true,
                    "Examples": [{"Value": "true"}, {"Value": "false"}],
                    "IsPassword": false
                },
                "Error": ""
            }"#,
        )
        .expect("question response");
        let question = response.question.expect("question details");
        assert_eq!(response.state, "*oauth-islocal,teamdrive,,");
        assert_eq!(question.name.as_deref(), Some("config_is_local"));
        assert_eq!(question.default_value.as_deref(), Some("true"));
        assert_eq!(
            question.examples,
            vec!["true".to_owned(), "false".to_owned()]
        );
    }
}
