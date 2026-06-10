use crate::errors::CoreError;

use super::obs_not_connected;

impl super::ObsWebSocketHandler {
    /// Start streaming in OBS
    pub async fn start_stream(&self) -> Result<(), CoreError> {
        let client = self.client.read().await;
        if let Some(ref client) = *client {
            // Flip the loop-prevention flag before driving OBS so the inbound
            // state event that follows doesn't trigger SpiritStream→OBS back.
            self.mark_triggered_by_us();
            client
                .streaming()
                .start()
                .await
                .map_err(|e| CoreError::Internal {
                    context: format!("Failed to start OBS stream: {e}"),
                })?;
            log::info!("Started OBS stream");
            Ok(())
        } else {
            Err(obs_not_connected())
        }
    }

    /// Stop streaming in OBS
    pub async fn stop_stream(&self) -> Result<(), CoreError> {
        let client = self.client.read().await;
        if let Some(ref client) = *client {
            self.mark_triggered_by_us();
            client
                .streaming()
                .stop()
                .await
                .map_err(|e| CoreError::Internal {
                    context: format!("Failed to stop OBS stream: {e}"),
                })?;
            log::info!("Stopped OBS stream");
            Ok(())
        } else {
            Err(obs_not_connected())
        }
    }

    /// Helper called by the `ObsTrigger` impl: drive OBS to start its
    /// stream when the active profile's direction allows SpiritStream
    /// → OBS triggering. Sets `triggered_by_us` first so the inbound
    /// state event doesn't bounce back through the OBS→SS cascade.
    pub(super) async fn ss_trigger_obs(&self, start: bool) {
        let cascade = match self.cascade_deps.read() {
            Ok(g) => g.clone(),
            Err(e) => {
                log::error!("obs cascade_deps read lock poisoned — SS→OBS trigger dropped: {e}");
                return;
            }
        };
        let Some(deps) = cascade else { return };
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
                log::warn!("SS→OBS cascade: failed to load profile '{active_name}': {err}");
                return;
            }
        };
        let direction = profile.settings.obs.direction;
        let allowed = matches!(
            direction,
            crate::models::ObsIntegrationDirection::SpiritstreamToObs
                | crate::models::ObsIntegrationDirection::Bidirectional
        );
        if !allowed {
            return;
        }
        let connected = matches!(
            self.state.read().await.connection_status,
            super::types::ObsConnectionStatus::Connected
        );
        if !connected {
            log::debug!("SS→OBS cascade: OBS not connected, skipping trigger");
            return;
        }
        let client_guard = self.client.read().await;
        let Some(ref client) = *client_guard else {
            return;
        };
        self.mark_triggered_by_us();
        let result = if start {
            client.streaming().start().await
        } else {
            client.streaming().stop().await
        };
        if let Err(err) = result {
            log::warn!(
                "SS→OBS cascade: OBS {} failed: {err}",
                if start { "start" } else { "stop" }
            );
        } else {
            log::info!(
                "SS→OBS cascade: OBS {} succeeded",
                if start { "started" } else { "stopped" }
            );
        }
    }
}

// Forward the workspace-level trait so `FFmpegHandler` can hold
// `Arc<dyn ObsTrigger>` without depending on the concrete handler.
#[async_trait::async_trait]
impl crate::services::ObsTrigger for super::ObsWebSocketHandler {
    async fn trigger_start(&self) {
        self.ss_trigger_obs(true).await;
    }
    async fn trigger_stop(&self) {
        self.ss_trigger_obs(false).await;
    }
}
