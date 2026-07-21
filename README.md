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
## 5.1 交叉编译（Windows → Linux）

项目依赖 `fastembed`（内嵌 ONNX Runtime），有原生 C/C++ 依赖，直接在 Windows 上交叉编译到 Linux 比较困难。推荐使用 `cross` 工具 + Docker 部署。

### 5.1.1 使用 cross 编译

```powershell
# 安装 cross
cargo install cross

# 编译（使用预配置的 Cross.toml）
cross build --release --target x86_64-unknown-linux-gnu

# 二进制在 target/x86_64-unknown-linux-gnu/release/qdrant-demo
```

### 5.1.2 Docker 部署

将编译好的二进制和模型文件部署到 Linux 服务器，用 Docker 运行。

**坑 1：glibc 兼容性**

cross 默认使用较新的 glibc（约 2.38），而 CentOS 7 只有 glibc 2.17。Docker 基础镜像必须选 `ubuntu:24.04`，原因是：

| 镜像 | glibc 版本 | 兼容？ |
|------|-----------|--------|
| CentOS 7 | 2.17 | ❌ |
| ubuntu:22.04 | 2.35 | ❌ |
| ubuntu:24.04 | 2.39 | ✅ |
| alpine | musl | ❌ ONNX 不支持 |

**坑 2：fastembed 模型缓存路径**

fastembed 默认使用**相对路径** `.fastembed_cache`（不是 HuggingFace 标准的 `~/.cache/huggingface`）。

```
# fastembed 默认 DEFAULT_CACHE_DIR = ".fastembed_cache"
```

容器内工作目录是 `/app`，所以实际缓存路径是 `/app/.fastembed_cache/`，挂载时要注意。

**坑 3：缓存目录命名**

fastembed 使用的模型 ID 是 `Xenova/bge-small-zh-v1.5`，缓存目录名是 `models--Xenova--bge-small-zh-v1.5`（注意是 `Xenova` 前缀，不是 `BAAI`）。

如果本地运行过 fastembed，缓存一般在 `~/.cache/fastembed/models--Xenova--bge-small-zh-v1.5/`。

**完整部署步骤：**

```bash
# 1. 上传文件到服务器 /www/rust-vector/
#    - qdrant-demo（二进制）
#    - models/chinese-clip/onnx/model_quantized.onnx
#    - .fastembed_cache/（BGE 模型缓存）

# 2. 构建镜像
docker build -t qdrant-demo:latest /www/rust-vector/

# 3. 启动（注意缓存挂载到 /app/.fastembed_cache）
docker run -d --name qdrant-demo \
  --network host \
  -v /www/rust-vector/.fastembed_cache:/app/.fastembed_cache \
  -e QDRANT_API_KEY=<your-key> \
  qdrant-demo:latest
```

Dockerfile 参考：

```dockerfile
FROM ubuntu:24.04

RUN apt-get update && apt-get install -y \
    ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY qdrant-demo .
COPY ./models/chinese-clip ./models/chinese-clip/

ENV HF_ENDPOINT=https://hf-mirror.com

EXPOSE 3000
CMD ["./qdrant-demo"]
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
