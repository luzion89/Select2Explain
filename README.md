# Select2Explain

Select2Explain 是一个面向 PC 场景的跨平台桌面应用，目标是在用户完成划词或选中一段描述后，结合当前上下文向 AI 发起解释请求，并在鼠标停留位置附近弹出临时窗口展示结果。

当前阶段已经完成首版工程骨架，包含 Tauri 宿主层、React 前端工作台、解释弹窗原型、配置页、历史页以及 Rust 命令接口占位实现。

## 产品目标

- 支持 macOS 和 Windows 桌面环境
- 提供接近“划词取译”的轻量交互，但输出内容是 AI 语义解释而非传统词典释义
- 尽可能理解当前上下文，而不是只解释孤立词语
- 采用临时悬浮窗展示结果，减少用户在不同应用之间切换
- 为后续引入 Prompt 模板、模型切换、历史记录和企业知识库扩展预留架构空间

## 文档索引

- [产品规划](./docs/product-plan.md)
- [技术架构](./docs/technical-architecture.md)
- [实施路线图](./docs/roadmap.md)

## 当前开发状态

- 已初始化 `Tauri 2 + Rust + React + TypeScript` 项目结构
- 已提供主工作台界面，可输入选中文本与上下文并触发解释预览
- 已提供设置页与历史页骨架
- 已提供 Rust `invoke` 命令接口，用于返回预览解释和保存 Provider 配置
- 当前 AI 返回仍为原型数据，尚未接入真实模型调用、系统快捷键、选区读取和悬浮独立窗口

## 目录概览

```text
Select2Explain/
├── docs/
├── src/
│   ├── app/
│   ├── components/
│   ├── features/
│   └── shared/
├── src-tauri/
│   └── src/
└── package.json
```

## 本地开发前置条件

启动前请先安装：

- Node.js 20+
- npm 10+
- Rust stable toolchain
- Cargo

当前这台机器缺少 `node`、`npm` 和 `cargo`，因此本次只完成了代码落地，未能实际安装依赖与运行构建。

## 启动方式

```bash
npm install
npm run tauri:dev
```

如果只需要查看前端界面，也可以使用：

```bash
npm install
npm run dev
```

## 当前推荐技术路线

- 桌面框架：Tauri 2
- 后端宿主：Rust
- 前端界面：React + TypeScript + Vite
- 本地存储：SQLite
- AI 接入：兼容 OpenAI 风格接口，优先抽象成 Provider 层
- 全局能力：系统快捷键、剪贴板监听、悬浮窗、权限引导、可选 OCR

## 为什么不是传统 Electron 优先

对于这类常驻后台、需要低资源占用、需要系统级窗口控制和权限集成的桌面工具，Tauri 在安装体积、内存占用和原生能力扩展方面更合适。Electron 仍然可行，但更适合作为备选方案而不是首选。

## 下一步

接下来的实现优先级建议如下：

1. 接入真实 AI Provider 和安全存储 API Key
2. 实现全局快捷键和选中文本读取主链路
3. 把解释预览从主窗口迁移到独立悬浮窗
4. 增加 SQLite 持久化与真实历史记录