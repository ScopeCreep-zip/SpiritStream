// SpiritStream Path Validation Service
// Security utility to prevent path traversal attacks

use std::path::{Path, PathBuf};

/// Validate that a path is within an allowed directory (prevents path traversal attacks).
///
/// # Arguments
/// * `path` - The path to validate
/// * `allowed_dir` - The directory the path must be within
///
/// # Returns
/// * `Ok(PathBuf)` - The canonicalized path if valid
/// * `Err(String)` - Error message if validation fails
pub fn validate_path_within(path: &Path, allowed_dir: &Path) -> Result<PathBuf, String> {
    // First check for obvious traversal attempts
    let path_str = path.to_string_lossy();
    if path_str.contains("..") {
        return Err("Path traversal detected: '..' not allowed".to_string());
    }

    // For paths that don't exist yet (like export targets), check the parent directory
    if !path.exists() {
        if let Some(parent) = path.parent() {
            let parent_canonical = parent
                .canonicalize()
                .map_err(|e| format!("Invalid path: parent directory does not exist: {e}"))?;
            let allowed_canonical = allowed_dir
                .canonicalize()
                .map_err(|e| format!("Invalid allowed directory: {e}"))?;

            if !parent_canonical.starts_with(&allowed_canonical) {
                return Err(
                    "Path traversal detected: path outside allowed directory".to_string(),
                );
            }

            // Return the intended path (parent + filename)
            if let Some(filename) = path.file_name() {
                return Ok(parent_canonical.join(filename));
            }
        }
        return Err("Invalid path: cannot determine parent directory".to_string());
    }

    // For existing paths, canonicalize and check
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("Invalid path: {e}"))?;
    let allowed_canonical = allowed_dir
        .canonicalize()
        .map_err(|e| format!("Invalid allowed directory: {e}"))?;

    if !canonical.starts_with(&allowed_canonical) {
        return Err("Path traversal detected: path outside allowed directory".to_string());
    }

    Ok(canonical)
}

/// Validate that a path is within any of the allowed directories.
///
/// # Arguments
/// * `path` - The path to validate
/// * `allowed_dirs` - List of directories the path may be within
///
/// # Returns
/// * `Ok(PathBuf)` - The canonicalized path if valid
/// * `Err(String)` - Error message if validation fails
pub fn validate_path_within_any(path: &Path, allowed_dirs: &[&Path]) -> Result<PathBuf, String> {
    for allowed_dir in allowed_dirs {
        if let Ok(validated) = validate_path_within(path, allowed_dir) {
            return Ok(validated);
        }
    }

    Err("Path traversal detected: path outside all allowed directories".to_string())
}

/// Validate file extension is allowed.
///
/// # Arguments
/// * `path` - The path to check
/// * `allowed_extensions` - List of allowed extensions (without dot, e.g., "json")
///
/// # Returns
/// * `Ok(())` - If extension is valid
/// * `Err(String)` - Error message if extension is not allowed
pub fn validate_extension(path: &Path, allowed_extensions: &[&str]) -> Result<(), String> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| "File must have an extension".to_string())?;

    if !allowed_extensions.contains(&extension) {
        return Err(format!(
            "Invalid file extension '{}'. Allowed: {}",
            extension,
            allowed_extensions.join(", ")
        ));
    }

    Ok(())
}

/// Sanitize a filename to prevent directory traversal.
/// Removes any path separators and '..' sequences.
///
/// # Arguments
/// * `filename` - The filename to sanitize
///
/// # Returns
/// * The sanitized filename
pub fn sanitize_filename(filename: &str) -> String {
    filename
        .replace(['/', '\\'], "_")
        .replace("..", "_")
        .trim()
        .to_string()
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
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("traversal"));
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
        assert!(validate_extension(bad_path, &["json", "jsonc"]).is_err());
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("test.txt"), "test.txt");
        assert_eq!(sanitize_filename("../../../etc/passwd"), "______etc_passwd");
        assert_eq!(sanitize_filename("file\\name"), "file_name");
    }
}
