use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub(crate) struct ProjectTreeEntry {
    pub relative: PathBuf,
    pub source: PathBuf,
    pub metadata: fs::Metadata,
}

#[derive(Debug, Clone)]
pub(crate) enum ProjectTreeIssue {
    External { path: PathBuf, target: PathBuf },
    Unavailable { path: PathBuf },
    Cycle { path: PathBuf, target: PathBuf },
    Enumeration { path: PathBuf },
    Unsupported { path: PathBuf },
}

#[derive(Debug, Clone)]
pub(crate) enum ProjectTreeEvent {
    Entry(ProjectTreeEntry),
    Issue(ProjectTreeIssue),
}

/// Walk a project as a logical tree while following only links that resolve
/// inside the selected project root. Link targets are exposed as ordinary
/// source paths so callers can materialize them instead of writing links to a
/// backup payload. The callback returns whether a directory should be read.
pub(crate) fn walk_project_tree<F>(root: &Path, mut visit: F) -> io::Result<()>
where
    F: FnMut(ProjectTreeEvent) -> io::Result<bool>,
{
    let metadata = fs::symlink_metadata(root)?;
    if is_filesystem_redirect(&metadata) || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "project root is not a regular directory",
        ));
    }
    let root_identity = canonical_identity(root)?;
    let mut active = HashSet::from([root_identity]);
    walk_directory(root, root, Path::new(""), &mut active, &mut visit)
}

fn walk_directory<F>(
    project_root: &Path,
    physical: &Path,
    relative: &Path,
    active: &mut HashSet<PathBuf>,
    visit: &mut F,
) -> io::Result<()>
where
    F: FnMut(ProjectTreeEvent) -> io::Result<bool>,
{
    let entries = match fs::read_dir(physical) {
        Ok(entries) => entries,
        Err(_) => {
            visit(ProjectTreeEvent::Issue(ProjectTreeIssue::Enumeration {
                path: physical.to_path_buf(),
            }))?;
            return Ok(());
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                visit(ProjectTreeEvent::Issue(ProjectTreeIssue::Enumeration {
                    path: physical.to_path_buf(),
                }))?;
                continue;
            }
        };
        let entry_path = entry.path();
        let name = entry.file_name();
        let entry_relative = if relative.as_os_str().is_empty() {
            PathBuf::from(&name)
        } else {
            relative.join(&name)
        };
        let link_metadata = match fs::symlink_metadata(&entry_path) {
            Ok(metadata) => metadata,
            Err(_) => {
                visit(ProjectTreeEvent::Issue(ProjectTreeIssue::Unavailable {
                    path: entry_path,
                }))?;
                continue;
            }
        };

        if is_filesystem_redirect(&link_metadata) {
            let target = match resolve_link_target(&entry_path) {
                Ok(target) => target,
                Err(_) => {
                    visit(ProjectTreeEvent::Issue(ProjectTreeIssue::Unavailable {
                        path: entry_path,
                    }))?;
                    continue;
                }
            };
            let target_identity = match canonical_identity(&target) {
                Ok(target) => target,
                Err(_) => {
                    visit(ProjectTreeEvent::Issue(ProjectTreeIssue::Unavailable {
                        path: entry_path,
                    }))?;
                    continue;
                }
            };
            let root_identity = canonical_identity(project_root)?;
            if !is_inside_project_root(&root_identity, &target_identity) {
                visit(ProjectTreeEvent::Issue(ProjectTreeIssue::External {
                    path: entry_path,
                    target: target_identity,
                }))?;
                continue;
            }
            let target_metadata = match fs::metadata(&target_identity) {
                Ok(metadata) => metadata,
                Err(_) => {
                    visit(ProjectTreeEvent::Issue(ProjectTreeIssue::Unavailable {
                        path: entry_path,
                    }))?;
                    continue;
                }
            };
            if !(target_metadata.is_file() || target_metadata.is_dir()) {
                visit(ProjectTreeEvent::Issue(ProjectTreeIssue::Unsupported {
                    path: entry_path,
                }))?;
                continue;
            }
            let should_descend = visit(ProjectTreeEvent::Entry(ProjectTreeEntry {
                relative: entry_relative,
                source: target_identity.clone(),
                metadata: target_metadata.clone(),
            }))?;
            if should_descend && target_metadata.is_dir() {
                if active.contains(&target_identity) {
                    visit(ProjectTreeEvent::Issue(ProjectTreeIssue::Cycle {
                        path: entry_path,
                        target: target_identity,
                    }))?;
                } else {
                    active.insert(target_identity.clone());
                    walk_directory(
                        project_root,
                        &target_identity,
                        &relative.join(name),
                        active,
                        visit,
                    )?;
                    active.remove(&target_identity);
                }
            }
            continue;
        }

        if link_metadata.is_dir() {
            let should_descend = visit(ProjectTreeEvent::Entry(ProjectTreeEntry {
                relative: entry_relative,
                source: entry_path.clone(),
                metadata: link_metadata.clone(),
            }))?;
            if !should_descend {
                continue;
            }
            let identity = match canonical_identity(&entry_path) {
                Ok(identity) => identity,
                Err(_) => {
                    visit(ProjectTreeEvent::Issue(ProjectTreeIssue::Unavailable {
                        path: entry_path,
                    }))?;
                    continue;
                }
            };
            if active.contains(&identity) {
                visit(ProjectTreeEvent::Issue(ProjectTreeIssue::Cycle {
                    path: entry_path,
                    target: identity,
                }))?;
                continue;
            }
            active.insert(identity.clone());
            walk_directory(
                project_root,
                &entry_path,
                &relative.join(name),
                active,
                visit,
            )?;
            active.remove(&identity);
        } else if link_metadata.is_file() {
            visit(ProjectTreeEvent::Entry(ProjectTreeEntry {
                relative: entry_relative,
                source: entry_path,
                metadata: link_metadata,
            }))?;
        } else {
            visit(ProjectTreeEvent::Issue(ProjectTreeIssue::Unsupported {
                path: entry_path,
            }))?;
        }
    }
    Ok(())
}

