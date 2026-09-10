<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow, cursorPosition } from '@tauri-apps/api/window'
import { LogicalPosition } from '@tauri-apps/api/dpi'
import { ref, onMounted, onUnmounted, computed } from 'vue'
import PetRenderer from './components/PetRenderer.vue'
import TransferOverlay from './components/TransferOverlay.vue'
import PrivacyDialog from './components/PrivacyDialog.vue'
import { useChat } from './composables/useChat'
import { useAnimation } from './composables/useAnimation'
import { usePetRenderer } from './composables/usePetRenderer'
import { useSpeechBubble } from './composables/useSpeechBubble'
import { useInitiative } from './composables/useInitiative'
import { useGhostLifecycle, type GhostStatus } from './composables/useGhost'
import { useScreenshot, type ScreenshotRegion } from './composables/useScreenshot'
import { useTransfer } from './composables/useTransfer'
import { useWindowManager } from './composables/useWindowManager'
import { applyTheme } from './composables/useTheme'
import { useMouseTracking, type HeadDirection } from './composables/useMouseTracking'
import type { HeadConfig } from './composables/usePetRenderer'

interface AppearanceConfig {
  hueRotate: number
  brightness: number
  saturate: number
  contrast: number
  opacity: number
  scale: number
}

const DEFAULT_APPEARANCE: AppearanceConfig = {
  hueRotate: 0,
  brightness: 1,
  saturate: 1,
  contrast: 1,
  opacity: 1,
  scale: 1,
}

function loadAppearance(): AppearanceConfig {
  try {
    const saved = localStorage.getItem('deskpet_appearance')
    return saved ? { ...DEFAULT_APPEARANCE, ...JSON.parse(saved) } : DEFAULT_APPEARANCE
  } catch { return DEFAULT_APPEARANCE }
}

const petAppearance = ref<AppearanceConfig>(loadAppearance())

const ghost = ref<GhostStatus | null>(null)
const loading = ref(false)
const error = ref('')

const { ghostId, pushSystemMessage, clearMessages, loadHistory, pushPetMessage, ttsEnabled, ttsRate, ttsPitch, ttsEngine, ttsVoice } = useChat()
const chatLoading = ref(false)
const { rendererType, spriteConfig, lottieConfig, tagRanges, frameDurations, framePositions, headConfig, headEnabled, setRenderer, setSpriteConfig, setHeadConfig, setHeadEnabled, parseAsepriteJson } = usePetRenderer()
// 单次动作时长按素材 tag 实时计算（Bounce 等改帧数后不必再改代码），故需注入精灵图时序
const { currentAnimationState, moodConfig, petX, petY, isFlipped, isPerformingBehavior, updateMood, setPersonality, startDailyRoutine, stopDailyRoutine, stopBlink, holdAnimation, playOneShot, playEmotionReaction, startSpeaking, stopSpeaking, onAnimationComplete } = useAnimation({ tagRanges, frameDurations })

// ===== 头部视觉追踪（仅 IDLE，素材存在则自动启用）=====
const headDirection = ref<HeadDirection>('center')
const headTracking = useMouseTracking({
  radius: 400,
  hysteresisDeg: 22.5,
  settleMs: 2000,
  moveThresholdPx: 5,
})
// 约定素材路径：public/pet/head_9dir.png（3×3 宫格：正/左上/上/右上/左/右/左下/下/右下）
const HEAD_SRC = '/pet/head_9dir.png'

function probeHeadAsset() {
  const img = new Image()
  img.onload = () => {
    const config: Partial<HeadConfig> = { src: HEAD_SRC }
    // 调参：localStorage.setItem('deskpet_head_slot', JSON.stringify({x:0,y:0,w:128,h:64}))
    try {
      const saved = localStorage.getItem('deskpet_head_slot')
      if (saved) config.headSlot = JSON.parse(saved)
    } catch { /* 忽略非法 JSON，走默认 */ }
    setHeadConfig(config)
    setHeadEnabled(true)
  }
  img.onerror = () => {
    setHeadEnabled(false)
  }
  img.src = HEAD_SRC
}

const showToolbar = ref(false)
const petColumnRef = ref<HTMLElement | null>(null)
const toolbarRef = ref<HTMLElement | null>(null)
let didDrag = false
let toolbarHideTimer: ReturnType<typeof setTimeout> | null = null
let cursorPollId: number | null = null
let cursorEventsIgnored = true

