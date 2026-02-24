use serde_json::{json, Value};

use crate::state::AppState;
use crate::util::{get_arg, get_opt_arg};
use spiritstream_server::services::{Encryption, ObsConfig};

pub(crate) async fn handle(state: &AppState, command: &str, payload: &Value) -> Result<Value, String> {
    match command {
        "obs_get_state" => {
            let obs_state = state.obs_handler.get_state().await;
            Ok(json!(obs_state))
        }
        "obs_get_config" => {
            let config = state.obs_handler.get_config().await;
            let mut response = json!(config);

            // Decrypt the password if it's encrypted
            if let Some(obj) = response.as_object_mut() {
                if let Some(pass_value) = obj.get("password") {
                    let pass_str = pass_value.as_str().unwrap_or("");
                    if !pass_str.is_empty() && Encryption::is_stream_key_encrypted(pass_str) {
                        match Encryption::decrypt_stream_key(pass_str, &state.app_data_dir) {
                            Ok(decrypted) => {
                                obj.insert("password".to_string(), json!(decrypted));
                            }
                            Err(e) => {
                                log::warn!("Failed to decrypt OBS password: {}", e);
                                obj.insert("password".to_string(), json!(""));
                            }
                        }
                    }
                }
            }
            Ok(response)
        }
        "obs_set_config" => {
            let host: String = get_arg(payload, "host")?;
            let port: u16 = get_arg(payload, "port")?;
            let password: Option<String> = get_opt_arg(payload, "password")?;
            let use_auth: bool = get_arg(payload, "useAuth")?;
            let direction: String = get_arg(payload, "direction")?;
            let auto_connect: bool = get_arg(payload, "autoConnect")?;

            // Get current config to preserve existing password if not provided
            let current_config = state.obs_handler.get_config().await;

            // Encrypt password if provided, otherwise keep existing
            let encrypted_password = if let Some(ref pass) = password {
                if pass.is_empty() {
                    String::new()
                } else {
                    state.obs_handler.encrypt_password(pass)?
                }
            } else {
                current_config.password
            };

            // Parse direction
            let dir = match direction.as_str() {
                "obs-to-spiritstream" => spiritstream_server::services::IntegrationDirection::ObsToSpiritstream,
                "spiritstream-to-obs" => spiritstream_server::services::IntegrationDirection::SpiritstreamToObs,
                "bidirectional" => spiritstream_server::services::IntegrationDirection::Bidirectional,
                _ => spiritstream_server::services::IntegrationDirection::Disabled,
            };

            let config = ObsConfig {
                host: host.clone(),
                port,
                password: encrypted_password.clone(),
                use_auth,
                direction: dir,
                auto_connect,
            };

            state.obs_handler.set_config(config).await;

            Ok(Value::Null)
        }
        "obs_connect" => {
            state.obs_handler.connect(state.event_bus.clone()).await?;
            Ok(Value::Null)
        }
        "obs_disconnect" => {
            state.obs_handler.disconnect(state.event_bus.clone()).await?;
            Ok(Value::Null)
        }
        "obs_start_stream" => {
            state.obs_handler.start_stream().await?;
            Ok(Value::Null)
        }
        "obs_stop_stream" => {
            state.obs_handler.stop_stream().await?;
            Ok(Value::Null)
        }
        "obs_is_connected" => {
            Ok(json!(state.obs_handler.is_connected().await))
        }
        _ => Err(format!("Unknown OBS command: {command}")),
    }
}
