<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen, emit } from '@tauri-apps/api/event'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { LogicalPosition, PhysicalSize } from '@tauri-apps/api/dpi'
import { open } from '@tauri-apps/plugin-dialog'
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'
import BubbleMessage from './components/BubbleMessage.vue'
import { getVirtualScreenBounds, clampToVirtualScreen } from './composables/useScreenBounds'
import { applyTheme } from './composables/useTheme'

interface ChatMessage {
  role: 'pet' | 'user' | 'system'
  content: string
  imageBase64?: string
}

const messages = ref<ChatMessage[]>([])
const chatLoading = ref(false)
const streamingContent = ref('')
const responseComplete = ref(false)
const inputValue = ref('')
const ghostName = ref('桌宠')
const answeringMode = ref<'Companion' | 'Assistant'>('Companion')
const pendingScreenShotBase64 = ref<string | null>(null)
const tailDirection = ref<'left' | 'right'>('left')
const ghostLoaded = ref(false)
const error = ref('')
const importingDoc = ref(false)
const importingDocTitle = ref('')
const dragOver = ref(false)

const messageListRef = ref<HTMLElement | null>(null)
const inputRef = ref<HTMLInputElement | null>(null)
const chatWindow = getCurrentWindow()

const placeholderText = computed(() =>
  pendingScreenShotBase64.value ? '想问这张截图什么？（直接回车=自动描述）' : '跟桌宠说说...'
)

const allowEmptySend = computed(() => !!pendingScreenShotBase64.value)

const bubbleStyle = computed(() => ({
  '--bubble-tail': tailDirection.value,
}))

const typingDots = ref(['●', '●', '●'])
let typingFrame = 0
let typingInterval: ReturnType<typeof setInterval> | null = null
const typingText = computed(() => typingDots.value.join(' '))

const impressionNote = ref('')

// ===== 语音输入 (STT) =====
const recording = ref(false)
const transcribing = ref(false)
const recordingSeconds = ref(0)
let recordingTimer: ReturnType<typeof setInterval> | null = null

// ===== 窗口 resize：拖动气泡边缘（透明窗口看不见系统边缘，热区放在窗口四周）=====
const MIN_WINDOW_W = 300
const MIN_WINDOW_H = 300

function startResize(dir: string, e: MouseEvent) {
  e.preventDefault()
  e.stopPropagation()
  const startX = e.screenX
  const startY = e.screenY

  chatWindow.outerPosition().then(async (pos) => {
    const sf = await chatWindow.scaleFactor()
    const startLeft = pos.x / sf
    const startTop = pos.y / sf
    const size = await chatWindow.innerSize()
    const startW = size.width / sf
    const startH = size.height / sf

    let lastX = startX
    let lastY = startY
    let rafId: number | null = null

    // 按方向计算新窗口位置/尺寸（逻辑像素，setSize 用物理像素）
    const apply = (mx: number, my: number) => {
      const dx = mx - startX
      const dy = my - startY
      let left = startLeft
      let top = startTop
      let w = startW
      let h = startH
      if (dir.includes('e')) w = startW + dx
      if (dir.includes('s')) h = startH + dy
      if (dir.includes('w')) { w = startW - dx; left = startLeft + dx }
      if (dir.includes('n')) { h = startH - dy; top = startTop + dy }
      if (w < MIN_WINDOW_W) { w = MIN_WINDOW_W; if (dir.includes('w')) left = startLeft + startW - MIN_WINDOW_W }
      if (h < MIN_WINDOW_H) { h = MIN_WINDOW_H; if (dir.includes('n')) top = startTop + startH - MIN_WINDOW_H }
      chatWindow.setPosition(new LogicalPosition(Math.round(left), Math.round(top))).catch(() => {})
      chatWindow.setSize(new PhysicalSize(Math.round(w * sf), Math.round(h * sf))).catch(() => {})
    }

    const onMove = (me: MouseEvent) => {
      lastX = me.screenX
      lastY = me.screenY
      if (rafId !== null) return
      rafId = requestAnimationFrame(() => {
        rafId = null
        apply(lastX, lastY)
      })
    }
    const onUp = () => {
      if (rafId !== null) {
        cancelAnimationFrame(rafId)
        rafId = null
      }
      document.removeEventListener('mousemove', onMove)
      document.removeEventListener('mouseup', onUp)
    }
    document.addEventListener('mousemove', onMove)
    document.addEventListener('mouseup', onUp)
  })
}

async function toggleRecording() {
  if (recording.value) {
    await stopAndTranscribe()
    return
  }
  try {
    await invoke('start_recording')
    recording.value = true
    recordingSeconds.value = 0
    recordingTimer = setInterval(() => { recordingSeconds.value++ }, 1000)
  } catch (e: any) {
    messages.value.push({ role: 'system', content: `录音失败: ${e}` })
  }
}