function onHover(visible: boolean) {
  if (visible) {
    if (toolbarHideTimer) { clearTimeout(toolbarHideTimer); toolbarHideTimer = null }
    showToolbar.value = true
  } else {
    toolbarHideTimer = setTimeout(() => {
      showToolbar.value = false
    }, 250)
  }
}

function onPetMouseDown(e: MouseEvent) {
  if (e.button !== 0) return
  currentAnimationState.value = 'dragged'
  isPerformingBehavior.value = true
  petY.value = -14
  didDrag = false
  stopDailyRoutine()

  const win = getCurrentWindow()
  const startMouseX = e.screenX
  const startMouseY = e.screenY

  win.outerPosition().then((pos) => {
    const dpiScale = window.devicePixelRatio
    const offsetX = pos.x / dpiScale - startMouseX
    const offsetY = pos.y / dpiScale - startMouseY
    let moved = false

    const onMove = (me: MouseEvent) => {
      moved = true
      win.setPosition(new LogicalPosition(me.screenX + offsetX, me.screenY + offsetY))
    }
    const onUp = () => {
      document.removeEventListener('mousemove', onMove)
      document.removeEventListener('mouseup', onUp)
      currentAnimationState.value = 'idle'
      isPerformingBehavior.value = false
      petY.value = 0
      startDailyRoutine()
      if (moved) didDrag = true
    }
    document.addEventListener('mousemove', onMove)
    document.addEventListener('mouseup', onUp)
  })
}

const screenshotScreenRegion = ref<ScreenshotRegion>({ x: 0, y: 0, width: 0, height: 0 })
const screenshotAnalysisLoading = ref(false)
const showEnhancedPrivacyDialog = ref(false)
const enhancedPrivacyAccepted = ref(localStorage.getItem('deskpet_enhanced_privacy') === 'true')
const answeringMode = ref<'Companion' | 'Assistant'>('Companion')
const previousAnsweringMode = ref<'Companion' | 'Assistant' | null>(null)

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

