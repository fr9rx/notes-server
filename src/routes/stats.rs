use axum::Json;
use axum::extract::State;

use crate::AppState;
use crate::db;
use crate::error::AppResult;
use crate::models::Stats;

/// Public: totals plus the live numbers the board's LED matrix shows.
pub async fn get(State(state): State<AppState>) -> AppResult<Json<Stats>> {
    let (courses, chapters, notes, images) = db::totals(&state.db).await?;
    let live = state.metrics.snapshot();
    Ok(Json(Stats {
        courses,
        chapters,
        notes,
        images,
        uptime_secs: live.uptime_secs,
        status: live.status.name().to_owned(),
        requests: live.requests,
        uploads: live.uploads,
    }))
}
