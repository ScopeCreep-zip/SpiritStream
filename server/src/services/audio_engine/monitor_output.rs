// Monitor Output
// Opens a cpal output stream on a user-selected device for audio monitoring.
// Sources with MonitorOnly are excluded from the main track outputs.
// Sources with MonitorAndOutput go to both monitoring and tracks.
//
// The mixer thread writes the monitor mix into a ring buffer;
// the cpal output callback reads from it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;

use crate::models::MonitoringType;

/// Monitor output state.
/// Reads from a ring buffer fed by the mixer thread and plays through a cpal output stream.
pub struct MonitorOutput {
    /// The cpal output stream (kept alive while monitoring is active)
    stream: Mutex<Option<cpal::Stream>>,
    /// Device ID currently used for monitoring
    device_id: Mutex<Option<String>>,
    /// Whether monitoring is active
    active: AtomicBool,
}

impl MonitorOutput {
    pub fn new() -> Self {
        Self {
            stream: Mutex::new(None),
            device_id: Mutex::new(None),
            active: AtomicBool::new(false),
        }
    }

    /// Start monitoring on the given output device.
    /// Returns a `rtrb::Producer<f32>` that the mixer thread should write monitor samples into.
    pub fn start(
        &self,
        device_id: Option<&str>,
        sample_rate: u32,
        channels: u16,
        buffer_size: usize,
    ) -> Result<rtrb::Producer<f32>, String> {
        // Stop any existing stream
        self.stop();

        let host = cpal::default_host();

        // Find the output device
        let device = if let Some(id) = device_id {
            find_output_device(&host, id)?
        } else {
            host.default_output_device()
                .ok_or_else(|| "No default output device available".to_string())?
        };

        let device_name = device
            .description()
            .map(|d| d.name().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        // Create ring buffer for monitor mix
        let capacity = buffer_size * channels as usize * 4; // ~4 ticks of headroom
        let (producer, consumer) = rtrb::RingBuffer::new(capacity);

        let consumer_arc = Arc::new(Mutex::new(Some(consumer)));
        let consumer_for_callback = consumer_arc.clone();

        // Build output stream config
        let stream_config = cpal::StreamConfig {
            channels,
            sample_rate,
            buffer_size: cpal::BufferSize::Default,
        };

        let stream = device
            .build_output_stream(
                &stream_config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let mut guard = consumer_for_callback.lock();
                    if let Some(consumer) = guard.as_mut() {
                        let available = consumer.slots();
                        let to_read = available.min(data.len());

                        if to_read > 0 {
                            if let Ok(chunk) = consumer.read_chunk(to_read) {
                                let slices = chunk.as_slices();
                                let first_len = slices.0.len();
                                data[..first_len].copy_from_slice(slices.0);
                                if !slices.1.is_empty() {
                                    data[first_len..first_len + slices.1.len()]
                                        .copy_from_slice(slices.1);
                                }
                                chunk.commit_all();

                                // Zero remaining samples if buffer underrun
                                for s in &mut data[to_read..] {
                                    *s = 0.0;
                                }
                            } else {
                                data.fill(0.0);
                            }
                        } else {
                            // No data available — output silence
                            data.fill(0.0);
                        }
                    } else {
                        data.fill(0.0);
                    }
                },
                |err| {
                    log::error!("[MonitorOutput] Stream error: {}", err);
                },
                None,
            )
            .map_err(|e| format!("Failed to build output stream: {}", e))?;

        stream
            .play()
            .map_err(|e| format!("Failed to start output stream: {}", e))?;

        *self.stream.lock() = Some(stream);
        *self.device_id.lock() = Some(device_name.clone());
        self.active.store(true, Ordering::Relaxed);

        log::info!(
            "[MonitorOutput] Started on '{}' ({}Hz, {}ch)",
            device_name,
            sample_rate,
            channels
        );

        Ok(producer)
    }

    /// Stop monitoring output.
    pub fn stop(&self) {
        if self.active.swap(false, Ordering::Relaxed) {
            // Drop the stream (stops playback)
            *self.stream.lock() = None;

            let device = self.device_id.lock().take();
            log::info!(
                "[MonitorOutput] Stopped (was: {})",
                device.unwrap_or_else(|| "none".to_string())
            );
        }
    }

    /// Whether monitoring is currently active
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    /// Get the current monitoring device ID
    pub fn current_device(&self) -> Option<String> {
        self.device_id.lock().clone()
    }
}

impl Drop for MonitorOutput {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Check if a source should be included in the monitor mix based on its monitoring type.
pub fn should_monitor(monitoring_type: &MonitoringType) -> bool {
    matches!(
        monitoring_type,
        MonitoringType::MonitorOnly | MonitoringType::MonitorAndOutput
    )
}

/// Check if a source should be included in the main track outputs.
pub fn should_output(monitoring_type: &MonitoringType) -> bool {
    matches!(
        monitoring_type,
        MonitoringType::None | MonitoringType::MonitorAndOutput
    )
}

fn find_output_device(host: &cpal::Host, device_id: &str) -> Result<cpal::Device, String> {
    let devices = host
        .output_devices()
        .map_err(|e| format!("Failed to enumerate output devices: {}", e))?;

    let device_id_lower = device_id.to_lowercase();

    for device in devices {
        if let Ok(desc) = device.description() {
            let name = desc.name().to_string();
            if name.to_lowercase() == device_id_lower || name.to_lowercase().contains(&device_id_lower)
            {
                return Ok(device);
            }
        }
        if let Ok(id) = device.id() {
            if id.to_string() == device_id {
                return Ok(device);
            }
        }
    }

    Err(format!("Output device not found: {}", device_id))
}
