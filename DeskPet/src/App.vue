<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { ref, onMounted, onUnmounted, computed, nextTick } from 'vue'
import BubbleDialogue from './components/BubbleDialogue.vue'
import PetRenderer from './components/PetRenderer.vue'
import EmotionTimeline from './components/EmotionTimeline.vue'
import { useChat } from './composables/useChat'
import { useBubbleTimer } from './composables/useBubbleTimer'
import { useAnimation } from './composables/useAnimation'
import { usePetRenderer } from './composables/usePetRenderer'

interface GhostStatus {
  ghostId: string
  name: string
  generation: number
  loveHate: number
  baseline: number
  personality: {
    openness: number
    conscientiousness: number
    extraversion: number
    agreeableness: number
    neuroticism: number
    creativity: number
  }
  movementStyle: string
  impression: {
    overallAffinity: number
    snippetCount: number
  }
  curiosityLevel: string
}

const ghost = ref<GhostStatus | null>(null)
const loading = ref(false)
const error = ref('')
const showPanel = ref(true)
const showPersonality = ref(false)

const { chatMessages, chatInput, chatLoading, sendMessage, pushUserMessage, pushSystemMessage, pushPetMessage, clearMessages } = useChat()
const { bubblesVisible, inputVisible, showInput, hideBubbles, toggleInput, resetHideTimer, onNewMessage, onUserActivity, bindChatLoading, onInputFocus, onInputBlur } = useBubbleTimer()
const { currentMood, currentAnimationState, moodConfig, isMoving, isPerformingIdle, petX, petY, facingDirection, isFlipped, movementStyle, updateMood, setPersonality, startIdleLoop, stopIdleLoop, movePetTo, resetPosition, playOneShot } = useAnimation()
const { rendererType, spriteConfig, tagRanges, frameDurations, setRenderer, setSpriteConfig, parseAsepriteJson } = usePetRenderer()

bindChatLoading(chatLoading)

const saveLoadPath = ref('')

const aiEndpoint = ref('https://api.deepseek.com/v1')
const aiApiKey = ref('')
const aiModel = ref('deepseek-chat')

const visionResult = ref('')
const screenshotScreenRegion = ref({ x: 0, y: 0, width: 0, height: 0 })
const screenshotAnalysisLoading = ref(false)

const loveHatePercent = computed(() => {
  if (!ghost.value) return 50
  return Math.round((ghost.value.loveHate + 100) / 2)
})

const affinityColor = computed(() => {
  if (!ghost.value) return '#888'
  const lh = ghost.value.loveHate
  if (lh > 50) return '#ff6b9d'
  if (lh > 20) return '#8bc34a'
  if (lh > -20) return '#ffb74d'
  if (lh > -50) return '#ff9800'
  return '#f44336'
})

const petEmoji = computed(() => {
  if (!ghost.value) return '🐱'
  if (chatLoading.value) return '💭'
  const config = moodConfig.value
  if (config) return config.expression
  const lh = ghost.value.loveHate
  if (lh > 50) return '😺'
  if (lh > 20) return '🐱'
  if (lh > -20) return '😐'
  if (lh > -50) return '😾'
  return '😿'
})

const petCssClass = computed(() => {
  const classes = ['pet-sprite-wrapper']
  if (screenshotBounce.value) classes.push('screenshot-bounce')
  return classes
})

const petName = computed(() => ghost.value?.name || '桌宠')

const moodText = computed(() => {
  if (!ghost.value) return ''
  const lh = ghost.value.loveHate
  if (lh > 50) return '很开心~'
  if (lh > 20) return '心情不错'
  if (lh > -20) return '还ok吧'
  if (lh > -50) return '有点烦'
  return '不太高兴...'
})

const bubbleDialogueRef = ref<InstanceType<typeof BubbleDialogue> | null>(null)
const screenshotBounce = ref(false)
let dragStartX = 0
let dragStartY = 0
let didDrag = false

function onPetMouseUp(e: MouseEvent) {
  const dx = Math.abs(e.screenX - dragStartX)
  const dy = Math.abs(e.screenY - dragStartY)
  if (dx < 5 && dy < 5 && !didDrag) {
    handlePetClick(e)
  }
}

async function generateGhost() {
  loading.value = true
  error.value = ''
  try {
    await invoke<string>('generate_ghost', { name: '小花' })
    ghost.value = JSON.parse(await invoke<string>('get_ghost_status'))
    updateMood(ghost.value.loveHate, ghost.value.curiosityLevel)
    if (ghost.value.personality) {
      setPersonality(ghost.value.personality)
    }
    pushSystemMessage(`${petName.value}的灵魂已注入！点击桌宠或按 Ctrl+Alt+C 开始对话。`)
    startIdleLoop()
  } catch (e: any) {
    error.value = e.toString()
  } finally {
    loading.value = false
  }
}

async function applyEvent(eventType: string, intensity: number) {
  if (!ghost.value) return
  try {
    const result = await invoke<string>('apply_event', { eventType, intensity, description: eventType })
    const parsed = JSON.parse(result)
    if (ghost.value) {
      ghost.value.loveHate = parsed.loveHate
      ghost.value.baseline = parsed.baseline
      ghost.value.impression.overallAffinity = parsed.overallAffinity
      updateMood(ghost.value.loveHate, ghost.value.curiosityLevel)
    }
  } catch (e: any) {
    console.error('Event error:', e)
  }
}

async function tickGhost() {
  if (!ghost.value) return
  try {
    const result = await invoke<{
      loveHate: number
      baseline: number
      inactivityEvent: {
        eventType: string
        intensity: number
        loveHateDelta: number
        hoursAway: number
      } | null
      curiosityTriggered: boolean
    }>('timeline_tick')
    if (ghost.value) {
      ghost.value.loveHate = result.loveHate
      ghost.value.baseline = result.baseline
      updateMood(ghost.value.loveHate, ghost.value.curiosityLevel)
    }
    if (result.inactivityEvent) {
      pushSystemMessage(`${result.inactivityEvent.hoursAway}小时没互动了...好感度变化 ${result.inactivityEvent.loveHateDelta > 0 ? '+' : ''}${result.inactivityEvent.loveHateDelta.toFixed(1)}`)
      onNewMessage()
    }
    if (result.curiosityTriggered && ghost.value) {
      playOneShot('curious', 800)
    }
  } catch (e: any) {
    console.error('Tick error:', e)
  }
}

