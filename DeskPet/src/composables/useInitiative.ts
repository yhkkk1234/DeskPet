import { type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export interface InitiativeDeps {
  ghost: Ref<{ curiosityLevel: string } | null>
  pushPetMessage: (content: string) => void
  showSpeechBubble: (text: string) => void
  ttsEnabled: Ref<boolean>
  ttsRate: Ref<number>
  ttsPitch: Ref<number>
  ttsEngine: Ref<'system' | 'edge'>
  ttsVoice: Ref<string>
}

/** 主动搭话：深夜问候/低电量提醒/剪贴板感知的配置同步与 60s 轮询 */
export function useInitiative(deps: InitiativeDeps) {
  let initiativeInterval: ReturnType<typeof setInterval> | null = null

  function startInitiativeLoop() {
    stopInitiativeLoop()
    initiativeInterval = setInterval(runInitiativeTick, 60000)
  }

  function stopInitiativeLoop() {
    if (initiativeInterval) {
      clearInterval(initiativeInterval)
      initiativeInterval = null
    }
  }

  async function syncInitiativeConfig() {
    try {
      const saved = localStorage.getItem('deskpet_initiative_config')
      const cfg = saved ? JSON.parse(saved) : {}
      await invoke('set_initiative_config', {
        nightGreeting: cfg.nightGreeting !== undefined ? cfg.nightGreeting : true,
        clipboardSense: cfg.clipboardSense === true,
        batteryAlert: cfg.batteryAlert !== undefined ? cfg.batteryAlert : true,
      })
    } catch (e) {
      console.warn('同步主动搭话配置失败:', e)
    }
  }

  function loadTTSFromStorage() {
    deps.ttsEnabled.value = localStorage.getItem('deskpet_tts_enabled') === 'true'
    deps.ttsRate.value = parseFloat(localStorage.getItem('deskpet_tts_rate') || '1.0')
    deps.ttsPitch.value = parseFloat(localStorage.getItem('deskpet_tts_pitch') || '1.1')
    deps.ttsEngine.value = (localStorage.getItem('deskpet_tts_engine') as 'system' | 'edge') || 'system'
    deps.ttsVoice.value = localStorage.getItem('deskpet_tts_voice') || 'zh-CN-XiaoxiaoNeural'
  }

  async function runInitiativeTick() {
    if (!deps.ghost.value) return
    try {
      await invoke('initiative_tick')
    } catch (e) {
      console.warn('主动搭话 tick 失败:', e)
    }
  }

  function onInitiativeMessage(text: string) {
    if (!text) return
    // 主窗口气泡 + TTS + 持久化（chatMessages 可见）
    deps.showSpeechBubble(text)
    deps.pushPetMessage(text)
  }

  function onTTSUpdated(cfg: Record<string, unknown>) {
    if (typeof cfg.enabled === 'boolean') deps.ttsEnabled.value = cfg.enabled
    if (typeof cfg.rate === 'number') deps.ttsRate.value = cfg.rate
    if (typeof cfg.pitch === 'number') deps.ttsPitch.value = cfg.pitch
    if (cfg.engine === 'system' || cfg.engine === 'edge') deps.ttsEngine.value = cfg.engine
    if (typeof cfg.voice === 'string') deps.ttsVoice.value = cfg.voice
  }

  return {
    startInitiativeLoop,
    stopInitiativeLoop,
    syncInitiativeConfig,
    loadTTSFromStorage,
    runInitiativeTick,
    onInitiativeMessage,
    onTTSUpdated,
  }
}