const moodText = computed(() => {
  if (!ghost.value) return ''
  const lh = ghost.value.loveHate
  if (lh > 50) return '很开心~'
  if (lh > 20) return '心情不错'
  if (lh > -20) return '还ok吧'
  if (lh > -50) return '有点烦'
  return '不太高兴...'
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

const petCssClass = computed(() => ['pet-sprite-wrapper'])

const petStyleOverride = computed(() => {
  const a = petAppearance.value
  return {
    transform: `translate(${petX.value}px, ${petY.value}px) scale(${a.scale})`,
    filter: `hue-rotate(${a.hueRotate}deg) brightness(${a.brightness}) saturate(${a.saturate}) contrast(${a.contrast})`,
    opacity: String(a.opacity),
  }
})

const petName = computed(() => ghost.value?.name || '桌宠')

// ===== 好奇心（增强模式后台分析 / 普通模式主动研究）=====
async function curiosityBackgroundAnalyze() {
  if (!ghost.value) return
  try {
    const result = await invoke<{ analyzed: boolean; activity?: string }>('curiosity_background_analyze')
    if (result.analyzed && result.activity) {
      pushSystemMessage(`(好奇心观察) ${result.activity}`)
      }
  } catch (e) {
    console.warn('好奇心后台分析失败:', e)
  }
}

async function curiosityResearch() {
  if (!ghost.value) return
  try {
    const result = await invoke<{ researched: boolean; interest?: string; finding?: string }>('curiosity_research')
    if (result.researched && result.finding) {
      pushSystemMessage(`(好奇心探索·${result.interest}) ${result.finding}`)
    }
  } catch (e) {
    console.warn('好奇心探索失败:', e)
  }
}

function acceptEnhancedPrivacy() {
  enhancedPrivacyAccepted.value = true
  localStorage.setItem('deskpet_enhanced_privacy', 'true')
  showEnhancedPrivacyDialog.value = false
  curiosityBackgroundAnalyze()
}

function declineEnhancedPrivacy() {
  showEnhancedPrivacyDialog.value = false
  if (ghost.value) {
    invoke('set_curiosity_level', { level: 'Normal' }).catch(() => {})
    ghost.value.curiosityLevel = 'Normal'
  }
}

// ===== 成就 =====
async function checkAndNotifyAchievements() {
  try {
    const result = await invoke<{ newAchievements: Array<{ key: string; name: string; description: string }> }>('check_achievements')
    if (result.newAchievements && result.newAchievements.length > 0) {
      for (const a of result.newAchievements) {
        // 主窗口宠物上方弹气泡（仪式感）+ happy 动画 + 记录进聊天历史
        showSpeechBubble(`🏆 ${a.name}！${a.description}`)
        playOneShot('happy', 1500)
        pushSystemMessage(`🏆 成就解锁: ${a.name} — ${a.description}`)
      }
    }
  } catch (e) {
    // 静默失败，不影响主流程
  }
}

// ===== 定时 tick + 睡眠 =====
let tickInterval: ReturnType<typeof setInterval> | null = null
let tickInProgress = false
let autoSaveInterval: ReturnType<typeof setInterval> | null = null
const asleep = ref(false)
let sleepRelease: (() => void) | null = null
let diaryInProgress = false

async function tickGhost() {
  if (!ghost.value) return
  if (tickInProgress) return
  tickInProgress = true
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
      sleepTriggered: boolean
      diaryTriggered: boolean
    }>('timeline_tick')
    if (ghost.value) {
      ghost.value.loveHate = result.loveHate
      ghost.value.baseline = result.baseline
      updateMood(ghost.value.loveHate, ghost.value.curiosityLevel)
    }
    if (result.inactivityEvent) {
      pushSystemMessage(`${result.inactivityEvent.hoursAway}小时没互动了...好感度变化 ${result.inactivityEvent.loveHateDelta > 0 ? '+' : ''}${result.inactivityEvent.loveHateDelta.toFixed(1)}`)
    }
    if (result.curiosityTriggered && ghost.value) {
      playOneShot('curious', 800)
      if (ghost.value.curiosityLevel === 'Enhanced') {
        if (!enhancedPrivacyAccepted.value) {
          showEnhancedPrivacyDialog.value = true
        } else {
          curiosityBackgroundAnalyze()
        }
      } else if (ghost.value.curiosityLevel === 'Normal') {
        curiosityResearch()
      }
    }
    if (result.sleepTriggered && !asleep.value) {
      asleep.value = true
      sleepRelease = holdAnimation('sleep')
    }
    if (result.diaryTriggered && !diaryInProgress) {
      diaryInProgress = true
      try {
        const diary = await invoke<{ diaryId: string; entryDate: string; diaryText: string }>('generate_diary')
        if (diary.diaryText) {
          pushSystemMessage(`📔 ${diary.entryDate} 的日记\n${diary.diaryText}`)
        }
      } catch (e) {
        console.warn('日记生成失败:', e)
      } finally {
        diaryInProgress = false
      }
    }
  } catch (e: any) {
    console.error('Tick error:', e)
  } finally {
    tickInProgress = false
  }
}

async function wakeUpPet() {
  if (!asleep.value || !ghost.value) return

  // 立即唤醒，不等 AI
  if (sleepRelease) {
    sleepRelease()
    sleepRelease = null
  }
  asleep.value = false

  invoke('record_interaction').catch(() => {})

  // 后台生成梦境
  try {
    const dream = await invoke<{ dreamId: string; dreamText: string }>('generate_dream')
    if (dream.dreamText) {
      pushSystemMessage(`💤 ${ghost.value.name}伸了个懒腰，迷迷糊糊地说... ${dream.dreamText}`)
    }
    checkAndNotifyAchievements()
  } catch (e) {
    console.warn('梦境生成失败:', e)
  }
}

// ===== 装配独立模块 =====
const { speechBubble, showSpeechBubble, clearSpeechBubble } = useSpeechBubble()

const initiative = useInitiative({
  ghost,
  pushPetMessage,
  showSpeechBubble,
  ttsEnabled,
  ttsRate,
  ttsPitch,
  ttsEngine,
  ttsVoice,
})

const ghostLifecycle = useGhostLifecycle({
  ghost,
  loading,
  error,
  chat: { ghostId, clearMessages, loadHistory, pushSystemMessage },
  anim: { updateMood, setPersonality, startDailyRoutine },
  petName,
  spriteConfig,
  parseAsepriteJson,
  setRenderer,
  setSpriteConfig,
})

const windows = useWindowManager({ asleep, wakeUpPet, pushSystemMessage })

const screenshot = useScreenshot({
  ghost,
  screenshotScreenRegion,
  screenshotAnalysisLoading,
  answeringMode,
  previousAnsweringMode,
  playOneShot,
  openChatWindow: windows.openChatWindow,
  pushSystemMessage,
  petX,
  petY,
})

