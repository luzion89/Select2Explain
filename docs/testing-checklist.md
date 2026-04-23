# Select2Explain 测试清单

## 目的

这份清单用于验证当前版本的 Windows 主链路是否与产品定义一致，并给出 macOS 的设计性测试方案。

## Block 0 — 构建与连通性

### 必做命令

1. `npm run build`
2. `cd src-tauri && cargo --config "source.crates-io.replace-with='rsproxy'" --config "source.rsproxy.registry='sparse+https://rsproxy.cn/index/'" check`

### 通过标准

- 前端打包成功。
- Rust 输出 `Finished`，没有 `error:`。

## Block 1 — Provider 层

### T1-1 正确 key 可用

执行真实 Provider 请求，确认返回正文，不是空内容或解析异常。

### T1-2 错误 key 错误体可读

使用错误 key，确认返回 `error.message` 风格的明确错误，而不是 `parse failed`。

### T1-3 UI 连接测试可用

在控制面板点击“测试连接”，应显示成功延迟或明确失败原因。

## Block 2 — Windows 运行级点击测试

### 准备

1. 启动应用：`npm run tauri:dev` 或生产构建后的 exe。
2. 在主控制面板保存 Base URL、文本模型、视觉模型。
3. 保存 API Key。
4. 打开“自动监听”，然后把主窗口关闭到托盘。
5. 如需定位问题，再打开“本地调试日志”。
6. 准备一个真实前台应用，例如浏览器、记事本或 VS Code。

### Windows 点击主流程

| 编号 | 操作 | 通过标准 |
|---|---|---|
| W1 | 在外部应用拖选一段文字 | 约 1 秒内出现 popup |
| W2 | popup 位置 | 出现在鼠标右上方，且没有越出屏幕边界 |
| W3 | popup 内容 | 显示选中文本摘要、AI 解释、来源应用或窗口信息 |
| W4 | 失焦隐藏 | 点击外部任意空白位置，popup 自动隐藏 |
| W5 | 新选区再次触发 | 在同一或另一应用选中不同文本，popup 更新为新解释 |
| W6 | 重复选区去重 | 连续保持同一选区不动，不应反复弹出同一解释 |
| W7 | 应用自过滤 | 在 Select2Explain 自己窗口内选中文字，不应触发 popup |
| W8 | 托盘恢复 | 通过托盘菜单“打开控制面板”可重新显示主窗口 |
| W9 | 调试日志 | 开启日志后，本地 `runtime.log` 中出现 startup / probe / capture / popup 相关记录 |

### Windows 失败路径

| 编号 | 操作 | 通过标准 |
|---|---|---|
| WE1 | 删除 API Key 后保留监听，再选中文字 | popup 显示可读错误，不崩溃 |
| WE2 | Base URL 改成无效地址，再选中文字 | popup 显示网络或 Provider 错误，不是空白 |
| WE3 | 使用“测试选区探测”按钮 | 控制面板能看到当前应用、窗口名、选区内容或明确说明没有选区 |
| WE4 | 使用“测试完整采集”按钮 | 控制面板能看到窗口文本长度和截图 base64 长度或明确错误 |
| WE5 | 在常见应用无反应时查看日志 | 能看到 `probe found no accessible selection` 或其他明确诊断，而不是只有 startup |

## Block 3 — macOS 设计性测试方案

当前没有在本轮环境执行 macOS 运行测试，只设计验收步骤：

1. 首次启动后检查 Accessibility 和 Screen Recording 权限引导。
2. 开启监听后，在 Safari / TextEdit / Xcode 中选中文字。
3. 验证 `poll_selection()` 能识别新的 AX 选区。
4. 验证 `capture_context()` 能返回截图、窗口文本、应用名、窗口标题与鼠标坐标。
5. 验证 popup 出现在鼠标右上方，点击空白处自动隐藏。
6. 验证本地 `runtime.log` 能记录 macOS probe / capture 事件。
7. 验证在本应用窗口内选区不会自触发。

## Block 4 — 安全检查

1. API Key 只进入系统安全存储，不写入普通配置文件。
2. 采集主链路不使用剪贴板兜底。
3. 默认先发选区和截图，全文仅在截图不足时补发。

## 本轮执行记录

日期：2026-04-22
执行人：Copilot

- Block 0：通过
   - `npm run build` 通过
   - `cargo check` 通过
- Block 1：通过
   - 真实 key 连通性通过
   - 错误 key 错误体可读
- Block 2：部分通过
   - 已验证 release 会加载本地 settings 和 runtime log
   - 已验证当前 Windows 代码在记事本、VS Code 这类常见应用里存在“前台窗口可见但选区不可访问”的兼容性问题
   - 已完成应用级代码与打包路径
   - 真实桌面点击流需在纯 GUI 会话中按上表继续逐项点测
- Block 3：仅设计，未执行
- Block 4：通过代码审查确认
