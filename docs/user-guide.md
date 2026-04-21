# Select2Explain 用户指南

## 功能简介

Select2Explain 是一款 macOS 桌面应用，只需在任意应用中用鼠标拖选文字，即可自动弹出 AI 解释。无需快捷键，无需切换窗口。

---

## 安装与首次配置

### 1. 授权辅助功能权限

Select2Explain 使用 macOS Accessibility API 检测你在其他应用中的选中文字，需要以下授权：

1. 打开「系统设置 → 隐私与安全性 → 辅助功能」
2. 找到 Select2Explain，打开开关
3. 如果列表中没有，点击 `+` 手动添加应用

> ⚠️ 未授权时，应用无法检测到选中文字，监控功能不会工作。

### 2. 配置 AI Provider

打开应用，在「设置」页面：

| 字段 | 说明 | 默认值 |
|------|------|--------|
| Base URL | AI API 地址 | `https://openrouter.ai/api/v1` |
| 文本模型 | 用于生成解释的模型 | `qwen/qwen2.5-vl-72b-instruct` |
| 视觉模型 | 用于分析截图的模型 | `qwen/qwen2.5-vl-72b-instruct` |

点击「保存配置」。

### 3. 配置 API Key

在「API Key」区域输入你的 API Key，点击「保存 Key」。

Key 会加密存储在 macOS 系统钥匙串中，不会明文保存在磁盘。

### 4. 测试连接

保存 API Key 后，点击「测试连接」按钮，验证配置是否正确。成功时显示延迟（ms）。

---

## 日常使用

1. 保持 Select2Explain 在后台运行
2. 在任意应用（浏览器、编辑器、PDF 阅读器等）中用鼠标拖选文字
3. 应用自动检测选中文字（约 600ms 轮询间隔）
4. 自动切换到「监控」视图，显示 AI 分析结果

---

## 支持的 AI Provider

任何兼容 OpenAI API 格式的服务均可使用：

- **OpenRouter**（默认）: `https://openrouter.ai/api/v1`
- **OpenAI**: `https://api.openai.com/v1`
- **本地 Ollama**: `http://localhost:11434/v1`
- 其他兼容服务

---

## 常见问题

**Q: 选中文字后没有反应？**
- 检查辅助功能权限是否已授权
- 某些应用（沙盒应用、某些 Electron 应用）可能不暴露 Accessibility 接口
- 确认 API Key 已配置

**Q: AI 解释很慢？**
- 应用会先截图发给视觉模型判断上下文，再调用文本模型解释，共两次 API 请求
- 可以切换到速度更快的模型

**Q: 如何更换 API Key？**
- 在设置页面直接输入新 Key 并保存，会覆盖旧 Key
- 点击「删除 Key」可清除
