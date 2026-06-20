<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { listen, emit } from '@tauri-apps/api/event'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { LogicalPosition, LogicalSize } from '@tauri-apps/api/dpi'
import { ref, onMounted, onUnmounted, computed } from 'vue'
import PetRenderer from './components/PetRenderer.vue'
import { useChat } from './composables/useChat'
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
  persona?: string
}

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

const { ghostId, pushSystemMessage, clearMessages, loadHistory } = useChat()
const chatLoading = ref(false)
const { currentAnimationState, moodConfig, petX, petY, isFlipped, isPerformingBehavior, updateMood, setPersonality, startDailyRoutine, stopDailyRoutine, stopBlink, holdAnimation, playOneShot, playEmotionReaction, startSpeaking, stopSpeaking, onAnimationComplete } = useAnimation()
const { rendererType, spriteConfig, lottieConfig, tagRanges, frameDurations, framePositions, setRenderer, setSpriteConfig, parseAsepriteJson } = usePetRenderer()

const showToolbar = ref(false)
let didDrag = false
let toolbarHideTimer: ReturnType<typeof setTimeout> | null = null
let shrinkTimer: ReturnType<typeof setTimeout> | null = null
let windowExpanded = false

const COLLAPSED_W = 150
const COLLAPSED_H = 158
const EXPANDED_W = 240
const EXPANDED_H = 195

async function expandWindow() {
  if (windowExpanded) return
  windowExpanded = true
  const win = getCurrentWindow()
  await win.setSize(new LogicalSize(EXPANDED_W, EXPANDED_H))
}

async function shrinkWindow() {
  if (!windowExpanded) return
  windowExpanded = false
  const win = getCurrentWindow()
  await win.setSize(new LogicalSize(COLLAPSED_W, COLLAPSED_H))
}

