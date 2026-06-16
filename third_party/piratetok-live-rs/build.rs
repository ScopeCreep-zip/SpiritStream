use std::fs;
use std::path::Path;

const MAX_LOC: usize = 800;
const MAX_LOC_PROTO: usize = 900;

const SILENT_ERROR_PATTERNS: &[&str] = &[".unwrap()", ".unwrap_or(", ".expect(", ".ok()", ".ok()?", ".map_or("];

const ERROR_SWALLOW_EXEMPT: &[&str] = &["src/main.rs"];

const LOC_EXEMPT: &[&str] = &[
    "src/structs/proto/messages.rs",
    "src/structs/proto/messages_ext.rs",
    "src/structs/proto/linker.rs",
    "src/structs/proto/types.rs",
    "src/structs/proto/user.rs",
    "src/structs/proto/gift_types.rs",
];

fn main() {
    let mut violations: Vec<String> = Vec::new();
    scan_directory(Path::new("src"), &mut violations);

    if !violations.is_empty() {
        let header = format!("\n========== BUILD CONSTITUTION VIOLATED ({} issues) ==========\n", violations.len());
        let rules = r#"
RULES:
  [R1] File size limit: max 800 LOC (900 for proto struct files)
  [R2] No silent error suppression: .unwrap(), .expect(), .ok() banned in library code
  [R3] No glob imports: use x::* banned (except mod.rs re-exports)
"#;
        let body = violations.join("\n");
        panic!("{}{}\n{}", header, body, rules);
    }
}

fn scan_directory(dir: &Path, violations: &mut Vec<String>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_directory(&path, violations);
        } else if path.extension().is_some_and(|e| e == "rs") {
            scan_file(&path, violations);
        }
    }
}

fn scan_file(path: &Path, violations: &mut Vec<String>) {
    // SpiritStream patch: normalize separators so the `src/bin/`,
    // `src/main.rs`, and proto LOC exemptions below match on Windows,
    // where read_dir yields backslash paths. Upstream 0.2.0 hardcodes
    // forward-slash patterns, so the build-constitution scan spuriously
    // panics on Windows only. No-op on Unix.
    let rel = path.to_string_lossy().replace('\\', "/");
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let lines: Vec<&str> = content.lines().collect();
    let loc = lines.len();

    // R1: file size
    let limit = if LOC_EXEMPT.iter().any(|e| rel.ends_with(e)) { MAX_LOC_PROTO } else { MAX_LOC };
    if loc > limit {
        violations.push(format!("  [R1] {} has {} lines (max {})", rel, loc, limit));
    }

    // R2: silent error suppression
    let is_exempt = ERROR_SWALLOW_EXEMPT.iter().any(|e| rel.ends_with(e)) || rel.contains("src/bin/");
    if !is_exempt {
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("///") {
                continue;
            }
            for pattern in SILENT_ERROR_PATTERNS {
                if line.contains(pattern) {
                    violations.push(format!("  [R2] {}:{} contains '{}' — propagate errors explicitly", rel, i + 1, pattern));
                }
            }
        }
    }

    // R3: glob imports (except mod.rs re-exports)
    let is_mod = rel.ends_with("mod.rs");
    if !is_mod {
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("use ") && trimmed.contains("::*") && !trimmed.starts_with("//") {
                violations.push(format!("  [R3] {}:{} glob import — use explicit imports", rel, i + 1,));
            }
        }
    }
}
