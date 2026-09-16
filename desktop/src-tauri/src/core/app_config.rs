pub use crate::core::cloud::CloudConfig;
use crate::core::error::{ErrorCode, RehomeError};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub const CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    Light,
    Dark,
    System,
}

impl Default for Appearance {
    fn default() -> Self {
        Self::System
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetentionPolicy {
    pub high_frequency_hours: u32,
    pub daily_days: u32,
    pub weekly_weeks: u32,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            high_frequency_hours: 48,
            daily_days: 30,
            weekly_weeks: 12,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    pub config_version: u32,
    pub codex_home: Option<PathBuf>,
    pub local_repository: Option<PathBuf>,
    pub selected_project_paths: Vec<PathBuf>,
    pub frequency_minutes: u32,
    pub retention: RetentionPolicy,
    pub cloud: CloudConfig,
    pub locale: String,
    pub appearance: Appearance,
    pub automatic_backup_enabled: bool,
}

pub fn default_config() -> AppConfig {
    AppConfig {
        config_version: CONFIG_VERSION,
        codex_home: None,
        local_repository: None,
        selected_project_paths: Vec::new(),
        frequency_minutes: 15,
        retention: RetentionPolicy::default(),
        cloud: CloudConfig::default(),
        locale: "zh-CN".to_owned(),
        appearance: Appearance::System,
        automatic_backup_enabled: false,
    }
}

pub fn validate_config(config: &AppConfig) -> Result<(), RehomeError> {
    if config.config_version != CONFIG_VERSION {
        return Err(RehomeError::new(
            ErrorCode::ConfigInvalid,
            "the ENHE configuration schema is not supported",
        ));
    }
    if !(1..=7 * 24 * 60).contains(&config.frequency_minutes) {
        return Err(RehomeError::new(
            ErrorCode::ConfigInvalid,
            "backup frequency must be between 1 minute and 7 days",
        ));
    }
    if config.retention.high_frequency_hours == 0
        || config.retention.daily_days == 0
        || config.retention.weekly_weeks == 0
    {
        return Err(RehomeError::new(
            ErrorCode::ConfigInvalid,
            "each retention period must keep at least one version",
        ));
    }
    if config.cloud.enabled {
        let cloud_is_configured = [
            config.cloud.remote_name.as_deref(),
            config.cloud.remote_path.as_deref(),
            config.cloud.repository_path.as_deref(),
        ]
        .into_iter()
        .all(|value| value.is_some_and(|value| !value.trim().is_empty()))
            && config
                .cloud
                .config_file
                .as_deref()
                .is_some_and(Path::is_file);
        if !cloud_is_configured {
            return Err(RehomeError::new(
                ErrorCode::CloudConfiguration,
                "cloud backup is enabled without a complete OneDrive configuration",
            ));
        }
    }
    Ok(())
}

pub fn serialize_config_for_test(config: &AppConfig) -> Result<String, RehomeError> {
    serde_json::to_string_pretty(config)
        .map_err(|error| RehomeError::new(ErrorCode::ConfigInvalid, error.to_string()))
}

pub fn load_config(path: &Path) -> Result<AppConfig, RehomeError> {
    if !path.is_file() {
        return Ok(default_config());
    }
    let bytes = fs::read(path).map_err(|error| config_io("read configuration", error))?;
    let config: AppConfig = serde_json::from_slice(&bytes).map_err(|error| {
        RehomeError::new(
            ErrorCode::ConfigInvalid,
            format!("configuration file is invalid: {error}"),
        )
    })?;
    validate_config(&config)?;
    Ok(config)
}

pub fn save_config(path: &Path, config: &AppConfig) -> Result<(), RehomeError> {
    validate_config(config)?;
    let parent = path
        .parent()
        .ok_or_else(|| RehomeError::new(ErrorCode::ConfigInvalid, "configuration has no parent"))?;
    fs::create_dir_all(parent)
        .map_err(|error| config_io("create configuration directory", error))?;
    let bytes = serialize_config_for_test(config)?.into_bytes();
    let temporary = path.with_extension("json.partial");
    fs::write(&temporary, bytes).map_err(|error| config_io("write configuration", error))?;
    fs::rename(&temporary, path).map_err(|error| config_io("commit configuration", error))?;
    Ok(())
}

pub fn default_config_path() -> Result<PathBuf, RehomeError> {
    let base = std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("LOCALAPPDATA"))
        .or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or_else(|| {
            RehomeError::new(
                ErrorCode::ConfigInvalid,
                "user data directory is unavailable",
            )
        })?;
    Ok(PathBuf::from(base)
        .join("ENHE")
        .join("Codex Backup")
        .join("config.json"))
}

pub fn default_secret_path() -> Result<PathBuf, RehomeError> {
    Ok(default_config_path()?
        .parent()
        .ok_or_else(|| {
            RehomeError::new(ErrorCode::ConfigInvalid, "secret directory is unavailable")
        })?
        .join("recovery-password.dpapi"))
}

pub fn protect_secret(path: &Path, secret: &str) -> Result<(), RehomeError> {
    if secret.is_empty() {
        return Err(RehomeError::new(
            ErrorCode::BackupPasswordRequired,
            "recovery password cannot be empty",
        ));
    }
    let protected = dpapi_protect(secret.as_bytes())?;
    let parent = path
        .parent()
        .ok_or_else(|| RehomeError::new(ErrorCode::ConfigInvalid, "secret has no parent"))?;
    fs::create_dir_all(parent).map_err(|error| config_io("create secret directory", error))?;
    let temporary = path.with_extension("dpapi.partial");
    fs::write(&temporary, protected).map_err(|error| config_io("write protected secret", error))?;
    fs::rename(&temporary, path).map_err(|error| config_io("commit protected secret", error))?;
    Ok(())
}

pub fn unprotect_secret(path: &Path) -> Result<String, RehomeError> {
    let protected = fs::read(path).map_err(|error| config_io("read protected secret", error))?;
    let secret = dpapi_unprotect(&protected)?;
    String::from_utf8(secret).map_err(|_| {
        RehomeError::new(
            ErrorCode::ConfigInvalid,
            "protected recovery password is not valid text",
        )
    })
}

fn config_io(stage: &str, error: impl std::fmt::Display) -> RehomeError {
    RehomeError::new(ErrorCode::ConfigInvalid, format!("{stage} failed: {error}"))
}

#[cfg(windows)]
fn dpapi_protect(bytes: &[u8]) -> Result<Vec<u8>, RehomeError> {
    use std::{ffi::c_void, ptr};
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB},
    };

    let mut input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };
    let success = unsafe {
        CryptProtectData(
            &mut input,
            ptr::null(),
            ptr::null(),
            ptr::null_mut(),
            ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if success == 0 {
        return Err(config_io(
            "protect recovery password",
            "Windows DPAPI failed",
        ));
    }
    let result =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        let _ = LocalFree(output.pbData as *mut c_void);
    }
    Ok(result)
}

#[cfg(not(windows))]
fn dpapi_protect(_bytes: &[u8]) -> Result<Vec<u8>, RehomeError> {
    Err(RehomeError::new(
        ErrorCode::ConfigInvalid,
        "user-scoped DPAPI is only available on Windows",
    ))
}

#[cfg(windows)]
fn dpapi_unprotect(bytes: &[u8]) -> Result<Vec<u8>, RehomeError> {
    use std::{ffi::c_void, ptr};
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
    };

    let mut input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };
    let success = unsafe {
        CryptUnprotectData(
            &mut input,
            ptr::null_mut(),
            ptr::null(),
            ptr::null_mut(),
            ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if success == 0 {
        return Err(config_io(
            "unprotect recovery password",
            "Windows DPAPI failed",
        ));
    }
    let result =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe {
        let _ = LocalFree(output.pbData as *mut c_void);
    }
    Ok(result)
}

#[cfg(not(windows))]
fn dpapi_unprotect(_bytes: &[u8]) -> Result<Vec<u8>, RehomeError> {
    Err(RehomeError::new(
        ErrorCode::ConfigInvalid,
        "user-scoped DPAPI is only available on Windows",
    ))
}