async function stopAndTranscribe() {
  recording.value = false
  if (recordingTimer) {
    clearInterval(recordingTimer)
    recordingTimer = null
  }
  transcribing.value = true
  try {
    const wavBase64 = await invoke<string>('stop_recording')
    const text = await invoke<string>('transcribe_audio', { wavBase64 })
    if (text) {
      inputValue.value = (inputValue.value + ' ' + text).trim()
      nextTick(() => inputRef.value?.focus())
    }
  } catch (e: any) {
    messages.value.push({ role: 'system', content: `语音转写失败: ${e}` })
  } finally {
    transcribing.value = false
  }
}

async function refreshImpressionNote() {
  try {
    const status = await invoke<string>('get_ghost_status')
    const parsed = JSON.parse(status)
    const snips: string[] | undefined = parsed?.impression?.snippets
    impressionNote.value = Array.isArray(snips) && snips.length ? snips[snips.length - 1] : ''
  } catch {
    // 没有 ghost 时静默，不打扰聊天
  }
}

function scrollToBottom() {
  nextTick(() => {
    if (messageListRef.value) {
      messageListRef.value.scrollTop = messageListRef.value.scrollHeight
    }
  })
}

watch(() => messages.value.length, scrollToBottom)
watch(() => streamingContent.value, scrollToBottom)

watch(() => chatLoading.value, (loading) => {
  if (loading) {
    if (typingInterval) clearInterval(typingInterval)
    typingInterval = setInterval(() => {
      typingFrame = (typingFrame + 1) % 3
      const dots = ['●', '●', '●']
      dots[typingFrame] = '◉'
      typingDots.value = dots
    }, 400)
  } else {
    if (typingInterval) {
      clearInterval(typingInterval)
      typingInterval = null
    }
    typingDots.value = ['●', '●', '●']
  }
})

async function positionNearPet() {
  try {
    const petWin = await WebviewWindow.getByLabel('pet')
    if (!petWin) return
    const petPos = await petWin.outerPosition()
    const petSize = await petWin.outerSize()
    const scale = await petWin.scaleFactor()
    const winSize = await chatWindow.outerSize()

    const petLogX = petPos.x / scale
    const petLogY = petPos.y / scale
    const petLogW = petSize.width / scale
    const chatLogW = winSize.width / scale
    const chatLogH = winSize.height / scale

    // 用虚拟屏幕（所有显示器合集）边界判断/夹紧，替代 window.screen（仅主屏）
    const bounds = await getVirtualScreenBounds()
    const boundsX = bounds.x / scale
    const boundsW = bounds.width / scale

    let x: number
    if (petLogX + petLogW + chatLogW + 20 < boundsX + boundsW) {
      x = Math.round(petLogX + petLogW + 6)
      tailDirection.value = 'left'
    } else {
      x = Math.round(petLogX - chatLogW - 6)
      tailDirection.value = 'right'
    }
    const y = Math.round(petLogY)

    const clamped = await clampToVirtualScreen(x, y, chatLogW, chatLogH, scale)

    await chatWindow.setPosition(new LogicalPosition(Math.round(clamped.x), Math.round(clamped.y)))
  } catch (e) {
    console.warn('Failed to position chat window:', e)
  }
}

async function loadInitialData() {
  error.value = ''
  let ghostId = ''
  try {
    const status = await invoke<string>('get_ghost_status')
    const parsed = JSON.parse(status)
    ghostName.value = parsed.name || '桌宠'
    ghostId = parsed.ghostId || ''
    ghostLoaded.value = true
  } catch {
    error.value = '尚未创建桌宠灵魂，请在主窗口生成'
  }

  answeringMode.value = (localStorage.getItem('deskpet_answering_mode') as 'Companion' | 'Assistant') || 'Companion'

  if (ghostId) {
    try {
      const history = await invoke<Array<{ id: string; role: string; content: string; createdAt: string }>>(
        'load_chat_history',
        { ghostId, limit: 100 },
      )
      messages.value = history.map(m => ({
        role: m.role === 'assistant' ? 'pet' : m.role as 'pet' | 'user' | 'system',
        content: m.content,
      }))
    } catch {
      console.warn('Failed to load chat history')
    }
  }
}

function speakSystem(text: string) {
  if (!window.speechSynthesis) return
  window.speechSynthesis.cancel()
  const utterance = new SpeechSynthesisUtterance(text)
  utterance.lang = 'zh-CN'
  utterance.rate = parseFloat(localStorage.getItem('deskpet_tts_rate') || '1.0')
  utterance.pitch = parseFloat(localStorage.getItem('deskpet_tts_pitch') || '1.1')
  utterance.volume = 0.9
  window.speechSynthesis.speak(utterance)
}

let currentEdgeAudio: HTMLAudioElement | null = null

async function speakEdge(text: string) {
  try {
    if (currentEdgeAudio) {
      currentEdgeAudio.pause()
      currentEdgeAudio = null
    }
    const base64 = await invoke<string>('speak_edge_tts', {
      text,
      voice: localStorage.getItem('deskpet_tts_voice') || 'zh-CN-XiaoxiaoNeural',
    })
    const blob = new Blob(
      [Uint8Array.from(atob(base64), c => c.charCodeAt(0))],
      { type: 'audio/mp3' },
    )
    const url = URL.createObjectURL(blob)
    const audio = new Audio(url)
    audio.volume = 0.9
    currentEdgeAudio = audio
    audio.onended = () => {
      URL.revokeObjectURL(url)
      if (currentEdgeAudio === audio) currentEdgeAudio = null
    }
    audio.onerror = () => {
      URL.revokeObjectURL(url)
      if (currentEdgeAudio === audio) currentEdgeAudio = null
    }
    await audio.play()
  } catch {
    speakSystem(text)
  }
}

