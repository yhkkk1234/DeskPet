import { ref, computed, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { GhostStatus } from './useGhost'

export interface TransferResult {
  oldSignature: string
  newSignature: string
  generation: number
  oldGeneration: number
  perturbation: Record<string, number>
  transferredName: string
}

export const personalityLabels: Record<string, string> = {
  openness: '开放',
  conscientiousness: '尽责',
  extraversion: '外向',
  agreeableness: '宜人',
  neuroticism: '情绪',
  creativity: '创造',
}

export const personalityKeys = ['openness', 'conscientiousness', 'extraversion', 'agreeableness', 'neuroticism', 'creativity']

export interface TransferDeps {
  ghost: Ref<GhostStatus | null>
  error: Ref<string>
  checkAchievements: () => Promise<void>
}

/** 灵魂传送：动画状态机 + 传送执行 + 结果展示 */
export function useTransfer(deps: TransferDeps) {
  const showTransfer = ref(false)
  const transferPhase = ref<'idle' | 'animating' | 'revealing' | 'done'>('idle')
  const transferResult = ref<TransferResult | null>(null)
  const transferAnimProgress = ref(0)
  let transferAnimInterval: ReturnType<typeof setInterval> | null = null

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
      deps.error.value = `灵魂传送失败: ${e}`
      transferPhase.value = 'idle'
      showTransfer.value = false
      return
    }

    // 传送已成功落盘。后续读取状态/成就即使失败也不再回退相位，
    // 否则会误报"传送失败"而实际新灵魂已经写入磁盘和内存。
    transferResult.value = result
    transferPhase.value = 'revealing'

    try {
      if (deps.ghost.value) {
        deps.ghost.value = JSON.parse(await invoke<string>('get_ghost_status'))
      }
    } catch (e) {
      console.warn('传送后读取 ghost 状态失败（不影响传送结果）:', e)
    }
    try {
      await deps.checkAchievements()
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

  function clearTransfer() {
    if (transferAnimInterval) {
      clearInterval(transferAnimInterval)
      transferAnimInterval = null
    }
  }

  return {
    showTransfer,
    transferPhase,
    transferResult,
    transferAnimProgress,
    transferParticles,
    startTransfer,
    finishTransfer,
    clearTransfer,
  }
}
