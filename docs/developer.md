# Select2Explain 开发者文档

## 技术栈

- **框架**: Tauri 2.x + React 18 + TypeScript 5 + Vite 5
- **Rust crates**: `tauri`, `tauri-plugin-shell`, `reqwest` (rustls-tls), `tokio`, `serde_json`, `anyhow`, `keyring 2.3.3`, `base64`
- **平台**: macOS（Accessibility API via osascript）

---

## 项目结构

```
Select2Explain/
├── src/                    # React 前端
│   ├── App.tsx             # 主组件（Settings + Monitor 视图）
│   ├── main.tsx            # React 入口
│   └── styles.css          # 暗色主题样式
├── src-tauri/
│   └── src/
│       ├── main.rs         # Tauri 入口 + 后台轮询任务
│       ├── commands/       # Tauri 命令（IPC 层）
│       ├── state/          # 共享状态类型
│       ├── platform/       # macOS Accessibility 封装
│       ├── providers/      # AI API 客户端（OpenAI 兼容）
│       └── services/
│           ├── screenshot.rs   # screencapture 封装
│           └── ai_pipeline.rs  # 两阶段 AI 流水线
└── docs/                   # 文档
```

---

## 架构说明

### 选中文字检测

`main.rs` 中通过 `tauri::async_runtime::spawn` 启动后台任务 `selection_monitor`，每 600ms 调用一次 `platform::get_selected_text()`。

`platform/mod.rs` 使用 osascript 读取前台应用的 `AXFocusedUIElement` 上的 `AXSelectedText` 属性。若检测到文字变化，向所有窗口 emit `"selection-changed"` 事件。

> 注意：`AXSelectedText` 需从 `AXFocusedUIElement`（焦点 UI 元素）读取，而非从 `front window`。绝大多数应用只在聚焦的文本元素上暴露该属性。

osascript 内置了自身进程过滤（`if name of frontApp is "select2explain" then return ""`），避免应用自己触发自己。

### 两阶段 AI 流水线（`ai_pipeline.rs`）

1. **Stage 1 - 视觉判断**: 截取当前窗口截图，发给视觉模型，判断截图上下文是否足以理解选中文字的含义
2. **Stage 2 - 文本解释**:
   - 若截图充分 + 有窗口文字 → 文本模型 + 上下文
   - 若截图充分 + 无窗口文字 → 视觉模型直接解释
   - 若截图不充分 → 文本模型 + 完整窗口文字

### API 密钥存储

使用 macOS Keychain（`keyring` crate），服务名 `com.tuntun.select2explain`，用户名 `api_key`。

---

## 开发命令

```bash
# 启动开发模式（热重载）
npm run tauri:dev

# TypeScript 类型检查
npx tsc --noEmit

# Rust 检查（不链接）
cd src-tauri && cargo check

# 生产构建
npm run tauri:build
```

---

## Tauri 命令列表

| 命令 | 参数 | 返回 | 说明 |
|------|------|------|------|
| `get_settings` | - | `{ baseUrl, model, visionModel, apiKeyConfigured }` | 读取当前配置 |
| `save_settings` | `settings: ProviderSettings` | `()` | 保存 Provider 配置 |
| `save_api_key` | `key: String` | `()` | 写入 Keychain |
| `delete_api_key` | - | `()` | 清除 Keychain 中的 Key |
| `explain_selection` | `selectedText: String` | `ExplainResponse` | 运行两阶段 AI 解释 |
| `test_connection` | - | `u64` (latency_ms) | 验证 API Key 和连接性 |

---

## 已知限制

- 仅支持 macOS（Accessibility API 依赖 osascript）
- 沙盒严格的 App Store 应用可能不暴露 `AXSelectedText`
- 截图使用 `screencapture -x -o -m`，需屏幕录制权限（macOS 10.15+）