function onHover(visible: boolean) {
  if (visible) {
    if (toolbarHideTimer) { clearTimeout(toolbarHideTimer); toolbarHideTimer = null }
    if (shrinkTimer) { clearTimeout(shrinkTimer); shrinkTimer = null }
    showToolbar.value = true
    expandWindow()
  } else {
    toolbarHideTimer = setTimeout(() => {
      showToolbar.value = false
      shrinkTimer = setTimeout(shrinkWindow, 200)
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

const screenshotScreenRegion = ref({ x: 0, y: 0, width: 0, height: 0 })
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
    opacity: a.opacity,
  }
})

const petName = computed(() => ghost.value?.name || '桌宠')



async function generateGhost() {
  loading.value = true
  error.value = ''
  try {
    await invoke<string>('generate_ghost', { name: '小花' })

    // 自动生成初始人设
    try {
      await invoke<string>('generate_persona')
    } catch {
      // AI 未配置时静默跳过，使用默认描述
    }

    const status = await invoke<string>('get_ghost_status')
    const parsed = JSON.parse(status)
    ghost.value = parsed
    ghostId.value = parsed.ghostId
    updateMood(parsed.loveHate, parsed.curiosityLevel)
    if (parsed.personality) {
      setPersonality(parsed.personality)
    }
    clearMessages()
    await loadHistory()
    pushSystemMessage(`${petName.value}的灵魂已注入！点击桌宠或按 Ctrl+Alt+C 开始对话。`)
    startDailyRoutine()
  } catch (e: any) {
    error.value = e.toString()
  } finally {
    loading.value = false
  }
}

async function loadAutosaveGhost(path: string) {
  loading.value = true
  error.value = ''
  try {
    await invoke<string>('load_ghost', { path })
    const status = await invoke<string>('get_ghost_status')
    const parsed = JSON.parse(status)
    ghost.value = parsed
    ghostId.value = parsed.ghostId
    updateMood(parsed.loveHate, parsed.curiosityLevel)
    if (parsed.personality) {
      setPersonality(parsed.personality)
    }
    await loadHistory()
    pushSystemMessage(`${petName.value}的灵魂已恢复！欢迎回来~`)
    startDailyRoutine()
  } catch (e: any) {
    console.warn('自动加载失败，创建新灵魂:', e)
    await generateGhost()
  } finally {
    loading.value = false
  }
}

async function autoSaveGhost() {
  if (!ghost.value) return
  try {
    await invoke('auto_save_ghost')
  } catch (e) {
    console.warn('自动保存失败:', e)
  }
}

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

async function checkAndNotifyAchievements() {
  try {
    const result = await invoke<{ newAchievements: Array<{ key: string; name: string; description: string }> }>('check_achievements')
    if (result.newAchievements && result.newAchievements.length > 0) {
      for (const a of result.newAchievements) {
        pushSystemMessage(`🏆 成就解锁: ${a.name} — ${a.description}`)
      }
    }
  } catch (e) {
    // 静默失败，不影响主流程
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

let tickInterval: ReturnType<typeof setInterval> | null = null
let tickInProgress = false
const asleep = ref(false)
let sleepRelease: (() => void) | null = null
let diaryInProgress = false
let autoSaveInterval: ReturnType<typeof setInterval> | null = null
let hotkeyUnlisten: (() => void) | null = null
let chatHotkeyUnlisten: (() => void) | null = null
let settingsHotkeyUnlisten: (() => void) | null = null
let quitHotkeyUnlisten: (() => void) | null = null
let chatPostProcessedUnlisten: (() => void) | null = null
let chatTokenUnlisten: (() => void) | null = null
let chatCompleteUnlisten: (() => void) | null = null
let settingsUpdatedUnlisten: (() => void) | null = null
let requestTransferUnlisten: (() => void) | null = null
let screenshotCapturedUnlisten: (() => void) | null = null
let screenshotCancelledUnlisten: (() => void) | null = null
let transferAnimInterval: ReturnType<typeof setInterval> | null = null

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

  // 启动时把 localStorage 中保存的 AI 配置还原到后端内存。
  // 后端 AppState.ai_config 启动为 None，不做这一步会导致每次重启应用后
  // 后端丢失配置，必须用户再点一次「保存」才能恢复对话/文档功能。
  // Key 本就存储在 localStorage，此处只是原样回传，不增加任何泄露面。
  //
  // TODO(安全): API Key 以明文存于 localStorage（WebView2 用户目录）。
  //   对本地单机桌宠够用；若将来 Key 涉及更高敏感度（云存储/支付等）或要分发上架，
  //   应改用 tauri-plugin-stronghold（加密保险库）替代 localStorage。
  try {
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
      })
    } else {
      // 首次启动或未配置：引导用户去设置 AI 接口。
      // 没有可用 AI 配置时，桌宠能动能展示但无法对话/情感/记忆——这是最大的新手流失点。
      // 延迟一点再弹，避免与初始化的 ghost 生成/窗口定位抢焦点。
      setTimeout(() => { openSettingsWindow() }, 1200)
    }
  } catch (e) {
    console.warn('还原 AI 配置失败:', e)
  }

  tickInterval = setInterval(tickGhost, 5000)
  autoSaveInterval = setInterval(autoSaveGhost, 120000)

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

  document.addEventListener('keydown', () => {})
})