function speak(text: string) {
  if (localStorage.getItem('deskpet_tts_enabled') !== 'true') return
  const engine = (localStorage.getItem('deskpet_tts_engine') as 'system' | 'edge') || 'system'
  if (engine === 'edge') {
    speakEdge(text)
  } else {
    speakSystem(text)
  }
}

// 把后端返回的技术性错误翻译成对用户友好的简短提示，
// 并去掉暴露的 URL/状态码细节。
function friendlyError(raw: string): string {
  const s = String(raw)
  if (s.includes('API Key 为空')) return '我还没办法说话——请先在设置里配置 AI 接口（API Key）'
  if (s.includes('[401]') || /api key|unauthor/i.test(s)) return 'AI 密钥错误或已失效，请检查设置中的 API Key'
  if (s.includes('[403]')) return 'AI 接口拒绝访问，请确认密钥权限或账户状态'
  if (s.includes('[429]')) return 'AI 调用过于频繁或额度不足，请稍后再试'
  if (/\[5\d\d\]/.test(s)) return 'AI 服务暂时不可用，请稍后再试'
  if (s.includes('网络请求失败') || s.includes('流式请求失败') || /network|timeout|connect/i.test(s)) return '网络连接失败，请检查网络后重试'
  if (s.includes('AI 接口未配置')) return '我还没办法说话——请先在设置里配置 AI 接口'
  // 兜底：去掉 URL 部分，截断过长的原始响应
  const cleaned = s.replace(/\(URL: [^)]+\)/g, '').replace(/\\n原始响应:[\s\S]*/g, '')
  return cleaned.length > 80 ? cleaned.slice(0, 80) + '…' : cleaned
}

function handleSend() {
  if (chatLoading.value) return
  const msg = inputValue.value.trim()
  if (!msg && !allowEmptySend.value) return

  if (pendingScreenShotBase64.value) {
    const base64 = pendingScreenShotBase64.value
    pendingScreenShotBase64.value = null
    executeScreenShotAnalysis(base64, msg)
    return
  }

  messages.value.push({ role: 'user', content: msg })
  inputValue.value = ''
  chatLoading.value = true
  streamingContent.value = ''
  responseComplete.value = false

  invoke('chat_with_pet', { message: msg }).catch((e: any) => {
    messages.value.push({ role: 'system', content: friendlyError(e) })
    chatLoading.value = false
    responseComplete.value = false
  })
}

async function executeScreenShotAnalysis(base64: string, question: string) {
  chatLoading.value = true
  messages.value.push({ role: 'user', content: question || '📷 截图', imageBase64: base64 })
  inputValue.value = ''
  try {
    const params: Record<string, unknown> = { imageBase64: base64 }
    if (question) params.question = question
    const result = await invoke<{ description: string; petName: string }>('analyze_screenshot', params)
    messages.value.push({ role: 'pet', content: result.description })
    speak(result.description)
  } catch (e: any) {
    messages.value.push({ role: 'system', content: `截图识别失败：${friendlyError(e)}` })
  } finally {
    chatLoading.value = false
  }
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    handleSend()
  }
  if (e.key === 'Escape') {
    inputValue.value = ''
    ;(e.target as HTMLElement)?.blur()
  }
}

function handleDragOver(e: DragEvent) {
  e.preventDefault()
  e.stopPropagation()
  if (e.dataTransfer) e.dataTransfer.dropEffect = 'copy'
  dragOver.value = true
}

function handleDragLeave(e: DragEvent) {
  e.preventDefault()
  e.stopPropagation()
  dragOver.value = false
}

function handleDrop(e: DragEvent) {
  e.preventDefault()
  e.stopPropagation()
  dragOver.value = false
}

async function handleFileSelect() {
  if (chatLoading.value || importingDoc.value) return
  try {
    const selected = await open({
      filters: [{ name: '文档', extensions: ['txt', 'docx'] }],
      multiple: false,
    })
    if (selected) {
      const filePath = typeof selected === 'string' ? selected : (selected as { path: string }).path
      if (filePath) await doImport(filePath)
    }
  } catch (e: any) {
    messages.value.push({ role: 'system', content: `文件选择失败: ${e}` })
  }
}

async function doImport(filePath: string) {
  const fileName = filePath.split(/[/\\]/).pop() || '未知文件'
  const nameWithoutExt = fileName.replace(/\.[^.]+$/, '')
  importingDocTitle.value = nameWithoutExt
  importingDoc.value = true

  messages.value.push({ role: 'system', content: `正在阅读《${nameWithoutExt}》……` })

  try {
    const result = await invoke<{
      id: string
      title: string
      file_type: string
      word_count: number
      import_status: string
    }>('import_document', { filePath })

    if (result.import_status === 'ready') {
      const wordCount = result.word_count
      const displayWords = wordCount > 10000
        ? `${(wordCount / 10000).toFixed(1)}万字`
        : `${wordCount}字`
      messages.value.push({ role: 'system', content: `《${result.title}》已加入，共${displayWords}。随时可以聊它！` })
    } else if (result.import_status === 'error') {
      messages.value.push({ role: 'system', content: `《${result.title}》导入失败，请重试` })
    }
  } catch (e: any) {
    messages.value.push({ role: 'system', content: `文件导入失败：${friendlyError(e)}` })
  } finally {
    importingDoc.value = false
    importingDocTitle.value = ''
  }
}

