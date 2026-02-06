// Source Lifecycle Manager
// Tracks which sources need active capture based on current scene state.
// Computes diffs when scenes change to determine which sources to start/stop.
// Supports Studio Mode (preview + program scenes both keep sources alive).

use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use crate::models::{Scene, SourceLayer};

/// Manages source lifecycle based on scene activation state.
/// Sources are reference-counted: a source stays active as long as at least
/// one active scene (program, preview, or multiview) uses it.
pub struct SourceLifecycleManager {
    /// source_id → reference count (how many active contexts use it)
    active_refs: RwLock<HashMap<String, usize>>,
}

/// Result of computing a scene transition diff.
#[derive(Debug, Clone)]
pub struct SceneDiff {
    /// Source IDs that need to be started (newly required)
    pub to_start: Vec<String>,
    /// Source IDs that can be stopped (no longer needed by any active context)
    pub to_stop: Vec<String>,
}

impl SourceLifecycleManager {
    pub fn new() -> Self {
        Self {
            active_refs: RwLock::new(HashMap::new()),
        }
    }

    /// Compute which sources to start/stop when switching scenes.
    /// Takes the old scene (if any), new scene, and optional preview scene (Studio Mode).
    /// Returns the diff of sources to start and stop.
    ///
    /// This method updates internal ref counts atomically.
    pub fn compute_scene_diff(
        &self,
        old_scene: Option<&Scene>,
        new_scene: &Scene,
        preview_scene: Option<&Scene>,
    ) -> SceneDiff {
        // Collect source IDs from old context (what was active before)
        let old_source_ids = Self::collect_active_sources(old_scene, preview_scene);

        // Collect source IDs for new context (what should be active now)
        let new_source_ids = Self::collect_active_sources(Some(new_scene), preview_scene);

        // Compute diff
        let to_start: Vec<String> = new_source_ids
            .difference(&old_source_ids)
            .cloned()
            .collect();

        let to_stop: Vec<String> = old_source_ids
            .difference(&new_source_ids)
            .cloned()
            .collect();

        // Update ref counts
        {
            let mut refs = self.active_refs.write().unwrap();

            // Decrement refs for stopped sources
            for source_id in &to_stop {
                if let Some(count) = refs.get_mut(source_id) {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        refs.remove(source_id);
                    }
                }
            }

            // Increment refs for started sources
            for source_id in &to_start {
                *refs.entry(source_id.clone()).or_insert(0) += 1;
            }
        }

