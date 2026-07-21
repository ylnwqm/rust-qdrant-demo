use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("请求参数错误: {0}")]
    BadRequest(String),
    #[error("内部错误: {0}")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            AppError::BadRequest(s) => (StatusCode::BAD_REQUEST, s),
            AppError::Internal(e) => {
                tracing::error!("内部错误: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, format!("内部错误: {}", e))
            }
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}