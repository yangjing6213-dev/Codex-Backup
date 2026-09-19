use rehome_desktop_lib::core::app_config::{
    default_config, serialize_config_for_test, validate_config, Appearance, CloudConfig,
};
use std::path::PathBuf;

#[test]
fn default_config_is_local_only_and_has_safe_schedule() {
    let config = default_config();
    assert_eq!(config.frequency_minutes, 15);
    assert!(!config.cloud.enabled);
    assert_eq!(config.appearance, Appearance::System);
}

#[test]
fn config_serialization_never_contains_recovery_password() {
    let config = default_config();
    let text = serialize_config_for_test(&config).unwrap();
    assert!(!text.contains("recovery-password"));
    assert!(!text.contains("auth.json"));
}

#[test]
fn invalid_frequency_is_rejected_before_persistence() {
    let mut config = default_config();
    config.frequency_minutes = 0;
    assert!(validate_config(&config).is_err());
}

#[test]
fn cloud_config_has_no_effect_when_disabled() {
    let config = CloudConfig {
        enabled: false,
        remote_name: Some("onedrive".to_owned()),
        remote_path: Some("ENHE".to_owned()),
        repository_path: Some("rclone:onedrive:ENHE".to_owned()),
        config_file: Some(PathBuf::from("rclone.conf")),
    };
    assert!(!config.enabled);
}

#[test]
fn old_config_loads_with_automatic_scan_and_uninitialized_selection() {
    let mut old = serde_json::to_value(default_config()).unwrap();
    for field in [
        "automatic_project_scan",
        "project_scan_roots",
        "project_selection_initialized",
    ] {
        old.as_object_mut().unwrap().remove(field);
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.json");
    std::fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
    let loaded = rehome_desktop_lib::core::app_config::load_config(&path).unwrap();
    let json = serde_json::to_value(loaded).unwrap();
    assert_eq!(json["automatic_project_scan"], true);
    assert_eq!(json["project_scan_roots"], serde_json::json!([]));
    assert_eq!(json["project_selection_initialized"], false);
}

#[test]
fn scan_preferences_survive_config_save_and_reload() {
    use rehome_desktop_lib::core::app_config::{load_config, save_config, AppConfig};

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.json");
    let mut json = serde_json::to_value(default_config()).unwrap();
    json["automatic_project_scan"] = serde_json::json!(false);
    json["project_scan_roots"] = serde_json::json!([directory.path().join("projects")]);
    json["project_selection_initialized"] = serde_json::json!(true);
    let config: AppConfig = serde_json::from_value(json.clone()).unwrap();
    save_config(&path, &config).unwrap();
    assert_eq!(
        serde_json::to_value(load_config(&path).unwrap()).unwrap(),
        json
    );
}
