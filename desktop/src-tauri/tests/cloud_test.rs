use rehome_desktop_lib::core::cloud::{
    build_rclone_config_continue_args, build_rclone_config_create_args, build_restic_copy_args,
    cloud_calls_allowed, remote_repository, CloudConfig, CloudState,
};
use std::{fs, path::Path};
use tempfile::tempdir;

#[test]
fn cloud_off_blocks_all_remote_calls() {
    assert!(!cloud_calls_allowed(CloudState::Off));
    assert!(cloud_calls_allowed(CloudState::Configuring));
    assert!(cloud_calls_allowed(CloudState::On));
}

#[test]
fn rclone_config_args_use_non_interactive_state_protocol() {
    let args = build_rclone_config_create_args(Path::new(r"C:\App\rclone.conf"), "enhe-onedrive");
    let text = args
        .iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(text.contains("config create"));
    assert!(text.contains("onedrive"));
    assert!(text.contains("--non-interactive"));
    assert!(!text.contains("config_is_local=true"));
    assert!(!text.contains("token"));
}

#[test]
fn rclone_config_continue_args_carry_only_the_current_answer() {
    let args = build_rclone_config_continue_args(
        Path::new(r"C:\App\rclone.conf"),
        "enhe-onedrive",
        "state-token",
        "OneDrive",
    );
    let text = args
        .iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(text.contains("--continue"));
    assert!(text.contains("--state state-token"));
    assert!(text.contains("--result OneDrive"));
    assert!(text.contains("--non-interactive"));
    assert!(!text.contains("refresh_token"));
}

#[test]
fn restic_copy_args_use_independent_source_and_target_password_files() {
    let args = build_restic_copy_args(
        Path::new(r"C:\Backups\local"),
        Path::new(r"rclone:enhe-onedrive:ENHE/restic"),
        Path::new(r"C:\Temp\source-password"),
        Path::new(r"C:\Temp\target-password"),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        Path::new(r"C:\Tools\rclone.exe"),
    );
    let text = args
        .iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(text.contains("copy"));
    assert!(text.contains("--from-repo"));
    assert!(text.contains("--from-password-file"));
    assert!(text.contains("--repo"));
    assert!(text.contains("rclone.program"));
    assert!(!text.contains("recovery-password"));
}

#[test]
fn cloud_repository_uses_the_rclone_restic_backend() {
    let root = tempdir().unwrap();
    let config_file = root.path().join("rclone.conf");
    fs::write(&config_file, "[enhe-onedrive]\ntype = onedrive\n").unwrap();
    let config = CloudConfig {
        enabled: true,
        remote_name: Some("enhe-onedrive".to_owned()),
        remote_path: Some("ENHE/Codex Backup".to_owned()),
        repository_path: Some("restic".to_owned()),
        config_file: Some(config_file),
    };
    assert_eq!(
        remote_repository(&config).unwrap(),
        "rclone:enhe-onedrive:ENHE/Codex Backup/restic"
    );
}

#[test]
fn cloud_paths_reject_remote_traversal() {
    let root = tempdir().unwrap();
    let config_file = root.path().join("rclone.conf");
    fs::write(&config_file, "[enhe-onedrive]\ntype = onedrive\n").unwrap();
    let config = CloudConfig {
        enabled: true,
        remote_name: Some("enhe-onedrive".to_owned()),
        remote_path: Some("ENHE/../outside".to_owned()),
        repository_path: Some("restic".to_owned()),
        config_file: Some(config_file),
    };
    assert!(remote_repository(&config).is_err());
}
