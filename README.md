<div align="center">
  <img src="./extension/icons/icon_tarot.svg" alt="Arcaferry Logo" width="120" height="120" />
  <h1>Arcaferry</h1>
  <p>✨ 角色卡提取与转换工具 | Character Card Extraction & Conversion Toolkit</p>

  <p>
    <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-Backend-000000?style=flat-square&logo=rust" alt="Rust"></a>
    <a href="https://github.com/tokio-rs/axum"><img src="https://img.shields.io/badge/Axum-API-5E4DB2?style=flat-square" alt="Axum"></a>
    <a href="https://developer.chrome.com/docs/extensions/develop/migrate/what-is-mv3"><img src="https://img.shields.io/badge/MV3-Extension-4285F4?style=flat-square&logo=googlechrome" alt="MV3"></a>
    <a href="https://playwright.dev/"><img src="https://img.shields.io/badge/Playwright-Sidecar-2EAD33?style=flat-square&logo=playwright" alt="Playwright"></a>
    <a href="https://www.docker.com/"><img src="https://img.shields.io/badge/Docker-Deploy-2496ED?style=flat-square&logo=docker" alt="Docker"></a>
  </p>
</div>

---

便捷的角色卡提取与转换工具，从类酒馆平台提取角色卡数据并转换为通用 CCv3 格式。

项目提供两种使用方式：

- **Arcaferry Pro（后端服务）**：功能完整，支持批量抓取、隐藏设定补全，可与 [Arcamage](https://github.com/Shizuku-Yume/Arcamage) 联动。
- **Lite（浏览器扩展）**：开箱即用，适合快速提取单张卡片，无需部署后端。

---

## 技术栈

| 层级 | 技术 |
|------|------|
| Pro（后端服务） | Rust · Axum · wreq（TLS 指纹） |
| Lite（浏览器扩展） | MV3 · JavaScript |
| Sidecar（可选） | Python · Camoufox · Playwright |
| 部署 | Docker 多阶段构建（Slim / Full），单容器运行 |

---

## 支持范围

### 已支持

- 平台：**Quack**（`quack.im`）
- 链接类型：`.../discovery/share/...`、`.../dream/...`、`.../studio/card/...`
- 世界书（Lorebook）提取
- 隐藏设定提取（Pro，需安装 Sidecar）
- 输出 CCv3 JSON / PNG（Pro）

### 暂未支持

- Quack 以外的平台

---

## 核心功能

### Arcaferry Pro（后端服务）

Arcaferry Pro 基于 Rust 构建，以 HTTP 服务形式运行，默认端口 `17236`。

| 接口 | 功能 |
|------|------|
| `/api/scrape` | 单卡抓取 |
| `/api/batch` | 批量抓取 |
| `/api/import` | 导入模式 |
| `/api/preview` | 预览模式 |
| `/api/status` | 服务状态检测 |

Pro 同时提供客户端模块，可接入 [Arcamage](https://github.com/Shizuku-Yume/Arcamage) 工作流。

#### 关于隐藏设定

Pro 支持隐藏设定提取，但该功能依赖 **Python Sidecar（Camoufox + Playwright）**。若未安装 Sidecar，基础抓取仍可正常使用，隐藏设定部分将降级并给出告警提示。

- **需要完整提取隐藏设定** → 使用包含 Sidecar 的完整环境（Full 包 / Full 镜像）
- **仅需基础卡片数据** → Slim 环境即可满足

### Lite（浏览器扩展）

Lite 是基于 MV3 的浏览器扩展，定位为轻量级的单卡提取方案。

- 无需运行 Rust 后端，直接在 Quack 页面内完成提取
- 交互简洁，上手即用
- 适合快速导出单张卡片，不希望搭建后端的场景

> Lite 侧重于轻便易用，不适用于批量场景。如需批量抓取，请使用 Pro。

---

## 快速开始

### Docker Compose（推荐）

```bash
# Slim 模式（不含 Sidecar）
docker compose --profile slim up --build -d

# Full 模式（含 Sidecar，支持隐藏设定提取）
docker compose --profile full up --build -d
```

### Docker Run

```bash
# Slim：构建并启动
docker build --target slim -t arcaferry:slim .
docker run -d --name arcaferry-slim \
  -p 17236:17236 \
  -e ARCAFERRY_PORT=17236 \
  --restart unless-stopped \
  arcaferry:slim
```

```bash
# Full：构建并启动
docker build --target full -t arcaferry:full .
docker volume create arcaferry-sidecar-profile

docker run -d --name arcaferry-full \
  -p 17236:17236 \
  -e ARCAFERRY_PORT=17236 \
  -e ARCAFERRY_SIDECAR_SCRIPT_PATH=/app/scripts/extract_hidden.py \
  -e ARCAFERRY_SIDECAR_PROFILE_DIR=/data/sidecar-profile \
  -v arcaferry-sidecar-profile:/data/sidecar-profile \
  --restart unless-stopped \
  arcaferry:full
```

> Slim 与 Full 默认均占用 17236 端口，请按需选择其一启动。

### 本地开发（Pro）

```bash
cd src-tauri
ARCAFERRY_PORT=17236 cargo run --bin server
```

启动后可通过 `GET http://127.0.0.1:17236/api/status` 验证服务状态。

### Lite 安装（开发者模式）

1. 打开 Chromium 内核浏览器的扩展管理页（如 `chrome://extensions`）
2. 开启「开发者模式」
3. 点击「加载已解压的扩展程序」
4. 选择本仓库的 `extension/` 目录

在浏览器中打开 Quack 的 share 或 dream 页面，点击扩展图标，按提示完成提取，导出为 JSON 或 PNG。

---

## 开发命令

Pro（在 `src-tauri/` 下执行）：

```bash
cargo test                                       # 运行测试
cargo clippy --all-targets -- -D warnings        # 代码检查
```

Sidecar 依赖（可选）：

```bash
pip install -r scripts/requirements.txt
python -m playwright install firefox
```

---

## 项目结构

```
arcaferry/
├── src-tauri/        # Arcaferry Pro（Rust 后端）
├── extension/        # Lite（MV3 浏览器扩展）
├── scripts/          # Sidecar 脚本（可选）
└── doc/              # 文档（API / 部署 / 参考）
```

---

## 文档

- [文档总览](doc/README.md)
- [API 接口参考](doc/API.md)
- [部署指南](doc/DEPLOYMENT.md)
- [Lite 扩展说明](doc/LITE.md)

---

## 使用声明

本项目仅供学习、交流与研究用途。请勿将提取所得数据用于商业目的或任何未经平台授权的用途。使用时请遵守目标平台的服务条款。
