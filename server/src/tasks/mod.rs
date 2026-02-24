mod chat_reconnect;
mod token_refresh;

use crate::state::AppState;

pub(crate) async fn start_background_tasks(state: AppState) {
    token_refresh::start_youtube_token_refresh_task(state.clone()).await;
    chat_reconnect::start_chat_reconnect_task(state).await;
}
