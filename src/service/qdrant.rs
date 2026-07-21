use anyhow::Context;
use qdrant_client::{
    Qdrant,
    qdrant::{
        self, point_id::PointIdOptions, points_selector::PointsSelectorOneOf,
        with_payload_selector::SelectorOptions, Condition, Filter, PointId,
        PointStruct, PrefetchQuery, Query, Value, Vector, vectors::VectorsOptions,
        vectors_config,
    },
};
use std::collections::HashMap;

use crate::config::Config;
use crate::model::search::{SearchFilters, SearchResult};

const COLLECTION_NAME: &str = "question";

pub struct QdrantService {
    client: Qdrant,
}

impl QdrantService {
    pub async fn try_new(config: &Config) -> anyhow::Result<Self> {
        let mut builder = Qdrant::from_url(&config.qdrant_url);
        if let Some(ref key) = config.qdrant_api_key {
            builder = builder.api_key(key.clone());
        }
        let client = builder.build().context("创建 Qdrant 客户端失败")?;
        client.health_check().await.context("Qdrant 健康检查失败")?;
        Ok(Self { client })
    }

    /** 确保 collection 存在，结构不匹配时自动重建 */
    pub async fn ensure_collection(&self, text_dim: u64, image_dim: u64) -> anyhow::Result<()> {
        let collections = self.client.list_collections().await?;
        let exists = collections.collections.iter().any(|c| c.name == COLLECTION_NAME);

        if exists {
            if let Ok(info) = self.client.collection_info(COLLECTION_NAME).await {
                if let Some(config) = info.result.and_then(|r| r.config) {
                    if let Some(vc) = config.params {
                        let has_image = vc.vectors_config.as_ref()
                            .and_then(|v| v.config.as_ref())
                            .map(|c| matches!(c, vectors_config::Config::ParamsMap(m) if m.map.contains_key("image")))
                            .unwrap_or(false);
                        if has_image {
                            tracing::info!("collection [{}] 已存在，跳过创建", COLLECTION_NAME);
                            return Ok(());
                        }
                    }
                }
            }
            tracing::info!("collection [{}] 结构不匹配，删除重建...", COLLECTION_NAME);
            self.client.delete_collection(COLLECTION_NAME).await?;
        }

        tracing::info!("创建 collection [{}], 文本={}维, 图像={}维", COLLECTION_NAME, text_dim, image_dim);
        let mut map = HashMap::new();
        for (name, dim) in [("stem", text_dim), ("analysis", text_dim), ("knowledge", text_dim), ("image", image_dim)] {
            map.insert(name.to_string(), qdrant::VectorParams {
                size: dim, distance: qdrant::Distance::Cosine.into(), ..Default::default()
            });
        }

        self.client.create_collection(qdrant::CreateCollection {
            collection_name: COLLECTION_NAME.into(),
            vectors_config: Some(qdrant::VectorsConfig {
                config: Some(vectors_config::Config::ParamsMap(qdrant::VectorParamsMap { map })),
            }),
            ..Default::default()
        }).await.context("创建 collection 失败")?;
        tracing::info!("collection [{}] 创建成功", COLLECTION_NAME);
        Ok(())
    }

    pub async fn upsert_points(
        &self, points: Vec<(u64, HashMap<String, Vec<f32>>, serde_json::Value)>,
    ) -> anyhow::Result<()> {
        let mut qdrant_points = Vec::with_capacity(points.len());
        for (id, vectors, payload) in points {
            let named_vectors: HashMap<String, Vector> = vectors.into_iter().map(|(k, v)| {
                (k, Vector { data: v, vector: None, ..Default::default() })
            }).collect();

            qdrant_points.push(PointStruct {
                id: Some(PointId { point_id_options: Some(PointIdOptions::Num(id)) }),
                vectors: Some(qdrant::Vectors {
                    vectors_options: Some(VectorsOptions::Vectors(qdrant::NamedVectors { vectors: named_vectors })),
                }),
                payload: payload.as_object().map(|obj| {
                    obj.iter().map(|(k, v)| (k.clone(), json_to_value(v.clone()))).collect()
                }).unwrap_or_default(),
            });
        }

        self.client.upsert_points(qdrant::UpsertPoints {
            collection_name: COLLECTION_NAME.into(), points: qdrant_points, ..Default::default()
        }).await.context("upsert_points 失败")?;
        Ok(())
    }

