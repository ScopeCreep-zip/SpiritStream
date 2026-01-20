// ProfileManager Service
// Handles profile persistence and encryption
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::path::PathBuf;
use crate::models::{Profile, ProfileSummary, RtmpInput};
use crate::services::Encryption;

// Magic bytes to identify encrypted profiles
const ENCRYPTED_MAGIC: &[u8] = b"MGLA";

/// Validate profile name to prevent path traversal attacks
fn validate_profile_name(name: &str) -> Result<(), String> {
    // No empty names
    if name.is_empty() {
        return Err("Profile name cannot be empty".to_string());
    }
    // No path separators
    if name.contains('/') || name.contains('\\') {
        return Err("Profile name cannot contain path separators".to_string());
    }
    // No path traversal
    if name.contains("..") {
        return Err("Profile name cannot contain '..'".to_string());
    }
    // Only alphanumeric, underscore, hyphen, and space
    if !name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == ' ') {
        return Err("Profile name can only contain letters, numbers, spaces, underscores, and hyphens".to_string());
    }
    // Reasonable length limit
    if name.len() > 100 {
        return Err("Profile name too long (max 100 characters)".to_string());
    }
    Ok(())
}

/// Manages profile storage and retrieval
pub struct ProfileManager {
    profiles_dir: PathBuf,
    app_data_dir: PathBuf,
    order_index_dir: PathBuf,
}

impl ProfileManager {
    /// Create a new ProfileManager with the given app data directory
    pub fn new(app_data_dir: PathBuf) -> Self {
        let profiles_dir = app_data_dir.join("profiles");
        let order_index_dir = app_data_dir.join("indexes");
        std::fs::create_dir_all(&profiles_dir).ok();
        std::fs::create_dir_all(&order_index_dir).ok();
        Self {
            profiles_dir,
            app_data_dir,
            order_index_dir,
        }
    }
 
    // To read order indexes for drag and drop on profiles
    pub fn read_order_index_map(&self) -> Result<HashMap<String, i32>, String> {
        let indexes_path = self.order_index_dir.join("order_indexes.json");
        
        if !indexes_path.exists() {
            let empty: HashMap<String, i32> = HashMap::new();
            let content = serde_json::to_string_pretty(&empty)
                .map_err(|e| format!("Failed to read empty order map: {e}"))?;
            std::fs::write(&indexes_path, content)
                .map_err(|e| format!("Failed to create order_indexes.json: {e}"))?;
            return Ok(empty);
        }

        let content = std::fs::read_to_string(&indexes_path)
            .map_err(|e| format!("Failed to read order_indexes.json: {e}"))?;
        let map: HashMap<String, i32> = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse order_indexes.json: {e}"))?;

        Ok(map)     
    }

    
    pub fn write_order_index_map(&self, map: &HashMap<String, i32>) -> Result<(), String> {
        let path = self.order_index_dir.join("order_indexes.json");
        let tmp  = self.order_index_dir.join("order_indexes.json.tmp");

        let content = serde_json::to_string_pretty(map)
            .map_err(|e| format!("Failed to serialize order map: {e}"))?;

        std::fs::write(&tmp, content)
            .map_err(|e| format!("Failed to write temp order_indexes.json: {e}"))?;

        std::fs::rename(&tmp, &path)
            .map_err(|e| format!("Failed to replace order_indexes.json: {e}"))?;

        Ok(())
    }

 

    /// This method can eventually be removed, it's purpose is to add order_index
    /// to profiles that were created before order_index was introduced
    pub async fn ensure_order_indexes(&self) -> Result<HashMap<String, i32>, String> {
        let names = self.get_all_names().await?;
        let mut map = self.read_order_index_map()?;

        let mut max = map.values().copied().max().unwrap_or(0);
        max = ((max + 9) / 10) * 10;
        
        let mut changed = false;

        for name in names.clone() { 
            match map.entry(name.clone()) {
                Entry::Vacant(e) => {
                    max += 10;
                    e.insert(max);
                    changed = true;
                }
                Entry::Occupied(_) => {}
            }
        }
        map.retain(|k, _| names.contains(k));
        if changed {
            self.write_order_index_map(&map)?;
        }

        Ok(map)
    }

    /// Get all profile names from the profiles directory
    pub async fn get_all_names(&self) -> Result<Vec<String>, String> {
        let mut names = Vec::new();


        let entries = std::fs::read_dir(&self.profiles_dir)
            .map_err(|e| e.to_string())?;

        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_stem() {
                // Accept both .json (unencrypted) and .mgs (encrypted) files
                let ext = path.extension().and_then(|e| e.to_str());
                if ext == Some("json") || ext == Some("mgs") {
                    names.push(name.to_string_lossy().to_string());
                }
            }
        }

