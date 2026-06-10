use std::sync::Arc;

use crate::services::{EventSink, FFmpegHandler, ProfileManager, SettingsManager};

use super::types::ObsStreamStatus;

/// OBS→SpiritStream cascade delay before starting the relay. Mirrors
/// the 2-second stabilization window the frontend used to apply
/// client-side; centralising it here so any client (Tauri / Docker /
/// CLI) gets the same behaviour without re-implementing the delay.
const OBS_TRIGGER_DELAY_MS: u64 = 2000;

/// Server-side cascade dependencies. Set once at startup by
/// `ServiceRegistry::build` after every service is constructed. When
/// present, `start_event_listener` runs the OBS→SpiritStream trigger
/// cascade in core (read active profile → check direction → call
/// `FFmpegHandler::start_all`) instead of relying on a frontend
/// observer to do it. Frontend keeps only display state.
#[derive(Clone)]
pub struct ObsCascadeDeps {
    pub profiles: Arc<ProfileManager>,
    pub settings: Arc<SettingsManager>,
    pub ffmpeg: Arc<FFmpegHandler>,
}

/// Wrap any `EventSink + Clone` into an `Arc<dyn EventSink>` so it can
/// flow into `FFmpegHandler::start_all`, which takes a trait-object
/// sink. The wrapper itself implements `EventSink` by delegating to
/// the inner clone.
struct EventSinkClone<E: EventSink + Send + Sync + 'static>(E);

impl<E: EventSink + Send + Sync + 'static> EventSink for EventSinkClone<E> {
    fn emit(&self, event: &str, payload: serde_json::Value) {
        self.0.emit(event, payload);
    }
}

