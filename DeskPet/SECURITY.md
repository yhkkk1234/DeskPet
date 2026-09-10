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

## 已修复（本次公开发布前整改）

- **天气 API 密钥曾可能明文进日志**：OpenWeather 的 `appid` 在 URL 查询串中，而 reqwest 会把完整 URL 附在错误末尾，经 `tracing::error!` 落盘。现由 `redact_api_key` 剥 URL + 掩码 `appid`（附 4 个回归测试，含「绝不部分掩码」性质测试）。经查历史日志未实际泄露。
- **README / 隐私弹窗措辞与实现不符**：原文称截图「不会上传」，实为发送到用户自配的 AI 接口。已改为如实说明。
- **`package-lock.json` 锁定第三方镜像**：95 个 `resolved` 原指向 `registry.npmmirror.com`，现改为 `registry.npmjs.org`（仅换域名，版本与 integrity 零变更）。
