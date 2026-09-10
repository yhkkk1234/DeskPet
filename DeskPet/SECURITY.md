# 安全说明

## 报告漏洞

请通过 [GitHub Security Advisories](../../security/advisories/new) 私下报告，或在 Issues 中开一条**不含敏感细节**的说明。请勿在公开 Issue 里粘贴 API Key、日志全文或截图。

---

## 数据流向（到底什么会离开你的电脑）

DeskPet **没有自己的服务器**，也不含任何遥测 / 分析 / 自动更新 SDK。但它是 AI 应用，以下内容会发送到**你自己在设置里填的那个 Endpoint**：

| 数据 | 何时发送 | 是否落盘 |
|------|----------|----------|
| 对话文本 | 每次对话 | 本地 SQLite |
| 记忆 / 印象 / 人格摘要 | 记忆压缩时 | 本地 SQLite / 加密存档 |
| **截图图像** | 截图识图、开启「增强好奇心」后 | **不落盘**，仅内存 |
| 导入的文档正文 | 生成摘要时 | 本地 SQLite |
| 语音录音 | 语音输入（需自配 Whisper 端点） | **不落盘**，仅内存 |
| 剪贴板内容前 40 字符 | 开启「剪贴板感知」后（默认关） | 不落盘 |

把 Endpoint 指向本地 Ollama 等离线服务，即可做到除 Edge TTS 外全链路不出网。

**密钥存储**：AI / 天气 / 生图密钥经 AES-256-GCM 加密后写入 `%APPDATA%/DeskPet/`（密钥由 `master.key` 经 HKDF-SHA256 派生），前端只显示掩码。

**日志**：请求失败时的错误信息会先经 `weather_service.rs::redact_api_key` 剥掉 URL 并把 `appid` 之类的凭据参数掩码，再写入 `%APPDATA%/DeskPet/logs/`。发现未脱敏的日志请直接提 Issue。

---

## 已知的安全权衡

### `assetProtocol.scope` 故意放行全部路径

`src-tauri/tauri.conf.json` 中：

```json
"assetProtocol": {
  "enable": true,
  "scope": { "requireLiteralLeadingDot": false, "allow": ["**/*"] }
}
```

**为什么不收窄**：DeskPet 允许用户把皮肤素材放在**任意目录**（设置 → 渲染器 → 选文件），素材通过 `convertFileSrc()` 以 `asset:` 协议加载，而 `assetProtocol.scope` 是它的唯一准入控制。收窄成 `$RESOURCE/**/*` 之类的固定目录会**直接导致「浏览到任意位置选素材」失效**——这是产品特性，不是疏漏。

**为什么风险可控**（三条同时成立才需要担心，这里都不成立）：

1. `connect-src 'self' asset:` —— WebView 无法向外部发起请求，读到的文件没有外发通道
2. 前端**没有任何 `v-html` / `innerHTML`** —— AI 返回内容不会被当 HTML/JS 执行，不存在注入点
3. 这不是 Web 应用，没有远程内容渲染；能触发读取的只有用户自己在原生文件对话框里的选择

**为什么用 object 形式而非 `["**/*"]`**：Array 写法在 Unix 上 `requireLiteralLeadingDot` 默认 `true`，通配符不匹配 `.` 开头的路径段，导致 `~/.config/...` 这类目录下的素材选不了。object 形式显式关掉这个限制，行为与 Windows 上一致。