let tickInterval: ReturnType<typeof setInterval> | null = null
let hotkeyUnlisten: (() => void) | null = null
let chatHotkeyUnlisten: (() => void) | null = null

async function loadSpriteJson() {
  try {
    const response = await fetch(spriteConfig.value.jsonSrc)
    if (response.ok) {
      const json = await response.json()
      parseAsepriteJson(json)
    } else {
      console.warn('Failed to load sprite JSON:', response.status)
    }
  } catch (e) {
    console.warn('Failed to load sprite JSON:', e)
  }
}

function loadRendererFromStorage() {
  const savedType = localStorage.getItem('deskpet_renderer_type') as 'css' | 'spritesheet' | 'spine' | null
  if (savedType) {
    setRenderer(savedType)
  }
  const savedSrc = localStorage.getItem('deskpet_sprite_src')
  const savedJsonSrc = localStorage.getItem('deskpet_sprite_json_src')
  if (savedSrc || savedJsonSrc) {
    setSpriteConfig({
      src: savedSrc || spriteConfig.value.src,
      jsonSrc: savedJsonSrc || spriteConfig.value.jsonSrc,
    })
  }
}

async function openSettingsWindow() {
  let settingsWin = await WebviewWindow.getByLabel('settings')
  if (!settingsWin) {
    settingsWin = new WebviewWindow('settings', {
      url: 'settings.html',
      title: 'DeskPet - 设置',
      width: 380,
      height: 600,
      minWidth: 320,
      minHeight: 400,
      resizable: true,
      transparent: true,
      decorations: false,
      alwaysOnTop: true,
      skipTaskbar: false,
      visible: true,
    })
    // 等待窗口加载完成
    await new Promise(resolve => setTimeout(resolve, 300))
    try {
      await settingsWin.setShadow(false)
    } catch (e) {
      console.warn('Failed to disable shadow:', e)
    }
  }
  const visible = await settingsWin.isVisible()
  if (visible) {
    await settingsWin.setFocus()
  } else {
    await settingsWin.show()
    await settingsWin.setFocus()
  }
}

onMounted(async () => {
  tickInterval = setInterval(tickGhost, 5000)
  generateGhost()
  loadSpriteJson()
  loadRendererFromStorage()

  hotkeyUnlisten = await listen('screenshot-hotkey', () => {
    triggerScreenshot()
  })

  chatHotkeyUnlisten = await listen('chat-hotkey', () => {
    handleChatHotkey()
  })

  await listen('settings-updated', async (event: any) => {
    const section = event.payload?.section
    if (section === 'renderer') {
      loadRendererFromStorage()
    } else if (section === 'curiosity' || section === 'ghost') {
      try {
        const status = await invoke<string>('get_ghost_status')
        ghost.value = JSON.parse(status)
        updateMood(ghost.value.loveHate, ghost.value.curiosityLevel)
      } catch (e) {
        console.error('Failed to refresh ghost status:', e)
      }
    }
  })

  await listen('request-transfer', () => {
    startTransfer()
  })

  await listen('screenshot-region-captured', (event: any) => {
    const { base64, region } = event.payload
    screenshotScreenRegion.value = region
    analyzeScreenshot(base64)
  })

  await listen('screenshot-cancelled', () => {
    screenshotAnalysisLoading.value = false
  })

  document.addEventListener('keydown', handleEscKey)
})

function handleEscKey(e: KeyboardEvent) {
  if (e.key === 'Escape' && inputVisible.value) {
    hideBubbles()
  }
}

onUnmounted(() => {
  if (tickInterval) clearInterval(tickInterval)
  if (hotkeyUnlisten) hotkeyUnlisten()
  if (chatHotkeyUnlisten) chatHotkeyUnlisten()
  document.removeEventListener('keydown', handleEscKey)
  stopIdleLoop()
})

function handleChatHotkey() {
  toggleInput()
  if (inputVisible.value) {
    nextTick(() => {
      bubbleDialogueRef.value?.focusInput()
    })
  }
}

function handlePetClick(e: MouseEvent) {
  e.stopPropagation()
  if (!ghost.value) return
  showInput()
  invoke('record_interaction').catch(() => {})
  nextTick(() => {
    bubbleDialogueRef.value?.focusInput()
  })
}

async function handleSendMessage(message: string) {
  pushUserMessage(message)
  chatLoading.value = true
  onNewMessage()

  try {
    const result = await invoke<{
      response: string
      loveHate: number
      baseline: number
      personality: Record<string, number>
      sentiment: {
        eventType: string
        loveHateHint: number
      }
      impression: {
        overallAffinity: number
        latestSnippet: string | null
      }
    }>('chat_with_pet', { message })

    pushPetMessage(result.response)

    if (ghost.value) {
      ghost.value.loveHate = result.loveHate
      ghost.value.baseline = result.baseline
      updateMood(ghost.value.loveHate, ghost.value.curiosityLevel)
      if (result.sentiment && result.sentiment.loveHateHint !== 0) {
        const hint = result.sentiment.loveHateHint > 0
          ? `(感到${result.sentiment.loveHateHint > 2 ? '很开心' : '有些开心'})`
          : `(感到${result.sentiment.loveHateHint < -2 ? '很不高兴' : '有点不高兴'})`
        pushSystemMessage(`情感: ${result.sentiment.eventType} ${hint}`)
      }
      if (result.impression && result.impression.latestSnippet) {
        ghost.value.impression.overallAffinity = result.impression.overallAffinity
      }
    }
    onNewMessage()
    screenshotBounce.value = true
    setTimeout(() => { screenshotBounce.value = false }, 500)
  } catch (e: any) {
    pushSystemMessage(`发送失败: ${e}`)
    onNewMessage()
  } finally {
    chatLoading.value = false
    resetHideTimer()
  }
}

async function triggerScreenshot() {
  if (screenshotAnalysisLoading.value) return
  screenshotAnalysisLoading.value = true
  pushSystemMessage('正在截取屏幕...')
  onNewMessage()

  try {
    const base64 = await invoke<string>('capture_screenshot')
    await invoke('store_screenshot_data', { data: base64 })
    await invoke('close_screenshot_window')

    const overlay = new WebviewWindow('screenshot-overlay', {
      url: 'screenshot.html',
      title: 'DeskPet - 截图',
      decorations: false,
      transparent: true,
      alwaysOnTop: true,
      skipTaskbar: true,
      resizable: false,
      focus: true,
      fullscreen: true,
    })

    overlay.once('tauri://error', (e: any) => {
      console.error('Screenshot overlay creation error:', e)
      pushSystemMessage(`截图窗口创建失败: ${e?.payload || e}`)
      screenshotAnalysisLoading.value = false
      onNewMessage()
    })
  } catch (e: any) {
    screenshotAnalysisLoading.value = false
    pushSystemMessage(`截图失败: ${e}`)
    onNewMessage()
  }
}

