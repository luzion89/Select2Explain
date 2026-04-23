# Select2Explain

Select2Explain 是一个常驻系统托盘的桌面小工具。用户在任意应用里用鼠标选中一段文字后，应用会自动抓取当前选区、当前窗口截图，并在必要时补充窗口全文，再把 AI 解释弹到鼠标右上方；点击任意其他位置后，浮窗自动隐藏。

## 当前状态

- Windows 主链路已落地：托盘常驻、自动监听、popup 浮窗、OpenAI 兼容 Provider、系统凭据存储、渐进式上下文 AI 管线。
- 运行时新增本地设置与日志：监听开关、日志开关会落到本地 `settings.json`，调试信息会写入本地 `runtime.log`。
- macOS 仍保留设计路径与部分旧实现，但本轮没有做运行级验证，也没有宣称与 Windows 等价。
- 调试入口已加入主控制面板，可直接验证“选区探测”和“完整采集”。

## 核心行为

1. 应用启动后显示控制面板，同时在后台保持运行。
2. 用户保存 Provider 配置与 API Key，并开启“自动监听”。
2.1 如需排查问题，可同时开启“本地调试日志”。
3. 后端检测到新的选区签名时，先采集选中文本和当前窗口截图。
4. AI 先基于“选中文本 + 截图”判断上下文是否足够；不足时再补发窗口全文。
5. popup 在鼠标右上方显示解释；失焦后自动隐藏。
6. 主窗口关闭时不会退出程序，可通过托盘菜单重新打开。

## 文档索引

- [产品规划](./docs/product-plan.md)
- [技术架构](./docs/technical-architecture.md)
- [测试清单](./docs/testing-checklist.md)
- [用户指南](./docs/user-guide.md)
- [开发者文档](./docs/developer.md)

## 技术栈

- 桌面框架：Tauri 2
- 宿主层：Rust
- 前端：React 18 + TypeScript + Vite
- AI 接入：OpenAI 兼容 Provider 抽象
- 密钥存储：Windows Credential Manager / macOS Keychain（通过 keyring crate）

## 目录概览

```text
Select2Explain/
├── docs/
├── src/
│   ├── App.tsx
│   ├── main.tsx
│   └── styles.css
├── src-tauri/
│   ├── capabilities/
│   ├── icons/
│   └── src/
│       ├── commands/
│       ├── platform/
│       ├── providers/
│       ├── services/
│       └── state/
└── package.json
```

## 本地开发前置条件

- Node.js 20+
- npm 10+
- Rust stable toolchain
- Windows 上建议已安装 WebView2 Runtime

## 常用命令

```bash
npm install
npm run build
cd src-tauri && cargo check
npm run tauri:dev
```

如果 `cargo` 访问官方 registry 较慢，可以在 Windows 上改用 rsproxy：

```bash
cd src-tauri
cargo --config "source.crates-io.replace-with='rsproxy'" --config "source.rsproxy.registry='sparse+https://rsproxy.cn/index/'" check
```

## 本轮已验证

- `npm run build`
- `cargo check`（使用 rsproxy）
- OpenRouter 实际 key 的连通性
- Windows 前景窗口、鼠标坐标、UI Automation 文本采集、截图能力
- release 启动会正确加载本地设置与日志开关
- 当前 Windows 选区读取在记事本、VS Code 这类常见应用里仍有兼容性缺口

## 下一步

1. 继续验证和优化原生 Windows 采集覆盖率，重点处理 VS Code、记事本这类宿主里的 UI Automation 兼容性缺口。
2. 为 popup 增加复制、固定和重新解释操作。
3. 为 macOS 补齐与 Windows 一致的后台监听和 popup 交互，再做单独验收。