function toggleAnsweringMode() {
  const newMode = answeringMode.value === 'Companion' ? 'Assistant' : 'Companion'
  answeringMode.value = newMode
  localStorage.setItem('deskpet_answering_mode', newMode)
  invoke('set_answering_mode', { mode: newMode }).catch(() => {})
}

function cancelScreenshot() {
  pendingScreenShotBase64.value = null
}

async function closeWindow() {
  window.speechSynthesis?.cancel()
  await chatWindow.close()
}

let tokenUnlisten: (() => void) | null = null
let completeUnlisten: (() => void) | null = null
let postProcessedUnlisten: (() => void) | null = null
let screenshotUnlisten: (() => void) | null = null
let repositionUnlisten: (() => void) | null = null
let themeUnlisten: (() => void) | null = null

onMounted(async () => {
  await loadInitialData()
  await positionNearPet()
  await refreshImpressionNote()
  // 定位完成后再显示，避免窗口先在默认位置闪现再移到目标位置
  try {
    await chatWindow.setShadow(false)
  } catch (e) {
    console.warn('Failed to disable shadow:', e)
  }
  await chatWindow.show()
  await chatWindow.setFocus()

  tokenUnlisten = await listen('chat:token', (event: any) => {
    streamingContent.value += (event.payload as string) || ''
  })

  completeUnlisten = await listen('chat:complete', (event: any) => {
    const data = event.payload as { response: string; generatedImage?: string }
    messages.value.push({ role: 'pet', content: data.response })
    speak(data.response)
    if (data.generatedImage) {
      messages.value.push({ role: 'pet', content: '[图像已生成]', imageBase64: data.generatedImage })
    }
    streamingContent.value = ''
    responseComplete.value = true
  })

  postProcessedUnlisten = await listen('chat:post-processed', (event: any) => {
    const data = event.payload as {
      loveHate: number
      baseline: number
      sentiment?: {
        eventType: string
        loveHateHint: number
      }
      impression?: {
        overallAffinity: number
        latestSnippet: string | null
      }
    }

    if (data.sentiment && data.sentiment.loveHateHint !== 0) {
      const hint = data.sentiment.loveHateHint > 0
        ? `(感到${data.sentiment.loveHateHint > 2 ? '很开心' : '有些开心'})`
        : `(感到${data.sentiment.loveHateHint < -2 ? '很不高兴' : '有点不高兴'})`
      messages.value.push({ role: 'system', content: `情感: ${data.sentiment.eventType} ${hint}` })
    }

    if (data.impression?.latestSnippet) {
      impressionNote.value = data.impression.latestSnippet
      messages.value.push({ role: 'system', content: `印象: ${data.impression.latestSnippet}` })
    }

    chatLoading.value = false
    responseComplete.value = false
  })

  screenshotUnlisten = await listen('chat:open-with-screenshot', (event: any) => {
    pendingScreenShotBase64.value = event.payload?.base64 || null
    nextTick(() => inputRef.value?.focus())
  })

  repositionUnlisten = await listen('chat:reposition', () => {
    positionNearPet()
  })

  themeUnlisten = await listen('settings-updated', (event: any) => {
    if (event.payload?.section === 'theme') applyTheme()
  })

  nextTick(() => inputRef.value?.focus())
  // 通知主窗口：监听器已全部注册、窗口已就绪，可以安全发事件了
  await emit('chat:ready')
})

onUnmounted(() => {
  if (tokenUnlisten) tokenUnlisten()
  if (completeUnlisten) completeUnlisten()
  if (postProcessedUnlisten) postProcessedUnlisten()
  if (screenshotUnlisten) screenshotUnlisten()
  if (repositionUnlisten) repositionUnlisten()
  if (themeUnlisten) themeUnlisten()
  if (typingInterval) clearInterval(typingInterval)
  window.speechSynthesis?.cancel()
})
</script>

