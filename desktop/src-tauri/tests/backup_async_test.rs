#![cfg(windows)]

use rehome_desktop_lib::{
    backup_workflow::{
        list_local_backups, restore_local_backup, run_due_backup, run_local_backup,
        LocalBackupCommandRequest, LocalBackupListRequest, LocalRestoreCommandRequest,
    },
    core::{
        app_config,
        error::{ErrorCode, RehomeError},
    },
};
use std::{
    env,
    ffi::OsString,
    fs,
    future::{poll_fn, Future},
    path::PathBuf,
    sync::{Mutex, MutexGuard},
    task::Poll,
    time::{Duration, Instant},
};
use tempfile::{tempdir, TempDir};

// Environment overrides are confined to this integration-test process.
static ENVIRONMENT: Mutex<()> = Mutex::new(());

struct Fixture {
    root: TempDir,
    previous: Vec<(&'static str, Option<OsString>)>,
    _environment: MutexGuard<'static, ()>,
}

impl Fixture {
    fn new() -> Self {
        let environment = ENVIRONMENT
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let root = tempdir().unwrap();
        let mut previous = Vec::new();
        for (name, value) in [
            ("APPDATA", root.path().join("app-data")),
            ("RESTIC_CACHE_DIR", root.path().join("restic-cache")),
            ("ENHE_RESTIC_PATH", root.path().join("missing-restic.exe")),
        ] {
            previous.push((name, env::var_os(name)));
            env::set_var(name, value);
        }
        fs::create_dir(root.path().join("codex")).unwrap();
        fs::create_dir(root.path().join("project")).unwrap();
        fs::write(root.path().join("project/README.md"), "synthetic project").unwrap();
        assert!(!app_config::default_config().cloud.enabled);
        Self {
            root,
            previous,
            _environment: environment,
        }
    }

    fn backup(&self) -> LocalBackupCommandRequest {
        LocalBackupCommandRequest {
            codex_home: self.root.path().join("codex"),
            project_paths: vec![self.root.path().join("project")],
            repository: self.root.path().join("repository"),
            recovery_password: "synthetic-test-password".into(),
            remember_password: false,
            source_device_id: None,
        }
    }

    fn list(&self) -> LocalBackupListRequest {
        LocalBackupListRequest {
            repository: self.root.path().join("repository"),
            recovery_password: "synthetic-test-password".into(),
        }
    }

    fn restore(&self) -> LocalRestoreCommandRequest {
        LocalRestoreCommandRequest {
            snapshot_id: "12345678".into(),
            repository: self.root.path().join("repository"),
            recovery_password: "synthetic-test-password".into(),
            target: self.root.path().join("restored"),
        }
    }

    fn lock_path(&self) -> PathBuf {
        app_config::default_config_path()
            .unwrap()
            .with_file_name("backup-worker.lock")
    }

