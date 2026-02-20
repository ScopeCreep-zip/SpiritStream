// Source Lifecycle Service
// Tracks active sources with two-tier counting (OBS active/showing pattern):
// - ref_count: source is in an active scene (audio continues)
// - showing_count: source is visible on canvas (video capture needed)
//
// When showing_count transitions 0→1, video capture starts.
// When showing_count transitions 1→0, video capture stops (audio continues).

use std::collections::HashMap;
use parking_lot::Mutex;

/// Tracks reference counts for a single source
#[derive(Debug, Clone)]
pub struct SourceRefState {
    /// Number of active references (source in scene, regardless of visibility)
    pub ref_count: u32,
    /// Number of visible references (source actually showing on canvas)
    pub showing_count: u32,
}

impl SourceRefState {
    fn new() -> Self {
        Self {
            ref_count: 0,
            showing_count: 0,
        }
    }

    /// Whether the source needs any capture pipeline (audio at minimum)
    pub fn is_active(&self) -> bool {
        self.ref_count > 0
    }

    /// Whether the source needs video capture (visible on canvas)
    pub fn is_showing(&self) -> bool {
        self.showing_count > 0
    }
}

/// Transition events emitted by the lifecycle service
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceTransition {
    /// Source became active (ref_count 0→1) — start audio capture
    Activated,
    /// Source became inactive (ref_count 1→0) — stop all capture
    Deactivated,
    /// Source became visible (showing_count 0→1) — start video capture
    Shown,
    /// Source became hidden (showing_count 1→0) — stop video capture, keep audio
    Hidden,
    /// No transition occurred
    None,
}

/// Service for tracking source lifecycle across program, preview, and multiview contexts
pub struct SourceLifecycleService {
    sources: Mutex<HashMap<String, SourceRefState>>,
}

impl SourceLifecycleService {
    pub fn new() -> Self {
        Self {
            sources: Mutex::new(HashMap::new()),
        }
    }

    /// Add a reference to a source (it's in an active scene).
    /// If `visible` is true, also increments showing_count.
    /// Returns the transition that occurred.
    pub fn add_ref(&self, source_id: &str, visible: bool) -> SourceTransition {
        let mut sources = self.sources.lock();
        let state = sources.entry(source_id.to_string()).or_insert_with(SourceRefState::new);

        let was_active = state.is_active();
        let was_showing = state.is_showing();

        state.ref_count += 1;
        if visible {
            state.showing_count += 1;
        }

        if !was_active && state.is_active() {
            if state.is_showing() {
                SourceTransition::Activated // Implies also shown
            } else {
                SourceTransition::Activated
            }
        } else if !was_showing && state.is_showing() {
            SourceTransition::Shown
        } else {
            SourceTransition::None
        }
    }

    /// Remove a reference to a source.
    /// If `visible` is true, also decrements showing_count.
    /// Returns the transition that occurred.
    pub fn remove_ref(&self, source_id: &str, visible: bool) -> SourceTransition {
        let mut sources = self.sources.lock();

        if let Some(state) = sources.get_mut(source_id) {
            let was_active = state.is_active();
            let was_showing = state.is_showing();

            state.ref_count = state.ref_count.saturating_sub(1);
            if visible {
                state.showing_count = state.showing_count.saturating_sub(1);
            }

            // Clean up entry if fully dereferenced
            if !state.is_active() {
                sources.remove(source_id);
                if was_active {
                    return SourceTransition::Deactivated;
                }
            } else if was_showing && !state.is_showing() {
                return SourceTransition::Hidden;
            }
        }

        SourceTransition::None
    }

    /// Update visibility of a source (eye toggle).
    /// Returns the transition that occurred.
    pub fn set_visibility(&self, source_id: &str, visible: bool) -> SourceTransition {
        let mut sources = self.sources.lock();

        if let Some(state) = sources.get_mut(source_id) {
            let was_showing = state.is_showing();

            if visible && !was_showing {
                // Hidden → Visible: start video capture
                state.showing_count += 1;
                return SourceTransition::Shown;
            } else if !visible && was_showing {
                // Visible → Hidden: stop video capture
                state.showing_count = state.showing_count.saturating_sub(1);
                if !state.is_showing() {
                    return SourceTransition::Hidden;
                }
            }
        }

        SourceTransition::None
    }

    /// Get the current state of a source
    pub fn get_state(&self, source_id: &str) -> Option<SourceRefState> {
        self.sources.lock().get(source_id).cloned()
    }

    /// Get all active source IDs
    pub fn active_sources(&self) -> Vec<String> {
        self.sources.lock()
            .iter()
            .filter(|(_, state)| state.is_active())
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Get all showing source IDs
    pub fn showing_sources(&self) -> Vec<String> {
        self.sources.lock()
            .iter()
            .filter(|(_, state)| state.is_showing())
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Clear all tracking state
    pub fn clear(&self) {
        self.sources.lock().clear();
    }
}

impl Default for SourceLifecycleService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_visible_ref() {
        let svc = SourceLifecycleService::new();
        let t = svc.add_ref("src1", true);
        assert_eq!(t, SourceTransition::Activated);

        let state = svc.get_state("src1").unwrap();
        assert_eq!(state.ref_count, 1);
        assert_eq!(state.showing_count, 1);
    }

    #[test]
    fn test_add_hidden_ref() {
        let svc = SourceLifecycleService::new();
        let t = svc.add_ref("src1", false);
        assert_eq!(t, SourceTransition::Activated);

        let state = svc.get_state("src1").unwrap();
        assert_eq!(state.ref_count, 1);
        assert_eq!(state.showing_count, 0);
        assert!(!state.is_showing());
    }

    #[test]
    fn test_visibility_toggle() {
        let svc = SourceLifecycleService::new();
        svc.add_ref("src1", true);

        let t = svc.set_visibility("src1", false);
        assert_eq!(t, SourceTransition::Hidden);
        assert!(!svc.get_state("src1").unwrap().is_showing());

        let t = svc.set_visibility("src1", true);
        assert_eq!(t, SourceTransition::Shown);
        assert!(svc.get_state("src1").unwrap().is_showing());
    }

    #[test]
    fn test_remove_ref_deactivates() {
        let svc = SourceLifecycleService::new();
        svc.add_ref("src1", true);
        let t = svc.remove_ref("src1", true);
        assert_eq!(t, SourceTransition::Deactivated);
        assert!(svc.get_state("src1").is_none());
    }

    #[test]
    fn test_multiple_refs() {
        let svc = SourceLifecycleService::new();
        svc.add_ref("src1", true);  // program scene
        svc.add_ref("src1", true);  // preview scene

        let state = svc.get_state("src1").unwrap();
        assert_eq!(state.ref_count, 2);
        assert_eq!(state.showing_count, 2);

        svc.remove_ref("src1", true);  // remove from preview
        let state = svc.get_state("src1").unwrap();
        assert_eq!(state.ref_count, 1);
        assert!(state.is_active());
        assert!(state.is_showing());
    }
}
