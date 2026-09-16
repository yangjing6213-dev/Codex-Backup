use crate::core::error::{ErrorCode, RehomeError};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
};

pub const TASK_NAME: &str = "ENHE Codex Backup - Current User";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct SchedulerStatus {
    pub enabled: bool,
    pub task_name: String,
    pub worker_path: Option<PathBuf>,
    pub interval_minutes: Option<u32>,
    pub next_run_time: Option<String>,
    pub last_result: Option<String>,
    pub message: Option<String>,
}

pub fn build_task_create_args(
    worker: &Path,
    interval_minutes: u32,
) -> Result<Vec<OsString>, RehomeError> {
    if !(1..=7 * 24 * 60).contains(&interval_minutes) {
        return Err(RehomeError::new(
            ErrorCode::ConfigInvalid,
            "scheduled backup frequency must be between 1 minute and 7 days",
        ));
    }
    let task_command = format!("\"{}\" --run-due", worker.display());
    Ok(vec![
        OsString::from("/Create"),
        OsString::from("/TN"),
        OsString::from(TASK_NAME),
        OsString::from("/TR"),
        OsString::from(task_command),
        OsString::from("/SC"),
        OsString::from("MINUTE"),
        OsString::from("/MO"),
        OsString::from(interval_minutes.to_string()),
        OsString::from("/F"),
    ])
}

pub fn build_task_query_args() -> Vec<OsString> {
    vec![
        OsString::from("/Query"),
        OsString::from("/TN"),
        OsString::from(TASK_NAME),
        OsString::from("/FO"),
        OsString::from("LIST"),
    ]
}

pub fn build_task_delete_args() -> Vec<OsString> {
    vec![
        OsString::from("/Delete"),
        OsString::from("/TN"),
        OsString::from(TASK_NAME),
        OsString::from("/F"),
    ]
}

pub fn register_current_user_task(
    worker: &Path,
    interval_minutes: u32,
) -> Result<SchedulerStatus, RehomeError> {
    let args = build_task_create_args(worker, interval_minutes)?;
    let output = Command::new(schtasks_path())
        .args(args)
        .output()
        .map_err(|error| scheduler_unavailable(error))?;
    if !output.status.success() {
        return Err(RehomeError::new(
            ErrorCode::SchedulerUnavailable,
            "Windows Task Scheduler rejected the ENHE task",
        ));
    }
    Ok(SchedulerStatus {
        enabled: true,
        task_name: TASK_NAME.to_owned(),
        worker_path: Some(worker.to_path_buf()),
        interval_minutes: Some(interval_minutes),
        next_run_time: None,
        last_result: None,
        message: None,
    })
}

pub fn query_current_user_task() -> Result<SchedulerStatus, RehomeError> {
    let output = Command::new(schtasks_path())
        .args(build_task_query_args())
        .output()
        .map_err(|error| scheduler_unavailable(error))?;
    if !output.status.success() {
        return Ok(SchedulerStatus {
            enabled: false,
            task_name: TASK_NAME.to_owned(),
            worker_path: None,
            interval_minutes: None,
            next_run_time: None,
            last_result: None,
            message: Some("the ENHE current-user task is not registered".to_owned()),
        });
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(parse_task_status(&text))
}

pub fn unregister_current_user_task() -> Result<(), RehomeError> {
    let output = Command::new(schtasks_path())
        .args(build_task_delete_args())
        .output()
        .map_err(|error| scheduler_unavailable(error))?;
    if !output.status.success()
        && !String::from_utf8_lossy(&output.stderr).contains("does not exist")
    {
        return Err(RehomeError::new(
            ErrorCode::SchedulerUnavailable,
            "Windows Task Scheduler could not remove the ENHE task",
        ));
    }
    Ok(())
}

fn parse_task_status(text: &str) -> SchedulerStatus {
    let mut status = SchedulerStatus {
        enabled: true,
        task_name: TASK_NAME.to_owned(),
        worker_path: None,
        interval_minutes: None,
        next_run_time: None,
        last_result: None,
        message: None,
    };
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "Task To Run" => {
                let worker = value
                    .trim_matches('"')
                    .split(" --run-due")
                    .next()
                    .unwrap_or(value);
                status.worker_path = Some(PathBuf::from(worker.trim_matches('"')));
            }
            "Next Run Time" => status.next_run_time = Some(value.to_owned()),
            "Last Result" => status.last_result = Some(value.to_owned()),
            _ => {}
        }
    }
    status
}

fn schtasks_path() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(r"C:\Windows\System32\schtasks.exe")
    } else {
        PathBuf::from("schtasks")
    }
}

fn scheduler_unavailable(error: std::io::Error) -> RehomeError {
    RehomeError::new(
        ErrorCode::SchedulerUnavailable,
        format!("Task Scheduler unavailable: {error}"),
    )
}
