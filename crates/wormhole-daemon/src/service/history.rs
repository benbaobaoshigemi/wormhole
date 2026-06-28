use std::time::Duration;

use crate::state::AppState;

pub async fn history_prune_loop(state: AppState) {
    loop {
        let retention = state.config.read().await.history_retention_days;
        let _ = state.db.prune_history(retention);
        tokio::time::sleep(Duration::from_secs(6 * 60 * 60)).await;
    }
}
