use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Halt the build with a diagnostic that includes the offending file
/// path and the parser/structural detail. M6: pre-fix every read /
/// parse / shape-check used `.expect("…")` which surfaced as a bare
/// rustc panic with no breadcrumb back to `streaming-platforms.json`.
/// `eprintln!` + `process::exit(1)` keeps Cargo happy and gives the
/// operator something to grep.
fn die(prefix: &str, path: &Path, detail: impl std::fmt::Display) -> ! {
    eprintln!(
        "spiritstream-core build.rs: {prefix} ({path}): {detail}",
        path = path.display(),
    );
    std::process::exit(1)
}

fn main() -> ExitCode {
    // Read the streaming platforms JSON (workspace root: ../../data/...).
    let json_path = PathBuf::from("..")
        .join("..")
        .join("data")
        .join("streaming-platforms.json");

    println!("cargo:rerun-if-changed={}", json_path.display());

    let json_content = match fs::read_to_string(&json_path) {
        Ok(s) => s,
        Err(e) => die("Failed to read streaming-platforms.json", &json_path, e),
    };

    let data: serde_json::Value = match serde_json::from_str(&json_content) {
        Ok(v) => v,
        Err(e) => die(
            "Failed to parse streaming-platforms.json (invalid JSON)",
            &json_path,
            format!("line {}, column {}: {}", e.line(), e.column(), e),
        ),
    };

    let services = match data.get("services").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => die(
            "Failed to parse streaming-platforms.json",
            &json_path,
            "missing or non-array `services` field",
        ),
    };

    // Generate enum variants
    let mut enum_code = String::from(
        "// Auto-generated from data/streaming-platforms.json\n\
         // DO NOT EDIT MANUALLY\n\n\
         #[allow(clippy::enum_variant_names, clippy::upper_case_acronyms)]\n\
         #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default, ts_rs::TS)]\n\
         #[ts(export, export_to = \"../../../packages/types/src/generated/\")]\n\
         pub enum Platform {\n",
    );

    let mut variant_counts: HashMap<String, usize> = HashMap::new();
    let mut first_variant: Option<String> = None;

    for service in services {
        let name = match service.get("name").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => die(
                "platform entry missing or non-string `name` field",
                &json_path,
                serde_json::to_string(service).unwrap_or_default(),
            ),
        };

        let default_url = match service.get("defaultUrl").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => die(
                "platform entry missing or non-string `defaultUrl` field",
                &json_path,
                name,
            ),
        };

        let placement = match service.get("streamKeyPlacement").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => die(
                "platform entry missing or non-string `streamKeyPlacement` field",
                &json_path,
                name,
            ),
        };

        // Filter: only include RTMP/RTMPS with "append" or "in_url_template" placement
        if !default_url.starts_with("rtmp://") && !default_url.starts_with("rtmps://") {
            continue;
        }

        if placement != "append" && placement != "in_url_template" {
            continue;
        }

        // Sanitize the name to a valid Rust identifier
        let variant = sanitize_to_variant(name);

        // Handle duplicate variants by appending a number
        let final_variant = if let Some(count) = variant_counts.get(&variant) {
            let new_count = count + 1;
            variant_counts.insert(variant.clone(), new_count);
            format!("{variant}{new_count}")
        } else {
            variant_counts.insert(variant.clone(), 1);
            variant.clone()
        };

        let is_first = first_variant.is_none();
        if is_first {
            first_variant = Some(final_variant.clone());
        }

        if is_first {
            enum_code.push_str(&format!(
                "    #[serde(rename = \"{name}\")]\n    #[default]\n    {final_variant},\n"
            ));
        } else {
            enum_code.push_str(&format!(
                "    #[serde(rename = \"{name}\")]\n    {final_variant},\n"
            ));
        }
    }

    enum_code.push_str("}\n");

    // Write to OUT_DIR. Cargo guarantees OUT_DIR is set when build
    // scripts run; failure to read it is a Cargo-internal issue.
    let out_dir = match env::var("OUT_DIR") {
        Ok(v) => v,
        Err(e) => die(
            "OUT_DIR is not set (cargo invocation broken?)",
            &json_path,
            e,
        ),
    };
    let dest_path = PathBuf::from(out_dir).join("generated_platforms.rs");
    if let Err(e) = fs::write(&dest_path, enum_code) {
        die("Failed to write generated_platforms.rs", &dest_path, e);
    }
    ExitCode::SUCCESS
}

/// Sanitize a platform name to a valid Rust enum variant
/// - Remove special characters
/// - Convert to PascalCase
/// - Ensure it starts with a letter
fn sanitize_to_variant(name: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;

    for ch in name.chars() {
        if ch.is_alphanumeric() {
            if capitalize_next {
                result.push(ch.to_ascii_uppercase());
                capitalize_next = false;
            } else {
                result.push(ch);
            }
        } else {
            // Skip special characters, but capitalize the next letter
            capitalize_next = true;
        }
    }

    // Ensure it starts with a letter
    if result.is_empty() || result.chars().next().unwrap().is_numeric() {
        result.insert(0, 'P');
    }

    result
}