async function analyzeScreenshot(base64: string) {
  if (!ghost.value) {
    pushSystemMessage('请先生成桌宠灵魂，再进行截图识别。')
    return
  }

  movePetToScreenshotRegion()

  playOneShot('surprise', 400)

  screenshotAnalysisLoading.value = true
  try {
    const result = await invoke<{ description: string; petName: string }>('analyze_screenshot', {
      imageBase64: base64,
    })
    visionResult.value = result.description
    pushPetMessage(result.description)
    showInput()
    onNewMessage()
    screenshotBounce.value = true
    setTimeout(() => { screenshotBounce.value = false }, 600)
  } catch (e: any) {
    pushSystemMessage(`截图识别失败: ${e}`)
    onNewMessage()
  } finally {
    screenshotAnalysisLoading.value = false
  }
}

async function movePetToScreenshotRegion() {
  try {
    const win = getCurrentWindow()
    const pos = await win.outerPosition()
    const winX = pos.x
    const winY = pos.y

    const region = screenshotScreenRegion.value
    const regionCenterX = region.x + region.width / 2
    const regionCenterY = region.y + region.height / 2

    const targetScreenX = regionCenterX + 40
    const targetScreenY = regionCenterY - 80

    const dx = targetScreenX - winX
    const dy = targetScreenY - winY

    const maxDx = window.innerWidth * 0.3
    const maxDy = window.innerHeight * 0.3
    const clampedDx = Math.max(-maxDx, Math.min(maxDx, dx))
    const clampedDy = Math.max(-maxDy, Math.min(maxDy, dy))

    await movePetTo(clampedDx, clampedDy)
  } catch (e) {
    console.warn('Failed to move pet to screenshot region:', e)
  }
}

async function saveAIConfig() {
  if (!aiEndpoint.value.trim()) {
    error.value = '请填写 API Endpoint'
    return
  }
  if (!aiApiKey.value.trim()) {
    error.value = '请填写 API Key'
    return
  }
  if (!aiModel.value.trim()) {
    error.value = '请填写 Model 名称'
    return
  }

  try {
    await invoke('configure_ai', {
      endpoint: aiEndpoint.value.trim(),
      apiKey: aiApiKey.value.trim(),
      model: aiModel.value.trim(),
    })
    error.value = ''
    pushSystemMessage('AI 配置已保存，可以开始对话了')
    onNewMessage()
  } catch (e: any) {
    error.value = '配置失败: ' + (e as string)
  }
}

async function setCuriosityLevel(level: string) {
  if (!ghost.value) return
  try {
    const result = await invoke<{
      level: string
      intensity: number
      focusWeights: Record<string, number>
      ownerCuriosity: number
    }>('set_curiosity_level', { level })
    if (ghost.value) {
      ghost.value.curiosityLevel = result.level
    }
  } catch (e: any) {
    error.value = `好奇心设置失败: ${e}`
  }
}

async function triggerCuriosity() {
  if (!ghost.value) return
  try {
    const result = await invoke<{
      triggered: boolean
      reason?: string
      intensity?: number
      interests?: Array<{ topic: string; weight: number }>
      loveHate?: number
    }>('trigger_curiosity')
    if (!result.triggered) {
      pushSystemMessage(result.reason || '好奇心不足，无法触发')
      onNewMessage()
      return
    }
    const interests = result.interests || []
    const topicLabels: Record<string, string> = {
      art_culture: '艺术与文化',
      science_tech: '科学与技术',
      social_people: '社交与人物',
      emotion_inner: '情感与内心',
      creative_imagination: '创意与想象',
    }
    const topTopics = interests.slice(0, 2).map((i: any) => topicLabels[i.topic] || i.topic).join('、')
    pushPetMessage(`我有点好奇${topTopics}方面的东西呢...想跟我聊聊吗？`)
    onNewMessage()
    if (result.loveHate !== undefined && ghost.value) {
      ghost.value.loveHate = result.loveHate
    }
  } catch (e: any) {
    error.value = `好奇心触发失败: ${e}`
  }
}

function togglePanel() {
  showPanel.value = !showPanel.value
}

async function saveGhost() {
  if (!ghost.value) return
  try {
    const path = saveLoadPath.value.trim() || 'deskpet.ghost'
    await invoke('save_ghost', { path })
    pushSystemMessage(`灵魂已保存到 ${path}`)
    onNewMessage()
    error.value = ''
  } catch (e: any) {
    error.value = `保存失败: ${e}`
  }
}

async function loadGhost() {
  const path = saveLoadPath.value.trim() || 'deskpet.ghost'
  try {
    await invoke('load_ghost', { path })
    ghost.value = JSON.parse(await invoke<string>('get_ghost_status'))
    updateMood(ghost.value.loveHate, ghost.value.curiosityLevel)
    if (ghost.value.personality) {
      setPersonality(ghost.value.personality)
    }
    startIdleLoop()
    pushSystemMessage(`灵魂已从 ${path} 加载`)
    onNewMessage()
    error.value = ''
  } catch (e: any) {
    error.value = `加载失败: ${e}`
  }
}

const personalityLabels: Record<string, string> = {
  openness: '开放',
  conscientiousness: '尽责',
  extraversion: '外向',
  agreeableness: '宜人',
  neuroticism: '情绪',
  creativity: '创造',
}

const eventGroups = [
  { label: '正向', events: [
    { type: 'UserCaredAboutPet', intensity: 0.8, name: '关心', cls: 'btn-positive' },
    { type: 'UserPraisedPet', intensity: 0.6, name: '夸奖', cls: 'btn-positive' },
    { type: 'UserSharedPersonalStory', intensity: 0.7, name: '分享', cls: 'btn-positive' },
    { type: 'FirstConversation', intensity: 1.0, name: '初次', cls: 'btn-positive' },
    { type: 'BirthdayCelebrated', intensity: 1.0, name: '生日', cls: 'btn-positive' },
  ]},
  { label: '中性', events: [
    { type: 'NormalChat', intensity: 0.3, name: '聊天', cls: 'btn-neutral' },
  ]},
  { label: '负向', events: [
    { type: 'UserGotAngry', intensity: 0.5, name: '生气', cls: 'btn-negative' },
    { type: 'UserIgnoredPet', intensity: 0.8, name: '忽略', cls: 'btn-negative' },
    { type: 'UserDismissedPet', intensity: 0.5, name: '敷衍', cls: 'btn-negative' },
  ]},
]

