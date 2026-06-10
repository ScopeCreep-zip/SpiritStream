// SpiritStream Path Validation Service
// Security utility to prevent path traversal attacks

use std::path::{Path, PathBuf};

use crate::errors::{CoreError, ValidationIssue};

fn outside_allowed(path: &Path) -> CoreError {
    CoreError::PathOutsideAllowedRoot {
        path: path.to_string_lossy().to_string(),
    }
}

/// Validate that a path is within an allowed directory (prevents path traversal attacks).
///
/// On success returns the canonicalized path. On failure returns
/// `CoreError::PathOutsideAllowedRoot { path }`; the transport layer maps
/// this to HTTP 403 / CLI exit code per `crates/transport-http/src/error.rs`.
pub fn validate_path_within(path: &Path, allowed_dir: &Path) -> Result<PathBuf, CoreError> {
    // First check for obvious traversal attempts
    let path_str = path.to_string_lossy();
    if path_str.contains("..") {
        return Err(outside_allowed(path));
    }

    // For paths that don't exist yet (like export targets), check the parent directory
    if !path.exists() {
        if let Some(parent) = path.parent() {
            let parent_canonical = parent.canonicalize().map_err(|_| outside_allowed(path))?;
            let allowed_canonical = allowed_dir
                .canonicalize()
                .map_err(|_| outside_allowed(allowed_dir))?;

            if !parent_canonical.starts_with(&allowed_canonical) {
                return Err(outside_allowed(path));
            }

            // Return the intended path (parent + filename)
            if let Some(filename) = path.file_name() {
                return Ok(parent_canonical.join(filename));
            }
        }
        return Err(outside_allowed(path));
    }

    // For existing paths, canonicalize and check
    let canonical = path.canonicalize().map_err(|_| outside_allowed(path))?;
    let allowed_canonical = allowed_dir
        .canonicalize()
        .map_err(|_| outside_allowed(allowed_dir))?;

    if !canonical.starts_with(&allowed_canonical) {
        return Err(outside_allowed(path));
    }

    Ok(canonical)
}

/// Validate that a path is within any of the allowed directories.
///
/// Returns the canonicalized path on the first match. Fails with
/// `CoreError::PathOutsideAllowedRoot` if none of the supplied roots contain
/// the path.
pub fn validate_path_within_any(path: &Path, allowed_dirs: &[&Path]) -> Result<PathBuf, CoreError> {
    for allowed_dir in allowed_dirs {
        if let Ok(validated) = validate_path_within(path, allowed_dir) {
            return Ok(validated);
        }
    }

    Err(outside_allowed(path))
}

/// Validate file extension is allowed.
///
/// Fails with `CoreError::ValidationFailed` carrying `code =
/// "invalid_file_extension"` plus the list of accepted extensions.
pub fn validate_extension(path: &Path, allowed_extensions: &[&str]) -> Result<(), CoreError> {
    let extension =
        path.extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| CoreError::ValidationFailed {
                reasons: vec![ValidationIssue {
                    code: "missing_file_extension".into(),
                    message: "File must have an extension".into(),
                    path: None,
                }],
            })?;

    if !allowed_extensions.contains(&extension) {
        return Err(CoreError::ValidationFailed {
            reasons: vec![ValidationIssue {
                code: "invalid_file_extension".into(),
                message: format!(
                    "Invalid file extension '{}'. Allowed: {}",
                    extension,
                    allowed_extensions.join(", ")
                ),
                path: None,
            }],
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_rejects_path_traversal() {
        let temp = tempdir().unwrap();
        let allowed = temp.path();
        let bad_path = allowed.join("../../../etc/passwd");

        let result = validate_path_within(&bad_path, allowed);
        assert!(matches!(
            result,
            Err(CoreError::PathOutsideAllowedRoot { .. })
        ));
    }

    #[test]
    fn test_accepts_valid_path() {
        let temp = tempdir().unwrap();
        let allowed = temp.path();
        let valid_file = allowed.join("test.txt");
        fs::write(&valid_file, "test").unwrap();

        let result = validate_path_within(&valid_file, allowed);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validates_extension() {
        let path = Path::new("/some/file.json");
        assert!(validate_extension(path, &["json", "jsonc"]).is_ok());

        let bad_path = Path::new("/some/file.exe");
        let err = validate_extension(bad_path, &["json", "jsonc"]).unwrap_err();
        assert!(matches!(err, CoreError::ValidationFailed { .. }));
    }
}
