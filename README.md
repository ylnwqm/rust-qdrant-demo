# qdrant-vector — 题目向量检索引擎

基于 Rust Axum + Qdrant + BGE-small-zh + Chinese-CLIP 的多模态题目向量检索引擎。纯 Rust 单进程，支持文本、图片、图文混合三种搜索模式。

## 技术栈

| 组件 | 技术 | 说明 |
|------|------|------|
| Web 框架 | Rust Axum 0.8 | 异步 HTTP 服务，JSON 传参 |
| 向量数据库 | Qdrant | Named Vectors + RRF 融合排序 |
| 文本 Embedding | BGE-small-zh-v1.5 | 512 维中文语义向量，首次启动自动下载 |
| 图像 Embedding | Chinese-CLIP-ViT-Large-Patch14 | 768 维 ONNX 量化版，需手动下载（392MB） |

## 部署步骤

### 1. 环境要求

- Rust 1.75+
- 内存 >= 4GB
- Qdrant 服务（已部署或本地 Docker）

### 2. 克隆项目

```bash
git clone https://github.com/ylnwqm/rust-qdrant-demo.git
cd rust-qdrant-demo
```

### 3. 下载 Chinese-CLIP 模型（392MB）

模型未包含在仓库中，需手动下载并放到指定路径。

```bash
# 魔搭社区（国内推荐，速度快）
git clone https://www.modelscope.cn/models/tiansz/chinese_clip-vit_large_patch14_quantized.git

# HuggingFace（需代理）
git clone https://huggingface.co/OFA-Sys/chinese-clip-vit-large-patch14

# 创建目录并复制模型文件
mkdir -p models/chinese-clip/onnx
cp chinese_clip-vit_large_patch14_quantized/onnx/model_quantized.onnx models/chinese-clip/onnx/
```

最终文件结构：
```
models/
└── chinese-clip/
    └── onnx/
        └── model_quantized.onnx    # 392MB
```

### 4. 配置环境变量

```bash
cp .env.example .env
```

编辑 `.env`，填入实际的 Qdrant 地址和 API Key：

```env
QDRANT_URL=http://<你的Qdrant地址>:6334
QDRANT_API_KEY=<你的API Key>
SERVER_HOST=0.0.0.0
SERVER_PORT=3000
HF_ENDPOINT=https://hf-mirror.com
```

### 5. 编译 & 启动

```bash
# 编译（首次需下载依赖，约 10-20 分钟）
cargo build --release

# 启动
./target/release/qdrant-vector
```

成功启动日志：
```
模型加载完成: 文本=512维 (BGE), 图像=768维 (Chinese-CLIP)
Qdrant 连接成功
collection [question] 就绪
服务启动: http://0.0.0.0:3000
```

### 6. 验证

```bash
curl http://localhost:3000/health
# 返回: OK
```

## API 接口

| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/api/question/upsert` | 新增/更新题目向量（id 存在则覆盖） |
| POST | `/api/question/delete` | 批量删除题目向量 |
| POST | `/api/question/search` | 题目搜索（纯文本 / 多模态 / 纯图片合一） |
| GET  | `/health` | 健康检查 |

搜索三种模式：

```json
// 纯文本
{ "query_text": "一年级 看图列式" }

// 纯图片（base64）
{ "query_image": "iVBORw0KGgoAAAANS..." }

// 图文混合（RRF 融合，精准度最高）
{ "query_text": "看图列式", "query_image": "base64..." }
```

详细接口文档见 [API.md](API.md)。

## 目录结构

```
├── .env.example                # 环境配置模板
├── .gitignore
├── Cargo.toml / Cargo.lock
├── API.md                      # 接口文档
├── models/
│   └── chinese-clip/
│       └── onnx/
│           └── model_quantized.onnx   # 需手动下载
├── .fastembed_cache/           # BGE 模型缓存（首次启动自动生成）
└── src/
    ├── main.rs                 # 服务入口
    ├── config.rs               # 配置加载
    ├── error.rs                # 统一错误处理
    ├── handler/
    │   ├── mod.rs              # 路由注册
    │   └── sync.rs             # upsert/search/delete 实现
    ├── model/
    │   ├── question.rs         # 题目数据模型+校验
    │   └── search.rs           # 搜索请求/响应
    └── service/
        ├── embedding.rs        # BGE 文本 + Chinese-CLIP 图像向量化
        └── qdrant.rs           # Qdrant named vectors + RRF 融合搜索
```

## 搜索原理

1. **文本向量**：HTML 去标签 → BGE-small-zh 生成 3 个 512 维向量（stem/analysis/knowledge）
2. **图像向量**：图片 resize 224×224 → normalize → Chinese-CLIP ONNX 推理 → 768 维
3. **RRF 融合**：Qdrant 对 4 路向量做 Reciprocal Rank Fusion 排序

Chinese-CLIP 专为中文图文理解训练，能有效区分不同年级的题目特征（如低年级算式图中的方框、减号）。
