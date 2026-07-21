//! wantong-vector: 题目向量化与语义搜索服务
//! 基于 axum + Qdrant + BGE-small-zh + Chinese-CLIP 的多模态检索引擎

mod config;
mod error;
mod handler;
mod model;
mod service;

use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use crate::config::Config;
use crate::handler::sync::AppState;
use crate::service::embedding::EmbeddingService;
use crate::service::qdrant::QdrantService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env()?;
    tracing::info!("配置加载成功");

    let embedding = EmbeddingService::try_new()?;
    let (text_dim, image_dim) = (embedding.text_dim(), embedding.image_dim());
    tracing::info!("Embedding 服务初始化: 文本={}维, 图像={}维", text_dim, image_dim);

    let qdrant = QdrantService::try_new(&config).await?;
    tracing::info!("Qdrant 连接成功");
    qdrant.ensure_collection(text_dim as u64, image_dim as u64).await?;
    tracing::info!("Qdrant collection [question] 就绪");

    let state = AppState { embedding: Arc::new(embedding), qdrant: Arc::new(qdrant) };
    let app = handler::create_router(state);
    let addr = format!("{}:{}", config.server_host, config.server_port);
    tracing::info!("服务启动: http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
