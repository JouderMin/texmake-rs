//! Lexical path normalization and output-directory safety checks.
//!
//! Configuration values are local, trusted input, but an output directory is
//! still a delete target for `texmake clean`, so it must never point at the
//! project root, outside the project, or at a regular file.

use std::path::{Component, Path, PathBuf};

/// Normalize `path` lexically: drop `.` components and fold `..` into the
/// preceding component.
///
/// This does not touch the filesystem, so it also works for paths that do not
/// exist yet. Going above an absolute root is clamped to the root; for relative
/// paths a leading `..` is preserved.
pub fn normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Component::RootDir.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => match normalized.components().next_back() {
                Some(Component::Normal(_)) => {
                    normalized.pop();
                }
                Some(Component::RootDir | Component::Prefix(_)) => {}
                _ => normalized.push(Component::ParentDir.as_os_str()),
            },
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

/// Resolve `output` against `root` and reject unsafe output directories.
///
/// `main_rel` is the recipe's main document, relative to `root`. The returned
/// path is absolute. Existing components are resolved for real before any
/// containment check: symbolic links are followed and `..` applies to the
/// resolved parent, so a link that escapes the project is rejected even when
/// the final component does not exist yet. Only the still-missing tail is
/// handled lexically, and a symbolic link that cannot be resolved (dangling or
/// looping) is an error.
pub fn validate_output(root: &Path, output: &str, main_rel: &str) -> Result<PathBuf, String> {
    let root = normalize(root);
    let root = match root.canonicalize() {
        Ok(canonical) => canonical,
        // A root that does not exist has no symbolic links to follow, so the
        // lexical path is already fully resolved.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => root,
        Err(error) => {
            return Err(format!(
                "cannot resolve the project root {}: {error}",
                root.display()
            ));
        }
    };

    let resolved = resolve_output(&root, output)?;

    if resolved == root {
        return Err("the output directory must not be the project root".to_owned());
    }
    if !resolved.starts_with(&root) {
        return Err(format!(
            "the output directory `{output}` is outside the project root"
        ));
    }
    if resolved.is_file() {
        return Err(format!(
            "the output path `{output}` is a file, not a directory"
        ));
    }

    let main = normalize(&root.join(main_rel));
    let main = main.canonicalize().unwrap_or(main);
    if main.starts_with(&resolved) {
        return Err(format!(
            "the output directory `{output}` contains the main document"
        ));
    }

    Ok(resolved)
}

/// Resolve `output` against the real path `root`, following symbolic links in
/// every component that exists.
///
/// Components are walked in order, mirroring what the filesystem does when the
/// directory is created, so `..` after a symbolic link goes to the link
/// target's parent instead of popping a lexical component. `output` may name
/// components that do not exist yet; those are kept in `missing` and appended
/// lexically. A symbolic link that exists but cannot be resolved (a dangling or
/// looping link) is an error rather than an ordinary missing directory.
fn resolve_output(root: &Path, output: &str) -> Result<PathBuf, String> {
    let mut resolved = root.to_path_buf();
    let mut missing: Vec<std::ffi::OsString> = Vec::new();

    for component in Path::new(output).components() {
        match component {
            Component::Prefix(prefix) => {
                resolved = PathBuf::from(prefix.as_os_str());
                missing.clear();
            }
            Component::RootDir => {
                resolved = PathBuf::from(Component::RootDir.as_os_str());
                missing.clear();
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if missing.pop().is_none()
                    && let Some(parent) = resolved.parent()
                {
                    resolved = parent.to_path_buf();
                }
            }
            Component::Normal(part) => {
                if !missing.is_empty() {
                    missing.push(part.to_os_string());
                    continue;
                }
                let candidate = resolved.join(part);
                match std::fs::symlink_metadata(&candidate) {
                    Ok(metadata) if metadata.file_type().is_symlink() => {
                        match candidate.canonicalize() {
                            Ok(canonical) => resolved = canonical,
                            Err(error) => {
                                return Err(format!(
                                    "the output path `{output}` contains the symbolic link \
                                     `{}`, which cannot be resolved: {error}",
                                    candidate.display()
                                ));
                            }
                        }
                    }
                    Ok(_) => resolved = candidate,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        missing.push(part.to_os_string());
                    }
                    Err(error) => {
                        return Err(format!("cannot resolve `{}`: {error}", candidate.display()));
                    }
                }
            }
        }
    }

    for part in missing {
        resolved.push(part);
    }
    Ok(resolved)
}