        Ok(names)
    }

    /// Get summaries of all profiles (for list display)
    /// Includes services list for each profile (Story 1.1, 4.1, 4.2)
    pub async fn get_all_summaries(&self) -> Result<Vec<ProfileSummary>, String> {
        let names = self.get_all_names().await?;
        let order_map = self.ensure_order_indexes().await?; //<- Can eventually be replaced

        // ensure_order_indexes can be replaced with this below eventually
        // the purpose for ensure_order_indexes is to keep things from breaking
        // if the user had profiles saved before merging this code,
        // this will ensure the index is added to previously saved
        // profiles 
        // eventual replacement -> let order_map = self.read_order_index_map()?;

        let mut summaries = Vec::new();

        for name in names {
            let is_encrypted = self.is_encrypted(&name);

            // Try to load the profile (unencrypted profiles only)
            // Encrypted profiles show minimal info since we can't read them without password
            if let Ok(profile) = self.load(&name, None).await {
                summaries.push(profile.to_summary(is_encrypted));
            } else if is_encrypted {
                // For encrypted profiles we can't read, show minimal summary
                summaries.push(ProfileSummary {
                    id: String::new(),
                    name: name.clone(),
                    resolution: "?".to_string(),
                    bitrate: 0,
                    target_count: 0,
                    services: Vec::new(),
                    is_encrypted: true,
                });
            }
        }
        summaries.sort_by_key(|s| order_map.get(&s.name).copied().unwrap_or(i32::MAX));
        Ok(summaries)
    }

    /// Load a profile by name
    /// If password is provided, will attempt to decrypt
    /// If no password, will try unencrypted first, then fail if encrypted
    pub async fn load(&self, name: &str, password: Option<&str>) -> Result<Profile, String> {
        // Validate profile name to prevent path traversal attacks
        validate_profile_name(name)?;

        // Try encrypted file first (if password provided)
        let encrypted_path = self.profiles_dir.join(format!("{name}.mgs"));
        let json_path = self.profiles_dir.join(format!("{name}.json"));

        // Check if encrypted version exists
        if encrypted_path.exists() {
            let data = std::fs::read(&encrypted_path)
                .map_err(|e| format!("Failed to read profile: {e}"))?;

            // Verify magic bytes
            if data.len() < ENCRYPTED_MAGIC.len() || &data[..ENCRYPTED_MAGIC.len()] != ENCRYPTED_MAGIC {
                return Err("Invalid encrypted profile format".to_string());
            }

            // Password required for encrypted profiles
            let password = password.ok_or("Password required for encrypted profile")?;

            // Decrypt (skip magic bytes)
            let decrypted = Encryption::decrypt(&data[ENCRYPTED_MAGIC.len()..], password)?;

            // Parse JSON
            let content = String::from_utf8(decrypted)
                .map_err(|e| format!("Invalid UTF-8 in decrypted profile: {e}"))?;

            return serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse profile: {e}"));
        }

        // Try unencrypted JSON file
        if json_path.exists() {
            let content = std::fs::read_to_string(&json_path)
                .map_err(|e| format!("Failed to read profile: {e}"))?;

            return serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse profile: {e}"));
        }

        Err(format!("Profile '{name}' not found"))
    }

    /// Delete a profile by name (both encrypted and unencrypted versions)
    pub async fn delete(&self, name: &str) -> Result<(), String> {
        log::info!("Deleting profile: {}", name);

        // Validate profile name to prevent path traversal attacks
        validate_profile_name(name)?;

        let json_path = self.profiles_dir.join(format!("{name}.json"));
        let mgs_path = self.profiles_dir.join(format!("{name}.mgs"));

        let mut deleted = false;

        if json_path.exists() {
            std::fs::remove_file(&json_path)
                .map_err(|e| format!("Failed to delete profile: {e}"))?;
            deleted = true;
        }

        if mgs_path.exists() {
            std::fs::remove_file(&mgs_path)
                .map_err(|e| format!("Failed to delete encrypted profile: {e}"))?;
            deleted = true;
        }

        if deleted {
            log::info!("Profile deleted successfully: {}", name);
            Ok(())
        } else {
            log::warn!("Profile not found for deletion: {}", name);
            Err(format!("Profile '{name}' not found"))
        }
    }

    /// Check if a profile is encrypted
    /// Returns false for invalid profile names (fails safely)
    pub fn is_encrypted(&self, name: &str) -> bool {
        // Validate profile name to prevent path traversal attacks
        // For this method, we return false for invalid names (fail safely)
        if validate_profile_name(name).is_err() {
            return false;
        }

        let mgs_path = self.profiles_dir.join(format!("{name}.mgs"));
        mgs_path.exists()
    }

    /// Check if the RTMP input port conflicts with any existing profile
    /// Returns Ok(()) if no conflict, or Err with conflicting profile name
    pub async fn validate_input_conflict(
        &self,
        profile_id: &str,
        input: &RtmpInput,
    ) -> Result<(), String> {
        let profile_names = self.get_all_names().await?;

        for name in profile_names {
            // Load each profile (try without password for unencrypted ones)
            if let Ok(existing) = self.load(&name, None).await {
                // Skip the profile being edited (same ID)
                if existing.id == profile_id {
                    continue;
                }

                // Check for port conflict on same bind address
                // Both "0.0.0.0" and specific IPs should be checked
                let bind_conflict = existing.input.bind_address == input.bind_address
                    || existing.input.bind_address == "0.0.0.0"
                    || input.bind_address == "0.0.0.0";

                if bind_conflict && existing.input.port == input.port {
                    return Err(format!(
                        "Port {} is already configured for profile '{}'. Only one profile can listen on a port at a time.",
                        input.port,
                        existing.name
                    ));
                }
            }
            // Skip encrypted profiles we can't read (they might conflict but we can't check)
        }

        Ok(())
    }

    /// Encrypt all stream keys in a profile
    fn encrypt_stream_keys(&self, profile: &mut Profile) -> Result<(), String> {
        for group in &mut profile.output_groups {
            for target in &mut group.stream_targets {
                // Skip if already encrypted or empty
                if !target.stream_key.is_empty() && !Encryption::is_stream_key_encrypted(&target.stream_key) {
                    target.stream_key = Encryption::encrypt_stream_key(&target.stream_key, &self.app_data_dir)?;
                }
            }
        }
        Ok(())
    }

    /// Decrypt all stream keys in a profile
    fn decrypt_stream_keys(&self, profile: &mut Profile) -> Result<(), String> {
        for group in &mut profile.output_groups {
            for target in &mut group.stream_targets {
                // Only decrypt if encrypted
                if Encryption::is_stream_key_encrypted(&target.stream_key) {
                    target.stream_key = Encryption::decrypt_stream_key(&target.stream_key, &self.app_data_dir)?;
                }
            }
        }
        Ok(())
    }

    /// Save a profile with optional stream key encryption
    /// encrypt_keys: Whether to encrypt individual stream keys (based on settings)
    pub async fn save_with_key_encryption(
        &self,
        profile: &Profile,
        password: Option<&str>,
        encrypt_keys: bool,
    ) -> Result<(), String> {
        log::info!("Saving profile: {} (encrypted: {}, stream keys encrypted: {})",
            profile.name,
            password.is_some(),
            encrypt_keys
        );

        // Validate profile name to prevent path traversal attacks
        validate_profile_name(&profile.name)?;

        // Clone the profile so we can modify it
        let mut profile_to_save = profile.clone();

        // Encrypt stream keys if the setting is enabled
        if encrypt_keys {
            self.encrypt_stream_keys(&mut profile_to_save)?;
        }

        // Serialize to JSON
        let content = serde_json::to_string_pretty(&profile_to_save)
            .map_err(|e| format!("Failed to serialize profile: {e}"))?;

        if let Some(pwd) = password {
            // Encrypt and save as .mgs file
            let encrypted = Encryption::encrypt(content.as_bytes(), pwd)?;

            // Prepend magic bytes
            let mut data = Vec::with_capacity(ENCRYPTED_MAGIC.len() + encrypted.len());
            data.extend_from_slice(ENCRYPTED_MAGIC);
            data.extend_from_slice(&encrypted);

            let path = self.profiles_dir.join(format!("{}.mgs", profile.name));
            std::fs::write(&path, data)
                .map_err(|e| format!("Failed to write encrypted profile: {e}"))?;

            // Remove unencrypted version if it exists
            let json_path = self.profiles_dir.join(format!("{}.json", profile.name));
            if json_path.exists() {
                std::fs::remove_file(&json_path).ok();
            }
        } else {
            // Save as unencrypted JSON
            let path = self.profiles_dir.join(format!("{}.json", profile.name));
            std::fs::write(&path, content)
                .map_err(|e| format!("Failed to write profile: {e}"))?;

            // Remove encrypted version if it exists
            let mgs_path = self.profiles_dir.join(format!("{}.mgs", profile.name));
            if mgs_path.exists() {
                std::fs::remove_file(&mgs_path).ok();
            }
        }

        log::info!("Profile saved successfully: {}", profile.name);
        Ok(())
    }

    /// Load a profile and always decrypt stream keys (if they were encrypted)
    pub async fn load_with_key_decryption(&self, name: &str, password: Option<&str>) -> Result<Profile, String> {
        log::info!("Loading profile: {}", name);
        let mut profile = self.load(name, password).await?;

        // Always try to decrypt stream keys (they'll be returned as-is if not encrypted)
        self.decrypt_stream_keys(&mut profile)?;

        log::info!("Profile loaded successfully: {} ({} output groups, {} total targets)",
            name,
            profile.output_groups.len(),
            profile.output_groups.iter().map(|g| g.stream_targets.len()).sum::<usize>()
        );
        Ok(profile)
    }
}