interface TransferResult {
  oldSignature: string
  newSignature: string
  generation: number
  oldGeneration: number
  perturbation: Record<string, number>
  transferredName: string
}

const showTransfer = ref(false)
const transferPhase = ref<'idle' | 'animating' | 'revealing' | 'done'>('idle')
const transferResult = ref<TransferResult | null>(null)
const transferAnimProgress = ref(0)

const personalityKeys = ['openness', 'conscientiousness', 'extraversion', 'agreeableness', 'neuroticism', 'creativity']

function startTransfer() {
  showTransfer.value = true
  transferPhase.value = 'animating'
  transferAnimProgress.value = 0
  transferResult.value = null

  let frame = 0
  const totalFrames = 120
  const animId = setInterval(() => {
    frame++
    transferAnimProgress.value = frame / totalFrames
    if (frame >= totalFrames) {
      clearInterval(animId)
      executeTransfer()
    }
  }, 25)
}

async function executeTransfer() {
  try {
    const result = await invoke<TransferResult>('transfer_ghost', {
      savePath: getGhostSavePath(),
    })
    transferResult.value = result
    transferPhase.value = 'revealing'

    if (ghost.value) {
      ghost.value = JSON.parse(await invoke<string>('get_ghost_status'))
    }
  } catch (e: any) {
    error.value = `灵魂传送失败: ${e}`
    transferPhase.value = 'idle'
    showTransfer.value = false
  }
}

function getGhostSavePath(): string {
  return 'ghost_transferred.ghost'
}

function finishTransfer() {
  transferPhase.value = 'done'
  showTransfer.value = false
}

const transferParticles = computed(() => {
  const particles = []
  for (let i = 0; i < 24; i++) {
    const angle = (i / 24) * Math.PI * 2
    const dist = 40 + Math.sin(angle * 3 + transferAnimProgress.value * 6) * 20
    particles.push({
      x: 50 + Math.cos(angle) * dist * transferAnimProgress.value,
      y: 50 + Math.sin(angle) * dist * transferAnimProgress.value,
      size: 2 + Math.random() * 3,
      opacity: 0.3 + Math.random() * 0.7,
      hue: 280 + Math.random() * 80,
    })
  }
  return particles
})
</script>