        SceneDiff { to_start, to_stop }
    }

    /// Update for Studio Mode preview scene change.
    /// Returns diff relative to the previous preview scene.
    pub fn compute_preview_diff(
        &self,
        old_preview: Option<&Scene>,
        new_preview: Option<&Scene>,
        program_scene: Option<&Scene>,
    ) -> SceneDiff {
        // Old context: program + old_preview
        let old_source_ids = Self::collect_active_sources(program_scene, old_preview);

        // New context: program + new_preview
        let new_source_ids = Self::collect_active_sources(program_scene, new_preview);

        let to_start: Vec<String> = new_source_ids
            .difference(&old_source_ids)
            .cloned()
            .collect();

        let to_stop: Vec<String> = old_source_ids
            .difference(&new_source_ids)
            .cloned()
            .collect();

        // Update ref counts
        {
            let mut refs = self.active_refs.write().unwrap();
            for source_id in &to_stop {
                if let Some(count) = refs.get_mut(source_id) {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        refs.remove(source_id);
                    }
                }
            }
            for source_id in &to_start {
                *refs.entry(source_id.clone()).or_insert(0) += 1;
            }
        }

        SceneDiff { to_start, to_stop }
    }

    /// Register all sources from a multiview context.
    /// Returns the source IDs that are newly started (weren't already active).
    pub fn activate_multiview_scenes(&self, scenes: &[Scene]) -> Vec<String> {
        let mut newly_started = Vec::new();
        let mut refs = self.active_refs.write().unwrap();

        for scene in scenes {
            for source_id in Self::visible_on_canvas_sources(scene) {
                let count = refs.entry(source_id.clone()).or_insert(0);
                if *count == 0 {
                    newly_started.push(source_id);
                }
                *count += 1;
            }
        }

        newly_started
    }

    /// Deactivate multiview scenes. Returns source IDs that should be stopped.
    pub fn deactivate_multiview_scenes(&self, scenes: &[Scene]) -> Vec<String> {
        let mut to_stop = Vec::new();
        let mut refs = self.active_refs.write().unwrap();

        for scene in scenes {
            for source_id in Self::visible_on_canvas_sources(scene) {
                if let Some(count) = refs.get_mut(&source_id) {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        refs.remove(&source_id);
                        to_stop.push(source_id);
                    }
                }
            }
        }

        to_stop
    }

    /// Check if a source is currently needed by any active context.
    pub fn is_source_active(&self, source_id: &str) -> bool {
        let refs = self.active_refs.read().unwrap();
        refs.get(source_id).copied().unwrap_or(0) > 0
    }

    /// Get current active source IDs and their ref counts.
    pub fn active_sources(&self) -> HashMap<String, usize> {
        self.active_refs.read().unwrap().clone()
    }

    /// Reset all tracking state (e.g., on profile change).
    pub fn reset(&self) {
        let mut refs = self.active_refs.write().unwrap();
        refs.clear();
    }

    /// Collect all unique source IDs that are visible and on-canvas
    /// from the given program and preview scenes.
    fn collect_active_sources(
        program: Option<&Scene>,
        preview: Option<&Scene>,
    ) -> HashSet<String> {
        let mut ids = HashSet::new();

        if let Some(scene) = program {
            for source_id in Self::visible_on_canvas_sources(scene) {
                ids.insert(source_id);
            }
        }

        if let Some(scene) = preview {
            for source_id in Self::visible_on_canvas_sources(scene) {
                ids.insert(source_id);
            }
        }

        ids
    }

    /// Get source IDs from visible layers whose bounding box overlaps the canvas.
    /// Skips off-canvas layers (AABB culling) to avoid starting captures for
    /// sources that the user can't see.
    fn visible_on_canvas_sources(scene: &Scene) -> Vec<String> {
        scene
            .layers
            .iter()
            .filter(|layer| is_on_canvas(layer, scene))
            .map(|layer| layer.source_id.clone())
            .collect()
    }
}

impl Default for SourceLifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

