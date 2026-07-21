use anyhow::Context;
use regex::Regex;
use std::sync::LazyLock;

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use ort::Session;

/** 多模态向量服务：BGE-small-zh 文本 + Chinese-CLIP ONNX 图像 */
pub struct EmbeddingService {
    text_model: TextEmbedding,
    text_dim: usize,
    image_dim: usize,
    clip_session: Session,
    client: reqwest::Client,
}

impl EmbeddingService {
    pub fn try_new() -> anyhow::Result<Self> {
        tracing::info!("正在加载 BGE-small-zh 文本模型...");
        let text_model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::BGESmallZHV15).with_show_download_progress(true),
        ).context("加载 BGE-small-zh 文本模型失败")?;

        let model_path = std::path::Path::new("models/chinese-clip/onnx/model_quantized.onnx");
        let model_path = if model_path.exists() { model_path.to_path_buf() }
            else { std::path::PathBuf::from("models/chinese-clip/onnx/model_quantized.onnx") };

        tracing::info!("正在加载 Chinese-CLIP ONNX 模型: {:?}", model_path);
        let clip_session = Session::builder()?
            .commit_from_file(model_path)
            .context("加载 Chinese-CLIP ONNX 模型失败")?;

        tracing::info!("模型加载完成: 文本=512维 (BGE), 图像=768维 (Chinese-CLIP)");
        Ok(Self {
            text_model, text_dim: 512, image_dim: 768,
            clip_session, client: reqwest::Client::new(),
        })
    }

    /// BGE-small-zh 文本向量化（512维）
    pub fn embed_text(&self, texts: Vec<String>) -> anyhow::Result<Vec<Vec<f32>>> {
        self.text_model.embed(texts, None).context("文本向量化失败")
    }

    /// Chinese-CLIP ONNX 图像向量化（768维，纯 Rust）
    pub fn embed_image(&self, image_bytes: &[u8]) -> anyhow::Result<Vec<f32>> {
        let img = image::load_from_memory(image_bytes).context("解析图片失败")?;
        let img = img.resize_exact(224, 224, image::imageops::FilterType::CatmullRom);
        let rgb = img.to_rgb8();

        let mean = [0.48145466_f32, 0.4578275, 0.40821073];
        let std = [0.26862954_f32, 0.26130258, 0.27577711];

        let mut pixel_values = ndarray::Array4::<f32>::zeros((1, 3, 224, 224));
        for (x, y, pixel) in rgb.enumerate_pixels() {
            for c in 0..3 {
                let val = pixel[c] as f32 / 255.0;
                pixel_values[[0, c, y as usize, x as usize]] = (val - mean[c]) / std[c];
            }
        }

        let dummy_ids = ndarray::Array2::<i64>::zeros((1, 52));
        let dummy_mask = ndarray::Array2::<i64>::zeros((1, 52));

        let outputs = self.clip_session.run(ort::inputs![
            "input_ids" => dummy_ids,
            "pixel_values" => pixel_values,
            "attention_mask" => dummy_mask,
        ]?).context("ONNX 推理失败")?;

        let vec: Vec<f32> = outputs["image_embeds"]
            .try_extract_tensor::<f32>()?.view().iter().copied().collect();

        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        Ok(if norm > 0.0 { vec.iter().map(|x| x / norm).collect() } else { vec })
    }

    /// 从 URL 下载图片并向量化
    pub async fn embed_image_from_url(&self, url: &str) -> anyhow::Result<Vec<f32>> {
        let bytes = self.client.get(url).send().await?
            .bytes().await.context("读取图片字节失败")?;
        self.embed_image(&bytes)
    }

    /// HTML → 纯文本（保留 LaTeX 公式 alt 文本）
    pub fn extract_text_from_html(html: &str) -> String {
        html_to_text(html)
    }

    pub fn text_dim(&self) -> usize { self.text_dim }
    pub fn image_dim(&self) -> usize { self.image_dim }
}

// ── HTML 文本提取 ──

static RE_IMG_ALT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<img[^>]+alt\s*=\s*"([^"]*)"[^>]*/?>"#).unwrap()
});
static RE_IMG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<img[^>]*/?>").unwrap());
static RE_MATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<span[^>]+class\s*=\s*"[^"]*math[^"]*"[^>]*>([\s\S]*?)</span>"#).unwrap()
});
static RE_STYLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<style[^>]*>[\s\S]*?</style>").unwrap());
static RE_SCRIPT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<script[^>]*>[\s\S]*?</script>").unwrap());
static RE_BLOCK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<(/?)(div|p|br|li|tr|h[1-6])[^>]*>").unwrap());
static RE_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]*>").unwrap());
static RE_WS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

fn html_to_text(html: &str) -> String {
    let mut text = html.to_string();

    text = RE_IMG_ALT.replace_all(&text, |caps: &regex::Captures| {
        let alt = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        if alt.is_empty() { "[图片]".into() } else { format!("[公式:{}]", alt) }
    }).to_string();

    text = RE_IMG.replace_all(&text, "[图片]").to_string();
    text = RE_MATH.replace_all(&text, "$1").to_string();
    text = RE_STYLE.replace_all(&text, "").to_string();
    text = RE_SCRIPT.replace_all(&text, "").to_string();
    text = RE_BLOCK.replace_all(&text, " ").to_string();
    text = RE_TAG.replace_all(&text, "").to_string();

    text = text.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
        .replace("&quot;", "\"").replace("&#92;", "\\").replace("&apos;", "'")
        .replace("&nbsp;", " ").replace("&#xA0;", " ");

    text = RE_WS.replace_all(&text, " ").to_string();
    text.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_to_text() {
        let text = html_to_text("<p>在Excel中，对于AD5单元格，其绝对单元格表示方法为()</p><img src='x.png'/>");
        assert!(text.contains("绝对单元格"));
        assert!(!text.contains("<p>"));
    }
}
