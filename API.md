# API 接口文档

> Base URL: `http://localhost:3000` | 编码: UTF-8 | Content-Type: `application/json`

---

## 1. 健康检查

```
GET /health
```

**响应**

```
200 OK
OK
```

---

## 2. 新增/更新题目向量

```
POST /api/question/upsert
```

`id` 已存在时覆盖全部向量（stem + analysis + knowledge_points + image），不存在时新增。

**请求体**

| 字段 | 类型 | 必填 | 说明 |
|------|------|:--:|------|
| questions | QuestionInput[] | **是** | 题目列表，不能为空 |

**QuestionInput**

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|------|------|:--:|--------|------|
| id | u64 | **是** | — | 题目唯一 ID，不能为 0 |
| stem | string | **是** | — | 题干 HTML（支持 LaTeX img alt），不能为空 |
| analysis | string | **是** | — | 解析 HTML，不能为空 |
| knowledge_points | string[] | 否 | `[]` | 关联知识点 |
| image_url | string | 否 | — | 配图 URL，自动下载并生成图像向量 |
| ptype_id | u64 | 否 | — | 题型 ID |
| parent_id | u64 | 否 | — | 父题 ID |
| subject_id | u64 | 否 | — | 学科 ID |
| grade_id | u64 | 否 | — | 年级 ID |
| difficult | f32 | 否 | — | 难度系数 |

**请求示例**

```json
{
  "questions": [
    {
      "id": 1,
      "stem": "<p>树上原来有<span>18</span>只小鸟，飞走了<span>7</span>只，还剩多少只？</p>",
      "analysis": "18 - 7 = 11（只），用总数减去飞走的就是剩下的。",
      "subject_id": 10001,
      "grade_id": 1,
      "difficult": 0.3,
      "knowledge_points": ["减法", "20以内减法"],
      "image_url": "http://oss.example.com/quest/xxx.png"
    }
  ]
}
```

**成功响应**

```json
{
  "synced_count": 1,
  "status": "ok"
}
```

**校验错误**

| 条件 | 状态码 | 错误信息 |
|------|:------:|------|
| questions 为空 | 400 | `questions 不能为空` |
| id 为 0 | 400 | `题目 id 不能为 0` |
| stem 为空 | 400 | `题目[N] stem 不能为空` |
| analysis 为空 | 400 | `题目[N] analysis 不能为空` |

---

## 3. 删除题目向量

```
POST /api/question/delete
```

**请求体**

| 字段 | 类型 | 必填 | 说明 |
|------|------|:--:|------|
| ids | u64[] | **是** | 待删除的题目 ID 列表，不能为空 |

**请求示例**

```json
{
  "ids": [1, 2, 3]
}
```

**成功响应**

```json
{
  "deleted_count": 3,
  "status": "ok"
}
```

---

## 4. 题目搜索（纯文本/多模态合一）

```
POST /api/question/search
```

根据传入的 `query_text` 和 `query_image` 自动选择搜索模式。

**请求体**

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|------|------|:--:|--------|------|
| query_text | string | 条件 | — | 搜索文本 |
| query_image | string | 条件 | — | 图片 base64（支持 `data:image/png;base64,` 前缀） |
| top_k | u64 | 否 | 10 | 返回结果数，范围 1-100 |
| score_threshold | f32 | 否 | 0.0 | 最低分数阈值 |
| weights | f32[3] | 否 | [0.6,0.25,0.15] | stem/analysis/knowledge 三路权重 |
| filters | object | 否 | — | 过滤条件 |

> `query_text` 和 `query_image` 至少提供一个。

**filters 过滤字段**（多个条件 AND 关系）

| 字段 | 类型 | 说明 |
|------|------|------|
| subject_id | u64 | 学科过滤 |
| grade_id | u64 | 年级过滤 |
| ptype_id | u64 | 题型过滤 |
| knowledge_point | string | 知识点过滤 |

**请求示例**

```json
// 纯文本搜索
{
  "query_text": "看图列式",
  "top_k": 5,
  "filters": { "grade_id": 1 }
}

// 多模态搜索（图片 base64）
{
  "query_text": "看图列式",
  "query_image": "iVBORw0KGgoAAAANSUhEUg...",
  "top_k": 5
}

// 纯图片搜索
{
  "query_image": "data:image/png;base64,iVBORw0KGgo...",
  "top_k": 5
}
```

**成功响应**

```json
{
  "status": "ok",
  "total": 3,
  "results": [
    {
      "question_id": 2,
      "stem": "<p>看图列式计算</p><p>□ - □ = □</p>",
      "score": 1.7,
      "subject_id": 10001,
      "grade_id": 1,
      "difficult": 0.25,
      "ptype_id": 1,
      "knowledge_points": ["看图列式", "减法"]
    }
  ]
}
```

**响应字段**

| 字段 | 类型 | 说明 |
|------|------|------|
| question_id | u64 | 题目 ID |
| stem | string | 题干原文（HTML） |
| score | f32 | RRF 融合分数，越高越相关（范围约 0~2） |
| subject_id | u64\|null | 学科 ID |
| grade_id | u64\|null | 年级 ID |
| difficult | f32\|null | 难度系数 |
| ptype_id | u64\|null | 题型 ID |
| knowledge_points | string[] | 知识点列表 |

**校验错误**

| 条件 | 状态码 | 错误信息 |
|------|:------:|------|
| 无参数 | 400 | `query_text 或 query_image 至少需要一个` |
| top_k 越界 | 400 | `top_k 必须在 1-100 之间` |

---

## 测试结果

### 第一轮：参数校验

| 测试用例 | 状态 | 结果 |
|----------|:----:|------|
| 搜索无参数 | 400 | `query_text 或 query_image 至少需要一个` |
| upsert 空数组 | 400 | `questions 不能为空` |
| upsert 缺必填字段 | 400 | `题目 id 不能为 0` |
| delete 空 ids | 400 | `ids 不能为空` |
| search top_k 溢出 | 400 | `top_k 必须在 1-100 之间` |

### 第二轮：CRUD 功能

| 测试用例 | 状态 | 结果 |
|----------|:----:|------|
| 插入 2 题 (ID=100,200) | 200 | `synced_count=2` |
| 文本搜索"苹果" | 200 | 3 结果，新插入的排前 |
| 图文混合搜索"苹果"+图片 | 200 | 3 结果，RRF 融合正常 |
| 删除 ID=100,200 | 200 | `deleted_count=2` |
| 删除后验证 | 200 | 已删除题目不在结果中 |

### 第三轮：边界场景

| 测试用例 | 状态 | 结果 |
|----------|:----:|------|
| 图文混合 + 无关关键词 | 200 | 返回 3 结果（正常降级） |
| filters grade_id=1 | 200 | 返回 5 结果（过滤生效） |
| data:image 前缀 base64 | 200 | 3 结果（自动去除前缀） |
| upsert 更新 ID=1 | 200 | 更新成功，搜索确认新 stem |
