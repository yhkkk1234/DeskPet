<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { LogicalPosition } from '@tauri-apps/api/dpi'
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'
import BubbleMessage from './components/BubbleMessage.vue'

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
    const screenW = window.screen.availWidth

    let x: number
    if (petLogX + petLogW + chatLogW + 20 < screenW) {
      x = Math.round(petLogX + petLogW + 6)
      tailDirection.value = 'left'
    } else {
      x = Math.max(0, Math.round(petLogX - chatLogW - 6))
      tailDirection.value = 'right'
    }
    const y = Math.round(Math.max(0, Math.min(petLogY, screen.availHeight - chatLogH)))

    await chatWindow.setPosition(new LogicalPosition(x, y))
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

async function speakEdge(text: string) {
  try {
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
    audio.play()
    audio.onended = () => URL.revokeObjectURL(url)
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
    messages.value.push({ role: 'system', content: `发送失败: ${e}` })
    chatLoading.value = false
    responseComplete.value = false
  })
}

async function executeScreenShotAnalysis(base64: string, question: string) {
  chatLoading.value = true
  messages.value.push({ role: 'user', content: '✓ 截图', imageBase64: base64 })
  try {
    const params: Record<string, unknown> = { imageBase64: base64 }
    if (question) params.question = question
    const result = await invoke<{ description: string; petName: string }>('analyze_screenshot', params)
    messages.value.push({ role: 'pet', content: result.description })
    speak(result.description)
  } catch (e: any) {
    messages.value.push({ role: 'system', content: `截图识别失败: ${e}` })
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

onMounted(async () => {
  await loadInitialData()
  await positionNearPet()

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

  nextTick(() => inputRef.value?.focus())
})

onUnmounted(() => {
  if (tokenUnlisten) tokenUnlisten()
  if (completeUnlisten) completeUnlisten()
  if (postProcessedUnlisten) postProcessedUnlisten()
  if (screenshotUnlisten) screenshotUnlisten()
  if (repositionUnlisten) repositionUnlisten()
  if (typingInterval) clearInterval(typingInterval)
  window.speechSynthesis?.cancel()
})
</script>

<template>
  <div class="chat-app">
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
        <button class="chat-send" :disabled="chatLoading || (!inputValue.trim() && !allowEmptySend)" @click="handleSend">
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
            <path d="M2 8L14 2L10 8L14 14L2 8Z" fill="currentColor" stroke="currentColor" stroke-width="0.5" stroke-linejoin="round"/>
          </svg>
        </button>
      </div>
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
  color: #333;
  user-select: none;
}

.chat-bubble {
  position: relative;
  width: 340px;
  max-height: 480px;
  background: rgba(255, 255, 255, 0.97);
  backdrop-filter: blur(16px);
  border-radius: 20px;
  box-shadow: 0 4px 24px rgba(0, 0, 0, 0.15), 0 0 0 1px rgba(0, 0, 0, 0.05);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  margin: 8px;
}

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
  border-right: 14px solid rgba(255, 255, 255, 0.97);
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
  border-left: 14px solid rgba(255, 255, 255, 0.97);
  filter: drop-shadow(1px 0 2px rgba(0, 0, 0, 0.06));
  z-index: 1;
}

/* Header */
.chat-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 14px;
  border-bottom: 1px solid rgba(0, 0, 0, 0.06);
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
  color: #333;
}

.header-mode-badge {
  font-size: 10px;
  font-weight: 600;
  padding: 2px 7px;
  border-radius: 6px;
}

.mode-companion {
  background: rgba(255, 152, 0, 0.1);
  color: #e65100;
}

.mode-assistant {
  background: rgba(33, 150, 243, 0.1);
  color: #1565c0;
}

.header-close {
  width: 26px;
  height: 26px;
  border: none;
  background: transparent;
  border-radius: 8px;
  color: #999;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 14px;
  transition: all 0.15s;
}

.header-close:hover {
  background: rgba(0, 0, 0, 0.06);
  color: #333;
}

/* Error */
.chat-error {
  padding: 40px 20px;
  text-align: center;
  color: #f44336;
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
  background: rgba(0, 0, 0, 0.12);
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
  background: rgba(255, 152, 0, 0.08);
  border-top: 1px solid rgba(255, 152, 0, 0.15);
  flex-shrink: 0;
}

