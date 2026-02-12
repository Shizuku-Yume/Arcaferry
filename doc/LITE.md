# Arcaferry Lite 扩展文档

## 1. 说明

Arcaferry Lite 是 Manifest V3 浏览器扩展，目录为：

```text
extension/
```

该扩展用于 Quack 页面中的单卡提取流程。

## 2. 适用范围

- 平台：Quack
- 页面类型：
  - `https://quack.im/discovery/share/*`
  - `https://quack.im/dream/*`

## 3. 安装

1. 打开扩展管理页（例如 `chrome://extensions`）
2. 启用开发者模式
3. 选择“加载已解压扩展程序”
4. 选择仓库中的 `extension/` 目录

## 4. 使用流程

1. 打开支持的 Quack 页面
2. 点击扩展图标
3. 按界面步骤执行提取
4. 导出 JSON 或 PNG

## 5. 与 Arcaferry Pro 对比

| 维度 | Lite | Arcaferry Pro |
|---|---|---|
| 运行方式 | 浏览器扩展 | 后端服务 |
| 典型场景 | 单卡提取 | 批量与自动化 |
| 批量抓取 | 不支持主流程 | 支持 `/api/batch` |
| 隐藏设定 | 依赖页面流程 | Full + Sidecar 完整支持 |

## 6. 常见问题

### 6.1 点击开始后无结果

- 检查当前页面是否为支持路径
- 刷新页面后重试
- 在 dream 页面确认会话数据已可用

### 6.2 何时使用 Pro

当需求包含批量抓取、稳定自动化时，建议使用 Arcaferry Pro（Full 模式）。
