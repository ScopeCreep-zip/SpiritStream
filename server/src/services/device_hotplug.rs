// Device Hotplug Monitor
// Detects audio device additions/removals and emits events.
// Uses cpal device enumeration polling (cross-platform).
//
// On device change: invalidates DeviceCache and emits "device_change" event
// so the frontend can refresh its device lists.
//
// Reconnection support: tracks recently disconnected devices. When a device
// reappears, emits "device_reconnected" so the frontend can auto-restart capture.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait};

use crate::app_state::EventBus;
use crate::services::events::EventSink;
use crate::services::device_discovery::DeviceCache;

/// Polling interval for device changes
const POLL_INTERVAL: Duration = Duration::from_secs(3);

pub struct DeviceHotplugMonitor {
    running: Arc<AtomicBool>,
    handle: std::sync::Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl DeviceHotplugMonitor {
    /// Start monitoring for device changes.
    /// Spawns a background thread that polls cpal device lists.
    pub fn start(event_bus: EventBus, device_cache: Arc<DeviceCache>) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let handle = std::thread::Builder::new()
            .name("ss-device-hotplug".into())
            .spawn(move || {
                log::info!("[DeviceHotplug] Monitor started (polling every {}s)", POLL_INTERVAL.as_secs());

                let host = cpal::default_host();
                let mut known_inputs = enumerate_device_ids(&host, true);
                let mut known_outputs = enumerate_device_ids(&host, false);
                // Track recently disconnected input devices for auto-reconnection
                let mut recently_disconnected: HashSet<String> = HashSet::new();

                while running_clone.load(Ordering::Relaxed) {
                    std::thread::sleep(POLL_INTERVAL);

                    if !running_clone.load(Ordering::Relaxed) {
                        break;
                    }

                    let current_inputs = enumerate_device_ids(&host, true);
                    let current_outputs = enumerate_device_ids(&host, false);

                    let inputs_changed = current_inputs != known_inputs;
                    let outputs_changed = current_outputs != known_outputs;

                    if inputs_changed || outputs_changed {
                        // Compute added/removed
                        let added_inputs: Vec<_> = current_inputs.difference(&known_inputs).cloned().collect();
                        let removed_inputs: Vec<_> = known_inputs.difference(&current_inputs).cloned().collect();
                        let added_outputs: Vec<_> = current_outputs.difference(&known_outputs).collect();
                        let removed_outputs: Vec<_> = known_outputs.difference(&current_outputs).collect();

                        log::info!(
                            "[DeviceHotplug] Device change detected: inputs(+{} -{}) outputs(+{} -{})",
                            added_inputs.len(),
                            removed_inputs.len(),
                            added_outputs.len(),
                            removed_outputs.len()
                        );

                        // Emit per-device disconnect events for removed inputs
                        for device_name in &removed_inputs {
                            recently_disconnected.insert(device_name.clone());
                            event_bus.emit(
                                "device_disconnected",
                                serde_json::json!({
                                    "deviceName": device_name,
                                    "type": "input",
                                }),
                            );
                            log::info!("[DeviceHotplug] Input device disconnected: '{}'", device_name);
                        }

                        // Check if any recently disconnected device has reappeared
                        for device_name in &added_inputs {
                            if recently_disconnected.remove(device_name) {
                                event_bus.emit(
                                    "device_reconnected",
                                    serde_json::json!({
                                        "deviceName": device_name,
                                        "type": "input",
                                    }),
                                );
                                log::info!("[DeviceHotplug] Input device reconnected: '{}'", device_name);
                            }
                        }

                        // Invalidate cache (uses tokio runtime from thread)
                        let cache = device_cache.clone();
                        let _ = std::thread::spawn(move || {
                            let rt = tokio::runtime::Builder::new_current_thread()
                                .enable_all()
                                .build();
                            if let Ok(rt) = rt {
                                rt.block_on(cache.invalidate());
                            }
                        }).join();

                        event_bus.emit(
                            "device_change",
                            serde_json::json!({
                                "type": if inputs_changed && outputs_changed { "both" }
                                        else if inputs_changed { "input" }
                                        else { "output" },
                                "addedInputs": added_inputs.len(),
                                "removedInputs": removed_inputs.len(),
                                "addedOutputs": added_outputs.len(),
                                "removedOutputs": removed_outputs.len(),
                            }),
                        );

                        known_inputs = current_inputs;
                        known_outputs = current_outputs;
                    }
                }

                log::info!("[DeviceHotplug] Monitor stopped");
            })
            .expect("Failed to spawn device hotplug thread");

        Self {
            running,
            handle: std::sync::Mutex::new(Some(handle)),
        }
    }

    /// Stop the hotplug monitor.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        if let Ok(mut h) = self.handle.lock() {
            if let Some(handle) = h.take() {
                let _ = handle.join();
            }
        }
    }
}

impl Drop for DeviceHotplugMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Enumerate device names/IDs into a HashSet for change detection.
fn enumerate_device_ids(host: &cpal::Host, input: bool) -> HashSet<String> {
    let devices = if input {
        host.input_devices()
    } else {
        host.output_devices()
    };

    devices
        .map(|iter| {
            iter.filter_map(|d| {
                d.description()
                    .ok()
                    .map(|desc| desc.name().to_string())
            })
            .collect()
        })
        .unwrap_or_default()
}