/// AABB check: is the layer visible and at least partially on the canvas?
fn is_on_canvas(layer: &SourceLayer, scene: &Scene) -> bool {
    if !layer.visible {
        return false;
    }
    let t = &layer.transform;
    // Layer right/bottom edges
    let right = t.x + t.width as i32;
    let bottom = t.y + t.height as i32;
    // Fully off-canvas in any direction?
    right > 0
        && t.x < scene.canvas_width as i32
        && bottom > 0
        && t.y < scene.canvas_height as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AudioMixer, Transform};

    fn make_scene(id: &str, layers: Vec<(&str, i32, i32, u32, u32)>) -> Scene {
        Scene {
            id: id.to_string(),
            name: id.to_string(),
            canvas_width: 1920,
            canvas_height: 1080,
            layers: layers
                .into_iter()
                .enumerate()
                .map(|(i, (source_id, x, y, w, h))| SourceLayer {
                    id: format!("layer_{}", i),
                    source_id: source_id.to_string(),
                    visible: true,
                    locked: false,
                    transform: Transform {
                        x,
                        y,
                        width: w,
                        height: h,
                        rotation: 0.0,
                        crop: None,
                    },
                    z_index: i as i32,
                })
                .collect(),
            audio_mixer: AudioMixer::default(),
            transition_in: None,
        }
    }

    #[test]
    fn test_basic_scene_switch() {
        let mgr = SourceLifecycleManager::new();

        let scene_a = make_scene("A", vec![("cam1", 0, 0, 1920, 1080)]);
        let scene_b = make_scene("B", vec![("cam2", 0, 0, 1920, 1080)]);

        // Switch from nothing to scene A
        let diff = mgr.compute_scene_diff(None, &scene_a, None);
        assert_eq!(diff.to_start, vec!["cam1"]);
        assert!(diff.to_stop.is_empty());

        // Switch from A to B
        let diff = mgr.compute_scene_diff(Some(&scene_a), &scene_b, None);
        assert_eq!(diff.to_start, vec!["cam2"]);
        assert_eq!(diff.to_stop, vec!["cam1"]);
    }

    #[test]
    fn test_shared_source_not_stopped() {
        let mgr = SourceLifecycleManager::new();

        let scene_a = make_scene("A", vec![
            ("cam1", 0, 0, 1920, 1080),
            ("overlay", 100, 100, 320, 240),
        ]);
        let scene_b = make_scene("B", vec![
            ("cam2", 0, 0, 1920, 1080),
            ("overlay", 100, 100, 320, 240),
        ]);

        // Start scene A
        let diff = mgr.compute_scene_diff(None, &scene_a, None);
        assert_eq!(diff.to_start.len(), 2);

        // Switch to B — overlay should NOT be stopped since it's in both
        let diff = mgr.compute_scene_diff(Some(&scene_a), &scene_b, None);
        assert!(diff.to_start.contains(&"cam2".to_string()));
        assert!(diff.to_stop.contains(&"cam1".to_string()));
        assert!(!diff.to_stop.contains(&"overlay".to_string()));
    }

    #[test]
    fn test_off_canvas_culled() {
        let mgr = SourceLifecycleManager::new();

        let scene = make_scene("A", vec![
            ("visible", 0, 0, 1920, 1080),
            ("off_right", 2000, 0, 320, 240),   // fully off right edge
            ("off_bottom", 0, 1200, 320, 240),   // fully off bottom edge
        ]);

        let diff = mgr.compute_scene_diff(None, &scene, None);
        assert_eq!(diff.to_start, vec!["visible"]);
    }

    #[test]
    fn test_studio_mode_preview() {
        let mgr = SourceLifecycleManager::new();

        let scene_cam1 = make_scene("scene_cam1", vec![("cam1", 0, 0, 1920, 1080)]);
        let scene_cam2 = make_scene("scene_cam2", vec![("cam2", 0, 0, 1920, 1080)]);

        // Step 1: Enable studio mode — set preview=cam2 first (no program yet)
        let diff = mgr.compute_preview_diff(None, Some(&scene_cam2), None);
        assert!(diff.to_start.contains(&"cam2".to_string()));

        // Step 2: Set program=cam1 (preview=cam2 stays)
        let diff = mgr.compute_scene_diff(None, &scene_cam1, Some(&scene_cam2));
        // cam2 already active from preview, only cam1 is new
        assert_eq!(diff.to_start, vec!["cam1".to_string()]);
        assert!(diff.to_stop.is_empty());

        // Step 3: TAKE — program becomes cam2, preview becomes cam1
        // Program switch: old=cam1, new=cam2 (preview stays cam2 during switch)
        let diff = mgr.compute_scene_diff(Some(&scene_cam1), &scene_cam2, Some(&scene_cam2));
        // Old: {cam1, cam2}, New: {cam2, cam2} — cam1 is no longer needed
        assert!(diff.to_stop.contains(&"cam1".to_string()));
        assert!(diff.to_start.is_empty());

        // Preview switch: old=cam2, new=cam1 (program is now cam2)
        let diff = mgr.compute_preview_diff(Some(&scene_cam2), Some(&scene_cam1), Some(&scene_cam2));
        // Old: {cam2, cam2} -> New: {cam2, cam1}
        assert!(diff.to_start.contains(&"cam1".to_string()));
        assert!(diff.to_stop.is_empty()); // cam2 still needed by program
    }

    #[test]
    fn test_is_source_active() {
        let mgr = SourceLifecycleManager::new();
        let scene = make_scene("A", vec![("cam1", 0, 0, 1920, 1080)]);

        assert!(!mgr.is_source_active("cam1"));

        mgr.compute_scene_diff(None, &scene, None);
        assert!(mgr.is_source_active("cam1"));
    }
}