    pub async fn delete_points(&self, ids: Vec<u64>) -> anyhow::Result<()> {
        let point_ids: Vec<PointId> = ids.into_iter().map(|id| PointId {
            point_id_options: Some(PointIdOptions::Num(id)),
        }).collect();

        self.client.delete_points(qdrant::DeletePoints {
            collection_name: COLLECTION_NAME.into(),
            points: Some(qdrant::PointsSelector {
                points_selector_one_of: Some(PointsSelectorOneOf::Points(
                    qdrant::PointsIdsList { ids: point_ids },
                )),
            }),
            ..Default::default()
        }).await.context("delete_points 失败")?;
        Ok(())
    }

    /** RRF 融合搜索：stem/analysis/knowledge 文本向量 + 可选 image 图像向量 */
    pub async fn search_with_image(
        &self, stem_vec: Vec<f32>, analysis_vec: Vec<f32>, knowledge_vec: Vec<f32>,
        image_vec: Option<Vec<f32>>, top_k: u64, score_threshold: f32,
        _weights: [f32; 3], filters: Option<&SearchFilters>,
    ) -> anyhow::Result<Vec<SearchResult>> {
        let filter = build_filter(filters);
        let mut prefetch = vec![
            prefetch_for("stem", stem_vec, filter.clone(), top_k + 20, score_threshold),
            prefetch_for("analysis", analysis_vec, filter.clone(), top_k + 20, score_threshold),
            prefetch_for("knowledge", knowledge_vec, filter, top_k + 20, score_threshold),
        ];

        if let Some(img_vec) = image_vec {
            prefetch.push(prefetch_for("image", img_vec, None, top_k + 20, score_threshold));
        }

        let query = Query { variant: Some(qdrant::query::Variant::Fusion(qdrant::Fusion::Rrf as i32)) };
        let points_result = self.client.query(qdrant::QueryPoints {
            collection_name: COLLECTION_NAME.into(),
            prefetch, query: Some(query),
            with_payload: Some(qdrant::WithPayloadSelector {
                selector_options: Some(SelectorOptions::Enable(true)),
            }),
            limit: Some(top_k), score_threshold: Some(score_threshold),
            ..Default::default()
        }).await.context("search 失败")?;

        Ok(build_results(points_result.result))
    }
}

fn prefetch_for(name: &str, vec: Vec<f32>, filter: Option<Filter>, limit: u64, threshold: f32) -> PrefetchQuery {
    PrefetchQuery {
        query: Some(Query::new_nearest(vec)),
        r#using: Some(name.into()),
        filter, limit: Some(limit),
        score_threshold: Some(threshold),
        ..Default::default()
    }
}

fn build_results(points: Vec<qdrant::ScoredPoint>) -> Vec<SearchResult> {
    points.into_iter().filter_map(|pt| {
        let id = pt.id?.point_id_options?;
        let qid = match id { PointIdOptions::Num(n) => n, _ => return None };
        let payload = pt.payload;
        let stem = payload.get("stem").and_then(value_to_str).unwrap_or_default();
        let kp = payload.get("knowledge_points").map(value_list_to_strings).unwrap_or_default();
        Some(SearchResult {
            question_id: qid, stem, score: pt.score as f32,
            subject_id: payload.get("subject_id").and_then(value_to_u64),
            grade_id: payload.get("grade_id").and_then(value_to_u64),
            knowledge_points: kp,
            ptype_id: payload.get("ptype_id").and_then(value_to_u64),
            difficult: payload.get("difficult").and_then(value_to_f32),
        })
    }).collect()
}

