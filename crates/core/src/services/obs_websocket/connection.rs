use obws::Client;
use std::sync::Arc;

use crate::errors::CoreError;
use crate::models::{ObsIntegrationDirection, ObsSettings};
use crate::services::{Encryption, EventSink, IntegrationDirection, ObsConfig};

use super::types::{ObsConnectionStatus, ObsStreamStatus};

/// Whether two OBS configs differ in a way that requires reconnecting an
/// already-open socket. `direction` and `auto_connect` are deliberately
/// excluded: `direction` only affects the trigger cascade (read live from the
/// config), and `auto_connect` only gates whether the supervisor runs — neither
/// needs the connection itself to be re-established.
pub(super) fn obs_connection_params_changed(old: &ObsConfig, new: &ObsConfig) -> bool {
    old.host != new.host
        || old.port != new.port
        || old.password != new.password
        || old.use_auth != new.use_auth
}

impl super::ObsWebSocketHandler {
    /// Apply the active profile's OBS settings to the live handler — the single
    /// A→B sync that keeps the profile as the one source of truth. Updates the
    /// runtime config, then matches the auto-connect supervisor to the settings.
    /// Called on profile activation AND whenever the active profile's OBS
    /// settings are saved, so settings never round-trip through a set-config
    /// endpoint.
    ///
    /// Reconnect-on-change: if a connection-affecting param (host/port/password/
    /// use_auth) changed while a socket was live, the old socket is torn down
    /// first so the supervisor reconnects with the new params — otherwise
    /// `spawn_auto_connect` would see "already connected" and silently keep the
    /// stale connection. A direction-only / auto_connect-only change does NOT
    /// churn the connection.
    pub async fn apply_profile_obs<E: EventSink + Send + Sync + Clone + 'static>(
        self: Arc<Self>,
        obs: &ObsSettings,
        event_sink: E,
    ) {
        let direction = match obs.direction {
            ObsIntegrationDirection::ObsToSpiritstream => IntegrationDirection::ObsToSpiritstream,
            ObsIntegrationDirection::SpiritstreamToObs => IntegrationDirection::SpiritstreamToObs,
            ObsIntegrationDirection::Bidirectional => IntegrationDirection::Bidirectional,
            ObsIntegrationDirection::Disabled => IntegrationDirection::Disabled,
        };
        let new_config = ObsConfig {
            host: obs.host.clone(),
            port: obs.port,
            password: obs.password.clone(),
            use_auth: obs.use_auth,
            direction,
            auto_connect: obs.auto_connect,
        };

        // Snapshot the live state + current config BEFORE overwriting, so we can
        // decide whether an already-open socket must be torn down to pick up new
        // connection params.
        let was_live = {
            let state = self.state.read().await;
            matches!(
                state.connection_status,
                ObsConnectionStatus::Connected | ObsConnectionStatus::Connecting
            )
        };
        let conn_changed = {
            let old = self.config.read().await;
            obs_connection_params_changed(&old, &new_config)
        };

        self.set_config(new_config).await;

        let in_use = obs.auto_connect || obs.direction != ObsIntegrationDirection::Disabled;
        if !in_use {
            // OBS integration turned off — stop the supervisor and drop any live
            // connection so a now-disabled profile stops talking to OBS.
            let _ = self.disconnect(event_sink).await;
            return;
        }
        // Integration is in use. Tear down a live-but-now-stale socket first so
        // the supervisor reconnects with the new params; otherwise it no-ops.
        if was_live && conn_changed {
            let _ = self.disconnect(event_sink.clone()).await;
        }
        self.spawn_auto_connect(event_sink).await;
    }

    /// Connect with exponential-backoff auto-retry. Spawned at profile
    /// activation when OBS integration is in use (an explicit
    /// `obs.auto_connect`, or any non-Disabled trigger direction), so an
    /// already-open OBS is picked up automatically and one that opens
    /// later is caught on a subsequent retry. Honors `shutdown_tx` so a
    /// manual `disconnect()` (or app shutdown) interrupts the retry loop;
    /// backoff curve 2s → 30s, multiplier 1.5.
    ///
    /// Single owner: aborts any prior supervisor loop before spawning a
    /// new one and stores the handle in `auto_connect_handle`, so repeated
    /// activations can't stack competing loops that race `connect()`.
    pub async fn spawn_auto_connect<E: EventSink + Send + Sync + Clone + 'static>(
        self: Arc<Self>,
        event_sink: E,
    ) {
        const INITIAL_DELAY_MS: u64 = 2000;
        const MAX_DELAY_MS: u64 = 30000;
        const BACKOFF_MULT: f64 = 1.5;

        // Tear down any prior supervisor before starting a fresh one.
        if let Some(prior) = self.auto_connect_handle.lock().await.take() {
            prior.abort();
        }

        let mut shutdown_rx = self.shutdown_tx.subscribe();
        let handler = self.clone();
        let handle = tokio::spawn(async move {
            let mut delay_ms = INITIAL_DELAY_MS;
            loop {
                if shutdown_rx.try_recv().is_ok() {
                    log::debug!("OBS auto-connect loop interrupted by shutdown");
                    break;
                }
                let already = matches!(
                    handler.state.read().await.connection_status,
                    ObsConnectionStatus::Connected | ObsConnectionStatus::Connecting
                );
                if already {
                    break;
                }
                match handler.connect(event_sink.clone()).await {
                    Ok(()) => {
                        log::info!("OBS auto-connect: connected");
                        break;
                    }
                    Err(err) => {
                        log::info!(
                            "OBS auto-connect: attempt failed ({err}); retrying in {delay_ms}ms"
                        );
                        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                        delay_ms =
                            ((delay_ms as f64) * BACKOFF_MULT).min(MAX_DELAY_MS as f64) as u64;
                    }
                }
            }
        });
        *self.auto_connect_handle.lock().await = Some(handle);
    }

    pub async fn connect<E: EventSink + Send + Sync + Clone + 'static>(
        &self,
        event_sink: E,
    ) -> Result<(), CoreError> {
        let config = self.config.read().await.clone();

        // Guard against double-connect: if a previous `connect()` is still
        // mid-handshake (state == Connecting) or already finished (Connected),
        // bail out without re-emitting "connecting" or starting a second
        // handshake. The second call would otherwise emit a duplicate event
        // AND race the first call's `client_guard` write — fixing the latter
        // properly needs a cancellation token; for now we just prevent the
        // duplicate state transition.
        {
            let state_read = self.state.read().await;
            if matches!(
                state_read.connection_status,
                ObsConnectionStatus::Connecting | ObsConnectionStatus::Connected
            ) {
                return Ok(());
            }
        }

        {
            let mut state = self.state.write().await;
            state.connection_status = ObsConnectionStatus::Connecting;
            state.error_message = None;
        }

        event_sink.emit(
            "obs://status",
            serde_json::json!({
                "status": "connecting",
                "host": config.host,
                "port": config.port
            }),
        );

        // Pass the password whenever one is set — the obs-websocket v5 server's
        // Hello dictates whether auth is required, and `obws` only answers the
        // challenge when the server asks. Gating on `use_auth` (a UI hint) used
        // to send NO auth when the box was unchecked, so an OBS with a password
        // (the 28+ default) rejected the Identify and the client never appeared
        // in OBS's connection list. A password set against an auth-disabled OBS
        // is harmless — `obws` won't send it unless challenged.
        let password = if !config.password.is_empty() {
            if Encryption::is_stream_key_encrypted(&config.password) {
                Some(Encryption::decrypt_stream_key(
                    &config.password,
                    &self.app_data_dir,
                )?)
            } else {
                Some(config.password.clone())
            }
        } else {
            None
        };

        let connect_result = Client::connect(&config.host, config.port, password).await;

        match connect_result {
            Ok(client) => {
                log::info!(
                    "Connected to OBS WebSocket at {}:{}",
                    config.host,
                    config.port
                );

                let version_info = client.general().version().await.ok();

                {
                    let mut state = self.state.write().await;
                    state.connection_status = ObsConnectionStatus::Connected;
                    state.error_message = None;
                    if let Some(ref info) = version_info {
                        state.obs_version = Some(info.obs_version.to_string());
                        state.websocket_version = Some(info.obs_web_socket_version.to_string());
                    }
                }

                if let Ok(stream_status) = client.streaming().status().await {
                    let mut state = self.state.write().await;
                    state.stream_status = if stream_status.active {
                        ObsStreamStatus::Active
                    } else {
                        ObsStreamStatus::Inactive
                    };
                }

                // Point OBS's stream service at the relay's RTMP ingest so its
                // StartStream pushes to SpiritStream — using the explicit IPv4
                // loopback URL, which sidesteps the `localhost`→`::1` resolution
                // that makes an IPv4-only ingest refuse OBS ("Failed to connect
                // to server"). Only when a trigger direction is set (the user
                // linked OBS ↔ SpiritStream); never touch OBS's stream config
                // for a Disabled/monitor-only connection. Best-effort: OBS
                // rejects this while actively streaming, and connecting must not
                // fail just because we couldn't pre-point it.
                if config.direction != IntegrationDirection::Disabled {
                    if let Some(server) = self.ingest_url.read().await.clone() {
                        let settings = serde_json::json!({
                            "server": server,
                            "key": "spiritstream",
                            "use_auth": false,
                        });
                        match client
                            .config()
                            .set_stream_service_settings("rtmp_custom", &settings)
                            .await
                        {
                            Ok(()) => log::info!("Pointed OBS stream service at relay ingest {server}"),
                            Err(e) => log::warn!(
                                "Could not point OBS stream service at relay ingest (will use OBS's own config): {e}"
                            ),
                        }
                    }
                }

                // Share via Arc so the event listener can own a clone for its
                // `client.events()` stream while the command path keeps driving
                // the same socket.
                let client = Arc::new(client);
                {
                    let mut client_guard = self.client.write().await;
                    *client_guard = Some(client.clone());
                }

                let state = self.state.read().await.clone();
                event_sink.emit(
                    "obs://status",
                    serde_json::json!({
                        "status": "connected",
                        "obsVersion": state.obs_version,
                        "websocketVersion": state.websocket_version,
                        "streamStatus": state.stream_status
                    }),
                );

                self.start_event_listener(client, event_sink).await;

                Ok(())
            }
            Err(e) => {
                let error_msg = format!("Failed to connect to OBS: {e}");
                log::error!("{error_msg}");

                {
                    let mut state = self.state.write().await;
                    state.connection_status = ObsConnectionStatus::Error;
                    state.error_message = Some(error_msg.clone());
                }

                event_sink.emit(
                    "obs://status",
                    serde_json::json!({
                        "status": "error",
                        "error": error_msg.clone(),
                    }),
                );

                Err(CoreError::Internal { context: error_msg })
            }
        }
    }

    pub async fn disconnect<E: EventSink>(&self, event_sink: E) -> Result<(), CoreError> {
        let _ = self.shutdown_tx.send(());

        // Stop the auto-connect supervisor: a manual disconnect is an
        // intentional "stay disconnected", so don't let the retry loop
        // immediately reconnect. (`shutdown_tx` already signals it, but a
        // loop sleeping in backoff wouldn't notice until its next tick.)
        if let Some(handle) = self.auto_connect_handle.lock().await.take() {
            handle.abort();
        }

        {
            let mut client = self.client.write().await;
            *client = None;
        }

        {
            let mut state = self.state.write().await;
            state.connection_status = ObsConnectionStatus::Disconnected;
            state.stream_status = ObsStreamStatus::Unknown;
            state.error_message = None;
            state.obs_version = None;
            state.websocket_version = None;
        }

        log::info!("Disconnected from OBS WebSocket");

        event_sink.emit(
            "obs://status",
            serde_json::json!({
                "status": "disconnected"
            }),
        );

        Ok(())
    }
}
