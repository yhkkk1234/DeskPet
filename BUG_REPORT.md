# DeskPet BUG 问题清单

> 生成日期: 2026-04-26
> 范围: 前端 UI + 逻辑

---

## 🔴 P0 — 关键 (已修复)

### BUG-001: 截图模式下发送按钮禁用状态错误

| 字段 | 值 |
|------|-----|
| 文件 | `src/components/BubbleDialogue.vue` |
| 行号 | 144 |
| 状态 | ✅ 已修复 |

**问题描述：**
`allowEmptySend=true`（截图待发送模式）时，发送按钮的 `:disabled` 绑定为 `chatLoading || !inputValue.trim()`。输入框为空时 `!inputValue.trim() === true`，按钮保持禁用，用户无法点击发送按钮提交截图请求（仅 Enter 键可用）。

**修复方案：**
将 `:disabled` 改为 `chatLoading || (!inputValue.trim() && !allowEmptySend)`，使 `allowEmptySend` 模式下空输入也能启用按钮。

---

### BUG-002: TTS 语音设置面板布局完全错乱

| 字段 | 值 |
|------|-----|
| 文件 | `src/Settings.vue` |
| 行号 | 858 (CSS) / 383-401 (template) |
| 状态 | ✅ 已修复 |

**问题描述：**
`.tts-rate-panel` 使用 `display: flex; align-items: center;`（水平 flex），但模板内包含 `<label>`、`<select>`、`<input type="range">`、`<span>` 等多个块级元素。所有子元素被强制水平排列在一行，导致引擎选择器、语速/音调滑块等布局完全崩溃。

**修复方案：**
1. 将 `.tts-rate-panel` 改为 `flex-direction: column` 垂直排列
2. 新增 `.slider-row` 类用于将 range input + value span 放在同一行
3. 模板中为语速和音调的滑块+数值对包裹 `.slider-row` 容器

---

## 🟠 P1 — 重要 (待修复)

### BUG-003: API Key 明文存储于 localStorage

| 字段 | 值 |
|------|-----|
| 文件 | `src/Settings.vue` |
| 行号 | 131-135 |
| 状态 | ⏳ 待修复 |

**问题描述：**
`saveAIConfig()` 中将 `aiApiKey`、`aiImageGenApiKey` 写入 `localStorage.setItem(...)`。虽然方便，但 API Key 明文存储在浏览器本地存储中存在安全风险。

**建议方案：**
API Key 应仅在后端内存中管理，前端只做透传。Settings 页面可展示 Key 是否已配置（如 `****` 掩码），但不应写入 localStorage。

---

### BUG-004: 气泡列表间距叠加

| 字段 | 值 |
|------|-----|
| 文件 | `src/components/BubbleMessage.vue:49` |
| 状态 | ✅ 已修复 |

**问题描述：**
`.bubble-area`（父容器）设置 `gap: 4px`，同时 `.bubble-wrapper`（子元素）设置 `margin-bottom: 6px`，导致气泡间实际间距 = 4 + 6 = 10px，与预期 4px 不符。

**修复方案：**
移除 `.bubble-wrapper` 的 `margin-bottom: 6px`，仅保留父容器 `gap: 4px`。

---

### BUG-005: EmotionTimeline.vue 是孤儿组件

| 字段 | 值 |
|------|-----|
| 文件 | `src/components/EmotionTimeline.vue` |
| 状态 | ✅ 已修复 |

**问题描述：**
`EmotionTimeline.vue` 定义了完整的情感时间线 UI，但未被任何组件导入使用。

**修复方案：**
集成到 `Settings.vue` 的"情感事件"区域，同时从上一版本恢复了 9 个情感事件按钮（正向/中性/负向分组），用户可点击直接调用 `apply_event` 后端命令增减好感度。

---

## 🟡 P2 — 轻微 (待修复)

### BUG-006: computed import 顺序不当

| 字段 | 值 |
|------|-----|
| 文件 | `src/components/PetCSSRenderer.vue` |
| 行号 | 13, 18, 22 |
| 状态 | ⏳ 待修复 |