const transfer = useTransfer({
  ghost,
  error,
  checkAchievements: checkAndNotifyAchievements,
})

const { generateGhost, loadAutosaveGhost, autoSaveGhost, loadSpriteJson, loadRendererFromStorage } = ghostLifecycle
const { triggerScreenshot, analyzeScreenshot, restoreAnsweringMode } = screenshot
const { transferPhase, transferResult, transferAnimProgress, transferParticles, startTransfer, finishTransfer, clearTransfer } = transfer
const { openChatWindow, openSettingsWindow, wakeUpAndOpenChat } = windows

// ===== 事件监听器 =====
let hotkeyUnlisten: (() => void) | null = null
let chatHotkeyUnlisten: (() => void) | null = null
let settingsHotkeyUnlisten: (() => void) | null = null
let quitHotkeyUnlisten: (() => void) | null = null
let initiativeMessageUnlisten: (() => void) | null = null
let ttsUpdatedUnlisten: (() => void) | null = null
let chatPostProcessedUnlisten: (() => void) | null = null
let chatTokenUnlisten: (() => void) | null = null
let chatCompleteUnlisten: (() => void) | null = null
let settingsUpdatedUnlisten: (() => void) | null = null
let requestTransferUnlisten: (() => void) | null = null
let screenshotCapturedUnlisten: (() => void) | null = null
let screenshotCancelledUnlisten: (() => void) | null = null

function handleChatHotkey() {
  wakeUpAndOpenChat()
}

function handlePetClick(e: MouseEvent) {
  e.stopPropagation()
  if (didDrag) return
  if (!ghost.value) return
  if (asleep.value) {
    wakeUpPet()
    return
  }
  invoke('record_interaction').catch(() => {})
  openChatWindow()
}

