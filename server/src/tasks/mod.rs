#[cfg(feature = "chat")]
mod chat_reconnect;
#[cfg(feature = "chat")]
mod token_refresh;

use crate::state::AppState;

pub(crate) async fn start_background_tasks(state: AppState) {
    #[cfg(feature = "chat")]
    {
        token_refresh::start_youtube_token_refresh_task(state.clone()).await;
        chat_reconnect::start_chat_reconnect_task(state.clone()).await;
    }
    let _ = &state; // suppress unused warning when chat disabled
}
