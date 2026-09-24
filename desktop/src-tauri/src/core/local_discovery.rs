use crate::core::app_config::AppConfig;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use walkdir::WalkDir;

const MAX_SCAN_DEPTH: usize = 8;
const MAX_ENTRIES_PER_ROOT: usize = 50_000;
const MAX_SCAN_DURATION: Duration = Duration::from_secs(20);
const MAX_REPORTED_ITEMS: usize = 100;
const MAX_VISIBLE_WARNINGS: usize = 3;
const PROJECT_FILE_MARKERS: &[&str] = &[
    "package.json",
    "Cargo.toml",
    "pyproject.toml",
    "go.mod",
    "pom.xml",
    "composer.json",
    "CMakeLists.txt",
];
const SKIPPED_DIRECTORY_NAMES: &[&str] = &[
    "$recycle.bin",
    "appdata",
    "build",
    "dist",
    "node_modules",
    "onedrive",
    "packages",
    "program files",
    "program files (x86)",
    "programdata",
    "recovery",
    "system volume information",
    "target",
    "temp",
    "tmp",
    ".cache",
    ".venv",
    "venv",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalProjectCandidate {
    pub path: PathBuf,
    pub name: String,
    pub markers: Vec<String>,
    pub file_count: u64,
    pub file_count_complete: bool,
    pub skipped_entries: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalDiscoveryResult {
    pub candidates: Vec<LocalProjectCandidate>,
    pub codex_homes: Vec<PathBuf>,
    pub conversation_count: u64,
    pub scanned_roots: Vec<PathBuf>,
    pub skipped_roots: Vec<PathBuf>,
    pub warnings: Vec<String>,
    pub permission_denied_count: u32,
    pub other_warning_count: u32,
    pub scan_limit_reached: bool,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalScanRequest {
    pub roots: Vec<PathBuf>,
    pub excluded_roots: Vec<PathBuf>,
}

impl LocalScanRequest {
    pub fn from_config(config: &AppConfig, roots: Option<Vec<PathBuf>>) -> Self {
        Self {
            roots: roots.unwrap_or_else(|| config.project_scan_roots.clone()),
            excluded_roots: config.local_repository.iter().cloned().collect(),
        }
    }
}

pub fn discover_local_candidates(
    request: LocalScanRequest,
    cancel: Arc<AtomicBool>,
) -> LocalDiscoveryResult {
    let collection = !request.roots.is_empty();
    let roots = if request.roots.is_empty() {
        fixed_drive_roots()
    } else {
        request.roots
    };
    discover_from_roots_with_mode(roots, &request.excluded_roots, cancel, collection)
}

/// One sequential worker batch, with metadata only. Missing or inaccessible paths retain
/// their identity and an explicitly incomplete count.
pub fn count_project_files(paths: Vec<PathBuf>) -> Vec<LocalProjectCandidate> {
    let cancel = AtomicBool::new(false);
    paths
        .into_iter()
        .map(|path| count_candidate(path, &[], &cancel))
        .collect()
}

fn count_candidate(
    path: PathBuf,
    excluded_roots: &[PathBuf],
    cancel: &AtomicBool,
) -> LocalProjectCandidate {
    let mut candidate = LocalProjectCandidate {
        name: path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned()),
        path,
        markers: Vec::new(),
        file_count: 0,
        file_count_complete: true,
        skipped_entries: 0,
    };
    if !candidate.path.is_absolute()
        || has_redirect_ancestor(&candidate.path).unwrap_or(true)
        || !candidate.path.is_dir()
    {
        candidate.file_count_complete = false;
        candidate.skipped_entries = 1;
        return candidate;
    }
    let mut entries = WalkDir::new(&candidate.path)
        .follow_links(false)
        .follow_root_links(false)
        .into_iter();
    while let Some(entry) = entries.next() {
        if cancel.load(Ordering::Relaxed) {
            // The unvisited remainder has unknown size; this records one skipped
            // traversal, not an invented number of missing files.
            candidate.skipped_entries += 1;
            break;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                candidate.skipped_entries += 1;
                continue;
            }
        };
        let metadata = match fs::symlink_metadata(entry.path()) {
            Ok(metadata) => metadata,
            Err(_) => {
                candidate.skipped_entries += 1;
                continue;
            }
        };
        if is_filesystem_redirect(&metadata)
            || (metadata.is_dir()
                && excluded_roots
                    .iter()
                    .any(|root| scan_path_identity(entry.path()).starts_with(root)))
        {
            if metadata.is_dir() {
                entries.skip_current_dir();
            }
            candidate.skipped_entries += 1;
            continue;
        }
        if metadata.is_file() {
            candidate.file_count += 1;
        } else if !metadata.is_dir() {
            candidate.skipped_entries += 1;
        }
        if entry.depth() == 1 {
            let name = entry.file_name().to_string_lossy();
            if (metadata.is_file() && is_project_file_marker(&name))
                || (metadata.is_dir() && name.eq_ignore_ascii_case(".git"))
            {
                candidate.markers.push(name.into_owned());
            }
        }
    }
    candidate.file_count_complete = candidate.skipped_entries == 0;
    candidate.markers.sort_unstable();
    candidate
}

pub(crate) fn is_filesystem_redirect(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

pub(crate) fn has_redirect_ancestor(path: &Path) -> io::Result<bool> {
    for ancestor in path.ancestors() {
        if is_filesystem_redirect(&fs::symlink_metadata(ancestor)?) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Entry point used by the explicitly elevated one-shot scanner. The result
/// is written atomically so the normal UI process never reads a partial JSON.
pub fn run_admin_local_scan(output_path: &Path) -> io::Result<()> {
    let request_path = output_path.with_extension("request.json");
    let request: LocalScanRequest = serde_json::from_slice(&fs::read(request_path)?)
        .map_err(|error| io::Error::new(ErrorKind::InvalidData, error.to_string()))?;
    let result = discover_local_candidates(request, Arc::new(AtomicBool::new(false)));
    let bytes = serde_json::to_vec(&result)
        .map_err(|error| io::Error::new(ErrorKind::InvalidData, error.to_string()))?;
    let partial_path = output_path.with_extension("json.partial");
    fs::write(&partial_path, bytes)?;
    fs::rename(partial_path, output_path)
}

#[cfg(test)]
fn discover_from_roots(
    roots: Vec<PathBuf>,
    excluded_roots: &[PathBuf],
    cancel: Arc<AtomicBool>,
) -> LocalDiscoveryResult {
    discover_from_roots_with_mode(roots, excluded_roots, cancel, false)
}

fn discover_from_roots_with_mode(
    roots: Vec<PathBuf>,
    excluded_roots: &[PathBuf],
    cancel: Arc<AtomicBool>,
    collection: bool,
) -> LocalDiscoveryResult {
    let mut result = LocalDiscoveryResult {
        candidates: Vec::new(),
        codex_homes: Vec::new(),
        conversation_count: 0,
        scanned_roots: Vec::new(),
        skipped_roots: Vec::new(),
        warnings: Vec::new(),
        permission_denied_count: 0,
        other_warning_count: 0,
        scan_limit_reached: false,
        cancelled: false,
    };
    let mut candidates = HashMap::new();
    let mut codex_home_keys = HashSet::new();
    let excluded_roots: Vec<_> = excluded_roots
        .iter()
        .map(|path| scan_path_identity(path))
        .collect();

    for root in roots {
        if cancel.load(Ordering::Relaxed) {
            result.cancelled = true;
            break;
        }
        result.scanned_roots.push(root.clone());
        scan_root(
            &root,
            collection,
            &excluded_roots,
            &cancel,
            &mut candidates,
            &mut codex_home_keys,
            &mut result,
        );
        if result.cancelled {
            break;
        }
    }

    result.candidates = candidates.into_values().collect();
    result
        .candidates
        .sort_by(|left, right| left.path.cmp(&right.path));
    result
}

fn scan_root(
    root: &Path,
    collection: bool,
    excluded_roots: &[PathBuf],
    cancel: &AtomicBool,
    candidates: &mut HashMap<String, LocalProjectCandidate>,
    codex_home_keys: &mut HashSet<String>,
    result: &mut LocalDiscoveryResult,
) {
    if is_excluded_project_path(root) {
        push_limited(&mut result.skipped_roots, root.to_path_buf());
        return;
    }
    if !root.is_absolute() || !root.is_dir() || has_redirect_ancestor(root).unwrap_or(true) {
        push_limited(&mut result.skipped_roots, root.to_path_buf());
        record_scan_warning(result, "Skipped unavailable local scan root");
        return;
    }

    let started = Instant::now();
    let mut stack = vec![(root.to_path_buf(), 0_usize)];
    let mut visited = HashMap::new();
    let mut entries_seen = 0_usize;

    while let Some((current, depth)) = stack.pop() {
        if cancel.load(Ordering::Relaxed) {
            result.cancelled = true;
            return;
        }
        if !collection
            && (entries_seen >= MAX_ENTRIES_PER_ROOT || started.elapsed() >= MAX_SCAN_DURATION)
        {
            result.scan_limit_reached = true;
            record_scan_warning(
                result,
                format!("Local project scan limit reached at {}", root.display()),
            );
            return;
        }

        let identity = fs::canonicalize(&current).unwrap_or_else(|_| current.clone());
        let identity_key = identity.to_string_lossy().to_ascii_lowercase();
        if is_excluded_project_path(&identity)
            || excluded_roots
                .iter()
                .any(|root| Path::new(&identity_key).starts_with(root))
        {
            push_limited(&mut result.skipped_roots, current);
            continue;
        }
        if visited.insert(identity_key, ()).is_some() {
            continue;
        }

        if collection && depth == 1 {
            let candidate = count_candidate(current, excluded_roots, cancel);
            candidates.insert(identity.to_string_lossy().to_ascii_lowercase(), candidate);
            result.cancelled = cancel.load(Ordering::Relaxed);
            continue;
        }

        let entries = match fs::read_dir(&current) {
            Ok(entries) => entries,
            Err(error) => {
                record_scan_error(
                    result,
                    format!(
                        "Could not read local project directory {}",
                        current.display()
                    ),
                    &error,
                );
                continue;
            }
        };

        let mut markers = Vec::new();
        let mut child_directories = Vec::new();
        for entry in entries {
            if cancel.load(Ordering::Relaxed) {
                result.cancelled = true;
                return;
            }
            entries_seen += 1;
            if !collection && entries_seen >= MAX_ENTRIES_PER_ROOT {
                break;
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    record_scan_error(
                        result,
                        format!("Could not inspect a child of {}", current.display()),
                        &error,
                    );
                    continue;
                }
            };
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(error) => {
                    record_scan_error(
                        result,
                        format!("Could not inspect {}", entry.path().display()),
                        &error,
                    );
                    continue;
                }
            };
            if entry
                .metadata()
                .map(|metadata| is_filesystem_redirect(&metadata))
                .unwrap_or(true)
            {
                push_limited(&mut result.skipped_roots, entry.path());
                record_scan_warning(result, "Skipped a filesystem redirect or unavailable entry");
                continue;
            }

            let name = entry.file_name().to_string_lossy().into_owned();
            if file_type.is_dir() {
                if name.eq_ignore_ascii_case(".git") {
                    markers.push(name);
                } else if name.eq_ignore_ascii_case(".codex") {
                    if is_codex_home(&entry.path()) {
                        let codex_home = entry.path();
                        let identity =
                            fs::canonicalize(&codex_home).unwrap_or_else(|_| codex_home.clone());
                        let identity_key = identity.to_string_lossy().to_ascii_lowercase();
                        if codex_home_keys.insert(identity_key) {
                            count_codex_conversations(&codex_home, cancel, result);
                            push_limited(&mut result.codex_homes, codex_home);
                            if cancel.load(Ordering::Relaxed) {
                                result.cancelled = true;
                                return;
                            }
                        }
                    } else {
                        push_limited(&mut result.skipped_roots, entry.path());
                    }
                } else if if collection {
                    is_excluded_project_directory(&name)
                } else {
                    should_skip_directory(&name)
                } {
                    push_limited(&mut result.skipped_roots, entry.path());
                } else if depth < MAX_SCAN_DEPTH {
                    child_directories.push(entry.path());
                }
            } else if file_type.is_file() && is_project_file_marker(&name) {
                markers.push(name);
            }
        }

        if !collection && !markers.is_empty() {
            markers.sort_unstable_by_key(|marker| marker.to_ascii_lowercase());
            markers.dedup_by_key(|marker| marker.to_ascii_lowercase());
            let key = identity.to_string_lossy().to_ascii_lowercase();
            let mut candidate = count_candidate(current, excluded_roots, cancel);
            candidate.markers = markers;
            candidates.insert(key, candidate);
            result.cancelled = cancel.load(Ordering::Relaxed);
            continue;
        }

        stack.extend(
            child_directories
                .into_iter()
                .rev()
                .map(|path| (path, depth + 1)),
        );
    }
}

fn is_project_file_marker(name: &str) -> bool {
    PROJECT_FILE_MARKERS
        .iter()
        .any(|marker| name.eq_ignore_ascii_case(marker))
        || name.to_ascii_lowercase().ends_with(".sln")
        || name.to_ascii_lowercase().ends_with(".csproj")
        || name.to_ascii_lowercase().ends_with(".fsproj")
}

fn should_skip_directory(name: &str) -> bool {
    is_excluded_project_directory(name)
        || SKIPPED_DIRECTORY_NAMES
            .iter()
            .any(|skipped| name.eq_ignore_ascii_case(skipped))
}

fn is_excluded_project_directory(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        ".codex"
            | ".cache"
            | ".tmp"
            | "marketplaces"
            | "dependencies"
            | "node_modules"
            | ".git"
            | ".venv"
            | "venv"
    ) || name
        .to_ascii_lowercase()
        .starts_with("codex-migration-simple-")
}

/// Candidate discovery only; this is not a project backup exclusion rule.
pub(crate) fn is_excluded_project_path(path: &Path) -> bool {
    path.to_string_lossy()
        .split(['/', '\\'])
        .any(is_excluded_project_directory)
}

fn scan_path_identity(path: &Path) -> PathBuf {
    PathBuf::from(
        fs::canonicalize(path)
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .to_ascii_lowercase(),
    )
}

fn is_codex_home(path: &Path) -> bool {
    ["sessions", "archived_sessions"]
        .iter()
        .any(|name| is_real_directory(&path.join(name)))
        || is_real_file(&path.join("session_index.jsonl"))
}

fn is_real_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_dir())
        .unwrap_or(false)
}

