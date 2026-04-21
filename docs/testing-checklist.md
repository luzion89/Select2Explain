# Select2Explain Smoke Test Checklist

**用途**：每次代码变更交付给用户前，必须逐项执行并全部通过。这是防止功能偏离和 regression 的最后防线。
**原则**：每一项都必须实际执行，不得靠“代码看起来没问题”来跳过。

---

## BLOCK 0 — 构建验证（每次改代码后必做）

| # | 执行命令 | 通过标准 |
|---|----------|----------|
| B1 | `cd Select2Explain && npx tsc --noEmit` | 零输出（零错误零警告） |
| B2 | `cd Select2Explain/src-tauri && cargo check 2>&1` | 输出 `Finished`，无 `error[` 行 |

**如果 B1 或 B2 失败，停止一切，先修复再继续。**

---

## BLOCK 1 — API 客户端层（providers/openai.rs）

这一块是历史 bug 高发区，必须在应用启动前先用命令行验证。

### T1-1: 用真实 key 测试 text API 可以成功返回

```bash
curl -s -X POST https://openrouter.ai/api/v1/chat/completions \
   -H "Authorization: Bearer $(cat debug/apk-key.txt)" \
   -H "Content-Type: application/json" \
   -d '{"model":"qwen/qwen2.5-vl-72b-instruct","messages":[{"role":"user","content":"say ok"}],"max_tokens":5}' \
   | python3 -c "import sys,json; d=json.load(sys.stdin); print('OK:', d['choices'][0]['message']['content'])"
```

通过标准：输出 `OK: ok` 或等价简短内容，不能是 `KeyError`、空输出或解析异常。

### T1-2: 用错误 key 测试错误信息可读

```bash
curl -s -X POST https://openrouter.ai/api/v1/chat/completions \
   -H "Authorization: Bearer sk-wrong-key" \
   -H "Content-Type: application/json" \
   -d '{"model":"qwen/qwen2.5-vl-72b-instruct","messages":[{"role":"user","content":"hi"}],"max_tokens":5}' \
   | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('error',{}).get('message','NO ERROR FIELD'))"
```

通过标准：输出可读错误消息，说明 provider 错误体含 `error.message`。

### T1-3: 验证 Rust HTTP 错误处理代码路径

人工确认 [src-tauri/src/providers/openai.rs](/Users/tuntun/Documents/AI/Select2Explain/src-tauri/src/providers/openai.rs) 满足以下两点：

- [ ] `call_text()` 中有 `if !status.is_success()` 分支，并在反序列化成功响应前返回错误
- [ ] `call_vision()` 中同样有 `if !status.is_success()` 分支

如果缺少上述检查，“Connection failed: parse failed” 会复现。

---

## BLOCK 2 — 应用启动验证

启动命令：

```bash
cd Select2Explain && npm run tauri:dev
```

| # | 检查项 | 通过标准 |
|---|--------|----------|
| A1 | 应用窗口出现 | 720x560 窗口，标题 Select2Explain |
| A2 | 默认显示设置页 | 可见“Provider 配置”标题，有 Base URL / 文本模型 / 视觉模型三个输入框 |
| A3 | 默认 Base URL | 字段值为 `https://openrouter.ai/api/v1` |
| A4 | 默认模型 | 文本模型和视觉模型均为 `qwen/qwen2.5-vl-72b-instruct` |
| A5 | 侧边栏状态指示 | 未配置 API Key 时显示红点 + “未配置” |

---

## BLOCK 3 — API Key 管理

| # | 操作 | 通过标准 |
|---|------|----------|
| K1 | 在“输入新 Key”框输入测试 key，点“保存 Key” | 提示“API Key 已保存到系统钥匙串” |
| K2 | 保存后检查侧边栏 | 状态变为绿点 + “AI 已就绪” |
| K3 | 重启应用后再打开设置页 | 显示“✓ API Key 已配置（存储于系统钥匙串）” |
| K4 | 点“删除 Key” | 状态回到红点，绿点消失 |
| K5 | 重新输入并保存 key（为后续测试准备） | 保存成功 |

测试 key 来源： [debug/apk-key.txt](/Users/tuntun/Documents/AI/Select2Explain/debug/apk-key.txt)

---

## BLOCK 4 — 测试连接功能

这是之前 parse failed bug 的发现点，必须覆盖正反两种情况。

| # | 操作 | 通过标准 |
|---|------|----------|
| C1 | 已保存正确 OpenRouter key，点“测试连接” | 显示绿色“连接成功，延迟 Xms”，不出现 parse failed |
| C2 | 先删除 key，尝试点“测试连接” | 按钮应为禁用状态 |
| C3 | 手动改 Base URL 为 `https://invalid.url/v1`，保存配置，再测试连接 | 显示红色明确错误，不是 parse failed |
| C4 | 恢复正确 Base URL 并保存配置 | 测试连接再次成功 |

