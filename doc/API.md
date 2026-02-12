# Arcaferry Pro API

## 1. 基本信息

- 默认地址：`http://127.0.0.1:17236`
- 默认端口：`17236`（环境变量 `ARCAFERRY_PORT` 可覆盖）
- 数据格式：`application/json`

## 2. 接口列表

| Method | Path | 说明 |
|---|---|---|
| GET | `/api/status` | 服务状态与能力信息 |
| POST | `/api/scrape` | 单条链接抓取 |
| POST | `/api/batch` | 批量抓取 |
| POST | `/api/import` | 导入模式（URL 或 JSON 输入） |
| POST | `/api/preview` | 预览信息 |
| GET | `/api/debug/tls` | TLS 调试信息（排障用途） |

## 3. 认证与 Cloudflare

在需要登录态或 Cloudflare 校验的场景，应同时提供：

- `cookies`（建议包含 `cf_clearance`）
- `user_agent`（应与生成 cookie 的浏览器一致）

当 `cookies` 与 `user_agent` 不匹配时，可能返回 `CLOUDFLARE_BLOCKED`。

## 4. 主要接口说明

## 4.1 `GET /api/status`

返回服务状态、版本和浏览器提取能力。

关键字段：

- `status`
- `version`
- `ready`
- `port`
- `supported_browsers`
- `browser_extraction_available`
- `browser_extraction_reason`

## 4.2 `POST /api/scrape`

用于抓取单条角色卡链接。

请求示例：

```json
{
  "url": "https://quack.im/discovery/share/xxx",
  "cookies": "cf_clearance=...; ...",
  "bearer_token": "...",
  "user_agent": "Mozilla/5.0 ...",
  "gemini_api_key": "...",
  "output_format": "json"
}
```

字段说明：

- `url`：必填
- `output_format`：`json`（默认）或 `png`
- `cookies` / `bearer_token` / `user_agent` / `gemini_api_key`：可选

返回说明：

- `success=true` 时返回 `card`
- `output_format=json` 时可返回 `avatar_base64`
- `output_format=png` 时可返回 `png_base64`
- `warnings` 用于提示降级信息（例如隐藏设定提取不可用）

## 4.3 `POST /api/batch`

用于批量抓取。

请求示例：

```json
{
  "urls": [
    "https://quack.im/discovery/share/a",
    "https://quack.im/discovery/share/b"
  ],
  "cookies": "cf_clearance=...; ...",
  "bearer_token": "...",
  "user_agent": "Mozilla/5.0 ...",
  "gemini_api_key": "...",
  "concurrency": 3,
  "output_format": "json"
}
```

说明：

- `concurrency` 默认值 `3`，服务端限制范围 `1~5`
- 返回体包含 `total`、`succeeded`、`failed`、`results`

## 4.4 `POST /api/import`

用于导入模式，支持 URL 输入或直接传 JSON。

请求示例：

```json
{
  "quack_input": "https://quack.im/discovery/share/xxx",
  "lorebook_json": "{...}",
  "cookies": "cf_clearance=...; ...",
  "bearer_token": "...",
  "user_agent": "Mozilla/5.0 ...",
  "gemini_api_key": "...",
  "mode": "full",
  "output_format": "json"
}
```

关键字段：

- `mode`：`full`（默认）/ `only_lorebook`
- `output_format`：`json` / `png`

## 4.5 `POST /api/preview`

返回预览信息，不执行完整导出。

请求示例：

```json
{
  "quack_input": "https://quack.im/discovery/share/xxx",
  "cookies": "cf_clearance=...; ...",
  "bearer_token": "...",
  "user_agent": "Mozilla/5.0 ..."
}
```

## 4.6 `GET /api/debug/tls`

返回 TLS 调试信息。该接口用于排障，不属于常规业务流程。

## 5. 常见错误码

| HTTP | error_code | 含义 |
|---|---|---|
| 400 | `INVALID_URL` | 输入 URL/ID 无法解析 |
| 400 | `PARSE_ERROR` | 输入或上游响应解析失败 |
| 401 | `UNAUTHORIZED` | 认证失败 |
| 429 | `RATE_LIMITED` | 请求频率受限 |
| 502 | `NETWORK_ERROR` | 网络或上游异常 |
| 503 | `CLOUDFLARE_BLOCKED` | Cloudflare 校验失败 |
| 504 | `TIMEOUT` | 请求超时 |

## 6. 安全说明

- 不要在仓库中保存 `cookies`、`bearer_token`、`gemini_api_key`
- 不要在日志中输出敏感信息
- 仅在可信环境存储认证信息

## 7. 最小示例

```bash
curl -X POST "http://127.0.0.1:17236/api/scrape" \
  -H "Content-Type: application/json" \
  -d '{
    "url": "https://quack.im/discovery/share/your_id",
    "output_format": "json"
  }'
```
