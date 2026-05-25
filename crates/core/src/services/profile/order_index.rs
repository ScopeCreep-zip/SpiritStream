//! Order-index file I/O for the profile picker drag-and-drop reorder.
//! Persists to `<app_data_dir>/indexes/order_indexes.json`.

use crate::errors::CoreError;
use std::collections::hash_map::Entry;
use std::collections::HashMap;

impl super::ProfileManager {
    /// Read the order-index map for drag-and-drop on the profile picker.
    pub fn read_order_index_map(&self) -> Result<HashMap<String, i32>, CoreError> {
        let indexes_path = self.order_index_dir.join("order_indexes.json");

        if !indexes_path.exists() {
            let empty: HashMap<String, i32> = HashMap::new();
            let content = serde_json::to_string_pretty(&empty)?;
            std::fs::write(&indexes_path, content)?;
            return Ok(empty);
        }

        let content = std::fs::read_to_string(&indexes_path)?;
        let map: HashMap<String, i32> = serde_json::from_str(&content)?;
        Ok(map)
    }

    pub fn write_order_index_map(&self, map: &HashMap<String, i32>) -> Result<(), CoreError> {
        let path = self.order_index_dir.join("order_indexes.json");
        let tmp = self.order_index_dir.join("order_indexes.json.tmp");

        let content = serde_json::to_string_pretty(map)?;
        std::fs::write(&tmp, content)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
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
}
