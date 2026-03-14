use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    // Configure FFmpeg shared libs path for ffmpeg-sys-next.
    configure_ffmpeg_libs();

    // Read the streaming platforms JSON
    let json_path = PathBuf::from("..")
        .join("data")
        .join("streaming-platforms.json");

    println!("cargo:rerun-if-changed={}", json_path.display());

    let json_content =
        fs::read_to_string(&json_path).expect("Failed to read streaming-platforms.json");

    let data: serde_json::Value =
        serde_json::from_str(&json_content).expect("Failed to parse streaming-platforms.json");

    let services = data["services"]
        .as_array()
        .expect("Expected 'services' array in JSON");

    // Generate enum variants
    let mut enum_code = String::from(
        "// Auto-generated from data/streaming-platforms.json\n\
         // DO NOT EDIT MANUALLY\n\n\
         #[allow(clippy::enum_variant_names, clippy::upper_case_acronyms)]\n\
         #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]\n\
         pub enum Platform {\n",
    );

    let mut variant_counts: HashMap<String, usize> = HashMap::new();
    let mut first_variant: Option<String> = None;

    for service in services {
        let name = service["name"].as_str().expect("Expected 'name' field");

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

/// Configure FFMPEG_DIR for ffmpeg-sys-next.
/// This function looks for FFmpeg shared libs in the standard locations:
/// 1. FFMPEG_DIR environment variable (if already set)
/// 2. Downloaded libs in app data directory
/// 3. System-installed libs (vcpkg, pkg-config)
fn configure_ffmpeg_libs() {
    // If FFMPEG_DIR is explicitly set, prefer that. Otherwise use the default app-data location.
    let ffmpeg_libs_dir = env::var("FFMPEG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| get_ffmpeg_libs_dir());

    if ffmpeg_libs_dir.exists()
        && ffmpeg_libs_dir.join("lib").exists()
        && ffmpeg_libs_dir.join("include").exists()
    {
        let ffmpeg_dir = ffmpeg_libs_dir.to_string_lossy();
        println!(
            "cargo:warning=Using FFmpeg shared libs from: {}",
            ffmpeg_dir
        );
        println!("cargo:rustc-env=FFMPEG_DIR={}", ffmpeg_dir);

        // Also need to tell the linker where to find the libs at runtime
        #[cfg(target_os = "windows")]
        {
            let bin_dir = ffmpeg_libs_dir.join("bin");
            if bin_dir.exists() {
                println!("cargo:rustc-link-search=native={}", bin_dir.display());
            }
            let lib_dir = ffmpeg_libs_dir.join("lib");
            if lib_dir.exists() {
                println!("cargo:rustc-link-search=native={}", lib_dir.display());
            }

            copy_ffmpeg_dlls_to_target_dirs(&bin_dir);
        }

        #[cfg(target_os = "linux")]
        {
            let lib_dir = ffmpeg_libs_dir.join("lib");
            if lib_dir.exists() {
                println!("cargo:rustc-link-search=native={}", lib_dir.display());
            }
        }
    } else {
        println!(
            "cargo:warning=FFmpeg shared libs not found at {}. \
             Run the app to download them, or set FFMPEG_DIR manually.",
            ffmpeg_libs_dir.display()
        );
        println!(
            "cargo:warning=Expected structure: {}/{{bin,lib,include}}/",
            ffmpeg_libs_dir.display()
        );
    }

    println!("cargo:rerun-if-env-changed=FFMPEG_DIR");
}

#[cfg(target_os = "windows")]
fn copy_ffmpeg_dlls_to_target_dirs(bin_dir: &PathBuf) {
    if !bin_dir.exists() {
        return;
    }

    let out_dir = match env::var("OUT_DIR") {
        Ok(path) => PathBuf::from(path),
        Err(_) => return,
    };

    // OUT_DIR looks like: target\debug\build\<crate-hash>\out
    let profile_dir = match out_dir.ancestors().nth(3) {
        Some(path) => path.to_path_buf(),
        None => return,
    };

    let destinations = [profile_dir.clone(), profile_dir.join("deps")];

    let entries = match fs::read_dir(bin_dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let source_path = entry.path();
        if source_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("dll"))
            != Some(true)
        {
            continue;
        }

        for destination in &destinations {
            let _ = fs::create_dir_all(destination);
            let target_path = destination.join(entry.file_name());
            let _ = fs::copy(&source_path, target_path);
        }
    }
}

/// Get the directory for FFmpeg shared libs (mirrors FFmpegDownloader::get_ffmpeg_libs_dir)
fn get_ffmpeg_libs_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
            return PathBuf::from(local_app_data)
                .join("SpiritStream")
                .join("ffmpeg-libs");
        }
        env::temp_dir().join("spiritstream-ffmpeg-libs")
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("SpiritStream")
                .join("ffmpeg-libs");
        }
        env::temp_dir().join("spiritstream-ffmpeg-libs")
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("spiritstream")
                .join("ffmpeg-libs");
        }
        env::temp_dir().join("spiritstream-ffmpeg-libs")
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        env::temp_dir().join("spiritstream-ffmpeg-libs")
    }
}
