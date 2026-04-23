# Select2Explain 技术架构

## 1. 总体结构

当前实现采用 Tauri 2 + Rust + React + TypeScript。主窗口负责配置，popup 窗口负责展示解释；托盘提供后台常驻入口；Rust 负责跨应用采集、AI 请求编排、系统热键和窗口事件分发。

## 2. 核心流程

### 2.1 后台监听主链路

1. `main.rs` 在 `setup` 阶段创建主窗口、popup 窗口和 tray。
2. `selection_monitor` 后台任务在 Windows 上会持续观察鼠标左键状态，只在一次按下后松开并经过短暂稳定延迟后，才调用平台层的 `poll_selection()` 做一次选区探测；其他平台暂时仍保留轮询兜底。
3. 若监听开关关闭，直接跳过并清空上次选区签名。
4. 若拿到新的选区签名，并且不是本应用窗口，则进入处理流程。
5. 处理流程调用 `capture_context()` 获取当前选区、窗口文本和窗口元信息。
6. `services/ai_pipeline.rs` 直接将“选区 + 当前窗口全文”送入 text-only 管线，请求一次 AI 解释。
7. loading 阶段后端只显示 popup，不主动抢占前台窗口焦点，避免污染当前宿主窗口的全文采集。
8. AI 返回结果后，后端再把 popup 切到可交互状态并聚焦，随后 emit `PopupPayload` 给 popup 窗口。
9. 前端 popup 根据鼠标坐标重新定位、按内容重新计算窗口高度，并在失焦时隐藏；后端也会轮询做一次失焦隐藏兜底。

### 2.3 系统热键

- 当前注册全局热键 `Ctrl + Alt + Q`。
- 热键会直接切换 `translation_enabled`，并立即持久化到本地设置。
- 控制面板打开时会接收 `settings:changed` 事件，保持 UI 与热键状态一致。

### 2.2 去重策略

- `state::RuntimeState` 记录 `last_selection_signature` 和 `in_flight`。
- 同一签名的选区不会重复请求。
- 没有选区时会清空签名，保证用户再次选中相同内容仍然可以触发。

## 3. 模块划分

### 3.1 前端

- `src/App.tsx`
	- `main` 窗口：控制面板、Provider 设置、Key 管理、连接测试、日志开关。
	- `popup` 窗口：解释卡片、错误展示、鼠标邻近定位、失焦自动隐藏。
- `src/styles.css`
	- 主控制面板和 popup 的统一视觉语言。

### 3.2 后端

- `src-tauri/src/main.rs`
	- tray、popup、窗口关闭转隐藏、后台监听、系统热键和 popup 事件分发。
- `src-tauri/src/commands/mod.rs`
	- 设置读取/保存、API Key 存储、手动连接测试。
- `src-tauri/src/platform/mod.rs`
	- 平台无关数据结构和分发入口。
- `src-tauri/src/platform/windows.rs`
	- Windows 采集实现。
- `src-tauri/src/services/ai_pipeline.rs`
	- 单阶段 text-only AI 管线。
- `src-tauri/src/providers/openai.rs`
	- OpenAI 兼容 HTTP 客户端。
- `src-tauri/src/state/mod.rs`
	- `ProviderSettings`、`RuntimeState`、`PopupPayload` 等共享类型。

## 4. Windows 实现细节

### 4.1 为什么改为原生 WinAPI + UI Automation

Windows 采集路径已经不再依赖 PowerShell 子进程，也不再通过运行时拼装脚本调用 `pwsh -NoProfile -STA -Command`。当前实现直接在 Rust 进程内调用 Win32 API 和 UI Automation 完成探测与上下文采集。

这个方案的主要收益：

- 不再额外拉起 PowerShell 前台进程，降低焦点扰动和轮询抖动。
- probe 阶段不再使用剪贴板 `Ctrl+C` 兜底，避免某些编辑器在“无选区时复制当前行”而引发循环弹窗。
- 鼠标释放、窗口元信息和选区读取都在同一进程内完成，超时、日志和错误边界更清晰。
- 彻底绕开脚本扫描与 PowerShell 执行策略带来的额外不确定性。

### 4.2 Windows 采集内容

`platform/windows.rs` 当前会收集：

- 前台窗口标题与进程名
- 当前鼠标屏幕坐标
- 当前窗口矩形区域
- UI Automation 选区文本
- 窗口全文摘要

采集分为两个模式：

- `probe`：只取选区与窗口元信息；默认发生在鼠标左键释放后的轻量探测阶段，只走无副作用的 UI Automation 读取，不触发剪贴板兜底。
- `full`：补取窗口全文；必要时才允许走原生剪贴板兜底，供 AI 请求前的完整上下文采集使用。