onMounted(async () => {
  try {
    await invoke('ensure_database')
  } catch (e) {
    console.warn('数据库初始化:', e)
  }

  answeringMode.value = (localStorage.getItem('deskpet_answering_mode') as 'Companion' | 'Assistant') || 'Companion'
  try {
    await invoke('set_answering_mode', { mode: answeringMode.value })
  } catch (e) {
    console.warn('同步回应模式失败:', e)
  }

  // 启动时恢复 AI 配置：
  // 1. 优先从后端加密文件解密加载（明文不进前端）
  // 2. 旧版本用户 localStorage 中仍有明文密钥——作为一次性迁移回传并加密落盘后清除
  try {
    const restored = await invoke<boolean>('restore_ai_config')
    if (!restored) {
      const savedEndpoint = localStorage.getItem('deskpet_ai_endpoint')
      const savedApiKey = localStorage.getItem('deskpet_ai_api_key')
      const savedModel = localStorage.getItem('deskpet_ai_model')
      if (savedEndpoint && savedApiKey && savedModel) {
        await invoke('configure_ai', {
          endpoint: savedEndpoint,
          apiKey: savedApiKey,
          model: savedModel,
          visionModel: localStorage.getItem('deskpet_ai_vision_model') || null,
          imageModel: localStorage.getItem('deskpet_ai_image_model') || null,
          imageGenEndpoint: localStorage.getItem('deskpet_ai_image_gen_endpoint') || null,
          imageGenApiKey: localStorage.getItem('deskpet_ai_image_gen_api_key') || null,
          sttEndpoint: localStorage.getItem('deskpet_ai_stt_endpoint') || null,
          sttApiKey: localStorage.getItem('deskpet_ai_stt_api_key') || null,
          sttModel: localStorage.getItem('deskpet_ai_stt_model') || null,
        })
        // 迁移成功：加密已落盘，清除明文
        localStorage.removeItem('deskpet_ai_api_key')
        localStorage.removeItem('deskpet_ai_image_gen_api_key')
        localStorage.removeItem('deskpet_ai_stt_api_key')
      } else {
        // 首次启动或未配置：引导用户去设置 AI 接口。
        // 没有可用 AI 配置时，桌宠能动能展示但无法对话/情感/记忆——这是最大的新手流失点。
        // 延迟一点再弹，避免与初始化的 ghost 生成/窗口定位抢焦点。
        setTimeout(() => { openSettingsWindow() }, 1200)
      }
    }
  } catch (e) {
    console.warn('还原 AI 配置失败:', e)
  }

  // 启动时恢复天气配置（加密文件 → 后端内存）
  try {
    const restoredWeather = await invoke<boolean>('restore_weather_config')
    if (!restoredWeather) {
      // 旧版用户 localStorage 明文迁移
      const savedWeatherKey = localStorage.getItem('deskpet_weather_api_key')
      const savedWeatherCity = localStorage.getItem('deskpet_weather_city')
      if (savedWeatherKey && savedWeatherCity) {
        await invoke('configure_weather', { apiKey: savedWeatherKey, city: savedWeatherCity })
        localStorage.removeItem('deskpet_weather_api_key')
      }
    }
  } catch (e) {
    console.warn('还原天气配置失败:', e)
  }

  tickInterval = setInterval(tickGhost, 5000)
  autoSaveInterval = setInterval(autoSaveGhost, 120000)
  initiative.startInitiativeLoop()
  initiative.syncInitiativeConfig()
  initiative.loadTTSFromStorage()

  try {
    const found = await invoke<{ found: boolean; path: string }>('find_last_ghost')
    if (found.found) {
      await loadAutosaveGhost(found.path)
    } else {
      generateGhost()
    }
  } catch {
    generateGhost()
  }

  loadSpriteJson()
  loadRendererFromStorage()
  probeHeadAsset()

  hotkeyUnlisten = await listen('screenshot-hotkey', () => {
    triggerScreenshot()
  })

  chatHotkeyUnlisten = await listen('chat-hotkey', () => {
    handleChatHotkey()
  })

  settingsHotkeyUnlisten = await listen('settings-hotkey', () => {
    openSettingsWindow()
  })

  quitHotkeyUnlisten = await listen('quit-hotkey', async () => {
    await autoSaveGhost()
    const win = getCurrentWindow()
    await win.close()
  })

  initiativeMessageUnlisten = await listen('initiative-message', (event: any) => {
    const payload = event.payload as { text: string; trigger: string }
    initiative.onInitiativeMessage(payload?.text)
  })

  ttsUpdatedUnlisten = await listen('tts-updated', (event: any) => {
    initiative.onTTSUpdated(event.payload || {})
  })

  settingsUpdatedUnlisten = await listen('settings-updated', async (event: any) => {
    const section = event.payload?.section
    if (section === 'renderer') {
      loadRendererFromStorage()
    } else if (section === 'curiosity' || section === 'ghost') {
      try {
        const status = await invoke<string>('get_ghost_status')
        const parsed = JSON.parse(status)
        ghost.value = parsed
        updateMood(parsed.loveHate, parsed.curiosityLevel)
      } catch (e) {
        console.error('Failed to refresh ghost status:', e)
      }
    } else if (section === 'answering') {
      answeringMode.value = (localStorage.getItem('deskpet_answering_mode') as 'Companion' | 'Assistant') || 'Companion'
    } else if (section === 'appearance') {
      petAppearance.value = loadAppearance()
    } else if (section === 'initiative') {
      initiative.syncInitiativeConfig()
    } else if (section === 'theme') {
      applyTheme()
    }
  })

  requestTransferUnlisten = await listen('request-transfer', () => {
    startTransfer()
  })

  screenshotCapturedUnlisten = await listen('screenshot-region-captured', (event: any) => {
    const { base64, region } = event.payload
    screenshotScreenRegion.value = region
    analyzeScreenshot(base64)
  })

  screenshotCancelledUnlisten = await listen('screenshot-cancelled', () => {
    screenshotAnalysisLoading.value = false
    restoreAnsweringMode()
  })

  chatPostProcessedUnlisten = await listen('chat:post-processed', (event: any) => {
    const data = event.payload as {
      loveHate: number
      baseline: number
      sentiment: {
        eventType: string
        loveHateHint: number
      }
      impression: {
        overallAffinity: number
        latestSnippet: string | null
      }
    }
    if (ghost.value) {
      ghost.value.loveHate = data.loveHate
      ghost.value.baseline = data.baseline
      updateMood(ghost.value.loveHate, ghost.value.curiosityLevel)
      if (data.sentiment) {
        playEmotionReaction(data.sentiment.eventType, data.sentiment.loveHateHint)
        if (data.sentiment.loveHateHint !== 0) {
          const hint = data.sentiment.loveHateHint > 0
            ? `(感到${data.sentiment.loveHateHint > 2 ? '很开心' : '有些开心'})`
            : `(感到${data.sentiment.loveHateHint < -2 ? '很不高兴' : '有点不高兴'})`
          pushSystemMessage(`情感: ${data.sentiment.eventType} ${hint}`)
        }
      }
      if (data.impression) {
        ghost.value.impression.overallAffinity = data.impression.overallAffinity
      }
    }
    chatLoading.value = false
    checkAndNotifyAchievements()
  })

  let speakingActive = false
  chatTokenUnlisten = await listen('chat:token', () => {
    if (!speakingActive) {
      speakingActive = true
      startSpeaking()
    }
  })
  chatCompleteUnlisten = await listen('chat:complete', () => {
    speakingActive = false
    stopSpeaking()
  })

  // === 点击穿透：透明区域穿透到桌面，立绘/工具栏区域可交互 ===
  // 窗口放大到400x400后透明区域较大，不穿透会挡住其他窗口的点击。
  // setIgnoreCursorEvents(true) 让整个窗口穿透，但会导致立绘也无法点击，
  // 所以用 requestAnimationFrame 轮询鼠标位置：在立绘区域恢复交互，离开则穿透。
  const win = getCurrentWindow()
  await win.setIgnoreCursorEvents(true)
  cursorEventsIgnored = true

  async function pollCursor() {
    try {
      const pos = await cursorPosition()
      const winPos = await win.outerPosition()
      const sf = await win.scaleFactor()
      const logicalX = (pos.x - winPos.x) / sf
      const logicalY = (pos.y - winPos.y) / sf

      const el = petColumnRef.value
      const shouldInteract = (() => {
        if (!el) return false
        const r = el.getBoundingClientRect()
        if (logicalX >= r.left && logicalX <= r.right && logicalY >= r.top && logicalY <= r.bottom) {
          return true
        }
        // 工具栏是 absolute 定位、溢出在 pet-column 下方，其 rect 不在 pet-column 盒子内，
        // 需要单独命中判断，否则鼠标移到工具栏时会被判定为离开 → 切穿透 → 按钮点不到
        const tb = toolbarRef.value
        if (tb) {
          const tr = tb.getBoundingClientRect()
          if (logicalX >= tr.left && logicalX <= tr.right && logicalY >= tr.top && logicalY <= tr.bottom) {
            return true
          }
        }
        return false
      })()

      if (shouldInteract && cursorEventsIgnored) {
        await win.setIgnoreCursorEvents(false)
        cursorEventsIgnored = false
      } else if (!shouldInteract && !cursorEventsIgnored) {
        await win.setIgnoreCursorEvents(true)
        cursorEventsIgnored = true
      }

      // 头部视觉追踪：复用同一帧的鼠标位置，计算与宠物中心的相对距离。
      // 宠物在窗口内居中，petX/petY 为其在窗口内的偏移（wander/teleport 时变化）。
      const petCenterX = window.innerWidth / 2 + petX.value
      const petCenterY = window.innerHeight / 2 + petY.value
      headDirection.value = headTracking.update(logicalX, logicalY, petCenterX, petCenterY)
    } catch {
      // 轮询失败时保持当前状态，下一帧重试
    }
    cursorPollId = requestAnimationFrame(pollCursor)
  }
  cursorPollId = requestAnimationFrame(pollCursor)

  document.addEventListener('keydown', () => {})
})

