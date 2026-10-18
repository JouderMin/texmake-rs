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
/// path is absolute. When the path exists it is canonicalized, following
/// symbolic links, so a link pointing outside the project is rejected too.
pub fn validate_output(root: &Path, output: &str, main_rel: &str) -> Result<PathBuf, String> {
    let root = normalize(root);
    let root = root.canonicalize().unwrap_or(root);

    let joined = normalize(&root.join(output));
    let resolved = joined.canonicalize().unwrap_or(joined);

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

/// Remove paths that are nested inside another entry in the list.
///
/// Used by `clean`, which must delete parent directories before their children
/// or simply skip the children.
#[allow(dead_code)] // consumed by `clean` in a later task
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
}
