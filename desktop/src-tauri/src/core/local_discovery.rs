use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

const MAX_SCAN_DEPTH: usize = 8;
const MAX_ENTRIES_PER_ROOT: usize = 50_000;
const MAX_SCAN_DURATION: Duration = Duration::from_secs(20);
const MAX_REPORTED_ITEMS: usize = 100;
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalDiscoveryResult {
    pub candidates: Vec<LocalProjectCandidate>,
    pub codex_homes: Vec<PathBuf>,
    pub conversation_count: u64,
    pub scanned_roots: Vec<PathBuf>,
    pub skipped_roots: Vec<PathBuf>,
    pub warnings: Vec<String>,
    pub cancelled: bool,
}

pub fn discover_local_candidates(cancel: Arc<AtomicBool>) -> LocalDiscoveryResult {
    discover_from_roots(fixed_drive_roots(), cancel)
}

fn discover_from_roots(roots: Vec<PathBuf>, cancel: Arc<AtomicBool>) -> LocalDiscoveryResult {
    let mut result = LocalDiscoveryResult {
        candidates: Vec::new(),
        codex_homes: Vec::new(),
        conversation_count: 0,
        scanned_roots: Vec::new(),
        skipped_roots: Vec::new(),
        warnings: Vec::new(),
        cancelled: false,
    };
    let mut candidates = HashMap::new();
    let mut codex_home_keys = HashSet::new();

    for root in roots {
        if cancel.load(Ordering::Relaxed) {
            result.cancelled = true;
            break;
        }
        result.scanned_roots.push(root.clone());
        scan_root(
            &root,
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
    cancel: &AtomicBool,
    candidates: &mut HashMap<String, LocalProjectCandidate>,
    codex_home_keys: &mut HashSet<String>,
    result: &mut LocalDiscoveryResult,
) {
    if !root.is_dir() {
        push_limited(
            &mut result.warnings,
            format!("Skipped unavailable local scan root: {}", root.display()),
        );
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
        if entries_seen >= MAX_ENTRIES_PER_ROOT || started.elapsed() >= MAX_SCAN_DURATION {
            push_limited(
                &mut result.warnings,
                format!("Local project scan limit reached at {}", root.display()),
            );
            return;
        }

        let identity = fs::canonicalize(&current).unwrap_or_else(|_| current.clone());
        let identity_key = identity.to_string_lossy().to_ascii_lowercase();
        if visited.insert(identity_key, ()).is_some() {
            continue;
        }

        let entries = match fs::read_dir(&current) {
            Ok(entries) => entries,
            Err(error) => {
                push_limited(
                    &mut result.warnings,
                    format!(
                        "Could not read local project directory {}: {error}",
                        current.display()
                    ),
                );
                continue;
            }
        };

        let mut markers = Vec::new();
        let mut child_directories = Vec::new();
        for entry in entries {
            entries_seen += 1;
            if entries_seen >= MAX_ENTRIES_PER_ROOT {
                break;
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    push_limited(
                        &mut result.warnings,
                        format!(
                            "Could not inspect a child of {}: {error}",
                            current.display()
                        ),
                    );
                    continue;
                }
            };
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(error) => {
                    push_limited(
                        &mut result.warnings,
                        format!("Could not inspect {}: {error}", entry.path().display()),
                    );
                    continue;
                }
            };
            if file_type.is_symlink() {
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
                            count_codex_conversations(
                                &codex_home,
                                cancel,
                                &mut result.conversation_count,
                                &mut result.warnings,
                            );
                            push_limited(&mut result.codex_homes, codex_home);
                            if cancel.load(Ordering::Relaxed) {
                                result.cancelled = true;
                                return;
                            }
                        }
                    } else {
                        push_limited(&mut result.skipped_roots, entry.path());
                    }
                } else if should_skip_directory(&name) {
                    push_limited(&mut result.skipped_roots, entry.path());
                } else if depth < MAX_SCAN_DEPTH {
                    child_directories.push(entry.path());
                }
            } else if file_type.is_file() && is_project_file_marker(&name) {
                markers.push(name);
            }
        }

        if !markers.is_empty() {
            markers.sort_unstable_by_key(|marker| marker.to_ascii_lowercase());
            markers.dedup_by_key(|marker| marker.to_ascii_lowercase());
            let key = identity.to_string_lossy().to_ascii_lowercase();
            let name = current
                .file_name()
                .and_then(|value| value.to_str())
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| current.to_string_lossy().into_owned());
            candidates
                .entry(key)
                .and_modify(|candidate| {
                    candidate.markers.extend(markers.iter().cloned());
                    candidate.markers.sort_unstable();
                    candidate.markers.dedup();
                })
                .or_insert(LocalProjectCandidate {
                    path: current.clone(),
                    name,
                    markers,
                });
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
    SKIPPED_DIRECTORY_NAMES
        .iter()
        .any(|skipped| name.eq_ignore_ascii_case(skipped))
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
    count: &mut u64,
    warnings: &mut Vec<String>,
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
            push_limited(
                warnings,
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
                push_limited(
                    warnings,
                    format!(
                        "Could not read Codex conversation directory {}: {error}",
                        current.display()
                    ),
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
                    push_limited(
                        warnings,
                        format!(
                            "Could not inspect a Codex conversation entry in {}: {error}",
                            current.display()
                        ),
                    );
                    continue;
                }
            };
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(error) => {
                    push_limited(
                        warnings,
                        format!("Could not inspect {}: {error}", entry.path().display()),
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
                *count += 1;
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
        let result = discover_from_roots(vec![root.path().to_path_buf()], cancel.clone());

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
            Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(result.codex_homes, vec![codex_home]);
        assert_eq!(result.conversation_count, 2);
    }
}
