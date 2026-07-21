use serde::{Deserialize, Serialize};

/** 题目 upsert 请求（id 存在则更新，不存在则新增） */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertRequest {
    pub questions: Vec<QuestionInput>,
}

impl UpsertRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.questions.is_empty() {
            return Err("questions 不能为空".into());
        }
        for q in &self.questions {
            q.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionInput {
    pub id: u64,
    pub ptype_id: Option<u64>,
    pub parent_id: Option<u64>,
    pub stem: String,
    pub analysis: String,
    pub subject_id: Option<u64>,
    pub grade_id: Option<u64>,
    pub difficult: Option<f32>,
    #[serde(default)]
    pub knowledge_points: Vec<String>,
    /// 配图 URL，自动下载并编码为 Chinese-CLIP 图像向量（768维）
    pub image_url: Option<String>,
}

impl QuestionInput {
    pub fn validate(&self) -> Result<(), String> {
        if self.id == 0 { return Err("题目 id 不能为 0".into()); }
        if self.stem.trim().is_empty() { return Err(format!("题目[{}] stem 不能为空", self.id)); }
        if self.analysis.trim().is_empty() { return Err(format!("题目[{}] analysis 不能为空", self.id)); }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertResponse {
    pub synced_count: usize,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteRequest {
    pub ids: Vec<u64>,
}

impl DeleteRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.ids.is_empty() { return Err("ids 不能为空".into()); }
        Ok(())
    }
}
