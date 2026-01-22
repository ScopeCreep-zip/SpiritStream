use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    // Read the streaming platforms JSON
    let json_path = PathBuf::from("..").join("data").join("streaming-platforms.json");

    println!("cargo:rerun-if-changed={}", json_path.display());

    let json_content = fs::read_to_string(&json_path)
        .expect("Failed to read streaming-platforms.json");

    let data: serde_json::Value = serde_json::from_str(&json_content)
        .expect("Failed to parse streaming-platforms.json");

    let services = data["services"]
        .as_array()
        .expect("Expected 'services' array in JSON");

    // Generate enum variants
    let mut enum_code = String::from(
        "// Auto-generated from data/streaming-platforms.json\n\
         // DO NOT EDIT MANUALLY\n\n\
         #[allow(clippy::enum_variant_names, clippy::upper_case_acronyms)]\n\
         #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]\n\
         pub enum Platform {\n"
    );

    let mut variant_counts: HashMap<String, usize> = HashMap::new();
    let mut first_variant: Option<String> = None;

    for service in services {
        let name = service["name"]
            .as_str()
            .expect("Expected 'name' field");

        let default_url = service["defaultUrl"]
            .as_str()
            .expect("Expected 'defaultUrl' field");

        let placement = service["streamKeyPlacement"]
            .as_str()
            .expect("Expected 'streamKeyPlacement' field");

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

    // Write to OUT_DIR
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = PathBuf::from(out_dir).join("generated_platforms.rs");
    fs::write(&dest_path, enum_code).expect("Failed to write generated_platforms.rs");
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