<template>
  <div class="chat-app" @dragover="handleDragOver" @dragleave="handleDragLeave" @drop="handleDrop">
    <!-- Drag overlay -->
    <div v-if="dragOver" class="drag-overlay">
      <span class="drag-icon">📄</span>
      <span>松开以导入文档</span>
    </div>

    <div class="chat-bubble" :class="'tail-' + tailDirection" :style="bubbleStyle">
      <!-- Header -->
      <div class="chat-header">
        <div class="header-left">
          <span class="header-pet-name">{{ ghostName }}</span>
          <span class="header-mode-badge" :class="'mode-' + answeringMode.toLowerCase()">
            {{ answeringMode === 'Companion' ? '陪伴' : '助力' }}
          </span>
        </div>
        <button class="header-close" @click="closeWindow" title="关闭">✕</button>
      </div>

      <!-- Error state -->
      <div v-if="error" class="chat-error">
        <span>{{ error }}</span>
      </div>

      <!-- Messages -->
      <div v-else class="chat-messages" ref="messageListRef">
        <BubbleMessage
          v-for="(msg, i) in messages"
          :key="i"
          :role="msg.role"
          :content="msg.content"
          :pet-name="ghostName"
          :image-base64="msg.imageBase64"
          :index="i"
        />
        <!-- Streaming -->
        <div v-if="streamingContent" class="bubble-wrapper bubble-pet bubble-enter">
          <span class="bubble-sender sender-pet">{{ ghostName }}</span>
          <div class="bubble-content bubble-streaming">
            <span class="bubble-text">{{ streamingContent }}<span class="streaming-cursor">|</span></span>
          </div>
          <div class="bubble-tail bubble-tail-left"></div>
        </div>
        <!-- Typing indicator -->
        <div v-if="chatLoading && !streamingContent && !responseComplete" class="bubble-wrapper bubble-pet bubble-enter">
          <span class="bubble-sender sender-pet">{{ ghostName }}</span>
          <div class="bubble-content pet-typing">
            <span class="typing-indicator">{{ typingText }}</span>
          </div>
          <div class="bubble-tail bubble-tail-left"></div>
        </div>
      </div>

      <!-- Screenshot pending banner -->
      <div v-if="pendingScreenShotBase64" class="screenshot-banner">
        <span class="screenshot-banner-icon">📷</span>
        <span class="screenshot-banner-text">截图已就绪</span>
        <button class="screenshot-banner-cancel" @click="cancelScreenshot">取消</button>
      </div>

      <!-- Import status -->
      <div v-if="importingDoc" class="import-banner">
        <span class="import-icon">📖</span>
        <span class="import-text">正在阅读《{{ importingDocTitle }}》……</span>
      </div>

      <!-- 它眼中：常驻印象行 -->
      <div v-if="impressionNote" class="impression-note" title="宠物最近对你的印象">
        <span class="impression-note-icon">💭</span>
        <span class="impression-note-text">{{ impressionNote }}</span>
      </div>

      <!-- 录音指示 -->
      <div v-if="recording" class="recording-indicator">
        <span class="rec-dot"></span>
        <span>录音中 {{ recordingSeconds }}s · 点击 🎤 结束</span>
      </div>
      <div v-if="transcribing" class="recording-indicator">
        <span class="loading-spinner"></span>
        <span>正在转写...</span>
      </div>

      <!-- Input area -->
      <div class="chat-input-area">
        <button
          class="mode-toggle"
          :class="'mode-' + answeringMode.toLowerCase()"
          @click="toggleAnsweringMode"
          :title="answeringMode === 'Companion' ? '陪伴模式' : '助力模式'"
        >
          {{ answeringMode === 'Companion' ? '♥' : '✦' }}
        </button>
        <input
          ref="inputRef"
          v-model="inputValue"
          class="chat-input"
          :placeholder="placeholderText"
          @keydown="handleKeydown"
        />
        <button
          class="chat-mic-btn"
          :class="{ 'mic-recording': recording }"
          :disabled="chatLoading || importingDoc || transcribing"
          @click="toggleRecording"
          :title="recording ? '停止并转写' : '语音输入 (🎤)'"
        >
          🎤
        </button>
        <button class="chat-file-btn" :disabled="chatLoading || importingDoc" @click="handleFileSelect" title="导入文档 (.txt/.docx)">
          📎
        </button>
        <button class="chat-send" :disabled="chatLoading || (!inputValue.trim() && !allowEmptySend)" @click="handleSend">
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
            <path d="M2 8L14 2L10 8L14 14L2 8Z" fill="currentColor" stroke="currentColor" stroke-width="0.5" stroke-linejoin="round"/>
          </svg>
        </button>
      </div>
    </div>

    <!-- 四周透明 resize 热区：鼠标放边缘变 resize 光标，拖动调整窗口大小（气泡同步跟随） -->
    <div class="chat-resize-frame">
      <div class="rs rs-n" @mousedown="startResize('n', $event)"></div>
      <div class="rs rs-s" @mousedown="startResize('s', $event)"></div>
      <div class="rs rs-e" @mousedown="startResize('e', $event)"></div>
      <div class="rs rs-w" @mousedown="startResize('w', $event)"></div>
      <div class="rs rs-ne" @mousedown="startResize('ne', $event)"></div>
      <div class="rs rs-nw" @mousedown="startResize('nw', $event)"></div>
      <div class="rs rs-se" @mousedown="startResize('se', $event)"></div>
      <div class="rs rs-sw" @mousedown="startResize('sw', $event)"></div>
    </div>
  </div>
</template>

