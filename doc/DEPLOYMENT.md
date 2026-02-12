# Arcaferry Pro 部署文档

## 1. 部署模式

Arcaferry Pro 提供两种部署模式：

- **Slim**：仅后端服务，不包含 Sidecar
- **Full**：后端服务 + Python Sidecar（支持隐藏设定提取）

## 2. 本地部署（非 Docker）

## 2.1 Slim

```bash
cd src-tauri
ARCAFERRY_PORT=17236 cargo run --bin server
```

## 2.2 Full

安装 Sidecar 依赖：

```bash
pip install -r scripts/requirements.txt
python -m playwright install firefox
```

启动后端服务：

```bash
cd src-tauri
ARCAFERRY_PORT=17236 cargo run --bin server
```

## 3. Docker 部署

仓库已提供 `Dockerfile` 和 `docker-compose.yml`。

## 3.1 使用 docker compose

### 3.1.1 Slim

```bash
docker compose --profile slim up --build -d
```

### 3.1.2 Full

```bash
docker compose --profile full up --build -d
```

## 3.2 使用 docker run

### 3.2.1 Slim

```bash
# 构建镜像
docker build --target slim -t arcaferry:slim .

# 启动容器
docker run -d --name arcaferry-slim \
  -p 17236:17236 \
  -e ARCAFERRY_PORT=17236 \
  --restart unless-stopped \
  arcaferry:slim
```

### 3.2.2 Full

```bash
# 构建镜像
docker build --target full -t arcaferry:full .

# 创建 sidecar profile 卷
docker volume create arcaferry-sidecar-profile

# 启动容器
docker run -d --name arcaferry-full \
  -p 17236:17236 \
  -e ARCAFERRY_PORT=17236 \
  -e ARCAFERRY_SIDECAR_SCRIPT_PATH=/app/scripts/extract_hidden.py \
  -e ARCAFERRY_SIDECAR_PROFILE_DIR=/data/sidecar-profile \
  -v arcaferry-sidecar-profile:/data/sidecar-profile \
  --restart unless-stopped \
  arcaferry:full
```

`arcaferry-slim` 与 `arcaferry-full` 默认都映射主机 `17236` 端口。请按需只启动其中一个，或修改映射端口。

## 4. 启动验证

```bash
curl http://127.0.0.1:17236/api/status
```

重点检查字段：

- `ready`
- `browser_extraction_available`

## 5. 环境变量

| 变量 | 说明 |
|---|---|
| `ARCAFERRY_PORT` | 服务端口（默认 17236） |
| `ARCAFERRY_SIDECAR_TIMEOUT_SECS` | Sidecar 超时（秒） |
| `ARCAFERRY_SIDECAR_PROFILE_DIR` | Sidecar 浏览器 Profile 目录 |
| `ARCAFERRY_SIDECAR_SCRIPT_PATH` | Sidecar 脚本路径覆盖 |
| `ARCAFERRY_SIDECAR_DEBUG` | Sidecar 调试输出开关 |
| `ARCAFERRY_AVATAR_TIMEOUT_SECS` | 头像下载超时（秒） |

## 6. 常见问题

### 6.1 隐藏设定未提取

优先检查：

- 是否使用 Full 模式
- Sidecar 依赖是否安装完成
- `cookies` 与 `user_agent` 是否匹配

### 6.2 批量抓取吞吐偏低

- 调整 `concurrency` 参数（有效范围 `1~5`）
- Hidden Sidecar 提取为串行保护，属于设计行为

## 7. 相关文档

- API 文档：`doc/API.md`
- Lite 扩展文档：`doc/LITE.md`
