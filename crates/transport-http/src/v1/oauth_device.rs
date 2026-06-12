//! Device Code Flow transport plumbing for `POST /oauth/{provider}/flow`.
//!
//! The flow endpoint is unified: core decides which grant a provider
//! uses. When `start_flow` answers `OAuthFlowRequiresDevice` (public
//! Twitch client), the handler calls [`run_device_flow`], which starts
//! the device authorization, spawns the background poll task (persist +
//! `oauth_complete`/`oauth_error` events — mirroring the loopback
//! callback task), and shapes the device variant of the response. The
//! frontend renders whichever variant arrives; it never chooses a flow.

use spiritstream_core::services::EventSink;

use crate::AppState;

/// Start the device flow and hand back the user-facing fields. The
/// `device_code` poll credential stays inside the spawned task — it
/// never crosses the wire.
pub(super) async fn run_device_flow(
    state: &AppState,
    provider: &str,
) -> Result<super::oauth::OAuthFlowResponse, crate::ApiError> {
    let start = state.oauth_service.start_device_flow(provider).await?;

    let response = super::oauth::OAuthFlowResponse::device(
        start.user_code.clone(),
        start.verification_uri.clone(),
        start.expires_in,
        start.interval,
    );

    let oauth_service = state.oauth_service.clone();
    let state_clone = state.clone();
    let provider_name = provider.to_string();
    tokio::spawn(async move {
        match oauth_service.poll_device_flow(&provider_name, &start).await {
            Ok(result) => {
                let now = chrono::Utc::now().timestamp();
                let expires_at = result
                    .tokens
                    .expires_in
                    .map(|e| now + e as i64)
                    .unwrap_or(0);
                match crate::update_profile_oauth_account(
                    &state_clone,
                    &provider_name,
                    result.tokens.access_token.clone(),
                    result.tokens.refresh_token.clone(),
                    expires_at,
                    &result.user_info,
                )
                .await
                {
                    Ok(()) => {
                        state_clone
                            .event_bus
                            .emit("oauth_complete", serde_json::json!(result.user_info));
                    }
                    Err(err) => {
                        log::error!("Failed to save OAuth profile settings: {err}");
                        state_clone.event_bus.emit(
                            "oauth_error",
                            serde_json::json!({
                                "provider": provider_name,
                                "reason": "persist_failed",
                            }),
                        );
                    }
                }
            }
            Err(err) => {
                log::warn!("Device flow for {provider_name} did not complete: {err:?}");
                state_clone.event_bus.emit(
                    "oauth_error",
                    serde_json::json!({
                        "provider": provider_name,
                        "reason": "device_flow_failed",
                        "kind": err.kind(),
                    }),
                );
            }
        }
    });

    Ok(response)
}