<style scoped>
.chat-app {
  width: 100vw;
  height: 100vh;
  position: relative;
  background: transparent;
  display: flex;
  align-items: center;
  justify-content: center;
  font-family: 'Segoe UI', system-ui, -apple-system, sans-serif;
  font-size: 13px;
  color: var(--t-text, #333);
  user-select: none;
}

.chat-bubble {
  position: relative;
  width: calc(100% - 48px);
  height: calc(100% - 48px);
  background: var(--t-panel-bg, rgba(255, 255, 255, 0.97));
  backdrop-filter: blur(var(--t-panel-blur, 16px));
  border-radius: var(--t-panel-radius, 20px);
  /* 阴影扩散 24px：边距 24px 让渐变完整淡出到窗口边缘，避免硬切 */
  box-shadow: var(--t-panel-shadow, 0 4px 24px rgba(0, 0, 0, 0.15), 0 0 0 1px rgba(0, 0, 0, 0.05));
  border: 1px solid var(--t-panel-border, transparent);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  margin: 24px;
}

/* 四周透明 resize 热区（无背景无半透明像素，只提供光标与拖拽） */
.chat-resize-frame {
  position: absolute;
  inset: 0;
  z-index: 999;
  pointer-events: none;
}

.chat-resize-frame .rs {
  position: absolute;
  pointer-events: auto;
  background: transparent;
}

.rs-n, .rs-s { left: 0; right: 0; height: 24px; }
.rs-e, .rs-w { top: 0; bottom: 0; width: 24px; }
.rs-n { top: 0; cursor: n-resize; }
.rs-s { bottom: 0; cursor: s-resize; }
.rs-e { right: 0; cursor: e-resize; }
.rs-w { left: 0; cursor: w-resize; }
.rs-ne { top: 0; right: 0; width: 28px; height: 28px; cursor: ne-resize; }
.rs-nw { top: 0; left: 0; width: 28px; height: 28px; cursor: nw-resize; }
.rs-se { bottom: 0; right: 0; width: 28px; height: 28px; cursor: se-resize; }
.rs-sw { bottom: 0; left: 0; width: 28px; height: 28px; cursor: sw-resize; }

/* Bubble tail - left (window on right of pet) */
.chat-bubble.tail-left::before {
  content: '';
  position: absolute;
  left: -14px;
  top: 40px;
  width: 0;
  height: 0;
  border-top: 9px solid transparent;
  border-bottom: 9px solid transparent;
  border-right: 14px solid var(--t-panel-bg, rgba(255, 255, 255, 0.97));
  filter: drop-shadow(-1px 0 2px rgba(0, 0, 0, 0.06));
  z-index: 1;
}

/* Bubble tail - right (window on left of pet) */
.chat-bubble.tail-right::before {
  content: '';
  position: absolute;
  right: -14px;
  top: 40px;
  width: 0;
  height: 0;
  border-top: 9px solid transparent;
  border-bottom: 9px solid transparent;
  border-left: 14px solid var(--t-panel-bg, rgba(255, 255, 255, 0.97));
  filter: drop-shadow(1px 0 2px rgba(0, 0, 0, 0.06));
  z-index: 1;
}

/* Header */
.chat-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 14px;
  border-bottom: 1px solid var(--t-divider, rgba(0, 0, 0, 0.06));
  flex-shrink: 0;
  -webkit-app-region: drag;
}

.chat-header button,
.chat-header .header-mode-badge {
  -webkit-app-region: no-drag;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 8px;
}

