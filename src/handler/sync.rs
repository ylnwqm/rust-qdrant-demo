use axum::{extract::State, Json};
use base64::Engine;
use serde_json::json;
use std::{collections::HashMap, sync::Arc};

use crate::error::AppError;
use crate::model::question::{DeleteRequest, UpsertRequest, UpsertResponse};
use crate::model::search::{SearchRequest, SearchResponse};
use crate::service::embedding::EmbeddingService;
use crate::service::qdrant::QdrantService;

#[derive(Clone)]
pub struct AppState {
    pub embedding: Arc<EmbeddingService>,
    pub qdrant: Arc<QdrantService>,
}

/** 批量新增/更新题目向量（id 存在则覆盖） */
pub async fn upsert_questions(
    State(state): State<AppState>,
    Json(req): Json<UpsertRequest>,
) -> Result<Json<UpsertResponse>, AppError> {
    req.validate().map_err(AppError::BadRequest)?;
    let count = req.questions.len();
    tracing::info!("同步 {} 条题目向量", count);

    let stems: Vec<String> = req.questions.iter()
        .map(|q| EmbeddingService::extract_text_from_html(&q.stem)).collect();
    let analyses: Vec<String> = req.questions.iter()
        .map(|q| EmbeddingService::extract_text_from_html(&q.analysis)).collect();
    let knowledges: Vec<String> = req.questions.iter()
        .map(|q| if q.knowledge_points.is_empty() { String::new() } else { q.knowledge_points.join("；") })
        .collect();

    let stem_embs = state.embedding.embed_text(stems)?;
    let analysis_embs = state.embedding.embed_text(analyses)?;
    let knowledge_embs = state.embedding.embed_text(knowledges)?;

    let mut image_embs = Vec::with_capacity(count);
    let empty_img = vec![0.0; state.embedding.image_dim()];
    for url_opt in req.questions.iter().map(|q| &q.image_url) {
        let emb = match url_opt {
            Some(url) if !url.is_empty() => state.embedding.embed_image_from_url(url).await
                .unwrap_or_else(|e| { tracing::warn!("图片编码失败 [{}]: {}", url, e); empty_img.clone() }),
            _ => empty_img.clone(),
        };
        image_embs.push(emb);
    }

    let points: Vec<_> = req.questions.iter().enumerate().map(|(i, q)| {
        let mut vectors = HashMap::new();
        vectors.insert("stem".into(), stem_embs[i].clone());
        vectors.insert("analysis".into(), analysis_embs[i].clone());
        vectors.insert("knowledge".into(), knowledge_embs[i].clone());
        vectors.insert("image".into(), image_embs[i].clone());
        let payload = json!({
            "question_id": q.id, "ptype_id": q.ptype_id, "stem": q.stem,
            "subject_id": q.subject_id, "grade_id": q.grade_id,
            "difficult": q.difficult, "knowledge_points": q.knowledge_points,
        });
        (q.id, vectors, payload)
    }).collect();

    state.qdrant.upsert_points(points).await?;
    Ok(Json(UpsertResponse { synced_count: count, status: "ok".into() }))
}

/** 批量删除题目向量 */
pub async fn delete_questions(
    State(state): State<AppState>,
    Json(req): Json<DeleteRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    req.validate().map_err(AppError::BadRequest)?;
    let count = req.ids.len();
    tracing::info!("删除 {} 条题目向量", count);
    state.qdrant.delete_points(req.ids).await?;
    Ok(Json(json!({ "deleted_count": count, "status": "ok" })))
}

/** 语义搜索（纯文本 / 纯图片 / 图文混合统一入口） */
pub async fn search(
    State(state): State<AppState>,
    Json(req): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, AppError> {
    let has_text = req.query_text.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
    let has_img = req.query_image.as_ref().map(|s| !s.is_empty()).unwrap_or(false);

    if !has_text && !has_img {
        return Err(AppError::BadRequest("query_text 或 query_image 至少需要一个".into()));
    }
    if !(1..=100).contains(&req.top_k) {
        return Err(AppError::BadRequest("top_k 必须在 1-100 之间".into()));
    }

    if has_text {
        let text = req.query_text.as_ref().unwrap();
        let preview = if text.len() > 200 {
            let safe_end = text.floor_char_boundary(200);
            format!("{}...({}字节)", &text[..safe_end], text.len())
        } else {
            text.clone()
        };
        tracing::info!("搜索: text={}, has_image={}", preview, has_img);
    } else {
        tracing::info!("搜索: text=(无), has_image={}", has_img);
    }

    let stem_vec = if has_text {
        let text = req.query_text.unwrap();
        let embs = state.embedding.embed_text(vec![text])?;
        embs.into_iter().next().unwrap_or_else(|| vec![0.0; state.embedding.text_dim()])
    } else {
        vec![0.0; state.embedding.text_dim()]
    };

    let img_vec = if has_img {
        let b64 = req.query_image.unwrap();
        let b64_clean = if let Some(idx) = b64.find(";base64,") { &b64[idx + 8..] } else { &b64 };
        let img_bytes = base64::engine::general_purpose::STANDARD
            .decode(b64_clean)
            .map_err(|e| AppError::BadRequest(format!("base64 解码失败: {}", e)))?;
        Some(state.embedding.embed_image(&img_bytes)?)
    } else {
        None
    };

    let weights = req.weights.unwrap_or_else(|| vec![0.6, 0.25, 0.15]);
    let w = [weights[0], weights[1], weights[2]];

    let results = state.qdrant.search_with_image(
        stem_vec, img_vec,
        req.top_k, req.score_threshold, w, req.filters.as_ref(),
    ).await?;
    let total = results.len();

    Ok(Json(SearchResponse { results, total, status: "ok".into() }))
}
