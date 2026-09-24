use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    CodexNotFound,
    BackupEngineUnavailable,
    BackupRepositoryInvalid,
    BackupPasswordRequired,
    BackupFailed,
    UnsafePath,
    ConfigInvalid,
    AdminScanUnavailable,
    SchedulerUnavailable,
    CloudDisabled,
    CloudConfiguration,
    CloudUnavailable,
    PackageInvalid,
    ChecksumMismatch,
    UnsupportedSchema,
    CodexRunning,
    CodexAppServerUnavailable,
    CodexAuthenticationRequired,
    CodexVerificationFailed,
    CodexCleanupUnconfirmed,
    MigrationJobNotFound,
    DiskSpaceInsufficient,
    ProjectConflict,
    RestoreFailed,
    RollbackFailed,
    RegistrationIncomplete,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RehomeError {
    pub code: ErrorCode,
    pub message: String,
}

impl RehomeError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for RehomeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for RehomeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_unconfirmed_error_has_stable_serialization() {
        let value = serde_json::json!({"code": "codex_cleanup_unconfirmed", "message": "synthetic cleanup failure"});
        let error: RehomeError = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(error).unwrap(), value);
    }
}