/// Remove paths that are nested inside another entry in the list.
///
/// Used by `clean`, which must delete parent directories before their children
/// or simply skip the children.
pub fn dedupe_nested(mut paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.sort();
    let mut kept: Vec<PathBuf> = Vec::with_capacity(paths.len());
    for path in paths {
        if !kept.iter().any(|parent| path.starts_with(parent)) {
            kept.push(path);
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_dot_and_dotdot() {
        let root = Path::new("/p");
        assert_eq!(
            validate_output(root, "build/../out", "main.tex").unwrap(),
            PathBuf::from("/p/out")
        );
    }

    #[test]
    fn rejects_output_outside_root() {
        assert!(validate_output(Path::new("/p"), "../out", "main.tex").is_err());
    }

    #[test]
    fn drops_nested_paths() {
        let paths = vec!["/p/build".into(), "/p/build/sub".into(), "/p/out".into()];
        assert_eq!(
            dedupe_nested(paths),
            vec![PathBuf::from("/p/build"), PathBuf::from("/p/out")]
        );
    }

    #[test]
    fn folds_dotdot_into_the_root() {
        assert_eq!(
            normalize(Path::new("/p/a/../../out")),
            PathBuf::from("/out")
        );
        assert_eq!(normalize(Path::new("/p/a/../b")), PathBuf::from("/p/b"));
    }

    #[test]
    fn rejects_project_root_as_output() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("main.tex"), "").unwrap();
        let error = validate_output(tmp.path(), ".", "main.tex").unwrap_err();
        assert!(error.contains("project root"), "{error}");
    }

    #[test]
    fn rejects_regular_file_as_output() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("main.tex"), "").unwrap();
        std::fs::write(tmp.path().join("out.pdf"), "").unwrap();
        let error = validate_output(tmp.path(), "out.pdf", "main.tex").unwrap_err();
        assert!(error.contains("file"), "{error}");
    }

    #[test]
    fn accepts_existing_and_new_nested_directories() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("main.tex"), "").unwrap();
        std::fs::create_dir_all(tmp.path().join("build/paper")).unwrap();
        let existing = validate_output(tmp.path(), "build/paper", "main.tex").unwrap();
        let new = validate_output(tmp.path(), "build/thesis", "main.tex").unwrap();
        assert_eq!(existing, new.parent().unwrap().join("paper"));
        assert!(new.ends_with("build/thesis"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_ancestor_leading_outside_the_root() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("main.tex"), "").unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), tmp.path().join("build")).unwrap();

        let error = validate_output(tmp.path(), "build/thesis", "main.tex").unwrap_err();
        assert!(error.contains("outside"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_dotdot_through_a_symlinked_ancestor() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("main.tex"), "").unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), tmp.path().join("build")).unwrap();

        let error = validate_output(tmp.path(), "build/../escaped", "main.tex").unwrap_err();
        assert!(error.contains("outside"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_dangling_symlink_in_the_output_path() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("main.tex"), "").unwrap();
        std::os::unix::fs::symlink(tmp.path().join("missing"), tmp.path().join("build")).unwrap();

        let error = validate_output(tmp.path(), "build/thesis", "main.tex").unwrap_err();
        assert!(error.contains("symbolic link"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_encountered_after_lexical_backtracking() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("main.tex"), "").unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), tmp.path().join("build")).unwrap();

        let error = validate_output(tmp.path(), "new/../build/thesis", "main.tex").unwrap_err();
        assert!(error.contains("outside"), "{error}");
    }

    #[test]
    fn allows_new_nested_directories_without_symlinks() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("main.tex"), "").unwrap();

        let resolved = validate_output(tmp.path(), "build/new/nested", "main.tex").unwrap();
        assert!(
            resolved.ends_with("build/new/nested"),
            "{}",
            resolved.display()
        );
    }

    #[test]
    fn folds_dotdot_between_missing_components() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("main.tex"), "").unwrap();
        let root = tmp.path().canonicalize().unwrap();

        let resolved = validate_output(tmp.path(), "new/../out", "main.tex").unwrap();
        assert_eq!(resolved, root.join("out"));
    }
}