**问题描述：**
`computed` 在第 13 行和第 18 行被使用，但 `import { computed } from 'vue'` 在第 22 行。SFC 编译阶段会提升 import，运行时无问题，但代码可读性差且可能引起 TypeScript / ESLint 警告。

**建议方案：**
将 `import { computed } from 'vue'` 移到文件顶部（第 1-2 行）。

---

### BUG-007: 截图占位提示永不重置

| 字段 | 值 |
|------|-----|
| 文件 | `src/App.vue` |
| 行号 | 146-148 |
| 状态 | ⏳ 待修复 |

**问题描述：**
```typescript
const screenShotPlaceholder = computed(() => 
  pendingScreenShotBase64.value ? '想问这张截图什么？（直接回车=自动描述）' : '跟桌宠说说...'
)
```
`screenShotPlaceholder` 仅依赖 `pendingScreenShotBase64` 状态。截图分析完成后（`pendingScreenShotBase64` 置为 null），输入框提示文本应自动恢复为"跟桌宠说说..."，但实际行为取决于父组件是否重置 input。当前代码中 `executeScreenShotAnalysis` 将 `pendingScreenShotBase64.value = null`，所以 placeholder 会正确恢复，无运行时问题。

**备注：** 确认当前行为正确，无需修复。

---

## P3 — 建议 (待处理)

### BUG-008: TTS 设置持久化发射冗余事件

| 字段 | 值 |
|------|-----|
| 文件 | `src/Settings.vue:102` + `src/App.vue:398-414` |
| 状态 | ⏳ 待修复 |

**问题描述：**
`saveTTSSettings()` 发射了两个事件：
1. `tts-updated`（通过 `syncTTSToMain`）
2. `settings-updated`（section: 'tts'）

但 `App.vue` 的 `settings-updated` 监听器没有处理 `'tts'` 分支。`tts-updated` 独立事件已在工作，所以 `settings-updated` 中的 `'tts'` 事件是冗余的。

**建议方案：**
移除 `saveTTSSettings` 中发射 `settings-updated` 的调用，或为 `settings-updated` 添加 `'tts'` 处理分支。

### BUG-009: 使用 `window.screen` 在多显示器场景下可能不准确

| 字段 | 值 |
|------|-----|
| 文件 | `src/App.vue` |
| 行号 | 675-676 |
| 状态 | ⏳ 待修复 |

**问题描述：**
```typescript
const screenW = window.screen.availWidth
const screenH = window.screen.availHeight
```
`movePetToScreenshotRegion()` 中使用 `window.screen.availWidth/Height` 来约束窗口位置。在多显示器环境下，`window.screen` 只返回主显示器的尺寸，可能导致桌宠窗口被放到错误的位置。

**建议方案：**
改用 Tauri API 获取当前屏幕的可用尺寸，如 `getCurrentWindow().availableMonitors()`。

---

## 架构变更

### 对话窗口独立化

| 变更 | 说明 |
|------|------|
| 原因 | 主窗口 170×220px 过小，气泡溢出不可见 |
| 方案 | 新建 `chat.html` + `src/chat-main.ts` + `src/ChatWindow.vue` |
| 效果 | 对话在独立 360×500 透明窗口中显示，支持漫画气泡尾巴 |
| 已删除旧文件 | `BubbleDialogue.vue`、`useBubbleTimer.ts` |
| 保留旧文件 | `BubbleMessage.vue`（ChatWindow 复用于消息渲染） |

**窗口间通信：**
- `chat:open-with-screenshot` — 截图数据传递
- `chat:reposition` — 主窗口拖拽后通知聊天窗口重新定位
- `chat:post-processed` — 好感度更新（App.vue 仅取 loveHate 用于宠物动画）

---

## 修复进度总览

| 严重度 | 总数 | 已修复 | 待修复 |
|--------|------|--------|--------|
| 🔴 P0 | 2 | 2 | 0 |
| 🟠 P1 | 3 | 2 | 1 |
| 🟡 P2 | 2 | 0 | 2 |
| P3 | 2 | 0 | 2 |
| **合计** | **9** | **4** | **5** |