**若未来要加固**：Tauri v2 支持「用户通过对话框选中的路径自动扩展作用域」（[tauri#3595](https://github.com/tauri-apps/tauri/commit/b744cd2758bbc5da39d6105fd82002dc6536dd16)）。届时可删除 `**/*`、只保留素材默认目录，并**实机验证选文件功能仍然可用**后再合入。注意这依赖对话框扩展 scope 的行为，改动前必须手测。

### CSP 含 `unsafe-eval`

`script-src` 包含 `'unsafe-inline' 'unsafe-eval'`。其中 `unsafe-eval` 是第三方 `lottie-web` 的运行时表达式求值所需要的（`lottie.js` 内使用直接 `eval`，构建时会有 `[EVAL]` 告警）。若移除 Lottie 渲染器，可一并收紧 CSP。

### 明文 SQLite

`%APPDATA%/DeskPet/deskpet.db` 未加密（对话、日记、记忆为明文）；Ghost 存档 `autosave.ghost` 与配置文件为 AES-256-GCM 加密。这是有意的取舍：数据库需要支持全文检索。**同目录下的 `master.key` 是解密所有 `.enc` / `.ghost` 的钥匙，请勿把该目录整体拷贝进任何仓库或分享。**

---

## 已知现象（不是缺陷，遇到时先看这里）

### 说话动作的时长由「最短展示时长」决定

`useAnimation.ts` 里的 `MIN_SPEAKING_MS = 2500` **不是随手写的魔法数字**，请勿当作冗余代码删除。

原因：说话动画的持续时间绑定在流式响应窗口上（首个 `chat:token` → `chat:complete`）。但很多服务商（含部分官方端点）并不逐字下发，而是**把整段答案缓冲完毕后一次性推送** —— 此时流式窗口可能只有 0.3 秒，而 `Speak` 素材一个循环是 820ms，宠物几乎来不及张嘴。

因此：**流式时长 < 2.5s 时补足到 2.5s；≥ 2.5s 时按真实时长，不做干预。** 会话标记 `isSpeakingActive` 仍是立即释放的，所以这个补偿不会阻塞拖拽或日常行为。

### 怎么判断你的接口是不是「真流式」

每次对话后端都会在 `%APPDATA%/DeskPet/logs/` 写一行：

```
[流式] 首个间隔=1893ms 后续间隔均值=8.4ms 最大=38ms token数=55 总长=82字 总耗时=2344ms 生成阶段均速=182字/秒
```

**判据看间隔，不要看「首字耗时 vs 总耗时」**（后者对快模型会误判：模型本身就快时，首字快、生成也快，两者天然接近）：

| 特征 | 结论 |
|---|---|
| 后续间隔均值 与 首个间隔 **同量级** | 真流式 —— 每个 token 都在等模型算出来 |
| 首个间隔 **远大于** 后续间隔均值 | 缓冲式 —— 内容早已生成完毕，只是分帧下发 |

实测案例（官方 `api.deepseek.com`）：首个间隔 1893ms，后续间隔均值 8.4ms（**225 : 1**），且最大间隔仅 38ms —— 连一次"模型算得慢些"的痕迹都没有，属缓冲式。生成阶段均速 182 字/秒同样超出常规逐 token 生成速度。

### 精灵图各动画之间存在色彩偏差

素材按动作分批绘制，不同批次会有饱和度 / 明度 / 冷暖的差异。单张图看不出，但动画切换瞬间会「闪」一下。仓库自带量化脚本：

```bash
pwsh -File docs/tools/color-audit.ps1
```

当前素材实测：`Wave` 最艳最冷、`Shiver` 最灰最暖（饱和度差 0.057 ≈ 25%，冷暖比差 0.103），`Sleep` 最亮 / `Stretch` 最暗（明度差 16.6）；其余 15 个 tag 与 `Idle` 基本一致。**这是已知的美术待办，不是渲染器缺陷。**

---

## 已修复（本次公开发布前整改）

- **天气 API 密钥曾可能明文进日志**：OpenWeather 的 `appid` 在 URL 查询串中，而 reqwest 会把完整 URL 附在错误末尾，经 `tracing::error!` 落盘。现由 `redact_api_key` 剥 URL + 掩码 `appid`（附 4 个回归测试，含「绝不部分掩码」性质测试）。经查历史日志未实际泄露。
- **README / 隐私弹窗措辞与实现不符**：原文称截图「不会上传」，实为发送到用户自配的 AI 接口。已改为如实说明。
- **`package-lock.json` 锁定第三方镜像**：95 个 `resolved` 原指向 `registry.npmmirror.com`，现改为 `registry.npmjs.org`（仅换域名，版本与 integrity 零变更）。
- **说话动作被定时任务打断 + 由此引发的永久卡死**：`tickGhost` 每 5 秒的 `playOneShot` 会覆盖说话状态；状态被改走后 `stopSpeaking` 的守卫失效，导致 `isPerformingBehavior` 永久为真、日常行为调度器不再重启。现由 `isSpeakingActive` 会话标记 + 定时任务让路解决。
- **一个环境依赖型 flaky 测试**：`test_clipboard_cooldown_respected` 用 `..Default::default()` 构造配置，而 `check_triggers` 按 深夜 → 电量 → 剪贴板 顺序短路返回，导致该测试在 22:00~02:00 或未插电且电量≤20% 时必然失败。已改为显式关闭其他触发源。