---

## BLOCK 5 — 选中文字检测主链路

这是核心功能。必须在真实应用中选中文字，验证事件触发。

前提：macOS“系统设置 → 隐私与安全性 → 辅助功能”已授权 Select2Explain。

| # | 操作 | 通过标准 |
|---|------|----------|
| S1 | 打开“文本编辑”或 Safari，拖选一段文字 | 约 600ms 内，应用自动切换到监控视图 |
| S2 | 监控视图显示选中文字内容 | 引用块中文字与实际选中内容一致 |
| S3 | 显示“AI 分析中…”状态 | 加载指示出现，不是空白 |
| S4 | AI 解释返回 | 结果区显示解释、来源 App 和延迟 |
| S5 | 在另一个应用选中不同文字 | 视图更新为新文字和新解释 |
| S6 | 在 Select2Explain 自己窗口内选中文字 | 不触发解释 |

---

## BLOCK 6 — 错误处理路径

| # | 操作 | 通过标准 |
|---|------|----------|
| E1 | 删除 API Key 后，在外部应用选中文字 | 显示可读错误，不崩溃 |
| E2 | 恢复正确 Key，使用无效 Base URL，再选中文字 | 显示可读错误，不是 parse failed |
| E3 | 点监控视图“关闭”按钮 | 回到空闲态“正在监听选中文字…” |

---

## BLOCK 7 — 安全确认（人工代码审查）

| # | 检查项 | 验证方法 |
|---|--------|----------|
| SC1 | API Key 不写磁盘 | 检查 [src-tauri/src/commands/mod.rs](/Users/tuntun/Documents/AI/Select2Explain/src-tauri/src/commands/mod.rs) 和 [src-tauri/src/state/mod.rs](/Users/tuntun/Documents/AI/Select2Explain/src-tauri/src/state/mod.rs)，确认仅使用 keyring |
| SC2 | API Key 输入框遮蔽显示 | 检查 [src/App.tsx](/Users/tuntun/Documents/AI/Select2Explain/src/App.tsx) 中 `type="password"` |
| SC3 | 不使用剪贴板兜底 | 检查 [src-tauri/src/platform/mod.rs](/Users/tuntun/Documents/AI/Select2Explain/src-tauri/src/platform/mod.rs) 无 clipboard 相关代码 |

---

## 执行记录模板

每次执行后记录结果：

```text
日期: ____-__-__
执行人: Copilot

BLOCK 0: B1[ ] B2[ ]
BLOCK 1: T1-1[ ] T1-2[ ] T1-3[ ]
BLOCK 2: A1[ ] A2[ ] A3[ ] A4[ ] A5[ ]
BLOCK 3: K1[ ] K2[ ] K3[ ] K4[ ] K5[ ]
BLOCK 4: C1[ ] C2[ ] C3[ ] C4[ ]
BLOCK 5: S1[ ] S2[ ] S3[ ] S4[ ] S5[ ] S6[ ]
BLOCK 6: E1[ ] E2[ ] E3[ ]
BLOCK 7: SC1[ ] SC2[ ] SC3[ ]

未通过项及原因:
-
```

---

## 2026-04-21 执行记录

```text
BLOCK 0: B1[✅] B2[✅]
BLOCK 1: T1-1[✅ qwen 文本请求成功] T1-2[✅ error.message 可读] T1-3[✅]
BLOCK 2: A1[✅] A2[✅] A3[✅] A4[✅] A5[✅]
BLOCK 3: K1[待用户点击验证] K2[待用户点击验证] K3[待验证] K4[待验证] K5[待验证]
BLOCK 4: C1[❌ 已发现并修复 parse failed 根因，且默认模型已切换到实测可用值] C2[✅] C3[待验证] C4[待验证]
BLOCK 5: S1[待真实点击测试] S2[待真实点击测试] S3[待真实点击测试] S4[待真实点击测试] S5[待真实点击测试] S6[待真实点击测试]
BLOCK 6: E1[待验证] E2[待验证] E3[待验证]
BLOCK 7: SC1[✅] SC2[✅] SC3[✅]

本次修复:
- 修复 [src-tauri/src/providers/openai.rs](/Users/tuntun/Documents/AI/Select2Explain/src-tauri/src/providers/openai.rs) 在非 2xx HTTP 响应时直接按成功结构反序列化，导致 “Connection failed: parse failed”
- 现在会优先读取错误响应体中的 `error.message`，返回真实错误原因
- 默认 OpenRouter 模型改为 `qwen/qwen2.5-vl-72b-instruct`，因为提供的测试 key 在当前区域下访问 `openai/gpt-4o-mini` 会收到 403 region 限制
```
