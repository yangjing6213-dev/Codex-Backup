use rehome_desktop_lib::core::scheduler::{build_task_create_args, TASK_NAME};
use std::path::Path;

#[test]
fn task_arguments_are_current_user_and_do_not_request_a_password() {
    let args = build_task_create_args(
        Path::new(r"C:\Program Files\ENHE Codex Backup\worker.exe"),
        15,
    )
    .unwrap();
    let text = args
        .iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(text.contains(TASK_NAME));
    assert!(text.contains("/SC MINUTE"));
    assert!(text.contains("/MO 15"));
    assert!(!text.contains("/RP"));
    assert!(text.contains("--run-due"));
}

#[test]
fn invalid_scheduler_interval_is_rejected() {
    assert!(build_task_create_args(Path::new("worker.exe"), 0).is_err());
}
