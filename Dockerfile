FROM ubuntu:24.04

# 安装运行时依赖（ONNX runtime 需要）
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY qdrant-demo .
COPY chinese-clip ./models/chinese-clip/

ENV HF_ENDPOINT=https://hf-mirror.com

EXPOSE 3000

CMD ["./qdrant-demo"]