onUnmounted(() => {
  autoSaveGhost()
  if (tickInterval) clearInterval(tickInterval)
  if (autoSaveInterval) clearInterval(autoSaveInterval)
  initiative.stopInitiativeLoop()
  clearSpeechBubble()
  clearTransfer()
  if (cursorPollId) cancelAnimationFrame(cursorPollId)
  if (hotkeyUnlisten) hotkeyUnlisten()
  if (chatHotkeyUnlisten) chatHotkeyUnlisten()
  if (settingsHotkeyUnlisten) settingsHotkeyUnlisten()
  if (quitHotkeyUnlisten) quitHotkeyUnlisten()
  if (initiativeMessageUnlisten) initiativeMessageUnlisten()
  if (ttsUpdatedUnlisten) ttsUpdatedUnlisten()
  if (settingsUpdatedUnlisten) settingsUpdatedUnlisten()
  if (requestTransferUnlisten) requestTransferUnlisten()
  if (screenshotCapturedUnlisten) screenshotCapturedUnlisten()
  if (screenshotCancelledUnlisten) screenshotCancelledUnlisten()
  if (chatPostProcessedUnlisten) chatPostProcessedUnlisten()
  if (chatTokenUnlisten) chatTokenUnlisten()
  if (chatCompleteUnlisten) chatCompleteUnlisten()
  document.removeEventListener('keydown', () => {})
  stopDailyRoutine()
  stopBlink()
  window.speechSynthesis?.cancel()
})
</script>

