use axum::{routing::post, Router};
use crate::handler::sync::{delete_questions, search, upsert_questions, AppState};

pub(crate) mod sync;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/question/upsert", post(upsert_questions))
        .route("/api/question/delete", post(delete_questions))
        .route("/api/question/search", post(search))
        .route("/health", axum::routing::get(health))
        .with_state(state)
}

async fn health() -> &'static str { "OK" }