<template>
  <div class="pet-app">
    <!-- Pet area - click to open chat -->
    <div class="pet-area" @click="handlePetClick">
      <div class="pet-status-bar">
        <div class="pet-affinity-fill" :style="{ width: loveHatePercent + '%', background: affinityColor }" />
      </div>
      <PetRenderer
        :animation-state="currentAnimationState"
        :mood-config="moodConfig"
        :emoji="petEmoji"
        :css-class="petCssClass"
        :style-override="{ transform: `translate(${petX}px, ${petY}px)` }"
        :renderer-type="rendererType"
        :sprite-config="spriteConfig"
        :tag-ranges="tagRanges"
        :frame-durations="frameDurations"
        :is-flipped="isFlipped"
      />
      <div class="pet-info">
        <span class="pet-name">{{ petName }}</span>
        <span v-if="ghost" class="pet-mood">{{ moodText }} · {{ movementStyle }}</span>
      </div>
      <div class="pet-drag-hint">
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
          <circle cx="3" cy="3" r="1" fill="#ccc"/>
          <circle cx="9" cy="3" r="1" fill="#ccc"/>
          <circle cx="3" cy="9" r="1" fill="#ccc"/>
          <circle cx="9" cy="9" r="1" fill="#ccc"/>
          <circle cx="3" cy="6" r="1" fill="#ccc"/>
          <circle cx="9" cy="6" r="1" fill="#ccc"/>
        </svg>
      </div>
      <button class="screenshot-btn" @click.stop="triggerScreenshot" title="截图 (Ctrl+Alt+X)">
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
          <path d="M5 2H6L7 3H10L11 2H12V3H14V13H2V3H4V2H5Z" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round"/>
          <circle cx="8" cy="8" r="2.5" stroke="currentColor" stroke-width="1.2"/>
        </svg>
      </button>
      <button class="chat-bubble-btn" @click.stop="toggleInput" title="对话 (Ctrl+Alt+C)">
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
          <path d="M2 3H14V10H8L5 13V10H2V3Z" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round"/>
        </svg>
      </button>
      <button class="settings-btn" @click.stop="openSettingsWindow" title="设置">
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
          <circle cx="8" cy="8" r="2" stroke="currentColor" stroke-width="1.2"/>
          <path d="M8 1V3M8 13V15M1 8H3M13 8H15M3.05 3.05L4.46 4.46M11.54 11.54L12.95 12.95M3.05 12.95L4.46 11.54M11.54 4.46L12.95 3.05" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
        </svg>
      </button>
      <div class="pet-chevron" :class="{ 'chevron-up': showPanel }">
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
          <path d="M3 5L7 9L11 5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
        </svg>
      </div>
    </div>

    <!-- Bubble dialogue -->
    <transition name="bubble-slide">
      <div v-if="bubblesVisible && (chatMessages.length > 0 || inputVisible)" class="bubble-dialogue-wrapper" @click.stop @mousedown="onUserActivity">
        <BubbleDialogue
          ref="bubbleDialogueRef"
          :messages="chatMessages"
          :pet-name="petName"
          :chat-loading="chatLoading"
          :love-hate="ghost?.loveHate ?? 0"
          @send="handleSendMessage"
          @input-focus="onInputFocus"
          @input-blur="onInputBlur"
        />
      </div>
    </transition>

    <!-- Collapsible panel -->
    <transition name="panel-slide">
      <div v-if="showPanel" class="panel">
        <template v-if="!ghost">
          <div class="panel-section">
            <button @click.stop="generateGhost" class="btn btn-generate" :disabled="loading">
              {{ loading ? '生成中...' : '✨ 生成桌宠灵魂' }}
            </button>
          </div>
        </template>

        <template v-if="ghost">
          <!-- Stats -->
          <div class="panel-section">
            <div class="stat-row">
              <span>好感度</span>
              <span class="stat-value" :style="{ color: affinityColor }">{{ ghost.loveHate.toFixed(1) }}</span>
              <span class="stat-dim">基线 {{ ghost.baseline.toFixed(1) }}</span>
            </div>
            <div class="stat-row">
              <span>印象</span>
              <span class="stat-value">{{ ghost.impression.overallAffinity.toFixed(1) }}</span>
            </div>
          </div>

          <!-- Personality (collapsible) -->
          <div class="panel-section">
            <div class="section-toggle" @click.stop="showPersonality = !showPersonality">
              <span>性格维度</span>
              <span class="toggle-icon">{{ showPersonality ? '▲' : '▼' }}</span>
            </div>
            <div v-if="showPersonality" class="personality-grid">
              <div class="personality-item" v-for="(val, key) in ghost.personality" :key="key">
                <span class="p-label">{{ personalityLabels[key as string] || key }}</span>
                <div class="p-bar"><div class="p-fill" :style="{width: (val*100)+'%'}"></div></div>
                <span class="p-val">{{ (val*100).toFixed(0) }}</span>
              </div>
            </div>
          </div>

          <!-- Events -->
          <div class="panel-section">
            <div class="section-label">情感事件</div>
            <div v-for="group in eventGroups" :key="group.label" class="event-group">
              <div class="event-group-label">{{ group.label }}</div>
              <div class="event-buttons">
                <button
                  v-for="ev in group.events"
                  :key="ev.type"
                  @click.stop="applyEvent(ev.type, ev.intensity)"
                  :class="['btn', ev.cls]"
                >{{ ev.name }}</button>
              </div>
            </div>
          </div>

          <!-- Emotion Timeline -->
          <div class="panel-section">
            <EmotionTimeline />
          </div>

        </template>

        <div v-if="error" class="error-msg">{{ error }}</div>
      </div>
    </transition>

    <div v-if="screenshotAnalysisLoading" class="screenshot-analysis-loading">
      <div class="loading-spinner"></div>
      <span>桌宠正在看截图...</span>
    </div>

    <!-- Soul Transfer Overlay -->
    <transition name="overlay-fade">
      <div v-if="showTransfer && (transferPhase === 'animating' || transferPhase === 'revealing')" class="transfer-overlay" @click.self>
        <!-- Animation Phase -->
        <div v-if="transferPhase === 'animating'" class="transfer-anim-container">
          <div class="transfer-core">
            <div class="transfer-ring" :style="{ transform: `scale(${0.5 + transferAnimProgress * 0.5})`, opacity: 1 - transferAnimProgress * 0.3 }">
              <div class="transfer-inner-ring"></div>
            </div>
            <div class="transfer-pet-icon">{{ petEmoji }}</div>
            <svg v-for="(p, i) in transferParticles" :key="i"
              class="transfer-particle"
              :style="{ left: p.x + '%', top: p.y + '%', opacity: p.opacity * transferAnimProgress }"
              width="6" height="6" viewBox="0 0 6 6">
              <circle cx="3" cy="3" :r="p.size" :fill="`hsl(${p.hue}, 70%, 65%)`" />
            </svg>
          </div>
          <div class="transfer-text">
            <span class="transfer-label">灵魂正在传送</span>
            <div class="transfer-progress-bar">
              <div class="transfer-progress-fill" :style="{ width: (transferAnimProgress * 100) + '%' }"></div>
            </div>
            <span class="transfer-percent">{{ Math.round(transferAnimProgress * 100) }}%</span>
          </div>
        </div>

        <!-- Reveal Phase -->
        <div v-if="transferPhase === 'revealing' && transferResult" class="transfer-reveal">
          <div class="reveal-title">传送完成</div>
          <div class="reveal-generation">第 {{ transferResult.generation }} 代灵魂</div>

          <div class="reveal-signatures">
            <div class="sig-item sig-old">
              <div class="sig-label">原体签名</div>
              <div class="sig-value sig-faded">{{ transferResult.oldSignature.slice(0, 8) }}...</div>
              <div class="sig-status">已消逝</div>
            </div>
            <div class="sig-arrow">→</div>
            <div class="sig-item sig-new">
              <div class="sig-label">新体签名</div>
              <div class="sig-value">{{ transferResult.newSignature.slice(0, 8) }}...</div>
              <div class="sig-status sig-active">已激活</div>
            </div>
          </div>

          <div class="reveal-perturbation">
            <div class="perturb-title">人格微偏</div>
            <div class="perturb-grid">
              <div v-for="key in personalityKeys" :key="key" class="perturb-item">
                <span class="perturb-label">{{ personalityLabels[key] || key }}</span>
                <div class="perturb-bar-container">
                  <div class="perturb-bar-bg">
                    <div class="perturb-bar-fill"
                      :class="{ 'perturb-positive': (transferResult.perturbation[key] || 0) > 0, 'perturb-negative': (transferResult.perturbation[key] || 0) < 0 }"
                      :style="{ width: Math.min(Math.abs(transferResult.perturbation[key] || 0) * 1000, 100) + '%' }">
                    </div>
                  </div>
                </div>
                <span class="perturb-value" :class="{ 'pv-pos': (transferResult.perturbation[key] || 0) > 0, 'pv-neg': (transferResult.perturbation[key] || 0) < 0 }">
                  {{ (transferResult.perturbation[key] || 0) > 0 ? '+' : '' }}{{ ((transferResult.perturbation[key] || 0) * 100).toFixed(1) }}%
                </span>
              </div>
            </div>
          </div>

          <button @click.stop="finishTransfer" class="btn btn-transfer-complete">确认 · 继续陪伴</button>
        </div>
      </div>
    </transition>
  </div>
</template>

<style scoped>
.pet-app {
  width: 100vw;
  height: 100vh;
  position: fixed;
  top: 0;
  left: 0;
  overflow: hidden;
  background: transparent;
  font-family: 'Segoe UI', system-ui, -apple-system, sans-serif;
  font-size: 13px;
  color: #333;
  user-select: none;
}