fn is_real_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
}

fn count_codex_conversations(
    codex_home: &Path,
    cancel: &AtomicBool,
    result: &mut LocalDiscoveryResult,
) {
    let mut stack = ["sessions", "archived_sessions"]
        .into_iter()
        .map(|name| codex_home.join(name))
        .filter(|path| is_real_directory(path))
        .collect::<Vec<_>>();
    let mut entries_seen = 0_usize;

    while let Some(current) = stack.pop() {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        if entries_seen >= MAX_ENTRIES_PER_ROOT {
            record_scan_warning(
                result,
                format!(
                    "Codex conversation scan limit reached at {}",
                    codex_home.display()
                ),
            );
            return;
        }
        let entries = match fs::read_dir(&current) {
            Ok(entries) => entries,
            Err(error) => {
                record_scan_error(
                    result,
                    format!(
                        "Could not read Codex conversation directory {}",
                        current.display()
                    ),
                    &error,
                );
                continue;
            }
        };
        for entry in entries {
            entries_seen += 1;
            if entries_seen >= MAX_ENTRIES_PER_ROOT {
                break;
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    record_scan_error(
                        result,
                        format!(
                            "Could not inspect a Codex conversation entry in {}",
                            current.display()
                        ),
                        &error,
                    );
                    continue;
                }
            };
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(error) => {
                    record_scan_error(
                        result,
                        format!("Could not inspect {}", entry.path().display()),
                        &error,
                    );
                    continue;
                }
            };
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                stack.push(entry.path());
            } else if file_type.is_file()
                && entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("jsonl"))
            {
                result.conversation_count += 1;
            }
            if cancel.load(Ordering::Relaxed) {
                return;
            }
        }
    }
}

