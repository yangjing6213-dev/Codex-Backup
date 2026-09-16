use enhe_codex_backup_test_api::core::restic::{
    build_backup_args, file_fingerprint, is_excluded_backup_path, payload_relative_path,
};
use std::{fs, path::Path};
use tempfile::tempdir;

// The alias keeps the intended public API explicit while the crate name remains
// compatible with the imported upstream integration tests.
extern crate rehome_desktop_lib as enhe_codex_backup_test_api;

#[test]
fn complete_backup_excludes_credentials_but_keeps_git_and_safe_examples() {
    assert!(is_excluded_backup_path(Path::new("auth.json")));
    assert!(is_excluded_backup_path(Path::new(".env")));
    assert!(is_excluded_backup_path(Path::new("rclone.conf")));
    assert!(is_excluded_backup_path(Path::new("credentials.json")));
    assert!(is_excluded_backup_path(Path::new(".codex/cookies.sqlite")));
    assert!(!is_excluded_backup_path(Path::new(".env.example")));
    assert!(!is_excluded_backup_path(Path::new(
        ".git/objects/pack/data"
    )));
    assert!(!is_excluded_backup_path(Path::new("src/main.rs")));
}

#[test]
fn payload_path_is_relative_and_rejects_escape() {
    let root = Path::new(r"C:\Projects\demo");
    assert_eq!(
        payload_relative_path("project-1", root, &root.join(r"src\main.rs")).unwrap(),
        Path::new("projects/project-1/src/main.rs")
    );
    assert!(payload_relative_path("project-1", root, Path::new(r"C:\Other\secret.txt")).is_err());
}

#[test]
fn restic_arguments_do_not_contain_password_material() {
    let args = build_backup_args(
        Path::new(r"C:\Backups\repo"),
        Path::new(r"C:\Temp\password-file"),
        Path::new(r"C:\Temp\payload"),
        &["enhe:local".to_owned(), "logical:123".to_owned()],
    );
    let rendered: Vec<String> = args
        .iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect();
    assert!(rendered.iter().any(|arg| arg == "--password-file"));
    assert!(rendered.iter().any(|arg| arg == r"C:\Temp\password-file"));
    assert!(!rendered.iter().any(|arg| arg == "recovery-password"));
}

#[test]
fn fingerprint_changes_when_selected_content_changes() {
    let root = tempdir().unwrap();
    let file = root.path().join("README.md");
    fs::write(&file, "before").unwrap();
    let before = file_fingerprint(root.path()).unwrap();
    fs::write(&file, "after").unwrap();
    let after = file_fingerprint(root.path()).unwrap();
    assert_ne!(before, after);
}