onUnmounted(() => {
  autoSaveGhost()
  if (tickInterval) clearInterval(tickInterval)
  if (autoSaveInterval) clearInterval(autoSaveInterval)
  if (transferAnimInterval) clearInterval(transferAnimInterval)
  if (hotkeyUnlisten) hotkeyUnlisten()
  if (chatHotkeyUnlisten) chatHotkeyUnlisten()
  if (settingsHotkeyUnlisten) settingsHotkeyUnlisten()
  if (quitHotkeyUnlisten) quitHotkeyUnlisten()
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

function handleChatHotkey() {
  wakeUpAndOpenChat()
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

async function wakeUpAndOpenChat() {
  if (asleep.value) {
    await wakeUpPet()
  }
  openChatWindow()
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

async function openChatWindow() {
  if (asleep.value) {
    await wakeUpPet()
  }
  let chatWin = await WebviewWindow.getByLabel('chat')
  if (!chatWin) {
    chatWin = new WebviewWindow('chat', {
      url: 'chat.html',
      title: 'DeskPet - 对话',
      width: 360,
      height: 500,
      minWidth: 300,
      minHeight: 300,
      resizable: true,
      transparent: true,
      decorations: false,
      alwaysOnTop: true,
      skipTaskbar: false,
      visible: true,
    })
    await new Promise(resolve => setTimeout(resolve, 200))
    try {
      await chatWin.setShadow(false)
    } catch (e) {
      console.warn('Failed to disable shadow:', e)
    }
  }
  const visible = await chatWin.isVisible()
  if (visible) {
    await chatWin.setFocus()
  } else {
    await chatWin.show()
    await chatWin.setFocus()
  }
}

async function triggerScreenshot() {
  if (screenshotAnalysisLoading.value) return
  screenshotAnalysisLoading.value = true

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
    })
  } catch (e: any) {
    screenshotAnalysisLoading.value = false
    pushSystemMessage(`截图失败: ${e}`)
  }
}

async function analyzeScreenshot(base64: string) {
  if (!ghost.value) {
    pushSystemMessage('请先生成桌宠灵魂，再进行截图识别。')
    return
  }

  movePetToScreenshotRegion()
  playOneShot('surprise', 400)

  if (answeringMode.value === 'Companion') {
    previousAnsweringMode.value = 'Companion'
    answeringMode.value = 'Assistant'
    invoke('set_answering_mode', { mode: 'Assistant' }).catch(() => {})
  }

  await openChatWindow()
  setTimeout(async () => {
    try {
      await emit('chat:open-with-screenshot', { base64 })
    } catch {
      pushSystemMessage('截图数据发送到对话窗口失败')
    }
  }, 300)

  screenshotAnalysisLoading.value = false
}

function restoreAnsweringMode() {
  if (previousAnsweringMode.value) {
    answeringMode.value = previousAnsweringMode.value
    invoke('set_answering_mode', { mode: previousAnsweringMode.value }).catch(() => {})
    previousAnsweringMode.value = null
  }
}

async function movePetToScreenshotRegion() {
  try {
    const win = getCurrentWindow()
    const region = screenshotScreenRegion.value
    if (!region || (region.width === 0 && region.height === 0)) return

    const winSize = await win.innerSize()
    const scaleFactor = await win.scaleFactor()
    const winLogicalW = winSize.width / scaleFactor
    const winLogicalH = winSize.height / scaleFactor

    const targetScreenX = region.x / scaleFactor + region.width / scaleFactor + 20
    const screenW = window.screen.availWidth
    const screenH = window.screen.availHeight

    let finalX = Math.max(0, Math.min(targetScreenX, screenW - winLogicalW))
    let finalY = Math.max(0, Math.min(region.y / scaleFactor - 40, screenH - winLogicalH))

    await win.setPosition(new LogicalPosition(Math.round(finalX), Math.round(finalY)))
    petX.value = 0
    petY.value = 0
  } catch (e) {
    console.warn('Failed to move window to screenshot region:', e)
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

  if (transferAnimInterval) clearInterval(transferAnimInterval)
  let frame = 0
  const totalFrames = 120
  transferAnimInterval = setInterval(() => {
    frame++
    transferAnimProgress.value = frame / totalFrames
    if (frame >= totalFrames) {
      if (transferAnimInterval) clearInterval(transferAnimInterval)
      transferAnimInterval = null
      executeTransfer()
    }
  }, 25)
}

async function executeTransfer() {
  // 传送本身：只有这一步失败才算"传送失败"
  let result: TransferResult
  try {
    result = await invoke<TransferResult>('transfer_ghost', {
      savePath: getGhostSavePath(),
    })
  } catch (e: any) {
    error.value = `灵魂传送失败: ${e}`
    transferPhase.value = 'idle'
    showTransfer.value = false
    return
  }

  // 传送已成功落盘。后续读取状态/成就即使失败也不再回退相位，
  // 否则会误报"传送失败"而实际新灵魂已经写入磁盘和内存。
  transferResult.value = result
  transferPhase.value = 'revealing'

  try {
    if (ghost.value) {
      ghost.value = JSON.parse(await invoke<string>('get_ghost_status'))
    }
  } catch (e) {
    console.warn('传送后读取 ghost 状态失败（不影响传送结果）:', e)
  }
  try {
    checkAndNotifyAchievements()
  } catch (e) {
    console.warn('传送后检查成就失败（不影响传送结果）:', e)
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
    <!-- Pet column: fixed-width container so pet doesn't shift on window resize -->
    <div class="pet-column" @mouseenter="onHover(true)" @mouseleave="onHover(false)">
      <div class="pet-area" @click="handlePetClick" @mousedown.prevent="onPetMouseDown">
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
          @animation-complete="onAnimationComplete"
        />
      </div>

      <!-- Hover toolbar (centered below pet) -->
      <transition name="toolbar-fade">
        <div v-if="showToolbar" class="hover-toolbar" @click.stop>
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
    <transition name="overlay-fade">
      <div v-if="showEnhancedPrivacyDialog" class="privacy-overlay" @click.self>
        <div class="privacy-dialog">
          <div class="privacy-title">🔍 增强好奇心 - 隐私授权</div>
          <div class="privacy-body">
            <p>增强好奇心模式会定期截取您的桌面屏幕，让桌宠了解您正在做什么。</p>
            <p><strong>隐私保护措施：</strong></p>
            <ul>
              <li>截图数据仅在本地处理，不会上传或存储原始截图</li>
              <li>仅将 AI 分析结果的文本摘要记入主人印象</li>
              <li>您可随时在设置中关闭或清除已收集的印象数据</li>
            </ul>
            <p class="privacy-warning">请确认您同意此功能的使用。</p>
          </div>
          <div class="privacy-buttons">
            <button class="btn btn-privacy-accept" @click.stop="acceptEnhancedPrivacy">同意并开启</button>
            <button class="btn btn-privacy-decline" @click.stop="declineEnhancedPrivacy">拒绝</button>
          </div>
        </div>
      </div>
    </transition>

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
  background: transparent;
  font-family: 'Segoe UI', system-ui, -apple-system, sans-serif;
  font-size: 13px;
  color: #333;
  user-select: none;
}

/* Pet Column */
.pet-column {
  position: relative;
  width: 150px;
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

/* Bubble dialogue */
.bubble-dialogue-wrapper {
  margin: 0 4px 4px;
}

/* Hover toolbar */
.hover-toolbar {
  position: absolute;
  top: 100%;
  left: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 10px 7px;
  background: rgba(255, 255, 255, 0.92);
  border-radius: 12px;
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.12);
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
  color: #333;
  line-height: 1.2;
}

.hover-lh-score {
  font-size: 11px;
  font-weight: 600;
}

.hover-mood-text {
  font-size: 10px;
  color: #888;
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

.hover-toolbar button {
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

.hover-toolbar button:hover {
  background: rgba(255, 107, 157, 0.1);
  transform: scale(1.15);
}

.toolbar-fade-enter-active { transition: all 0.2s ease; }
.toolbar-fade-leave-active { transition: all 0.15s ease; }
.toolbar-fade-enter-from,
.toolbar-fade-leave-to { opacity: 0; transform: translateY(-4px); }

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

.privacy-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  z-index: 2000;
  display: flex;
  align-items: center;
  justify-content: center;
}

.privacy-dialog {
  background: white;
  border-radius: 16px;
  padding: 24px;
  max-width: 320px;
  width: 90%;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.2);
}

.privacy-title {
  font-size: 15px;
  font-weight: 700;
  margin-bottom: 12px;
  color: #333;
}

.privacy-body {
  font-size: 12px;
  line-height: 1.7;
  color: #555;
}

.privacy-body p {
  margin: 6px 0;
}

.privacy-body ul {
  margin: 6px 0;
  padding-left: 18px;
}

.privacy-body li {
  margin: 3px 0;
}

.privacy-warning {
  color: #f44336;
  font-weight: 600;
  margin-top: 10px;
}

.privacy-buttons {
  display: flex;
  gap: 8px;
  margin-top: 16px;
}

.btn-privacy-accept {
  flex: 1;
  background: linear-gradient(135deg, #4caf50, #8bc34a);
  color: white;
  border: none;
  padding: 10px 16px;
  border-radius: 10px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}

.btn-privacy-decline {
  flex: 1;
  background: #f5f5f5;
  color: #666;
  border: 1px solid #ddd;
  padding: 10px 16px;
  border-radius: 10px;
  font-size: 13px;
  cursor: pointer;
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