fn push_limited<T>(items: &mut Vec<T>, value: T) {
    if items.len() < MAX_REPORTED_ITEMS {
        items.push(value);
    }
}

fn record_scan_warning(result: &mut LocalDiscoveryResult, message: impl Into<String>) {
    result.other_warning_count = result.other_warning_count.saturating_add(1);
    if result.warnings.len() < MAX_VISIBLE_WARNINGS {
        result.warnings.push(message.into());
    }
}

fn record_scan_error(
    result: &mut LocalDiscoveryResult,
    message: impl Into<String>,
    error: &io::Error,
) {
    if error.kind() == ErrorKind::PermissionDenied || error.raw_os_error() == Some(5) {
        result.permission_denied_count = result.permission_denied_count.saturating_add(1);
        return;
    }
    record_scan_warning(result, message);
}

#[cfg(windows)]
fn fixed_drive_roots() -> Vec<PathBuf> {
    use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;

    (b'A'..=b'Z')
        .filter_map(|letter| {
            let root = format!("{}:\\", letter as char);
            let wide = root
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let drive_type = unsafe { GetDriveTypeW(wide.as_ptr()) };
            (drive_type == 3).then(|| PathBuf::from(root))
        })
        .collect()
}

#[cfg(not(windows))]
fn fixed_drive_roots() -> Vec<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .into_iter()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
    };
    use tempfile::tempdir;

    #[test]
    fn cancelled_file_count_is_partial_even_when_no_files_were_visited() {
        let root = tempdir().unwrap();
        fs::write(root.path().join("file"), b"synthetic").unwrap();
        let candidate = count_candidate(root.path().to_path_buf(), &[], &AtomicBool::new(true));
        assert_eq!(candidate.file_count, 0);
        assert!(!candidate.file_count_complete);
        assert_eq!(candidate.skipped_entries, 1);
    }

    #[test]
    fn repository_nested_in_project_is_skipped_and_counts_are_partial() {
        let root = tempdir().unwrap();
        fs::create_dir(root.path().join("repo")).unwrap();
        fs::write(root.path().join("repo/internal"), b"synthetic").unwrap();
        fs::write(root.path().join("regular"), b"synthetic").unwrap();
        let candidate = count_candidate(
            root.path().to_path_buf(),
            &[scan_path_identity(&root.path().join("repo"))],
            &AtomicBool::new(false),
        );
        assert_eq!(candidate.file_count, 1);
        assert!(!candidate.file_count_complete);
        assert_eq!(candidate.skipped_entries, 1);
    }

    #[test]
    fn discovers_project_markers_and_skips_dependency_directories() {
        let root = tempdir().expect("temporary scan root");
        let project = root.path().join("demo");
        fs::create_dir_all(project.join(".git")).expect("git marker");
        fs::write(project.join("package.json"), b"{}").expect("package marker");
        fs::create_dir_all(project.join("node_modules").join("nested"))
            .expect("dependency directory");
        fs::write(
            project
                .join("node_modules")
                .join("nested")
                .join("Cargo.toml"),
            b"[package]",
        )
        .expect("ignored marker");

        let result = discover_from_roots(
            vec![root.path().to_path_buf()],
            &[],
            Arc::new(AtomicBool::new(false)),
        );

        assert!(!result.cancelled);
        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].path, project);
        assert_eq!(result.candidates[0].markers, vec![".git", "package.json"]);
    }

    #[test]
    fn deduplicates_nested_markers_and_honors_cancellation() {
        let root = tempdir().expect("temporary scan root");
        let project = root.path().join("demo");
        fs::create_dir_all(project.join("src")).expect("project source");
        fs::write(project.join("Cargo.toml"), b"[package]").expect("project marker");
        fs::write(project.join("src").join("Cargo.toml"), b"[package]").expect("nested marker");

        let cancel = Arc::new(AtomicBool::new(true));
        let result = discover_from_roots(vec![root.path().to_path_buf()], &[], cancel.clone());

        assert!(result.cancelled);
        assert!(result.candidates.is_empty());
        assert!(cancel.load(Ordering::Relaxed));
    }

    #[test]
    fn discovers_codex_home_and_conversation_files_without_reading_contents() {
        let root = tempdir().expect("temporary scan root");
        let codex_home = root.path().join("user").join(".codex");
        fs::create_dir_all(codex_home.join("sessions")).expect("sessions directory");
        fs::create_dir_all(codex_home.join("archived_sessions")).expect("archive directory");
        fs::write(
            codex_home.join("sessions").join("active.jsonl"),
            b"not parsed",
        )
        .expect("active conversation");
        fs::write(
            codex_home.join("archived_sessions").join("archived.jsonl"),
            b"not parsed",
        )
        .expect("archived conversation");

        let result = discover_from_roots(
            vec![root.path().to_path_buf()],
            &[],
            Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(result.codex_homes, vec![codex_home]);
        assert_eq!(result.conversation_count, 2);
    }

    #[test]
    fn aggregates_permission_errors_without_retaining_directory_details() {
        let mut result = LocalDiscoveryResult {
            candidates: Vec::new(),
            codex_homes: Vec::new(),
            conversation_count: 0,
            scanned_roots: Vec::new(),
            skipped_roots: Vec::new(),
            warnings: Vec::new(),
            permission_denied_count: 0,
            other_warning_count: 0,
            scan_limit_reached: false,
            cancelled: false,
        };

        record_scan_error(
            &mut result,
            "Could not read local project directory C:\\Windows\\system32: access denied",
            &std::io::Error::new(std::io::ErrorKind::PermissionDenied, "os error 5"),
        );
        record_scan_error(
            &mut result,
            "Could not inspect C:\\Windows\\secret: access denied",
            &std::io::Error::new(std::io::ErrorKind::PermissionDenied, "os error 5"),
        );
        record_scan_warning(&mut result, "Local scan limit reached");

        assert_eq!(result.permission_denied_count, 2);
        assert_eq!(result.other_warning_count, 1);
        assert_eq!(result.warnings, vec!["Local scan limit reached"]);
        assert!(result
            .warnings
            .iter()
            .all(|warning| !warning.contains("Windows") && !warning.contains("os error 5")));
    }

    #[test]
    fn heuristic_scan_excludes_archives_and_stops_beneath_discovered_projects() {
        let root = tempdir().unwrap();
        for relative in [
            "real",
            "real/nested",
            "archive-notes",
            "Codex-Migration-Simple-tools",
            "Codex-Migration-Simple-2026-08-01/projects/old",
            "CODEX-MIGRATION-SIMPLE-2026-08-02/projects/old",
            ".codex/.tmp/copy",
            ".tmp/copy",
            "marketplaces/plugin",
            "dependencies/copy",
        ] {
            let project = root.path().join(relative);
            fs::create_dir_all(&project).unwrap();
            fs::write(project.join("package.json"), b"{}").unwrap();
        }
        let result = discover_from_roots(
            vec![root.path().to_path_buf()],
            &[],
            Arc::new(AtomicBool::new(false)),
        );
        let names: Vec<_> = result
            .candidates
            .iter()
            .map(|p| p.path.strip_prefix(root.path()).unwrap().to_path_buf())
            .collect();
        assert_eq!(
            names,
            vec![PathBuf::from("archive-notes"), PathBuf::from("real")]
        );
    }

    #[test]
    fn explicit_roots_cannot_reenter_excluded_trees() {
        let root = tempdir().unwrap();
        for relative in [
            ".codex/plugins/project",
            ".cache/project",
            ".tmp/project",
            "marketplaces/project",
            "dependencies/project",
            "Codex-Migration-Simple-backup/projects/project",
        ] {
            let project = root.path().join(relative);
            fs::create_dir_all(&project).unwrap();
            fs::write(project.join("Cargo.toml"), b"[package]").unwrap();
            let result = discover_from_roots(vec![project], &[], Arc::new(AtomicBool::new(false)));
            assert!(
                result.candidates.is_empty(),
                "unexpected candidates under {relative}"
            );
        }
    }

    #[test]
    fn configured_roots_restrict_scan_and_rescan_replaces_candidates() {
        let root = tempdir().unwrap();
        let first = root.path().join("first");
        let second = root.path().join("second");
        for project in [&first, &second] {
            fs::create_dir_all(project.join("project")).unwrap();
            fs::write(project.join("project/package.json"), b"{}").unwrap();
        }
        let mut config = crate::core::app_config::default_config();
        config.project_scan_roots = vec![first.clone()];
        let scan = |request| discover_local_candidates(request, Arc::new(AtomicBool::new(false)));
        let initial = scan(LocalScanRequest::from_config(&config, None));
        assert_eq!(initial.scanned_roots, vec![first.clone()]);
        assert_eq!(
            initial
                .candidates
                .iter()
                .map(|p| &p.path)
                .collect::<Vec<_>>(),
            vec![&first.join("project")]
        );
        let rescan = scan(LocalScanRequest::from_config(
            &config,
            Some(vec![second.clone()]),
        ));
        assert_eq!(
            rescan
                .candidates
                .iter()
                .map(|p| &p.path)
                .collect::<Vec<_>>(),
            vec![&second.join("project")]
        );
        assert_eq!(config.project_scan_roots, vec![first]);
        assert!(LocalScanRequest::from_config(&config, Some(vec![]))
            .roots
            .is_empty());
        assert!(
            LocalScanRequest::from_config(&crate::core::app_config::default_config(), None)
                .roots
                .is_empty()
        );
    }

    #[test]
    fn invalid_nonempty_roots_never_fall_back_to_drives() {
        let root = tempdir().unwrap();
        let file = root.path().join("file");
        fs::write(&file, b"not a directory").unwrap();
        let roots = vec![
            PathBuf::new(),
            root.path().join("missing"),
            file,
            PathBuf::from("relative"),
        ];
        let mut config = crate::core::app_config::default_config();
        config.project_scan_roots = roots.clone();
        for override_roots in [None, Some(roots.clone())] {
            let result = discover_local_candidates(
                LocalScanRequest::from_config(&config, override_roots),
                Arc::new(AtomicBool::new(false)),
            );
            assert_eq!(result.scanned_roots, roots);
            assert_eq!(result.skipped_roots, roots);
            assert_eq!(result.other_warning_count, 4);
            assert!(result.candidates.is_empty());
        }
    }

    #[test]
    fn configured_repository_is_excluded_by_directory_boundary() {
        let root = tempdir().unwrap();
        let repository = root.path().join("backups");
        let real = root.path().join("backups-app");
        let copy = repository.join("projects/copy");
        for project in [&real, &copy] {
            fs::create_dir_all(project).unwrap();
            fs::write(project.join("Cargo.toml"), b"[package]").unwrap();
        }
        let mut config = crate::core::app_config::default_config();
        config.project_scan_roots = vec![root.path().to_path_buf(), copy];
        config.local_repository = Some(repository);
        let result = discover_local_candidates(
            LocalScanRequest::from_config(&config, None),
            Arc::new(AtomicBool::new(false)),
        );
        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].path, real);
    }

    #[test]
    fn admin_json_request_uses_same_roots_without_elevation() {
        let root = tempdir().unwrap();
        let project = root.path().join("project");
        let repository = project.join("backups");
        for path in [&project, &repository] {
            fs::create_dir_all(path).unwrap();
            fs::write(path.join("Cargo.toml"), b"[package]").unwrap();
        }
        let mut config = crate::core::app_config::default_config();
        config.project_scan_roots = vec![root.path().to_path_buf()];
        config.local_repository = Some(repository);
        let output = root.path().join("result.json");
        assert!(run_admin_local_scan(&output).is_err());
        fs::write(
            output.with_extension("request.json"),
            serde_json::to_vec(&LocalScanRequest::from_config(&config, None)).unwrap(),
        )
        .unwrap();
        run_admin_local_scan(&output).unwrap();
        let result: LocalDiscoveryResult =
            serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
        assert_eq!(result.scanned_roots, vec![root.path().to_path_buf()]);
        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.candidates[0].path, project);
        assert!(!output.with_extension("json.partial").exists());
    }
}
