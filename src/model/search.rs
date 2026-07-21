use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query_text: Option<String>,
    /// 图片 base64（可选，提供则走多模态搜索）
    pub query_image: Option<String>,
    #[serde(default = "default_top_k")]
    pub top_k: u64,
    #[serde(default)]
    pub score_threshold: f32,
    pub weights: Option<Vec<f32>>,
    pub filters: Option<SearchFilters>,
}

fn default_top_k() -> u64 { 10 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchFilters {
    pub subject_id: Option<u64>,
    pub grade_id: Option<u64>,
    pub ptype_id: Option<u64>,
    pub knowledge_point: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub question_id: u64,
    pub stem: String,
    pub score: f32,
    pub subject_id: Option<u64>,
    pub grade_id: Option<u64>,
    pub knowledge_points: Vec<String>,
    pub ptype_id: Option<u64>,
    pub difficult: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub total: usize,
    pub status: String,
}