<template>
  <div class="pet-app">
    <!-- Error banner: 灵魂生成/加载失败时显示，原 bug 为此状态完全不渲染 -->
    <div v-if="error" class="pet-error">
      <span class="pet-error-text">{{ error }}</span>
      <button class="pet-error-dismiss" @click="error = ''">✕</button>
    </div>

    <!-- Pet column: fixed-width container so pet doesn't shift on window resize -->
    <div class="pet-column" ref="petColumnRef" @mouseenter="onHover(true)" @mouseleave="onHover(false)">
      <div class="pet-area" @click="handlePetClick" @mousedown.prevent="onPetMouseDown">
        <!-- 主动搭话气泡：宠物上方浮现 -->
        <transition name="bubble-pop">
          <div v-if="speechBubble" class="speech-bubble" @click.stop="openChatWindow">
            <span class="speech-bubble-text">{{ speechBubble }}</span>
            <div class="speech-bubble-tail"></div>
          </div>
        </transition>
        <PetRenderer
          :animation-state="currentAnimationState"
          :mood-config="moodConfig"
          :emoji="petEmoji"
          :css-class="petCssClass"
          :style-override="petStyleOverride"
          :renderer-type="rendererType"
          :sprite-config="spriteConfig"
          :lottie-config="lottieConfig"
          :tag-ranges="tagRanges"
          :frame-durations="frameDurations"
          :frame-positions="framePositions"
          :is-flipped="isFlipped"
          :head-direction="headDirection"
          :head-enabled="headEnabled"
          :head-config="headConfig"
          @animation-complete="onAnimationComplete"
        />
      </div>

      <!-- Hover toolbar (centered below pet) -->
      <transition name="toolbar-fade">
        <div v-if="showToolbar" ref="toolbarRef" class="hover-toolbar" @click.stop>
          <div class="hover-info">
            <span class="hover-pet-name">{{ petName }} <span class="hover-lh-score" :style="{ color: affinityColor }">{{ ghost?.loveHate.toFixed(0) }}</span></span>
            <span class="hover-mood-text">{{ moodText }} · 印象 {{ ghost?.impression.overallAffinity.toFixed(0) }}</span>
            <div class="hover-mood-bar">
              <div class="hover-mood-fill" :style="{ width: loveHatePercent + '%', background: affinityColor }"></div>
            </div>
          </div>
          <div class="hover-actions">
            <button @click.stop="openChatWindow" title="对话 (Ctrl+Alt+C)">💬</button>
            <button @click.stop="triggerScreenshot" title="截图 (Ctrl+Alt+X)">📸</button>
            <button @click.stop="openSettingsWindow" title="设置 (Ctrl+Alt+S)">⚙️</button>
          </div>
        </div>
      </transition>
    </div><!-- /pet-column -->

    <div v-if="screenshotAnalysisLoading" class="screenshot-analysis-loading">
      <div class="loading-spinner"></div>
      <span>桌宠正在看截图...</span>
    </div>

    <!-- Enhanced Curiosity Privacy Dialog -->
    <PrivacyDialog
      v-if="showEnhancedPrivacyDialog"
      @accept="acceptEnhancedPrivacy"
      @decline="declineEnhancedPrivacy"
    />

    <!-- Soul Transfer Overlay -->
    <TransferOverlay
      :phase="transferPhase"
      :progress="transferAnimProgress"
      :result="transferResult"
      :pet-emoji="petEmoji"
      :particles="transferParticles"
      @finish="finishTransfer"
    />
  </div>
</template>