.header-pet-name {
  font-size: 14px;
  font-weight: 700;
  color: var(--t-header-text, #333);
}

.header-mode-badge {
  font-size: 10px;
  font-weight: 600;
  padding: 2px 7px;
  border-radius: 6px;
}

.mode-companion {
  background: var(--t-badge-cp-bg, rgba(255, 152, 0, 0.1));
  color: var(--t-badge-cp-text, #e65100);
}

.mode-assistant {
  background: var(--t-badge-as-bg, rgba(33, 150, 243, 0.1));
  color: var(--t-badge-as-text, #1565c0);
}

.header-close {
  width: 26px;
  height: 26px;
  border: none;
  background: transparent;
  border-radius: 8px;
  color: var(--t-text-3, #999);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 14px;
  transition: all 0.15s;
}

.header-close:hover {
  background: var(--t-bg-2, rgba(0, 0, 0, 0.06));
  color: var(--t-header-text, #333);
}

/* Error */
.chat-error {
  padding: 40px 20px;
  text-align: center;
  color: var(--t-error-text, #f44336);
  font-size: 13px;
}

/* Messages */
.chat-messages {
  flex: 1;
  overflow-y: auto;
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-height: 0;
}

.chat-messages::-webkit-scrollbar {
  width: 4px;
}

.chat-messages::-webkit-scrollbar-track {
  background: transparent;
}

.chat-messages::-webkit-scrollbar-thumb {
  background: var(--t-scroll-thumb, rgba(0, 0, 0, 0.12));
  border-radius: 2px;
}

/* Bubble message overrides for larger chat window */
.chat-messages :deep(.bubble-wrapper) {
  max-width: 280px;
}

.chat-messages :deep(.bubble-system .bubble-content) {
  max-width: 300px;
}

.chat-messages :deep(.bubble-pet) {
  align-self: flex-start;
}

.chat-messages :deep(.bubble-user) {
  align-self: flex-end;
}

/* Screenshot banner */
.screenshot-banner {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  background: var(--t-badge-cp-bg, rgba(255, 152, 0, 0.08));
  border-top: 1px solid var(--t-badge-cp-text, rgba(255, 152, 0, 0.15));
  flex-shrink: 0;
}

.screenshot-banner-icon {
  font-size: 14px;
}

.screenshot-banner-text {
  flex: 1;
  font-size: 12px;
  color: var(--t-badge-cp-text, #e65100);
  font-weight: 500;
}

.screenshot-banner-cancel {
  font-size: 11px;
  padding: 3px 10px;
  border: 1px solid var(--t-badge-cp-text, rgba(255, 152, 0, 0.3));
  border-radius: 6px;
  background: var(--t-panel-bg, rgba(255, 255, 255, 0.8));
  color: var(--t-badge-cp-text, #e65100);
  cursor: pointer;
  transition: all 0.15s;
}

.screenshot-banner-cancel:hover {
  background: var(--t-badge-cp-bg, #fff3e0);
}

/* Input area */
.chat-input-area {
  display: flex;
  gap: 6px;
  padding: 8px 12px 10px;
  border-top: 1px solid var(--t-divider, rgba(0, 0, 0, 0.06));
  flex-shrink: 0;
  background: var(--t-bg-2, rgba(0, 0, 0, 0.02));
}

.impression-note {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 5px 14px 0;
  font-size: 10.5px;
  color: var(--t-text-2, #9a8fc0);
  flex-shrink: 0;
  overflow: hidden;
  white-space: nowrap;
}

.impression-note-icon {
  flex-shrink: 0;
}

.impression-note-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mode-toggle {
  width: 32px;
  height: 34px;
  border: 2px solid var(--t-border, #ddd);
  border-radius: 10px;
  background: var(--t-btn-bg, #fafafa);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 14px;
  transition: all 0.15s;
  flex-shrink: 0;
  padding: 0;
}

.mode-toggle:hover {
  transform: scale(1.1);
}

.mode-toggle.mode-companion {
  border-color: var(--t-badge-cp-text, rgba(255, 152, 0, 0.5));
  background: var(--t-badge-cp-bg, rgba(255, 152, 0, 0.08));
  color: var(--t-badge-cp-text, #e65100);
}

.mode-toggle.mode-assistant {
  border-color: var(--t-badge-as-text, rgba(33, 150, 243, 0.4));
  background: var(--t-badge-as-bg, rgba(33, 150, 243, 0.06));
  color: var(--t-badge-as-text, #1565c0);
}

.chat-input {
  flex: 1;
  min-width: 0;
  padding: 7px 12px;
  border: 2px solid var(--t-input-border, #e0e0e0);
  border-radius: 12px;
  font-size: 13px;
  outline: none;
  transition: border-color 0.2s, box-shadow 0.2s;
  background: var(--t-input-bg, #fff);
  color: var(--t-text, #333);
  font-family: inherit;
}

.chat-input::placeholder {
  color: var(--t-text-3, #999);
}

.chat-input:focus {
  border-color: var(--t-input-focus, #ff6b9d);
  box-shadow: 0 0 0 3px var(--t-input-glow, rgba(255, 107, 157, 0.12));
}

.chat-send {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border: none;
  border-radius: 12px;
  background: var(--t-send-bg, linear-gradient(135deg, #ff6b9d, #c084fc));
  background-size: var(--t-grad-size, 100% 100%);
  color: var(--t-send-text, #fff);
  cursor: pointer;
  transition: opacity 0.15s, transform 0.15s;
  flex-shrink: 0;
}

.chat-send:hover:not(:disabled) {
  opacity: 0.9;
  transform: scale(1.05);
  animation: var(--t-btn-pop, none) 0.35s ease;
}

.chat-send:not(:disabled) {
  animation: var(--t-send-glow, none) 2s ease-in-out infinite, var(--t-grad-flow, none) 3s ease infinite;
}

.chat-send:disabled {
  opacity: 0.4;
  cursor: not-allowed;
  transform: none;
}

/* Reuse BubbleMessage styles for inline streaming/typing */
.bubble-wrapper {
  position: relative;
  max-width: 280px;
}

.bubble-pet {
  align-self: flex-start;
}

.bubble-enter {
  animation: var(--t-msg-enter, msgIn) var(--t-msg-dur, 0.25s) ease both;
}

.bubble-sender {
  display: block;
  font-size: 10px;
  font-weight: 700;
  margin-bottom: 1px;
  letter-spacing: 0.3px;
}

.sender-pet {
  color: var(--t-sender-pet, #ff6b9d);
}

.bubble-content {
  padding: 8px 12px;
  font-size: 13px;
  line-height: 1.6;
  word-break: break-word;
}

.bubble-text {
  white-space: pre-wrap;
}

.bubble-pet .bubble-content {
  background: var(--t-bp-bg, #fff);
  border: var(--t-bp-border, 2.5px solid #333);
  border-radius: var(--t-bp-radius, 16px 16px 16px 4px);
  box-shadow: var(--t-bp-shadow, 3px 3px 0 #333);
  color: var(--t-bp-text, #333);
}

.bubble-tail-left {
  position: absolute;
  width: 0;
  height: 0;
  bottom: -10px;
  left: 16px;
  border-left: 8px solid transparent;
  border-right: 8px solid transparent;
  border-top: 10px solid var(--t-bp-tail, #333);
}

.bubble-tail-left::after {
  content: '';
  position: absolute;
  top: -12px;
  left: -7px;
  border-left: 7px solid transparent;
  border-right: 7px solid transparent;
  border-top: 9px solid var(--t-bp-bg, #fff);
}

.pet-typing {
  background: var(--t-bp-bg, #fff) !important;
  border: var(--t-bp-border, 2.5px solid #333) !important;
  border-radius: var(--t-bp-radius, 16px 16px 16px 4px) !important;
  box-shadow: var(--t-bp-shadow, 3px 3px 0 #333) !important;
  padding: 8px 14px !important;
}

.typing-indicator {
  font-size: 14px;
  letter-spacing: 3px;
  color: var(--t-typing, #ff6b9d);
}

.bubble-streaming {
  background: var(--t-bp-bg, #fff) !important;
  border: var(--t-bp-border, 2.5px solid #333) !important;
  border-radius: var(--t-bp-radius, 16px 16px 16px 4px) !important;
  box-shadow: var(--t-bp-shadow, 3px 3px 0 #333) !important;
  padding: 8px 14px !important;
  animation: streamPulse 2s ease-in-out infinite;
}

@keyframes streamPulse {
  0%, 100% { box-shadow: var(--t-bp-shadow, 3px 3px 0 #333); }
  50% { box-shadow: var(--t-bp-shadow-glow, 3px 3px 0 #ff6b9d); }
}

.streaming-cursor {
  animation: cursorBlink 0.6s step-end infinite;
  color: var(--t-typing, #ff6b9d);
  font-weight: bold;
  margin-left: 1px;
}

@keyframes cursorBlink {
  50% { opacity: 0; }
}

.chat-file-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 36px;
  border: 2px solid var(--t-border, #e0e0e0);
  border-radius: 10px;
  background: var(--t-btn-bg, #fafafa);
  cursor: pointer;
  font-size: 16px;
  transition: all 0.15s;
  flex-shrink: 0;
  padding: 0;
}

.chat-file-btn:hover:not(:disabled) {
  border-color: var(--t-secondary, #c084fc);
  background: var(--t-btn-hover-bg, #faf5ff);
  transform: scale(1.1);
}

.chat-file-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.chat-mic-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 36px;
  border: 2px solid var(--t-border, #e0e0e0);
  border-radius: 10px;
  background: var(--t-btn-bg, #fafafa);
  cursor: pointer;
  font-size: 15px;
  transition: all 0.15s;
  flex-shrink: 0;
  padding: 0;
}

.chat-mic-btn:hover:not(:disabled) {
  border-color: var(--t-secondary, #c084fc);
  background: var(--t-btn-hover-bg, #faf5ff);
  transform: scale(1.1);
}

.chat-mic-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.chat-mic-btn.mic-recording {
  border-color: var(--t-error-text, #f44336);
  background: rgba(244, 67, 54, 0.12);
  animation: micPulse 1s ease-in-out infinite;
}

@keyframes micPulse {
  0%, 100% { transform: scale(1); }
  50% { transform: scale(1.15); }
}

.recording-indicator {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 14px 0;
  font-size: 10.5px;
  color: var(--t-error-text, #e53935);
  flex-shrink: 0;
}

.rec-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--t-error-text, #f44336);
  animation: recBlink 1s step-start infinite;
}

@keyframes recBlink {
  50% { opacity: 0.2; }
}

.recording-indicator .loading-spinner {
  width: 10px;
  height: 10px;
  border: 2px solid rgba(0, 0, 0, 0.15);
  border-top-color: var(--t-spinner, #ff6b9d);
  border-radius: 50%;
  animation: recSpin 0.7s linear infinite;
}

@keyframes recSpin {
  to { transform: rotate(360deg); }
}

.drag-overlay {
  position: absolute;
  inset: 0;
  background: color-mix(in srgb, var(--t-secondary, #c084fc) 14%, transparent);
  border: 3px dashed var(--t-secondary, #c084fc);
  border-radius: var(--t-panel-radius, 20px);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  z-index: 100;
  font-size: 15px;
  color: var(--t-secondary, #7c3aed);
  font-weight: 600;
  pointer-events: none;
  backdrop-filter: blur(2px);
}

.drag-icon {
  font-size: 36px;
}

.import-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 14px;
  background: var(--t-config-bg, rgba(192, 132, 252, 0.08));
  border-top: 1px solid var(--t-config-border, rgba(192, 132, 252, 0.2));
  flex-shrink: 0;
}

.import-icon {
  font-size: 14px;
  animation: importPulse 1.5s ease-in-out infinite;
}

@keyframes importPulse {
  0%, 100% { opacity: 0.4; }
  50% { opacity: 1; }
}

.import-text {
  font-size: 12px;
  color: var(--t-secondary, #7c3aed);
  font-weight: 500;
}
</style>
