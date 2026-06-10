//! Order-index file I/O for the profile picker drag-and-drop reorder.
//! Persists to `<app_data_dir>/indexes/order_indexes.json`.

use crate::errors::CoreError;
use crate::services::write_owner_only_atomic;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

impl super::ProfileManager {
    /// Read the order-index map for drag-and-drop on the profile picker.
    pub fn read_order_index_map(&self) -> Result<HashMap<String, i32>, CoreError> {
        let indexes_path = self.order_index_dir.join("order_indexes.json");

        if !indexes_path.exists() {
            let empty: HashMap<String, i32> = HashMap::new();
            let content = serde_json::to_string_pretty(&empty)?;
            // I5: even the initial-write path is owner-only atomic so a
            // crash here can never leave a world-readable partial file.
            write_owner_only_atomic(&indexes_path, content.as_bytes())?;
            return Ok(empty);
        }

        let content = std::fs::read_to_string(&indexes_path)?;
        let map: HashMap<String, i32> = serde_json::from_str(&content)?;
        Ok(map)
    }

    /// I5: route through `write_owner_only_atomic` so a partial write
    /// can't leave a 0644 `order_indexes.json.tmp` on disk, and the
    /// final file always lands at 0600 on Unix.
    pub fn write_order_index_map(&self, map: &HashMap<String, i32>) -> Result<(), CoreError> {
        let path = self.order_index_dir.join("order_indexes.json");
        let content = serde_json::to_string_pretty(map)?;
        write_owner_only_atomic(&path, content.as_bytes())
    }

    /// This method can eventually be removed; its purpose is to add
    /// `order_index` to profiles that were created before order_index was
    /// introduced.
    pub async fn ensure_order_indexes(&self) -> Result<HashMap<String, i32>, CoreError> {
        let names = self.get_all_names().await?;
        let mut map = self.read_order_index_map()?;

        let mut max = map.values().copied().max().unwrap_or(0);
        max = ((max + 9) / 10) * 10;

        let mut changed = false;

        // I7: pre-build a set so the `names.contains(k)` lookup below
        // is O(1) and the loop is O(n) instead of O(n²). Also iterate
        // by reference rather than `names.clone()`-ing the entire
        // vector on every call.
        let name_set: HashSet<&str> = names.iter().map(String::as_str).collect();

        for name in &names {
            match map.entry(name.clone()) {
                Entry::Vacant(e) => {
                    max += 10;
                    e.insert(max);
                    changed = true;
                }
                Entry::Occupied(_) => {}
            }
        }
        map.retain(|k, _| name_set.contains(k.as_str()));
        if changed {
            self.write_order_index_map(&map)?;
        }

        Ok(map)
    }
}

#[cfg(test)]
mod tests {
    use super::super::ProfileManager;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn manager() -> (ProfileManager, TempDir) {
        let dir = TempDir::new().unwrap();
        (ProfileManager::new(dir.path().to_path_buf()), dir)
    }

    #[test]
    fn read_on_missing_file_creates_an_empty_owner_only_map() {
        let (mgr, dir) = manager();
        let map = mgr.read_order_index_map().unwrap();
        assert!(map.is_empty());

        let path = dir.path().join("indexes").join("order_indexes.json");
        assert!(path.exists(), "first read must materialise the file");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "order index file must be owner-only");
        }
    }

    #[test]
    fn write_then_read_round_trips_the_map() {
        let (mgr, _dir) = manager();
        let mut map = HashMap::new();
        map.insert("alpha".to_string(), 10);
        map.insert("beta".to_string(), 20);
        mgr.write_order_index_map(&map).unwrap();

        let read_back = mgr.read_order_index_map().unwrap();
        assert_eq!(read_back, map);
    }

    #[tokio::test]
    async fn ensure_assigns_increasing_multiples_of_ten_to_unindexed_profiles() {
        let (mgr, _dir) = manager();
        for (i, name) in ["one", "two", "three"].iter().enumerate() {
            save_named(&mgr, name, 1935 + i as u16).await;
        }

        let map = mgr.ensure_order_indexes().await.unwrap();
        assert_eq!(map.len(), 3);
        let mut values: Vec<i32> = map.values().copied().collect();
        values.sort_unstable();
        assert_eq!(values, vec![10, 20, 30]);
    }

    #[tokio::test]
    async fn ensure_preserves_existing_indexes_and_only_fills_gaps() {
        let (mgr, _dir) = manager();
        save_named(&mgr, "kept", 1935).await;
        save_named(&mgr, "added", 1936).await;

        let mut seed = HashMap::new();
        seed.insert("kept".to_string(), 100);
        mgr.write_order_index_map(&seed).unwrap();

        let map = mgr.ensure_order_indexes().await.unwrap();
        assert_eq!(map.get("kept"), Some(&100), "existing index untouched");
        // New profile is placed above the current max, rounded to the next ten.
        assert_eq!(map.get("added"), Some(&110));
    }

    #[tokio::test]
    async fn ensure_drops_indexes_for_profiles_that_no_longer_exist() {
        let (mgr, _dir) = manager();
        save_named(&mgr, "survivor", 1935).await;

        let mut seed = HashMap::new();
        seed.insert("survivor".to_string(), 10);
        seed.insert("ghost".to_string(), 20);
        mgr.write_order_index_map(&seed).unwrap();

        let map = mgr.ensure_order_indexes().await.unwrap();
        assert!(map.contains_key("survivor"));
        assert!(!map.contains_key("ghost"), "stale entry must be pruned");
    }

    async fn save_named(mgr: &ProfileManager, name: &str, port: u16) {
        use crate::models::{Profile, ProfileSettings, RtmpInput};
        let profile = Profile {
            id: name.into(),
            name: name.into(),
            encrypted: false,
            input: RtmpInput {
                input_type: "rtmp".into(),
                bind_address: "127.0.0.1".into(),
                port,
                application: "live".into(),
            },
            output_groups: vec![],
            settings: ProfileSettings::default(),
            pii_blocklist: vec![],
            pii_fuzzy: false,
            anonymous_logging: true,
            anonymous_salt: String::new(),
        };
        mgr.save_with_key_encryption(&profile, None)
            .await
            .expect("save profile");
    }
}