impl super::ObsWebSocketHandler {
    /// Start listening for OBS events via polling.
    ///
    /// When `cascade_deps` is set, the listener runs the
    /// OBS→SpiritStream trigger cascade in core on every active↔inactive
    /// transition (read direction from active profile → optionally
    /// call `FFmpegHandler::start_all` / `stop_all`). The frontend
    /// only sees informational `obs://stream_state` events and an
    /// optional `stream_started_by_obs` / `stream_stopped_by_obs` —
    /// it never decides whether to start streaming.
    pub(super) async fn start_event_listener<E: EventSink + Send + Sync + Clone + 'static>(
        &self,
        event_sink: E,
    ) {
        let state = self.state.clone();
        let client = self.client.clone();
        let triggered_by_us = self.triggered_by_us.clone();
        let cascade_deps = self.cascade_deps.clone();
        let cascade_start_handle = self.cascade_start_handle.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        // I1: if a previous poll task is already running (reconnect
        // flow), abort it before spawning the replacement. The aborted
        // task observes a CancelError immediately so the old poll loop
        // doesn't continue racing the fresh client.
        if let Some(prev) = self.listener_handle.lock().await.take() {
            prev.abort();
        }

        let handle = tokio::spawn(async move {
            loop {
                if shutdown_rx.try_recv().is_ok() {
                    log::debug!("OBS event listener shutting down");
                    break;
                }

                let client_guard = client.read().await;
                if let Some(ref obs_client) = *client_guard {
                    let sink = event_sink.clone();

                    match obs_client.streaming().status().await {
                        Ok(stream_status) => {
                            let new_status = if stream_status.active {
                                ObsStreamStatus::Active
                            } else {
                                ObsStreamStatus::Inactive
                            };

                            let mut state_guard = state.write().await;
                            if state_guard.stream_status != new_status {
                                state_guard.stream_status = new_status;
                                drop(state_guard);

                                let was_self_triggered = triggered_by_us
                                    .swap(false, std::sync::atomic::Ordering::SeqCst);

                                sink.emit(
                                    "obs://stream_state",
                                    serde_json::json!({
                                        "status": new_status,
                                        "active": stream_status.active,
                                        "triggeredByUs": was_self_triggered,
                                    }),
                                );

                                // Cascade: when not self-triggered, ask
                                // the active profile's `obs.direction`
                                // whether to drive SpiritStream and run
                                // the action server-side.
                                if !was_self_triggered {
                                    let deps_snapshot = match cascade_deps.read() {
                                        Ok(g) => g.clone(),
                                        Err(e) => {
                                            log::error!(
                                                "obs cascade_deps read lock poisoned — OBS→SS cascade dropped: {e}"
                                            );
                                            None
                                        }
                                    };
                                    if let Some(deps) = deps_snapshot {
                                        Self::run_obs_to_ss_cascade(
                                            stream_status.active,
                                            deps,
                                            event_sink.clone(),
                                            cascade_start_handle.clone(),
                                        )
                                        .await;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::debug!("Failed to poll OBS stream status: {e}");
                        }
                    }
                } else {
                    break;
                }
                drop(client_guard);

                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        });
        *self.listener_handle.lock().await = Some(handle);
    }

    /// Run the OBS→SpiritStream trigger cascade. Decides whether to
    /// start/stop SpiritStream based on the active profile's
    /// `obs.direction` and current FFmpeg state. Spawns a delayed task
    /// for the actual start so OBS has time to settle. The delayed-start
    /// task handle is captured in `cascade_start_handle` (I1) so OBS
    /// oscillation inside the delay window cancels the stale start
    /// instead of queueing a duplicate.
    async fn run_obs_to_ss_cascade<E: EventSink + Send + Sync + 'static>(
        obs_now_active: bool,
        deps: ObsCascadeDeps,
        event_sink: E,
        cascade_start_handle: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
    ) {
        let Ok(settings) = deps.settings.load() else {
            return;
        };
        let Some(active_name) = settings.last_profile.clone() else {
            return;
        };
        let profile = match deps
            .profiles
            .load_with_key_decryption(&active_name, None)
            .await
        {
            Ok(p) => p,
            Err(err) => {
                log::warn!("OBS cascade: failed to load active profile '{active_name}': {err}");
                return;
            }
        };
        let direction = profile.settings.obs.direction;
        let allowed = matches!(
            direction,
            crate::models::ObsIntegrationDirection::ObsToSpiritstream
                | crate::models::ObsIntegrationDirection::Bidirectional
        );
        if !allowed {
            return;
        }

        if obs_now_active {
            // OBS started → start SpiritStream if not already.
            let already = deps.ffmpeg.active_count() > 0;
            if already {
                log::debug!("OBS cascade: SpiritStream already streaming, skipping start trigger");
                return;
            }
            let eligible: Vec<_> = profile
                .output_groups
                .iter()
                .filter(|g| !g.stream_targets.is_empty())
                .cloned()
                .collect();
            if eligible.is_empty() {
                log::warn!("OBS cascade: no eligible output groups, skipping start trigger");
                return;
            }
            let incoming_url = format!(
                "rtmp://{}:{}/{}",
                profile.input.bind_address, profile.input.port, profile.input.application
            );
            let count = eligible.len();
            let delay = std::time::Duration::from_millis(OBS_TRIGGER_DELAY_MS);
            let ffmpeg = deps.ffmpeg.clone();
            let sink_arc: Arc<dyn EventSink> = Arc::new(EventSinkClone(event_sink));
            let handle = tokio::spawn(async move {
                tokio::time::sleep(delay).await;
                match ffmpeg.start_all(&eligible, &incoming_url, sink_arc.clone()) {
                    Ok(_) => {
                        log::info!("OBS cascade: started SpiritStream ({count} groups)");
                        sink_arc.emit(
                            "stream_started_by_obs",
                            serde_json::json!({ "groupCount": count }),
                        );
                    }
                    Err(err) => {
                        log::error!("OBS cascade: failed to start SpiritStream: {err}");
                    }
                }
            });
            // Single-flight: cancel any prior delayed-start before
            // recording the new one. Lock contention here is fine —
            // we only reach this branch on an active-transition.
            let mut guard = cascade_start_handle.lock().await;
            if let Some(prev) = guard.take() {
                prev.abort();
            }
            *guard = Some(handle);
        } else {
            // OBS stopped → stop SpiritStream if streaming.
            if deps.ffmpeg.active_count() == 0 {
                return;
            }
            match deps.ffmpeg.stop_all() {
                Ok(_) => {
                    log::info!("OBS cascade: stopped SpiritStream");
                    event_sink.emit("stream_stopped_by_obs", serde_json::json!({}));
                }
                Err(err) => {
                    log::error!("OBS cascade: failed to stop SpiritStream: {err}");
                }
            }
        }
    }
}