pub(crate) fn resolve_link_target(link: &Path) -> io::Result<PathBuf> {
    match fs::read_link(link) {
        Ok(raw_target) => {
            let target = if raw_target.is_absolute() {
                raw_target
            } else {
                link.parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join(raw_target)
            };
            canonical_identity(&target)
        }
        Err(read_error) => canonical_identity(link).map_err(|_| read_error),
    }
}

pub(crate) fn canonical_identity(path: &Path) -> io::Result<PathBuf> {
    fs::canonicalize(path)
}

pub(crate) fn is_inside_project_root(project_root: &Path, target: &Path) -> bool {
    #[cfg(windows)]
    {
        let root = PathBuf::from(project_root.to_string_lossy().to_ascii_lowercase());
        let target = PathBuf::from(target.to_string_lossy().to_ascii_lowercase());
        target == root || target.starts_with(root)
    }
    #[cfg(not(windows))]
    {
        target == project_root || target.starts_with(project_root)
    }
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

#[cfg(test)]
mod tests {
    use super::{canonical_identity, is_inside_project_root};
    use std::path::Path;

    #[test]
    fn containment_is_boundary_aware() {
        assert!(is_inside_project_root(
            Path::new("C:/Projects/app"),
            Path::new("C:/Projects/app/src")
        ));
        assert!(!is_inside_project_root(
            Path::new("C:/Projects/app"),
            Path::new("C:/Projects/app-other")
        ));
    }

    #[test]
    fn canonical_identity_resolves_existing_synthetic_path() {
        let root = tempfile::tempdir().expect("temporary directory");
        let nested = root.path().join("nested");
        std::fs::create_dir_all(&nested).expect("nested directory");
        assert_eq!(
            canonical_identity(&nested).unwrap(),
            nested.canonicalize().unwrap()
        );
    }
}
