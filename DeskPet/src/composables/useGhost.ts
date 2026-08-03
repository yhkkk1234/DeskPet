import { type Ref, type ComputedRef } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { RendererType, SpriteConfig } from './usePetRenderer'

export interface GhostStatus {
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

export interface GhostLifecycleDeps {
  ghost: Ref<GhostStatus | null>
  loading: Ref<boolean>
  error: Ref<string>
  chat: {
    ghostId: Ref<string>
    clearMessages: () => void
    loadHistory: () => Promise<void>
    pushSystemMessage: (content: string, persist?: boolean) => void
  }
  anim: {
    updateMood: (loveHate: number, curiosityLevel: string) => void
    setPersonality: (p: GhostStatus['personality']) => void
    startDailyRoutine: () => void
  }
  petName: ComputedRef<string>
  spriteConfig: Ref<SpriteConfig>
  parseAsepriteJson: (json: any) => void
  setRenderer: (t: RendererType) => void
  setSpriteConfig: (c: Partial<SpriteConfig>) => void
}

/** ghost 生命周期：生成/自动加载/自动保存 + 渲染器恢复 */
export function useGhostLifecycle(deps: GhostLifecycleDeps) {
  async function generateGhost() {
    deps.loading.value = true
    deps.error.value = ''
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
      deps.ghost.value = parsed
      deps.chat.ghostId.value = parsed.ghostId
      deps.anim.updateMood(parsed.loveHate, parsed.curiosityLevel)
      if (parsed.personality) {
        deps.anim.setPersonality(parsed.personality)
      }
      deps.chat.clearMessages()
      await deps.chat.loadHistory()
      deps.chat.pushSystemMessage(`${deps.petName.value}的灵魂已注入！点击桌宠或按 Ctrl+Alt+C 开始对话。`)
      deps.anim.startDailyRoutine()
    } catch (e: any) {
      deps.error.value = e.toString()
    } finally {
      deps.loading.value = false
    }
  }

  async function loadAutosaveGhost(path: string) {
    deps.loading.value = true
    deps.error.value = ''
    try {
      await invoke<string>('load_ghost', { path })
      const status = await invoke<string>('get_ghost_status')
      const parsed = JSON.parse(status)
      deps.ghost.value = parsed
      deps.chat.ghostId.value = parsed.ghostId
      deps.anim.updateMood(parsed.loveHate, parsed.curiosityLevel)
      if (parsed.personality) {
        deps.anim.setPersonality(parsed.personality)
      }
      // 先清除旧的欢迎消息，避免每次重启累积重复的"灵魂已恢复"
      await invoke('purge_welcome_messages').catch(() => {})
      await deps.chat.loadHistory()
      deps.chat.pushSystemMessage(`${deps.petName.value}的灵魂已恢复！欢迎回来~`)
      deps.anim.startDailyRoutine()
    } catch (e: any) {
      console.warn('自动加载失败，创建新灵魂:', e)
      await generateGhost()
    } finally {
      deps.loading.value = false
    }
  }

  async function autoSaveGhost() {
    if (!deps.ghost.value) return
    try {
      await invoke('auto_save_ghost')
    } catch (e) {
      console.warn('自动保存失败:', e)
    }
  }

  async function loadSpriteJson() {
    try {
      const response = await fetch(deps.spriteConfig.value.jsonSrc)
      if (response.ok) {
        const json = await response.json()
        deps.parseAsepriteJson(json)
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
      deps.setRenderer(savedType)
    }
    const savedSrc = localStorage.getItem('deskpet_sprite_src')
    const savedJsonSrc = localStorage.getItem('deskpet_sprite_json_src')
    if (savedSrc || savedJsonSrc) {
      deps.setSpriteConfig({
        src: savedSrc || deps.spriteConfig.value.src,
        jsonSrc: savedJsonSrc || deps.spriteConfig.value.jsonSrc,
      })
    }
  }

  return { generateGhost, loadAutosaveGhost, autoSaveGhost, loadSpriteJson, loadRendererFromStorage }
}