/* Pet Area */
.pet-area {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px 10px 10px;
  background: rgba(255, 255, 255, 0.92);
  backdrop-filter: blur(12px);
  border-radius: 16px;
  box-shadow: 0 2px 16px rgba(0, 0, 0, 0.1), 0 0 0 1px rgba(0, 0, 0, 0.04);
  transition: background 0.2s, box-shadow 0.2s;
  position: relative;
  z-index: 10;
  margin: 8px;
  -webkit-app-region: drag;
}

.pet-area button,
.pet-area input,
.pet-area a {
  -webkit-app-region: no-drag;
}

.pet-area:hover {
  background: rgba(255, 255, 255, 0.97);
  box-shadow: 0 4px 20px rgba(255, 107, 157, 0.15), 0 0 0 1px rgba(0, 0, 0, 0.06);
}

.pet-status-bar {
  position: absolute;
  top: -4px;
  left: 50%;
  width: 80px;
  height: 3px;
  background: rgba(0, 0, 0, 0.1);
  border-radius: 2px;
  overflow: hidden;
  transform: translateX(-50%);
}

.pet-affinity-fill {
  height: 100%;
  border-radius: 2px;
  transition: width 0.5s ease, background 0.5s ease;
}

.screenshot-bounce {
  animation: screenshotBounce 0.5s cubic-bezier(0.34, 1.56, 0.64, 1) 1 !important;
}

@keyframes screenshotBounce {
  0% { transform: translateY(0) scale(1); }
  30% { transform: translateY(-12px) scale(1.15); }
  50% { transform: translateY(-8px) scale(1.05); }
  70% { transform: translateY(4px) scale(0.95); }
  100% { transform: translateY(0) scale(1); }
}

@keyframes idleBounce {
  0%, 100% { transform: translateY(0); }
  50% { transform: translateY(-4px); }
}

@keyframes speakingBounce {
  0%, 100% { transform: translateY(0) scale(1); }
  50% { transform: translateY(-2px) scale(1.05); }
}

.pet-info {
  display: flex;
  flex-direction: column;
  gap: 1px;
  flex: 1;
  min-width: 0;
}

.pet-name {
  font-size: 14px;
  font-weight: 700;
  color: #333;
  line-height: 1.3;
}

.pet-mood {
  font-size: 11px;
  color: #888;
  line-height: 1.3;
}

.screenshot-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: none;
  background: transparent;
  border-radius: 8px;
  color: #999;
  cursor: pointer;
  transition: all 0.15s;
  flex-shrink: 0;
}

.screenshot-btn:hover {
  background: rgba(255, 107, 157, 0.1);
  color: #ff6b9d;
}

.chat-bubble-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: none;
  background: transparent;
  border-radius: 8px;
  color: #999;
  cursor: pointer;
  transition: all 0.15s;
  flex-shrink: 0;
}

.chat-bubble-btn:hover {
  background: rgba(192, 132, 252, 0.12);
  color: #c084fc;
}

.settings-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: none;
  background: transparent;
  border-radius: 8px;
  color: #999;
  cursor: pointer;
  transition: all 0.15s;
  flex-shrink: 0;
}

.settings-btn:hover {
  background: rgba(150, 150, 150, 0.12);
  color: #666;
}

.pet-chevron {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  color: #999;
  transition: transform 0.3s ease;
  flex-shrink: 0;
}

.chevron-up {
  transform: rotate(180deg);
}

.pet-drag-hint {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 20px;
  flex-shrink: 0;
  opacity: 0.4;
  transition: opacity 0.2s;
  cursor: grab;
}

.pet-drag-hint:hover {
  opacity: 0.8;
}

.pet-drag-hint:active {
  cursor: grabbing;
}

/* Bubble dialogue transition */
.bubble-slide-enter-active {
  transition: all 0.3s ease;
}

.bubble-slide-leave-active {
  transition: all 0.25s ease;
}

.bubble-slide-enter-from {
  opacity: 0;
  transform: translateY(-10px);
}

.bubble-slide-leave-to {
  opacity: 0;
  transform: translateY(-8px);
}

.bubble-dialogue-wrapper {
  position: relative;
  z-index: 5;
}

/* Panel - collapsible */
.panel {
  margin: 0 8px 8px;
  background: rgba(255, 255, 255, 0.93);
  backdrop-filter: blur(12px);
  border-radius: 16px;
  box-shadow: 0 2px 16px rgba(0, 0, 0, 0.08), 0 0 0 1px rgba(0, 0, 0, 0.04);
  padding: 10px 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: calc(100vh - 100px);
  overflow-y: auto;
  overflow-x: hidden;
}

.panel-slide-enter-active,
.panel-slide-leave-active {
  transition: all 0.3s ease;
  max-height: 600px;
  opacity: 1;
}

.panel-slide-enter-from,
.panel-slide-leave-to {
  max-height: 0;
  opacity: 0;
  padding-top: 0;
  padding-bottom: 0;
  margin-top: 0;
  margin-bottom: 0;
}