## 5. popup 与窗口管理

### 5.1 popup 窗口属性

- 无边框
- 透明宿主窗口
- 置顶
- 跳过任务栏
- 默认隐藏
- 由后端在运行时创建，不写死在 `tauri.conf.json`
- 内容由前端绘制为圆角卡片，不保留可见标题条或拖拽条

### 5.2 popup 定位策略

- 前端根据后端传回的鼠标物理坐标调用 `monitorFromPoint()`。
- 结合当前 popup 尺寸和当前屏幕 `workArea` 做边界裁剪。
- 优先出现在鼠标右上方；上方空间不足时，退回到鼠标下方。
- popup 高度不再固定，前端会按卡片内容重新测量窗口高度；若内容过长，则在屏幕工作区允许的最大高度内增长，并由解释区自行滚动。

### 5.3 隐藏策略

- popup 在显示后会显式聚焦，并监听 `WINDOW_BLUR`；点击其他位置时会自动隐藏。
- 若前端 `WINDOW_BLUR` 事件缺失，后端仍会在监听循环中检查“popup 已显示且失焦”的状态，并主动隐藏窗口。
- 主窗口关闭时不销毁，只转为隐藏，保证后台监听和 tray 继续运行。

## 6. 配置与安全

- Provider 设置通过 `services/settings_store.rs` 落到本地 `settings.json`，前端通过 `get_settings` / `save_settings` 读写。
- API Key 使用 `keyring` crate 写入系统安全存储。
- 默认 Base URL 为 `https://openrouter.ai/api/v1`。
- 默认文本模型为 `google/gemini-2.5-flash-lite`。
- 调试日志写入本地 `runtime.log`，路径位于系统本地应用数据目录下的 `Select2Explain` 目录。

### 6.1 构建缓存与脚本

- 仓库根 `.cargo/config.toml` 已固定 `target-dir` 到 `src-tauri/target`，避免每轮构建重新生成新的目标目录。
- `npm run tauri:build` 现在走 `tauri build --no-bundle`，只产出 release 可执行文件，不再在日常验证时触发 WiX/NSIS 安装器下载。
- 仅当需要安装包时再执行 `npm run tauri:bundle`。

## 7. 调试能力

当前调试入口以本地日志为主：

- `runtime.log`：监听状态、popup 显示/隐藏与概要错误。
- `interaction.log`：更细的采集、AI 请求与 provider 返回轨迹。

## 8. macOS 代码路径

当前已经补出独立的 `platform/macos.rs`，并与 Windows 共用同一套 `poll_selection()` / `capture_context()` 接口。macOS 路径目前包含：

1. 通过 Accessibility 读取 `AXSelectedText`。
2. 通过 `System Events` 读取前台应用名与窗口标题。
3. 通过 JXA + AppKit 读取鼠标坐标。
4. 通过 `screencapture` 获取截图。
5. 通过本地日志记录 probe / capture 成功与失败信息。

这部分已经完成代码实现和接口对齐，但由于当前开发机不是 macOS，本轮没有做实际编译与运行验收。

## 9. 下一步技术方向

1. 继续优化原生 Windows 采集，重点验证更多宿主应用下的 UI Automation 覆盖率。
2. 补充 popup 内的复制、固定和重试动作。
3. 为 macOS 做单独验收，重点验证鼠标坐标换算、权限引导和 popup 定位。

这些信息应写入本地日志，便于快速排查跨平台问题。

## 9. 测试策略

### 9.1 单元测试

- Prompt 构造
- Provider 适配
- 数据持久化
- 窗口定位算法

### 9.2 集成测试

- 从触发解释到结果入库的完整链路
- 配置切换与 Provider 切换

### 9.3 手工回归

这个项目大量依赖系统行为，因此必须保留人工验证清单：

- 多屏幕
- 不同 DPI 缩放
- 常见宿主应用
- 失焦和重新聚焦行为
- 权限首次授予与撤销后的表现

## 10. 目录规划建议

后续初始化工程时建议采用如下结构：

```text
Select2Explain/
├── src/
│   ├── app/
│   ├── features/
│   ├── shared/
│   └── pages/
├── src-tauri/
│   ├── src/
│   │   ├── commands/
│   │   ├── services/
│   │   ├── providers/
│   │   ├── storage/
│   │   └── platform/
│   └── Cargo.toml
├── docs/
└── package.json
```

## 11. 当前阶段结论

若目标是尽快做出一个真正可用、可常驻、可扩展的跨平台桌面解释工具，那么 Tauri 2 + Rust + React/TypeScript 是当前最稳妥的首选方案。