.screenshot-banner-icon {
  font-size: 14px;
}

.screenshot-banner-text {
  flex: 1;
  font-size: 12px;
  color: #e65100;
  font-weight: 500;
}

.screenshot-banner-cancel {
  font-size: 11px;
  padding: 3px 10px;
  border: 1px solid rgba(255, 152, 0, 0.3);
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.8);
  color: #e65100;
  cursor: pointer;
  transition: all 0.15s;
}

.screenshot-banner-cancel:hover {
  background: #fff3e0;
}

/* Input area */
.chat-input-area {
  display: flex;
  gap: 6px;
  padding: 8px 12px 10px;
  border-top: 1px solid rgba(0, 0, 0, 0.06);
  flex-shrink: 0;
  background: rgba(0, 0, 0, 0.02);
}

.mode-toggle {
  width: 32px;
  height: 34px;
  border: 2px solid #ddd;
  border-radius: 10px;
  background: #fafafa;
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

.chat-input {
  flex: 1;
  padding: 7px 12px;
  border: 2px solid #e0e0e0;
  border-radius: 12px;
  font-size: 13px;
  outline: none;
  transition: border-color 0.2s, box-shadow 0.2s;
  background: #fff;
  font-family: inherit;
}

.chat-input:focus {
  border-color: #ff6b9d;
  box-shadow: 0 0 0 3px rgba(255, 107, 157, 0.12);
}

.chat-send {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border: none;
  border-radius: 12px;
  background: linear-gradient(135deg, #ff6b9d, #c084fc);
  color: white;
  cursor: pointer;
  transition: opacity 0.15s, transform 0.15s;
  flex-shrink: 0;
}

.chat-send:hover:not(:disabled) {
  opacity: 0.9;
  transform: scale(1.05);
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
  animation: slideIn 0.25s ease both;
}

@keyframes slideIn {
  from { opacity: 0; transform: translateY(6px); }
  to { opacity: 1; transform: translateY(0); }
}

.bubble-sender {
  display: block;
  font-size: 10px;
  font-weight: 700;
  margin-bottom: 1px;
  letter-spacing: 0.3px;
}

.sender-pet {
  color: #ff6b9d;
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
  background: #fff;
  border: 2.5px solid #333;
  border-radius: 16px 16px 16px 4px;
  box-shadow: 3px 3px 0 #333;
  color: #333;
}

.bubble-tail-left {
  position: absolute;
  width: 0;
  height: 0;
  bottom: -10px;
  left: 16px;
  border-left: 8px solid transparent;
  border-right: 8px solid transparent;
  border-top: 10px solid #333;
}

.bubble-tail-left::after {
  content: '';
  position: absolute;
  top: -12px;
  left: -7px;
  border-left: 7px solid transparent;
  border-right: 7px solid transparent;
  border-top: 9px solid #fff;
}

.pet-typing {
  background: #fff !important;
  border: 2.5px solid #333 !important;
  border-radius: 16px 16px 16px 4px !important;
  box-shadow: 3px 3px 0 #333 !important;
  padding: 8px 14px !important;
}

.typing-indicator {
  font-size: 14px;
  letter-spacing: 3px;
  color: #ff6b9d;
  animation: typingPulse 1s ease-in-out infinite;
}

@keyframes typingPulse {
  0%, 100% { opacity: 0.6; }
  50% { opacity: 1; }
}

.bubble-streaming {
  background: #fff !important;
  border: 2.5px solid #333 !important;
  border-radius: 16px 16px 16px 4px !important;
  box-shadow: 3px 3px 0 #333 !important;
  padding: 8px 14px !important;
  animation: streamPulse 2s ease-in-out infinite;
}

@keyframes streamPulse {
  0%, 100% { box-shadow: 3px 3px 0 #333; }
  50% { box-shadow: 3px 3px 0 #ff6b9d; }
}

.streaming-cursor {
  animation: cursorBlink 0.6s step-end infinite;
  color: #ff6b9d;
  font-weight: bold;
  margin-left: 1px;
}

@keyframes cursorBlink {
  50% { opacity: 0; }
}
</style>