fn build_filter(filters: Option<&SearchFilters>) -> Option<Filter> {
    let f = filters?;
    let mut conditions: Vec<Condition> = Vec::new();
    if let Some(sid) = f.subject_id { conditions.push(condition_eq("subject_id", sid as i64)); }
    if let Some(gid) = f.grade_id { conditions.push(condition_eq("grade_id", gid as i64)); }
    if let Some(ptid) = f.ptype_id { conditions.push(condition_eq("ptype_id", ptid as i64)); }
    if let Some(ref kp) = f.knowledge_point {
        conditions.push(Condition {
            condition_one_of: Some(qdrant::condition::ConditionOneOf::Field(qdrant::FieldCondition {
                key: "knowledge_points".into(),
                r#match: Some(qdrant::Match {
                    match_value: Some(qdrant::r#match::MatchValue::Text(kp.clone())),
                }),
                ..Default::default()
            })),
        });
    }
    if conditions.is_empty() { None } else { Some(Filter { must: conditions, ..Default::default() }) }
}

fn condition_eq(key: &str, val: i64) -> Condition {
    Condition {
        condition_one_of: Some(qdrant::condition::ConditionOneOf::Field(qdrant::FieldCondition {
            key: key.into(),
            r#match: Some(qdrant::Match {
                match_value: Some(qdrant::r#match::MatchValue::Integer(val)),
            }),
            ..Default::default()
        })),
    }
}

// ── Value 转换工具函数 ──

fn json_to_value(v: serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value { kind: Some(qdrant::value::Kind::NullValue(0)) },
        serde_json::Value::Bool(b) => Value { kind: Some(qdrant::value::Kind::BoolValue(b)) },
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value { kind: Some(qdrant::value::Kind::IntegerValue(i)) }
            } else {
                Value { kind: Some(qdrant::value::Kind::DoubleValue(n.as_f64().unwrap_or(0.0))) }
            }
        }
        serde_json::Value::String(s) => Value { kind: Some(qdrant::value::Kind::StringValue(s)) },
        serde_json::Value::Array(arr) => {
            Value { kind: Some(qdrant::value::Kind::ListValue(qdrant::ListValue {
                values: arr.into_iter().map(json_to_value).collect(),
            }))}
        }
        serde_json::Value::Object(_) => Value { kind: Some(qdrant::value::Kind::NullValue(0)) },
    }
}

fn value_to_str(v: &Value) -> Option<String> {
    match &v.kind {
        Some(qdrant::value::Kind::StringValue(s)) => Some(s.clone()),
        Some(qdrant::value::Kind::IntegerValue(i)) => Some(i.to_string()),
        Some(qdrant::value::Kind::DoubleValue(d)) => Some(d.to_string()),
        _ => None,
    }
}

fn value_to_u64(v: &Value) -> Option<u64> {
    match &v.kind {
        Some(qdrant::value::Kind::IntegerValue(i)) if *i >= 0 => Some(*i as u64),
        Some(qdrant::value::Kind::DoubleValue(d)) => Some(*d as u64),
        _ => None,
    }
}

fn value_to_f32(v: &Value) -> Option<f32> {
    match &v.kind {
        Some(qdrant::value::Kind::DoubleValue(d)) => Some(*d as f32),
        Some(qdrant::value::Kind::IntegerValue(i)) => Some(*i as f32),
        _ => None,
    }
}

fn value_list_to_strings(v: &Value) -> Vec<String> {
    match &v.kind {
        Some(qdrant::value::Kind::ListValue(list)) => list.values.iter().filter_map(value_to_str).collect(),
        Some(qdrant::value::Kind::StringValue(s)) => vec![s.clone()],
        _ => vec![],
    }
}