<style scoped>
.pet-app {
  width: 100vw;
  height: 100vh;
  position: fixed;
  top: 0;
  left: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  background: transparent;
  font-family: 'Segoe UI', system-ui, -apple-system, sans-serif;
  font-size: 13px;
  color: var(--t-text, #333);
  user-select: none;
}

.pet-error {
  position: absolute;
  top: 8px;
  left: 8px;
  right: 8px;
  background: rgba(244, 67, 54, 0.92);
  color: white;
  padding: 8px 12px;
  border-radius: 10px;
  font-size: 12px;
  z-index: 200;
  display: flex;
  align-items: flex-start;
  gap: 8px;
  animation: fadeIn 0.25s ease;
}

.pet-error-text {
  flex: 1;
  word-break: break-word;
}

.pet-error-dismiss {
  background: rgba(255, 255, 255, 0.2);
  border: none;
  color: white;
  width: 22px;
  height: 22px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 12px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}

.pet-column {
  position: relative;
}

/* Pet Area */
.pet-area {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  padding: 8px 4px 0;
  cursor: grab;
}

/* 主动搭话气泡 */
.speech-bubble {
  position: absolute;
  bottom: 100%;
  left: 50%;
  transform: translateX(-50%);
  max-width: 220px;
  min-width: 60px;
  margin-bottom: 6px;
  padding: 7px 12px;
  background: var(--t-speech-bg, #fff);
  border: var(--t-speech-border, 2px solid #333);
  border-radius: 12px;
  box-shadow: var(--t-speech-shadow, 2px 2px 0 rgba(0, 0, 0, 0.2));
  z-index: 20;
  cursor: pointer;
  text-align: center;
}

.speech-bubble-text {
  font-size: 12px;
  color: var(--t-speech-text, #333);
  line-height: 1.5;
  display: block;
}

.speech-bubble-tail {
  position: absolute;
  left: 50%;
  bottom: -8px;
  transform: translateX(-50%) rotate(45deg);
  width: 12px;
  height: 12px;
  background: var(--t-speech-bg, #fff);
  border-right: var(--t-speech-border, 2px solid #333);
  border-bottom: var(--t-speech-border, 2px solid #333);
}

.bubble-pop-enter-active,
.bubble-pop-leave-active {
  transition: opacity 0.25s ease, transform 0.25s ease;
}

.bubble-pop-enter-from,
.bubble-pop-leave-to {
  opacity: 0;
  transform: translateX(-50%) translateY(6px);
}

/* Hover toolbar */
.hover-toolbar {
  position: absolute;
  top: 100%;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 10px 7px;
  background: var(--t-toolbar-bg, rgba(255, 255, 255, 0.92));
  border-radius: 12px;
  box-shadow: var(--t-toolbar-shadow, 0 2px 12px rgba(0, 0, 0, 0.12));
  white-space: nowrap;
  -webkit-app-region: no-drag;
  pointer-events: auto;
  z-index: 200;
}

.hover-info {
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 48px;
}

.hover-pet-name {
  font-size: 12px;
  font-weight: 700;
  color: var(--t-toolbar-text, #333);
  line-height: 1.2;
}

.hover-lh-score {
  font-size: 11px;
  font-weight: 600;
}

.hover-mood-text {
  font-size: 10px;
  color: var(--t-text-2, #888);
  line-height: 1.2;
}

.hover-mood-bar {
  height: 3px;
  background: rgba(0, 0, 0, 0.1);
  border-radius: 2px;
  overflow: hidden;
  width: 56px;
}

.hover-mood-fill {
  height: 100%;
  border-radius: 2px;
  transition: width 0.5s ease, background 0.5s ease;
}

.hover-actions {
  display: flex;
  gap: 2px;
}

.hover-actions button {
  width: 26px;
  height: 26px;
  border: none;
  background: transparent;
  border-radius: 8px;
  font-size: 14px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s;
  padding: 0;
}

.hover-actions button:hover {
  background: var(--t-toolbar-btn-hover, rgba(255, 107, 157, 0.1));
  transform: scale(1.15);
}

.toolbar-fade-enter-active { transition: all 0.2s ease; }
.toolbar-fade-leave-active { transition: all 0.15s ease; }
.toolbar-fade-enter-from,
.toolbar-fade-leave-to { opacity: 0; transform: translateY(-4px); }

.screenshot-analysis-loading {
  position: fixed;
  top: 16px;
  left: 50%;
  transform: translateX(-50%);
  background: var(--t-overlay-bg, rgba(0, 0, 0, 0.8));
  color: var(--t-overlay-text, white);
  padding: 12px 20px;
  border-radius: 12px;
  font-size: 13px;
  z-index: 1004;
  display: flex;
  align-items: center;
  gap: 10px;
  animation: fadeIn 0.3s ease;
}

.loading-spinner {
  width: 32px;
  height: 32px;
  border: 3px solid rgba(255, 255, 255, 0.3);
  border-top: 3px solid var(--t-spinner, #ff6b9d);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@keyframes fadeIn {
  from { opacity: 0; transform: translateX(-50%) translateY(-8px); }
  to { opacity: 1; transform: translateX(-50%) translateY(0); }
}
</style>
