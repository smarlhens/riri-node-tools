use crate::detect::{WorkspaceError, WorkspaceProject};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceMember {
    pub name: String,
    pub dir: PathBuf,
    pub manifest_path: PathBuf,
}

#[allow(clippy::result_large_err)]
pub(crate) fn enumerate(
    project: &WorkspaceProject,
) -> Result<Vec<WorkspaceMember>, WorkspaceError> {
    let mut out: Vec<WorkspaceMember> = Vec::new();
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
    walk(
        project.root(),
        project.root(),
        &project.globs,
        &mut out,
        &mut seen,
    )?;
    out.sort_by(|a, b| a.manifest_path.cmp(&b.manifest_path));
    Ok(out)
}

#[allow(clippy::result_large_err)]
fn walk(
    dir: &Path,
    root: &Path,
    globs: &globset::GlobSet,
    out: &mut Vec<WorkspaceMember>,
    seen: &mut BTreeSet<PathBuf>,
) -> Result<(), WorkspaceError> {
    let read = std::fs::read_dir(dir).map_err(|source| WorkspaceError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    for entry in read.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with('.') || name_str == "node_modules" {
            continue;
        }
        let entry_path = entry.path();
        let Ok(meta) = entry.file_type() else {
            continue;
        };
        if !meta.is_dir() || meta.is_symlink() {
            continue;
        }

        let rel = entry_path.strip_prefix(root).unwrap_or(&entry_path);
        let rel_str = path_to_forward_slash(rel);
        if globs.is_match(&rel_str) {
            let manifest = entry_path.join("package.json");
            if manifest.is_file() {
                let canonical = manifest.canonicalize().unwrap_or_else(|_| manifest.clone());
                if seen.insert(canonical) {
                    let member_name = member_name(&manifest, &rel_str);
                    out.push(WorkspaceMember {
                        name: member_name,
                        dir: entry_path.clone(),
                        manifest_path: manifest,
                    });
                }
            }
        }

        walk(&entry_path, root, globs, out, seen)?;
    }
    Ok(())
}

fn member_name(manifest: &Path, rel: &str) -> String {
    let Ok(raw) = std::fs::read_to_string(manifest) else {
        return rel.to_string();
    };
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return rel.to_string();
    };
    parsed
        .get("name")
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty())
        .map_or_else(|| rel.to_string(), str::to_string)
}

fn path_to_forward_slash(path: &Path) -> String {
    path.components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(s.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}
