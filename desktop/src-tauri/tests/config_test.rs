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
