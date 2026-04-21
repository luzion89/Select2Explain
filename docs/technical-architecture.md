# Select2Explain 技术架构与选型

## 1. 总体结论

推荐使用以下技术组合：

- 桌面应用框架：Tauri 2
- 原生宿主层：Rust
- 前端 UI：React + TypeScript + Vite
- 数据层：SQLite + `sqlx` 或 `rusqlite`
- 配置与密钥存储：系统钥匙串 + 本地配置文件
- AI 访问层：Provider 抽象，兼容 OpenAI 风格接口

这套方案的核心目标是兼顾跨平台、系统集成能力、安装体积、性能和后续扩展性。

## 2. 技术选型比较

### 2.1 Tauri vs Electron

#### 选择 Tauri 的原因

- 更小的安装包和更低的常驻资源占用，适合后台驻留型工具
- Rust 宿主层更适合处理系统权限、窗口管理、原生事件与安全边界
- 做悬浮窗、托盘、全局快捷键、剪贴板等系统能力时更容易控制细节

#### Electron 的优点

- 生态成熟
- 前端团队上手成本更低
- 三方桌面能力库较多

#### 为什么当前不优先 Electron

本项目不是重前端页面的文档类桌面产品，而是轻 UI、强系统交互的常驻工具。资源占用与系统融合优先级更高，因此 Tauri 更合适。

### 2.2 React vs Vue / Svelte

选择 React + TypeScript，原因如下：

- 生态成熟，适合搭建设置页、历史面板、悬浮解释窗等多视图界面
- 与 Tauri 社区示例和前端工具链兼容良好
- 后续若引入状态机、富文本渲染、流式输出组件，生态更完整

### 2.3 SQLite vs 纯文件存储

历史记录、Prompt 模板、Provider 配置、调试日志等都适合结构化存储。SQLite 比 JSON 文件更利于后续检索、清理和迁移。

## 3. 架构分层

建议采用四层结构：

### 3.1 Presentation Layer

职责：

- 悬浮解释窗
- 设置页
- 历史记录页
- 权限引导页

建议：

- React 组件只处理展示与交互状态
- 复杂业务流程交给应用服务层

### 3.2 Application Layer

职责：

- 解释请求编排
- Prompt 构造
- Provider 路由
- 错误处理与重试策略
- 历史记录写入

### 3.3 Domain Layer

核心领域对象建议包括：

- `SelectionPayload`
- `ContextSnapshot`
- `ExplainRequest`
- `ExplainResult`
- `PromptTemplate`
- `ProviderConfig`

### 3.4 Infrastructure Layer

职责：

- Tauri 原生能力封装
- 剪贴板与快捷键
- 鼠标位置与窗口定位
- SQLite 读写
- Keychain / Credential Manager
- HTTP 请求封装

## 4. 核心模块设计

### 4.1 选区获取模块

MVP 采用可靠性优先策略：

- 主路径：用户先选中文本，再按快捷键触发
- 实现路径：触发时优先尝试读取系统可获取的选中内容
- Fallback：若失败，则通过受控剪贴板流程获取文本

建议模块接口：

- `capture_selection()`
- `capture_context()`
- `get_active_app_info()`

### 4.2 上下文组装模块

输入来源：

- 选中文本
- 活动应用名
- 窗口标题
- 用户配置的解释风格
- 可选模板

输出：

- 标准化 Prompt 请求体

要求：

- 对不同场景使用不同模板，例如阅读、代码、商务沟通
- 控制 token 长度，避免无边界扩张

### 4.3 AI Provider 模块

统一抽象：

- `ProviderAdapter`
- `ModelDescriptor`
- `CompletionRequest`
- `CompletionResponse`

首版建议支持：

- OpenAI 兼容接口
- DeepSeek 兼容接口
- 自定义 Base URL

这样后续切换模型时不需要修改上层业务。

### 4.4 弹窗与窗口管理模块

需要支持：

- 鼠标当前位置计算
- 多屏坐标换算
- DPI 缩放适配
- 避免超出屏幕边界
- 临时窗和固定窗两种模式

关键点：

- 默认使用无边框、置顶、非任务栏窗口
- 需要精细处理焦点行为，避免弹窗抢走用户当前输入焦点

### 4.5 配置与安全模块

建议区分：

- 普通配置：本地文件保存
- 敏感信息：系统安全存储

macOS 使用 Keychain，Windows 使用 Credential Manager。

## 5. 跨平台关键能力清单

### 5.1 macOS

- 全局快捷键
- 剪贴板访问
- Accessibility 权限引导
- 悬浮窗置顶与位置控制
- 托盘常驻

### 5.2 Windows

- 全局快捷键
- 剪贴板访问
- 活动窗口信息获取
- 悬浮窗置顶与透明边框控制
- 托盘常驻

### 5.3 权限策略

必须把权限处理当作产品功能而不是纯技术细节。首启建议做引导式检查：

1. 检查快捷键能力是否可用
2. 检查剪贴板访问是否正常
3. 在 macOS 上检测 Accessibility 授权状态
4. 权限缺失时提供明确操作说明和重试按钮

## 6. 数据模型建议

### 6.1 explain_history

- `id`
- `selected_text`
- `context_summary`
- `prompt_template_id`
- `provider_name`
- `model_name`
- `response_text`
- `latency_ms`
- `source_app`
- `created_at`

### 6.2 provider_config

- `id`
- `provider_type`
- `base_url`
- `model_name`
- `is_default`
- `created_at`

### 6.3 prompt_template

- `id`
- `name`
- `scene`
- `system_prompt`
- `user_prompt_pattern`
- `is_builtin`

## 7. Prompt 策略

建议从一开始就把 Prompt 模板化，而不是把文案硬编码在业务里。

基础模板至少分三类：

- 通用阅读解释
- 代码/技术解释
- 商务或自然语言沟通解释

每类模板都要包含：

- 解释目标
- 输出格式约束
- 语气控制
- 是否输出歧义项

## 8. 可观测性与调试

首版就建议加入最小可观测能力：

- 请求耗时
- Provider 错误码
- 选区获取失败原因
- 权限缺失状态
- 用户是否使用 fallback 流程

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