.panel-section {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.stat-row {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: #666;
}

.stat-value {
  font-weight: 700;
  font-size: 13px;
}

.stat-dim {
  color: #aaa;
  font-size: 11px;
}

.section-toggle {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 12px;
  font-weight: 600;
  color: #555;
  cursor: pointer;
  padding: 2px 0;
}

.toggle-icon {
  font-size: 10px;
  color: #999;
}

.personality-grid {
  display: flex;
  flex-direction: column;
  gap: 3px;
  padding: 4px 0;
}

.personality-item {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
}

.p-label {
  width: 28px;
  text-align: right;
  color: #888;
  flex-shrink: 0;
}

.p-bar {
  flex: 1;
  height: 5px;
  background: #eee;
  border-radius: 3px;
  overflow: hidden;
}

.p-fill {
  height: 100%;
  background: linear-gradient(90deg, #ff6b9d, #c084fc);
  border-radius: 3px;
  transition: width 0.5s ease;
}

.p-val {
  width: 26px;
  text-align: right;
  color: #888;
  font-size: 10px;
}

.section-label {
  font-size: 11px;
  font-weight: 600;
  color: #888;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.event-group {
  margin-bottom: 2px;
}

.event-group-label {
  font-size: 10px;
  color: #aaa;
  margin-bottom: 2px;
}

.event-buttons {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.btn {
  padding: 4px 10px;
  border: none;
  border-radius: 8px;
  font-size: 11px;
  cursor: pointer;
  transition: all 0.15s;
  font-weight: 500;
  line-height: 1.4;
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-positive {
  background: #e8f5e9;
  color: #2e7d32;
  border: 1px solid #a5d6a7;
}

.btn-positive:hover {
  background: #c8e6c9;
}

.btn-neutral {
  background: #f5f5f5;
  color: #666;
  border: 1px solid #ddd;
}

.btn-neutral:hover {
  background: #eee;
}

.btn-negative {
  background: #ffebee;
  color: #c62828;
  border: 1px solid #ef9a9a;
}

.btn-negative:hover {
  background: #ffcdd2;
}

.btn-generate {
  background: linear-gradient(135deg, #ff6b9d, #c084fc);
  color: white;
  border: none;
  padding: 8px 16px;
  font-size: 13px;
  width: 100%;
}

.btn-generate:hover:not(:disabled) {
  opacity: 0.9;
}

.btn-config {
  background: #f0f0f0;
  color: #555;
  border: 1px solid #ddd;
  width: 100%;
  font-size: 11px;
}

.btn-config:hover {
  background: #e5e5e5;
}

.btn-full {
  width: 100%;
  margin-top: 4px;
}

/* AI Config */
.ai-config {
  background: rgba(192, 132, 252, 0.06);
  border: 1px solid rgba(192, 132, 252, 0.15);
  border-radius: 10px;
  padding: 10px;
  margin-top: 4px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.config-label {
  font-size: 10px;
  font-weight: 600;
  color: #888;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.config-input {
  padding: 5px 10px;
  border: 1.5px solid rgba(192, 132, 252, 0.2);
  border-radius: 6px;
  font-size: 12px;
  outline: none;
  transition: border-color 0.2s;
  background: #fff;
}

.config-input:focus {
  border-color: #c084fc;
}

.config-slide-enter-active,
.config-slide-leave-active {
  transition: all 0.2s ease;
  max-height: 300px;
  opacity: 1;
}

.config-slide-enter-from,
.config-slide-leave-to {
  max-height: 0;
  opacity: 0;
  padding-top: 0;
  padding-bottom: 0;
}

.error-msg {
  color: #f44336;
  font-size: 11px;
  padding: 6px 8px;
  background: #ffebee;
  border-radius: 8px;
}

/* Scrollbar */
.panel::-webkit-scrollbar {
  width: 4px;
}

.panel::-webkit-scrollbar-track {
  background: transparent;
}

.panel::-webkit-scrollbar-thumb {
  background: rgba(0, 0, 0, 0.15);
  border-radius: 2px;
}

/* Curiosity */
.curiosity-panel {
  padding: 6px 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

/* Save/Load */
.save-load-panel {
  padding: 6px 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.save-load-buttons {
  display: flex;
  gap: 6px;
}

.btn-save {
  flex: 1;
  background: #e3f2fd;
  color: #1565c0;
  border: 1px solid #90caf9;
  font-size: 11px;
  padding: 6px 12px;
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.15s;
}

.btn-save:hover {
  background: #bbdefb;
}

.btn-load {
  flex: 1;
  background: #f3e5f5;
  color: #7b1fa2;
  border: 1px solid #ce93d8;
  font-size: 11px;
  padding: 6px 12px;
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.15s;
}

.btn-load:hover {
  background: #e1bee7;
}

/* Renderer */
.renderer-buttons {
  display: flex;
  gap: 4px;
  margin-top: 4px;
}

.btn-ren-active {
  font-size: 11px;
  padding: 4px 10px;
  border-radius: 8px;
  cursor: pointer;
  border: none;
  color: white;
  background: linear-gradient(135deg, #ff6b9d, #c084fc);
}

.btn-ren-off {
  background: #f0f0f0;
  color: #666;
  border: 1px solid #ddd;
  font-size: 11px;
  padding: 4px 10px;
  border-radius: 8px;
  cursor: pointer;
  flex: 1;
}

.btn-ren-off:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.sprite-config {
  margin-top: 6px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.sprite-info {
  font-size: 10px;
  color: #999;
  padding: 2px 0;
}

.curiosity-status {
  font-size: 12px;
  color: #555;
}

.curiosity-off { color: #aaa; }
.curiosity-normal { color: #4caf50; }
.curiosity-enhanced { color: #7c4dff; font-weight: 700; }

.curiosity-levels {
  display: flex;
  gap: 4px;
}

.btn-cur-off {
  background: #f0f0f0;
  color: #666;
  border: 1px solid #ddd;
  font-size: 11px;
  padding: 4px 10px;
  border-radius: 8px;
  cursor: pointer;
  flex: 1;
}

.btn-cur-active {
  font-size: 11px;
  padding: 4px 10px;
  border-radius: 8px;
  cursor: pointer;
  border: none;
  flex: 1;
  color: white;
}

.btn-cur-active.btn-cur-off { background: #4caf50; }
.btn-cur-off:hover { background: #e5e5e5; }

.btn-curiosity-trigger {
  background: linear-gradient(135deg, #4caf50, #8bc34a);
  color: white;
  border: none;
  padding: 6px 14px;
  border-radius: 8px;
  font-size: 12px;
  cursor: pointer;
  width: 100%;
}

.btn-curiosity-trigger:hover {
  opacity: 0.9;
}

.curiosity-hint {
  font-size: 10px;
  color: #ff9800;
  margin: 0;
}

/* Transfer */
.transfer-intro {
  padding: 6px 0;
}

.transfer-desc {
  font-size: 11px;
  color: #777;
  line-height: 1.6;
  margin: 4px 0;
}

.transfer-warning {
  font-size: 10px;
  color: #f44336;
  margin: 2px 0 8px;
}

.btn-transfer {
  background: linear-gradient(135deg, #7c4dff, #448aff);
  color: white;
  border: none;
  padding: 8px 16px;
  border-radius: 10px;
  font-size: 13px;
  width: 100%;
  cursor: pointer;
  font-weight: 600;
  transition: opacity 0.2s;
}

.btn-transfer:hover {
  opacity: 0.9;
}

.btn-transfer-complete {
  background: linear-gradient(135deg, #ff6b9d, #c084fc);
  color: white;
  border: none;
  padding: 12px 24px;
  border-radius: 12px;
  font-size: 14px;
  cursor: pointer;
  font-weight: 700;
  margin-top: 16px;
  transition: opacity 0.2s;
}

.btn-transfer-complete:hover {
  opacity: 0.9;
}

.transfer-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  background: rgba(10, 5, 20, 0.95);
  z-index: 2000;
  display: flex;
  align-items: center;
  justify-content: center;
  color: white;
  font-family: 'Segoe UI', system-ui, sans-serif;
}

.transfer-anim-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 32px;
}

.transfer-core {
  position: relative;
  width: 200px;
  height: 200px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.transfer-ring {
  position: absolute;
  width: 160px;
  height: 160px;
  border-radius: 50%;
  border: 2px solid rgba(124, 77, 255, 0.4);
  animation: transferPulse 1.5s ease-in-out infinite;
}

.transfer-inner-ring {
  width: 100%;
  height: 100%;
  border-radius: 50%;
  border: 1px solid rgba(68, 138, 255, 0.3);
  animation: transferSpin 3s linear infinite;
}

@keyframes transferPulse {
  0%, 100% { transform: scale(1); opacity: 0.6; }
  50% { transform: scale(1.1); opacity: 1; }
}

@keyframes transferSpin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.transfer-pet-icon {
  font-size: 48px;
  z-index: 10;
  animation: transferFloat 2s ease-in-out infinite;
}

@keyframes transferFloat {
  0%, 100% { transform: translateY(0); }
  50% { transform: translateY(-8px); }
}

.transfer-particle {
  position: absolute;
  pointer-events: none;
  animation: transferParticleFade 2s ease-out infinite;
}

@keyframes transferParticleFade {
  0% { opacity: 0.8; transform: scale(1); }
  100% { opacity: 0.2; transform: scale(0.5); }
}

.transfer-text {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
}

.transfer-label {
  font-size: 18px;
  font-weight: 700;
  background: linear-gradient(135deg, #7c4dff, #448aff, #ff6b9d);
  background-size: 200% 200%;
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
  animation: transferGradient 3s ease infinite;
}

@keyframes transferGradient {
  0% { background-position: 0% 50%; }
  50% { background-position: 100% 50%; }
  100% { background-position: 0% 50%; }
}

.transfer-progress-bar {
  width: 200px;
  height: 4px;
  background: rgba(255, 255, 255, 0.1);
  border-radius: 2px;
  overflow: hidden;
}

.transfer-progress-fill {
  height: 100%;
  background: linear-gradient(90deg, #7c4dff, #448aff, #ff6b9d);
  border-radius: 2px;
  transition: width 0.1s linear;
}

.transfer-percent {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.5);
}

/* Reveal Phase */
.transfer-reveal {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
  padding: 24px;
  max-width: 320px;
  animation: revealFadeIn 0.6s ease;
}

@keyframes revealFadeIn {
  from { opacity: 0; transform: translateY(10px); }
  to { opacity: 1; transform: translateY(0); }
}

.reveal-title {
  font-size: 22px;
  font-weight: 800;
  background: linear-gradient(135deg, #7c4dff, #ff6b9d);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
}

.reveal-generation {
  font-size: 14px;
  color: rgba(255, 255, 255, 0.6);
}

.reveal-signatures {
  display: flex;
  align-items: center;
  gap: 12px;
  margin: 8px 0;
}

.sig-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  padding: 10px 14px;
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
}

.sig-old {
  opacity: 0.4;
}

.sig-new {
  border-color: rgba(124, 77, 255, 0.3);
  background: rgba(124, 77, 255, 0.08);
}

.sig-label {
  font-size: 10px;
  color: rgba(255, 255, 255, 0.5);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.sig-value {
  font-size: 12px;
  font-weight: 600;
  font-family: 'Courier New', monospace;
}

.sig-faded {
  text-decoration: line-through;
  color: rgba(255, 255, 255, 0.3);
}

.sig-status {
  font-size: 9px;
  padding: 2px 6px;
  border-radius: 4px;
  background: rgba(255, 255, 255, 0.08);
}

.sig-active {
  color: #7c4dff;
  background: rgba(124, 77, 255, 0.15);
}

.sig-arrow {
  font-size: 18px;
  color: rgba(255, 255, 255, 0.3);
}

.reveal-perturbation {
  width: 100%;
  margin-top: 4px;
}

.perturb-title {
  font-size: 12px;
  color: rgba(255, 255, 255, 0.5);
  text-transform: uppercase;
  letter-spacing: 0.5px;
  margin-bottom: 8px;
  text-align: center;
}

.perturb-grid {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.perturb-item {
  display: flex;
  align-items: center;
  gap: 8px;
}

.perturb-label {
  width: 28px;
  text-align: right;
  font-size: 11px;
  color: rgba(255, 255, 255, 0.6);
  flex-shrink: 0;
}

.perturb-bar-container {
  flex: 1;
}

.perturb-bar-bg {
  height: 6px;
  background: rgba(255, 255, 255, 0.08);
  border-radius: 3px;
  overflow: hidden;
}

.perturb-bar-fill {
  height: 100%;
  border-radius: 3px;
  transition: width 0.8s ease;
}

.perturb-positive {
  background: linear-gradient(90deg, rgba(124, 77, 255, 0.6), rgba(124, 77, 255, 1));
}

.perturb-negative {
  background: linear-gradient(90deg, rgba(255, 107, 157, 0.6), rgba(255, 107, 157, 1));
}

.perturb-value {
  width: 48px;
  font-size: 11px;
  font-weight: 600;
  font-family: 'Courier New', monospace;
  flex-shrink: 0;
}

.pv-pos {
  color: #b388ff;
}

.pv-neg {
  color: #ff6b9d;
}

.loading-spinner {
  width: 32px;
  height: 32px;
  border: 3px solid rgba(255, 255, 255, 0.3);
  border-top: 3px solid #ff6b9d;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.screenshot-analysis-loading {
  position: fixed;
  top: 16px;
  left: 50%;
  transform: translateX(-50%);
  background: rgba(0, 0, 0, 0.8);
  color: white;
  padding: 12px 20px;
  border-radius: 12px;
  font-size: 13px;
  z-index: 1004;
  display: flex;
  align-items: center;
  gap: 10px;
  animation: fadeIn 0.3s ease;
}

@keyframes fadeIn {
  from { opacity: 0; transform: translateX(-50%) translateY(-8px); }
  to { opacity: 1; transform: translateX(-50%) translateY(0); }
}

.overlay-fade-enter-active,
.overlay-fade-leave-active {
  transition: opacity 0.2s ease;
}

.overlay-fade-enter-from,
.overlay-fade-leave-to {
  opacity: 0;
}
</style>