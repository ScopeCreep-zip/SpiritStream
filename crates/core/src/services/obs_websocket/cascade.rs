use std::sync::Arc;

use futures_util::StreamExt;
use obws::events::{Event, OutputState};
use obws::Client;

use crate::services::{EventSink, FFmpegHandler, ProfileManager, SettingsManager};

use super::types::ObsStreamStatus;

/// Server-side cascade dependencies. Set once at startup by
/// `ServiceRegistry::build` after every service is constructed. When
/// present, the OBS event listener runs the OBS→SpiritStream trigger
/// cascade in core (read active profile → check direction → call
/// `FFmpegHandler::start_all` / `stop_all`) instead of relying on a
/// frontend observer to do it. Frontend keeps only display state.
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
    /// Subscribe to OBS's event stream for the just-connected `client` and
    /// run the OBS→SpiritStream trigger cascade.
    ///
    /// Why events, not polling: OBS reports `outputActive == true` only
    /// AFTER its RTMP output has successfully connected. When SpiritStream
    /// IS the RTMP ingest, OBS can't connect until the relay is listening,
    /// and the relay only starts once we see the start signal — a deadlock
    /// that made "Start Streaming" from OBS never reach SpiritStream (it
    /// failed with "Failed to connect to server"). The `StreamStateChanged`
    /// event fires `OutputState::Starting` the instant the user clicks
    /// Start, BEFORE the connection attempt, so we bring the relay up while
    /// OBS is still (re)connecting. App-initiated starts already work because
    /// the relay is up first; this restores the OBS-initiated direction.
    ///
    /// When `cascade_deps` is set, the listener runs the cascade in core on
    /// `Starting` (start) and `Stopped` (stop). The frontend only sees
    /// informational `obs://stream_state` events and the resulting
    /// `stream_started_by_obs` / `stream_stopped_by_obs` — it never decides
    /// whether to start streaming.
    pub(super) async fn start_event_listener<E: EventSink + Send + Sync + Clone + 'static>(
        &self,
        client: Arc<Client>,
        event_sink: E,
    ) {
        let state = self.state.clone();
        let triggered_by_us = self.triggered_by_us.clone();
        let cascade_deps = self.cascade_deps.clone();
        let mut shutdown_rx = self.shutdown_tx.subscribe();

        // Single listener: abort any prior task before spawning the
        // replacement (reconnect flow) so two event streams don't race the
        // cascade.
        if let Some(prev) = self.listener_handle.lock().await.take() {
            prev.abort();
        }

        let handle = tokio::spawn(async move {
            // `events()` borrows `*client` for the stream's lifetime; the Arc
            // is moved into this task so the borrow — and the underlying
            // socket — lives exactly as long as the loop, and is released
            // when the task ends (shutdown / disconnect / OBS close).
            let events = match client.events() {
                Ok(stream) => stream,
                Err(e) => {
                    log::warn!("OBS event subscription failed: {e}");
                    return;
                }
            };
            futures_util::pin_mut!(events);

            loop {
                let event = tokio::select! {
                    _ = shutdown_rx.recv() => {
                        log::debug!("OBS event listener shutting down");
                        break;
                    }
                    next = events.next() => match next {
                        Some(ev) => ev,
                        None => break,
                    },
                };

                let Event::StreamStateChanged {
                    active,
                    state: out_state,
                } = event
                else {
                    continue;
                };

                // OBS output state → display status. Every transition
                // refreshes the widget; only Starting/Stopped drive the
                // cascade below.
                let status = match out_state {
                    OutputState::Starting | OutputState::Reconnecting => ObsStreamStatus::Starting,
                    OutputState::Started
                    | OutputState::Reconnected
                    | OutputState::Resumed
                    | OutputState::Paused => ObsStreamStatus::Active,
                    OutputState::Stopping => ObsStreamStatus::Stopping,
                    OutputState::Stopped => ObsStreamStatus::Inactive,
                    _ => {
                        if active {
                            ObsStreamStatus::Active
                        } else {
                            ObsStreamStatus::Inactive
                        }
                    }
                };
                {
                    let mut guard = state.write().await;
                    guard.stream_status = status;
                }

                // The cascade fires on exactly two transitions: Starting →
                // start SpiritStream (relay up NOW so OBS's (re)connect
                // lands), Stopped → stop it. Started / Stopping / Reconnecting
                // are display-only. The `triggered_by_us` flag is consumed on
                // these two events so a SpiritStream→OBS drive doesn't bounce
                // back into a re-trigger.
                let cascade_start = match out_state {
                    OutputState::Starting => true,
                    OutputState::Stopped => false,
                    _ => {
                        event_sink.emit(
                            "obs://stream_state",
                            serde_json::json!({ "status": status, "active": active }),
                        );
                        continue;
                    }
                };

                let was_self_triggered =
                    triggered_by_us.swap(false, std::sync::atomic::Ordering::SeqCst);

                event_sink.emit(
                    "obs://stream_state",
                    serde_json::json!({
                        "status": status,
                        "active": active,
                        "triggeredByUs": was_self_triggered,
                    }),
                );

                if was_self_triggered {
                    continue;
                }
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
                    Self::run_obs_to_ss_cascade(cascade_start, deps, event_sink.clone()).await;
                }
            }
        });
        *self.listener_handle.lock().await = Some(handle);
    }

    /// Run the OBS→SpiritStream trigger cascade. Decides whether to
    /// start/stop SpiritStream based on the active profile's
    /// `obs.direction` and current FFmpeg state. The start path runs
    /// immediately (no settling delay): OBS has just issued its start
    /// request and is connecting to the relay, so the relay must be
    /// listening before OBS's retry window closes.
    async fn run_obs_to_ss_cascade<E: EventSink + Send + Sync + 'static>(
        obs_now_active: bool,
        deps: ObsCascadeDeps,
        event_sink: E,
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
            if deps.ffmpeg.active_count() > 0 {
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
            let sink_arc: Arc<dyn EventSink> = Arc::new(EventSinkClone(event_sink));
            match deps
                .ffmpeg
                .start_all(&eligible, &incoming_url, sink_arc.clone())
            {
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