    fn gated_engine(&self) {
        // A real child process waits for the command future to yield. The timeout
        // makes an inline/blocking regression fail instead of hanging the suite.
        fs::write(
            self.root.path().join("wait.ps1"),
            r#"Set-Content -LiteralPath (Join-Path $PSScriptRoot 'entered') -Value 'engine started'
$deadline = [DateTime]::UtcNow.AddSeconds(10)
while (-not (Test-Path -LiteralPath (Join-Path $PSScriptRoot 'release'))) {
    if ([DateTime]::UtcNow -gt $deadline) { exit 91 }
    Start-Sleep -Milliseconds 10
}
exit 23
"#,
        )
        .unwrap();
        let executable = self.root.path().join("restic.cmd");
        let powershell = env::var_os("WINDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
            .join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
        fs::write(
            &executable,
            format!(
                "@echo off\r\n\"{}\" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"%~dp0wait.ps1\"\r\nexit /b %errorlevel%\r\n",
                powershell.display()
            ),
        )
        .unwrap();
        env::set_var("ENHE_RESTIC_PATH", executable);
    }

    fn assert_yields<T>(
        &self,
        operation: impl Future<Output = Result<T, RehomeError>>,
        mutating: bool,
        expected_code: ErrorCode,
        expected_message: &str,
    ) {
        let mut operation = Box::pin(operation);
        let release = self.root.path().join("release");
        let entered = self.root.path().join("entered");
        let mut lock_during_work = None;
        let result = tauri::async_runtime::block_on(poll_fn(|context| {
            match operation.as_mut().poll(context) {
                Poll::Pending => {
                    if lock_during_work.is_none() {
                        let deadline = Instant::now() + Duration::from_secs(5);
                        while !entered.exists() && Instant::now() < deadline {
                            std::thread::sleep(Duration::from_millis(10));
                        }
                        lock_during_work = Some(entered.exists() && self.lock_path().exists());
                    }
                    fs::write(&release, "runtime remained responsive").unwrap();
                    Poll::Pending
                }
                Poll::Ready(result) => Poll::Ready(result),
            }
        }));
        assert!(
            release.exists(),
            "command blocked its executor until restic exited"
        );
        let error = result.err().expect("synthetic engine must fail");
        assert_eq!(error.code, expected_code);
        assert_eq!(error.message, expected_message);
        assert!(entered.exists(), "synthetic engine did not start");
        assert_eq!(
            lock_during_work,
            Some(mutating),
            "mutation lock must cover the running engine"
        );
        assert!(
            !self.lock_path().exists(),
            "failed operation leaked its lock"
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for (name, value) in &self.previous {
            match value {
                Some(value) => env::set_var(name, value),
                None => env::remove_var(name),
            }
        }
    }
}

#[test]
fn backup_yields_while_restic_is_blocked() {
    let fixture = Fixture::new();
    fixture.gated_engine();
    fixture.assert_yields(
        run_local_backup(fixture.backup()),
        true,
        ErrorCode::BackupFailed,
        "backup engine failed (exit 23)",
    );
}

#[test]
fn list_yields_while_restic_is_blocked() {
    let fixture = Fixture::new();
    fixture.gated_engine();
    fixture.assert_yields(
        list_local_backups(fixture.list()),
        false,
        ErrorCode::BackupFailed,
        "backup engine failed (exit 23)",
    );
}

#[test]
fn restore_yields_while_restic_is_blocked() {
    let fixture = Fixture::new();
    fixture.gated_engine();
    fixture.assert_yields(
        restore_local_backup(fixture.restore()),
        true,
        ErrorCode::RestoreFailed,
        "restore engine failed (exit 23)",
    );
}

#[test]
fn backup_restore_and_sync_worker_share_the_mutation_lock() {
    let fixture = Fixture::new();
    let lock = fixture.lock_path();
    fs::create_dir_all(lock.parent().unwrap()).unwrap();
    fs::write(&lock, "another synthetic worker").unwrap();

    assert_eq!(
        tauri::async_runtime::block_on(run_local_backup(fixture.backup()))
            .unwrap_err()
            .code,
        ErrorCode::SchedulerUnavailable
    );
    assert_eq!(
        tauri::async_runtime::block_on(restore_local_backup(fixture.restore()))
            .unwrap_err()
            .code,
        ErrorCode::SchedulerUnavailable
    );
    assert_eq!(
        run_due_backup().unwrap_err().code,
        ErrorCode::SchedulerUnavailable
    );
    assert_eq!(
        fs::read_to_string(&lock).unwrap(),
        "another synthetic worker"
    );
    // Read-only listing is allowed while a mutator holds the application lock.
    assert_eq!(
        tauri::async_runtime::block_on(list_local_backups(fixture.list()))
            .unwrap_err()
            .code,
        ErrorCode::BackupEngineUnavailable
    );
}

#[test]
fn errors_are_preserved_and_release_the_mutation_lock() {
    let fixture = Fixture::new();
    let mut backup = fixture.backup();
    backup.recovery_password.clear();
    assert_eq!(
        tauri::async_runtime::block_on(run_local_backup(backup))
            .unwrap_err()
            .code,
        ErrorCode::BackupPasswordRequired
    );
    assert!(!fixture.lock_path().exists());

    let mut restore = fixture.restore();
    restore.recovery_password.clear();
    assert_eq!(
        tauri::async_runtime::block_on(restore_local_backup(restore))
            .unwrap_err()
            .code,
        ErrorCode::BackupPasswordRequired
    );
    assert!(!fixture.lock_path().exists());
    assert_eq!(run_due_backup().unwrap_err().code, ErrorCode::ConfigInvalid);
    assert!(!fixture.lock_path().exists());

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_enhe-codex-backup"))
        .arg("--run-due")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("scheduled backup is disabled"));
    assert!(!fixture.lock_path().exists());
}

#[test]
fn data_only_request_reaches_the_backup_engine() {
    let fixture = Fixture::new();
    let mut request = fixture.backup();
    request.project_paths.clear();
    let error = tauri::async_runtime::block_on(run_local_backup(request)).unwrap_err();
    assert_eq!(error.code, ErrorCode::BackupEngineUnavailable);
}

#[test]
#[ignore = "requires bundled restic.exe; uses only synthetic local data"]
fn bundled_restic_round_trip_through_async_commands() {
    let fixture = Fixture::new();
    let executable = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/restic.exe");
    assert!(executable.is_file(), "bundled restic.exe is required");
    env::set_var("ENHE_RESTIC_PATH", executable);

    tauri::async_runtime::block_on(async {
        let snapshot = run_local_backup(fixture.backup()).await.unwrap();
        assert!(snapshot.complete);
        assert!(!fixture.lock_path().exists());
        let snapshots = list_local_backups(fixture.list()).await.unwrap();
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].restic_snapshot_id, snapshot.restic_snapshot_id);
        assert_eq!(snapshots[0].file_count, snapshot.manifest.file_count);

        let mut request = fixture.restore();
        request.snapshot_id = snapshot.restic_snapshot_id;
        let report = restore_local_backup(request).await.unwrap();
        assert!(report.complete);
        let project = fs::read_dir(report.restored_root.join("projects"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(
            fs::read_to_string(project.join("README.md")).unwrap(),
            "synthetic project"
        );
        assert!(!fixture.lock_path().exists());

        // Missing selected content must still produce a partial snapshot.
        let mut request = fixture.backup();
        request
            .project_paths
            .push(fixture.root.path().join("missing-project"));
        let partial = run_local_backup(request).await.unwrap();
        assert!(!partial.complete);
        assert!(!partial.manifest.missing.is_empty());
        assert!(!fixture.lock_path().exists());
        let snapshots = list_local_backups(fixture.list()).await.unwrap();
        let listed = snapshots
            .iter()
            .find(|item| item.restic_snapshot_id == partial.restic_snapshot_id)
            .unwrap();
        assert_eq!(listed.file_count, partial.manifest.file_count);

        // Explicitly empty project selections back up only Codex data.
        fs::write(
            fixture.root.path().join("codex/history.jsonl"),
            r#"{"message":"synthetic conversation"}"#,
        )
        .unwrap();
        let mut data_request = fixture.backup();
        data_request.project_paths.clear();
        let data = run_local_backup(data_request).await.unwrap();
        assert!(data.complete);
        assert!(data.manifest.project_paths.is_empty());
        let mut restore = fixture.restore();
        restore.snapshot_id = data.restic_snapshot_id;
        let restored = restore_local_backup(restore).await.unwrap();
        assert_eq!(
            fs::read_to_string(restored.restored_root.join("codex/history.jsonl")).unwrap(),
            r#"{"message":"synthetic conversation"}"#
        );
    });
}
