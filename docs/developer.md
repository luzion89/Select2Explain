# Select2Explain 开发者文档

## 当前技术栈

- 桌面宿主：Tauri 2.x
- 前端：React 18 + TypeScript 5 + Vite 5
- Rust crates：`tauri`、`tauri-plugin-shell`、`reqwest`、`tokio`、`serde_json`、`anyhow`、`keyring`、`base64`、`dirs`
- 当前实现平台：Windows

## 项目结构

```text
Select2Explain/
├── src/
│   ├── App.tsx             # 主控制面板 + popup 窗口入口
│   ├── main.tsx            # React 入口
│   └── styles.css          # 主窗口与 popup 样式
├── src-tauri/
│   ├── capabilities/
│   │   └── default.json    # main / popup 权限声明
│   ├── icons/
│   └── src/
│       ├── commands/       # Tauri IPC 命令
│       ├── platform/       # Windows 采集 + macOS 设计路径
│       ├── providers/      # OpenAI 兼容 Provider
│       ├── services/       # AI pipeline / screenshot
│       ├── state/          # 共享状态与 payload
│       └── main.rs         # tray、popup、后台监听入口
└── docs/
```

## 当前实现重点

### 1. 后台监听

- `main.rs` 中的 `selection_monitor` 每 650ms 轮询一次 `platform::poll_selection()`。
- 只有在 `translation_enabled` 为 `true` 时才会处理新的选区。
- `RuntimeState` 用于防止重复触发与并发请求堆积。

### 2. Windows 平台采集

- `platform/windows.rs` 使用 Rust 进程内的 Win32 API + UI Automation + GDI 原生实现。
- 通过 Win32 + UI Automation + GDI 采集：
   - 前台应用名
   - 窗口标题
   - 鼠标坐标
   - 选中文本
   - 窗口全文摘要
   - 窗口截图
- 采集模式分成 `probe` 和 `full`，以降低后台轮询成本。
- 当某个前台窗口没有可访问选区时，会把“空选区”诊断写入本地日志，避免 release 形态下静默失败。

### 2.1 macOS 平台采集

- `platform/macos.rs` 现在也实现了 `poll_selection()` 和 `capture_context()`。
- 通过 Accessibility 读取 `AXSelectedText`。
- 通过 `System Events` 获取前台应用和窗口标题。
- 通过 JXA + AppKit 获取鼠标坐标。
- 通过 `services/screenshot.rs` 的 `screencapture` 路径生成截图。
- 这部分代码尚未在本轮环境做编译或真机测试。

### 3. AI 管线

`services/ai_pipeline.rs` 负责渐进式上下文策略：

1. 先把选区和截图发给视觉模型，判断截图是否足够理解语境。
2. 若足够且有窗口全文，则转给文本模型生成解释。
3. 若不足，则补发窗口全文。

### 4. popup 窗口

- popup 由 Rust 在运行时创建，不写在 `tauri.conf.json` 中。
- 前端 popup 收到 `popup:show` 后会根据鼠标坐标和当前屏幕边界重新定位。
- popup 监听 `WINDOW_BLUR`，失焦即隐藏。

### 5. 设置与日志

- `services/settings_store.rs` 会把 Provider 配置、监听开关和调试日志开关落到本地 `settings.json`。
- `services/debug_log.rs` 会把运行时 probe / capture / AI pipeline / popup 事件写入本地 `runtime.log`。
- 这些文件默认位于系统本地应用数据目录下的 `Select2Explain` 目录。

## Tauri 命令

| 命令 | 参数 | 返回 | 用途 |
|---|---|---|---|
| `get_settings` | - | `{ baseUrl, model, visionModel, translationEnabled, debugLoggingEnabled, apiKeyConfigured, debugLogPath }` | 读取当前设置 |
| `save_settings` | `settings` | `()` | 保存 Base URL / 模型 / 自动监听开关 / 日志开关 |
| `save_api_key` | `key` | `()` | 保存 API Key |
| `delete_api_key` | - | `()` | 删除 API Key |
| `test_connection` | - | `u64` | 验证 Provider 连通性 |
| `explain_selection` | `selectedText` | `ExplainResponse` | 手动执行解释 |
| `debug_probe_selection` | - | `SelectionProbe | null` | 调试当前选区探测 |
| `debug_capture_context` | - | `{ available, snapshot }` | 调试完整上下文采集 |

## 开发命令

```bash
npm install
npm run build

cd src-tauri
cargo --config "source.crates-io.replace-with='rsproxy'" --config "source.rsproxy.registry='sparse+https://rsproxy.cn/index/'" check

cd ..
npm run tauri:dev
```

## 当前已知限制

- Windows 采集已改为原生 Win32/UI Automation 路径，但不同宿主应用的 UI Automation 暴露能力仍有兼容性差异。
- 当前 Windows 选区读取对常见应用的兼容性不足；本轮复测中，记事本和 VS Code 都没有通过 UI Automation 暴露可用选区，因此会出现“监听已开但没有 popup”的现象。
- `debug_capture_context` 当前会返回完整截图 base64，适合调试，但不适合长期作为常规接口暴露。
- macOS 已补齐代码路径，但没有在本轮做实际验收。
