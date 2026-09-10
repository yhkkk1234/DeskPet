import { ref, computed, watch } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { LogicalPosition } from '@tauri-apps/api/dpi'
import { clampToVirtualScreen } from './useScreenBounds'
import type { TagFrameRange } from './usePetRenderer'

export type MoodState = 'love_high' | 'love_low' | 'neutral' | 'cold' | 'distant' | 'curious'
export type AnimationState = 'idle' | 'happy' | 'content' | 'curious' | 'cold' | 'distant' | 'speaking' | 'surprise' | 'blink' | 'dragged' | 'walk' | 'yawn' | 'sleep' | 'pout' | 'stretch' | 'spin' | 'wave' | 'bounce' | 'poke' | 'shiver' | 'look_around'
export type DailyBehavior = 'bounce' | 'wave' | 'look_around' | 'stretch' | 'yawn' | 'sleep' | 'pout' | 'poke' | 'spin' | 'shiver' | 'wander' | 'face_left' | 'face_right' | 'teleport' | 'peek'
export type MovementStyle = 'bouncy' | 'slide' | 'float' | 'walk' | 'teleport'
export type FacingDirection = 'left' | 'right'

export interface MoodAnimationConfig {
  mood: MoodState
  animations: DailyBehavior[]
  idleFrequency: number
  expression: string
  bounceSpeed: number
  bounceHeight: number
  scaleRange: [number, number]
}

export const STATE_TO_CSS_CLASS: Record<AnimationState, string> = {
  idle: 'idle-bounce',
  happy: 'anim-happy',
  content: 'anim-content',
  curious: 'anim-curious',
  cold: 'anim-cold',
  distant: 'anim-distant',
  speaking: 'anim-speaking',
  surprise: 'anim-surprise',
  blink: 'anim-blink',
  dragged: 'anim-dragged',
  walk: 'anim-walk',
  yawn: 'anim-yawn',
  sleep: 'anim-sleep',
  pout: 'anim-pout',
  stretch: 'anim-stretch',
  // 新增专属动画 state：CSS 渲染器无对应关键帧，复用相近动作的样式
  spin: 'anim-happy',
  wave: 'anim-content',
  bounce: 'anim-happy',
  poke: 'anim-curious',
  shiver: 'anim-cold',
  look_around: 'anim-curious',
}

export const STATE_TO_SPRITE_ROW: Record<AnimationState, number> = {
  idle: 1,
  happy: 2,
  content: 3,
  curious: 4,
  cold: 5,
  distant: 6,
  speaking: 7,
  surprise: 8,
  blink: 0,
  dragged: 15,
  walk: 9,
  yawn: 10,
  sleep: 12,
  pout: 16,
  stretch: 11,
  // 新增专属动画 state：走 tag range 机制，行号仅在无 tag 时 fallback 使用
  spin: 2,
  wave: 3,
  bounce: 2,
  poke: 4,
  shiver: 5,
  look_around: 4,
}

// FPS 全部设 0 = 走 JSON duration 算平均 FPS（业内主流做法）。
// 注释里的数字是原硬编码值，在 Aseprite 调帧时长时可作参考目标。
// 调优工作流：在 Aseprite 里改帧时长 → 导出 JSON → 运行时自动生效，无需改代码。
export const STATE_TO_FPS: Record<AnimationState, number> = {
  idle: 0,       // 原 4
  happy: 0,      // 原 6
  content: 0,    // 原 4
  curious: 0,    // 原 5
  cold: 0,       // 原 3
  distant: 0,    // 原 2
  speaking: 0,   // 原 6
  surprise: 0,   // 原 8
  blink: 0,      // 原 5
  dragged: 0,    // 原 6
  walk: 0,       // 原 8
  yawn: 0,       // 原 4
  sleep: 0,      // 原 3
  pout: 0,       // 原 4
  stretch: 0,    // 原 5
  spin: 0,       // 原 0（新增，JSON 算出 11，建议在 Aseprite 调慢）
  wave: 0,       // 原 0（新增，JSON 算出 5）
  bounce: 0,     // 原 0（新增，10 帧素材 JSON 算出 9.7）
  poke: 0,       // 原 0（新增，JSON 算出 5）
  shiver: 0,     // 原 0（新增，JSON 算出 8，建议在 Aseprite 调慢）
  look_around: 0, // 原 0（新增，JSON 算出 7，建议在 Aseprite 调慢）
}

export const STATE_TO_LOOP: Record<AnimationState, boolean> = {
  idle: true,
  happy: true,
  content: true,
  curious: false,
  cold: true,
  distant: true,
  speaking: true,
  surprise: false,
  blink: false,
  dragged: true,
  walk: true,
  yawn: false,
  sleep: true,
  pout: false,
  stretch: false,
  // 新增专属动画 state
  spin: false,
  wave: true,
  // 单次弹跳（区别于 happy 循环）：保持循环，由 ACTION_CYCLES.bounce=2 决定播两遍，
  // 收尾时长按「2 遍素材 ÷ 播放速率」算，见 getActionDurationMs。
  bounce: true,
  poke: false,
  shiver: true,
  look_around: true,
}

const MOOD_CONFIGS: Record<MoodState, MoodAnimationConfig> = {
  love_high: {
    mood: 'love_high',
    animations: ['bounce', 'spin', 'wave', 'peek', 'wander'],
    idleFrequency: 3,
    expression: '😊',
    bounceSpeed: 1.8,
    bounceHeight: 6,
    scaleRange: [1.0, 1.08],
  },
  love_low: {
    mood: 'love_low',
    animations: ['wave', 'bounce', 'look_around', 'wander'],
    idleFrequency: 5,
    expression: '🙂',
    bounceSpeed: 2.2,
    bounceHeight: 4,
    scaleRange: [1.0, 1.04],
  },
  neutral: {
    mood: 'neutral',
    animations: ['look_around', 'stretch', 'face_left', 'face_right', 'wander'],
    idleFrequency: 8,
    expression: '😐',
    bounceSpeed: 2.5,
    bounceHeight: 2,
    scaleRange: [1.0, 1.02],
  },
  cold: {
    mood: 'cold',
    animations: ['shiver', 'pout', 'yawn', 'face_left'],
    idleFrequency: 12,
    expression: '😒',
    bounceSpeed: 3.0,
    bounceHeight: 1,
    scaleRange: [0.98, 1.0],
  },
  distant: {
    mood: 'distant',
    animations: ['yawn', 'sleep', 'face_left'],
    idleFrequency: 18,
    expression: '😤',
    bounceSpeed: 3.5,
    bounceHeight: 0,
    scaleRange: [0.97, 1.0],
  },
  curious: {
    mood: 'curious',
    animations: ['poke', 'look_around', 'bounce', 'peek', 'wander'],
    idleFrequency: 3,
    expression: '🤔',
    bounceSpeed: 1.5,
    bounceHeight: 5,
    scaleRange: [1.0, 1.06],
  },
}



function loveHateToMood(loveHate: number, curiosityLevel: string): MoodState {
  if (curiosityLevel === 'Enhanced' || curiosityLevel === 'Normal') {
    const curiosity = curiosityLevel === 'Enhanced' ? 0.3 : 0.1
    if (Math.random() < curiosity) return 'curious'
  }
  if (loveHate > 50) return 'love_high'
  if (loveHate > 20) return 'love_low'
  if (loveHate > -20) return 'neutral'
  if (loveHate > -50) return 'cold'
  return 'distant'
}

const BEHAVIOR_TO_ANIMATION: Record<DailyBehavior, AnimationState> = {
  bounce: 'bounce',
  wave: 'wave',
  look_around: 'look_around',
  stretch: 'stretch',
  yawn: 'yawn',
  sleep: 'sleep',
  pout: 'pout',
  poke: 'poke',
  spin: 'spin',
  shiver: 'shiver',
  wander: 'walk',
  face_left: 'idle',
  face_right: 'idle',
  teleport: 'surprise',
  peek: 'curious',
}

// 单次动作的「最短展示时长」（ms）。实际收尾时长完整公式见 getActionDurationMs：
//   max(本表值, 素材长度 × 循环遍数 ÷ 播放速率 + 缓冲)
// 素材长度由 public/pet/pet_spritesheet.json 的 tag 帧时长实时算出，
// 所以表里只需要写「素材播完后还想多保持一会儿姿势」的少数几个动作。
// 注意：本表只做「下限」，写小或干脆不写都不会截断素材——真正的截断来源是写死的上限。
const ACTION_DURATION_MIN_MS: Partial<Record<DailyBehavior, number>> = {
  pout: 2000, // 姿态动画：素材 1140ms，多停留一会儿更符合「不高兴」
  yawn: 3000, // 素材 590ms，打哈欠要慢
  sleep: 4000, // 素材 1280ms，睡眠保持
}

// 单次动作播放几遍素材。
// bounce = 2：素材本身是「大跳(腾空25px) + 小回弹(10px)」的完整两段式，播一遍约 1 秒
// 一闪而过；播两遍约 2 秒才有「蹦蹦跳跳」的重复感。每遍首尾都是站姿帧（84/93），
// 循环接缝不突兀。
// 代价：播放期间 isPerformingBehavior 锁住日常行为池，所以不宜超过 2~3 遍。
const ACTION_CYCLES: Partial<Record<DailyBehavior, number>> = {
  bounce: 2,
}

// 各意图动作的素材「基准播放速率」。bounce = 0.8：原速 1030ms/10帧 = 9.7fps，
// 单次跳跃仅 650ms，偏快偏「抽」；0.8× 后约 8fps、单跳 810ms，接近此类动画常见的
// 6~8fps 手感。再乘上由 moodConfig.bounceSpeed 推出的心情系数（见 computeBouncePlaybackRate），
// 最终倍率 = 基准 × 心情系数。会和 ACTION_CYCLES 一起参与收尾时长计算
// （见 getActionDurationMs），保证「放慢」不会反过来把素材截断。
const BASE_PLAYBACK_RATE: Partial<Record<DailyBehavior, number>> = {
  bounce: 0.8,
}

/** bounce 的基准速率（未注入 moodConfig 时的兜底），渲染端与时长计算共用 */
export const BOUNCE_BASE_PLAYBACK_RATE = BASE_PLAYBACK_RATE.bounce ?? 1

/**
 * bounce 的最终播放倍率 = 基准速率 × 心情系数。
 * 心情系数 = clamp(1.1 - bounceSpeed × 0.1, 0.6, 1.4)，以 bounceSpeed = 2.5（neutral）
 * 为 1.0 基准 → 高好感 0.92×（更欢快）、好奇 0.95×、疏远 0.75×（敷衍地快弹一下）。
 *
 * 做成导出的纯函数是为了让「时长计算」和「渲染帧率」永远用同一个公式，
 * 避免两处各算一遍再次漂移。
 */
export function computeBouncePlaybackRate(bounceSpeed: number | undefined): number {
  if (bounceSpeed === undefined) return BOUNCE_BASE_PLAYBACK_RATE
  const moodFactor = Math.min(1.4, Math.max(0.6, 1.1 - bounceSpeed * 0.1))
  return BOUNCE_BASE_PLAYBACK_RATE * moodFactor
}

const MOVEMENT_EASING: Record<MovementStyle, (t: number) => number> = {
  bouncy: (t) => {
    const c4 = (2 * Math.PI) / 3
    return t === 0 ? 0 : t === 1 ? 1 : -Math.pow(2, 10 * t - 10) * Math.sin((t * 10 - 10.75) * c4)
  },
  slide: (t) => 1 - Math.pow(1 - t, 3),
  float: (t) => t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2,
  walk: (t) => t,
  teleport: (_t) => 1,
}

function deriveMovementStyle(personality: { openness: number; conscientiousness: number; extraversion: number; agreeableness: number; neuroticism: number; creativity: number }): MovementStyle {
  const scores: [MovementStyle, number][] = [
    ['bouncy', personality.extraversion],
    ['slide', personality.agreeableness],
    ['float', personality.openness],
    ['walk', personality.conscientiousness],
    ['teleport', personality.creativity],
  ]
  scores.sort((a, b) => b[1] - a[1])
  return scores[0][0]
}

export function useAnimation(opts: {
  tagRanges?: { value: Map<string, TagFrameRange> }
  frameDurations?: { value: Map<number, number> }
} = {}) {
  const currentMood = ref<MoodState>('neutral')
  const currentBehavior = ref<DailyBehavior>('bounce')
  const currentAnimationState = ref<AnimationState>('idle')
  const isMoving = ref(false)
  const petX = ref(0)
  const petY = ref(0)
  const isPerformingBehavior = ref(false)
  const facingDirection = ref<FacingDirection>('right')
  const movementStyle = ref<MovementStyle>('slide')
  const isFlipped = computed(() => facingDirection.value === 'left')
  let animationGen = 0

  const moodConfig = computed(() => MOOD_CONFIGS[currentMood.value])

  function setPersonality(personality: { openness: number; conscientiousness: number; extraversion: number; agreeableness: number; neuroticism: number; creativity: number }) {
    movementStyle.value = deriveMovementStyle(personality)
  }

  function updateMood(loveHate: number, curiosityLevel: string) {
    currentMood.value = loveHateToMood(loveHate, curiosityLevel)
  }

  function setAnimationState(state: AnimationState) {
    currentAnimationState.value = state
  }

  /** 对话进行中（从首个 token 到 chat:complete）。用于让定时触发的动作给说话让路 */
  const isSpeakingActive = ref(false)

  // 说话的「最短展示时长」。LLM 首字节延迟长、吐字快，一轮回复的流式窗口经常只有 1 秒出头，
  // 而 Speak 素材一个循环是 820ms（4 帧，内含 2 次张嘴）—— 结束太快时宠物只「说」了两下嘴
  // 就闭嘴，看起来像动作没播完。这里给一个下限，让观感上真的像在说话。
  const MIN_SPEAKING_MS = 2500
  let speakingStartedAt = 0
  let speakTailTimer: ReturnType<typeof setTimeout> | null = null

  function clearSpeakTail() {
    if (speakTailTimer) {
      clearTimeout(speakTailTimer)
      speakTailTimer = null
    }
  }

  function holdAnimation(state: AnimationState): () => void {
    // 说话期间不让位：holdAnimation 会无条件覆盖 currentAnimationState，
    // 若与说话抢状态，stopSpeaking 的收尾逻辑会被跳过（见 stopSpeaking 注释）
    if (isSpeakingActive.value) return () => {}
    const gen = ++animationGen
    isPerformingBehavior.value = true
    currentAnimationState.value = state
    stopDailyRoutine()
    return () => {
      if (animationGen === gen) {
        currentAnimationState.value = 'idle'
        isPerformingBehavior.value = false
      }
      startDailyRoutine()
    }
  }

  function playOneShot(state: AnimationState, durationMs = 600): Promise<void> {
    // 说话优先于定时触发的动作（tickGhost 每 5 秒会调 playOneShot('curious') 等）。
    // 不加这道守卫的话，回复只要超过 5 秒，说话姿势就会被当场换掉；
    // 更糟的是状态被换走后 stopSpeaking 的守卫失效，isPerformingBehavior 会永久卡住。
    if (isSpeakingActive.value) return Promise.resolve()
    const gen = ++animationGen
    isPerformingBehavior.value = true
    currentAnimationState.value = state
    stopDailyRoutine()
    return new Promise((resolve) => {
      setTimeout(() => {
        if (animationGen === gen) {
          currentAnimationState.value = 'idle'
          isPerformingBehavior.value = false
          startDailyRoutine()
        }
        resolve()
      }, durationMs)
    })
  }

  let dailyTimer: ReturnType<typeof setTimeout> | null = null

  const DEFAULT_ACTION_MS = 800

  /**
   * 动作收尾时长 = max(最短展示时长, 素材实际长度 × 循环遍数 ÷ 播放速率 + 缓冲)。
   *
   * 全部按素材实时算，而不是写死：此前 bounce 吃默认 800ms，而 Bounce tag 已从
   * 7 帧扩到 10 帧（84-93，1030ms），末 2 帧落地/回弹永远播不到。素材再改时长
   * （改 Aseprite 帧时长后重导 JSON）这里会自动跟随，不会重现同类截断。
   *
   * 节奏相关量（循环遍数、放慢倍率）也必须进这个公式：只放慢渲染而不放大时长，
   * 等于换个方式重新截断素材。
   */
  function getActionDurationMs(action: DailyBehavior, playbackRate = 1): number {
    const state = BEHAVIOR_TO_ANIMATION[action]
    const range = opts.tagRanges?.value.get(state)
    let tagMs = 0
    if (range) {
      for (let i = range.from; i <= range.to; i++) {
        tagMs += opts.frameDurations?.value.get(i) ?? 0
      }
    }
    const cycles = ACTION_CYCLES[action] ?? 1
    const rate = playbackRate > 0 ? playbackRate : 1
    const min = ACTION_DURATION_MIN_MS[action] ?? 0
    // +60ms 缓冲：避免定时器比渲染器早一帧抢跑，导致末帧被吞
    return Math.max(min, tagMs > 0 ? (tagMs * cycles) / rate + 60 : DEFAULT_ACTION_MS)
  }

  /**
   * 动作的素材播放速率。bounce 由心情驱动（moodConfig.bounceSpeed），
   * 其余动作保持原速，避免顺手改掉现有观感。
   */
  function resolvePlaybackRate(action: DailyBehavior): number {
    if (action === 'bounce') return computeBouncePlaybackRate(moodConfig.value?.bounceSpeed)
    return BASE_PLAYBACK_RATE[action] ?? 1
  }

  function executeAction(action: DailyBehavior): Promise<void> {
    const gen = ++animationGen
    return new Promise((resolve) => {
      currentBehavior.value = action

      if (action === 'face_left') {
        facingDirection.value = 'left'
        currentAnimationState.value = BEHAVIOR_TO_ANIMATION[action]
        isPerformingBehavior.value = true
        setTimeout(() => {
          if (animationGen === gen) {
            isPerformingBehavior.value = false
          }
          resolve()
        }, 500)
        return
      }
      if (action === 'face_right') {
        facingDirection.value = 'right'
        currentAnimationState.value = BEHAVIOR_TO_ANIMATION[action]
        isPerformingBehavior.value = true
        setTimeout(() => {
          if (animationGen === gen) {
            isPerformingBehavior.value = false
          }
          resolve()
        }, 500)
        return
      }
      if (action === 'wander') {
        const direction = Math.random() < 0.5 ? -1 : 1
        facingDirection.value = direction < 0 ? 'left' : 'right'
        const distance = 60 + Math.random() * 120
        isPerformingBehavior.value = true

        const dpiScale = window.devicePixelRatio || 1
        const win = getCurrentWindow()

        win.outerPosition().then(async (pos) => {
          if (animationGen !== gen) { resolve(); return }

          const logicalX = pos.x / dpiScale
          const logicalY = pos.y / dpiScale
          const targetX = logicalX + direction * distance
          const winW = window.innerWidth
          const winH = window.innerHeight

          // 用虚拟屏幕（所有显示器合集）边界夹紧，替代 window.screen（仅主屏）
          const clamped = await clampToVirtualScreen(targetX, logicalY, winW, winH, dpiScale)
          const clampedX = clamped.x
          const clampedY = clamped.y

          if (movementStyle.value === 'teleport') {
            currentAnimationState.value = 'surprise'
            setTimeout(() => {
              if (animationGen !== gen) { resolve(); return }
              win.setPosition(new LogicalPosition(Math.round(clampedX), Math.round(clampedY)))
              facingDirection.value = Math.random() < 0.5 ? 'left' : 'right'
              setTimeout(() => {
                if (animationGen === gen) {
                  currentAnimationState.value = 'idle'
                  isPerformingBehavior.value = false
                }
                resolve()
              }, 300)
            }, 400)
          } else {
            currentAnimationState.value = 'walk'
            const startTime = performance.now()
            const walkDuration = 1200
            const startX = logicalX
            const startY = logicalY
            const dx = clampedX - startX
            const dy = clampedY - startY
            const easingFn = MOVEMENT_EASING[movementStyle.value]
            let lastSetPos = 0

            function animateWindow(now: number) {
              const elapsed = now - startTime
              const progress = Math.min(1, elapsed / walkDuration)
              const t = easingFn(progress)

              if (now - lastSetPos > 30 || progress >= 1) {
                const cx = startX + dx * t
                const cy = startY + dy * t
                win.setPosition(new LogicalPosition(Math.round(cx), Math.round(cy)))
                lastSetPos = now
              }

              if (progress < 1) {
                requestAnimationFrame(animateWindow)
              } else {
                if (animationGen === gen) {
                  facingDirection.value = Math.random() < 0.5 ? 'left' : 'right'
                  currentAnimationState.value = 'idle'
                  isPerformingBehavior.value = false
                }
                resolve()
              }
            }
            requestAnimationFrame(animateWindow)
          }
        }).catch(() => {
          if (animationGen !== gen) { resolve(); return }
          currentAnimationState.value = 'walk'
          setTimeout(() => {
            if (animationGen === gen) {
              currentAnimationState.value = 'idle'
              isPerformingBehavior.value = false
            }
            resolve()
          }, 1200)
        })
        return
      }
      if (action === 'teleport') {
        const tx = (Math.random() - 0.5) * window.innerWidth * 0.4
        const ty = (Math.random() - 0.5) * 20
        isPerformingBehavior.value = true
        currentAnimationState.value = 'surprise'
        setTimeout(() => {
          if (animationGen !== gen) { resolve(); return }
          petX.value = tx
          petY.value = ty
          facingDirection.value = Math.random() < 0.5 ? 'left' : 'right'
          setTimeout(() => {
            if (animationGen === gen) {
              isPerformingBehavior.value = false
            }
            resolve()
          }, 300)
        }, 400)
        return
      }
      if (action === 'peek') {
        const prevFacing = facingDirection.value
        facingDirection.value = Math.random() < 0.5 ? 'left' : 'right'
        currentAnimationState.value = 'curious'
        isPerformingBehavior.value = true
        setTimeout(() => {
          if (animationGen === gen) {
            facingDirection.value = prevFacing
            currentAnimationState.value = 'idle'
            isPerformingBehavior.value = false
          }
          resolve()
        }, 1200)
        return
      }

      currentAnimationState.value = BEHAVIOR_TO_ANIMATION[action]
      isPerformingBehavior.value = true

      const duration = getActionDurationMs(action, resolvePlaybackRate(action))
      setTimeout(() => {
        if (animationGen === gen) {
          currentAnimationState.value = 'idle'
          isPerformingBehavior.value = false
        }
        resolve()
      }, duration)
    })
  }

  function triggerDailyBehavior() {
    if (isPerformingBehavior.value || isMoving.value) return
    const config = moodConfig.value
    if (!config) return

    const actions = config.animations
    const action = actions[Math.floor(Math.random() * actions.length)]
    executeAction(action)
  }

  function scheduleNextDaily() {
    if (dailyTimer) clearTimeout(dailyTimer)
    const config = moodConfig.value
    if (!config) return

    const baseInterval = 60000 / config.idleFrequency
    const variance = baseInterval * 0.4
    const interval = baseInterval + (Math.random() * 2 - 1) * variance

    dailyTimer = setTimeout(() => {
      triggerDailyBehavior()
      scheduleNextDaily()
    }, interval)
  }

  function startDailyRoutine() {
    stopDailyRoutine()
    scheduleNextDaily()
  }

  function stopDailyRoutine() {
    if (dailyTimer) {
      clearTimeout(dailyTimer)
      dailyTimer = null
    }
  }

  let blinkTimer: ReturnType<typeof setTimeout> | null = null

  function scheduleBlink() {
    stopBlink()
    const minDelay = 3000
    const maxDelay = 8000
    const delay = minDelay + Math.random() * (maxDelay - minDelay)
    blinkTimer = setTimeout(() => {
      if (currentAnimationState.value === 'idle' && !isPerformingBehavior.value && !isMoving.value) {
        triggerBlink()
      }
      scheduleBlink()
    }, delay)
  }

  function stopBlink() {
    if (blinkTimer) {
      clearTimeout(blinkTimer)
      blinkTimer = null
    }
  }

  /**
   * 让「当前这一代」动画失效：代际 +1 后，所有在飞的 setTimeout / release 回调
   * 都会因 `animationGen === gen` 失配而跳过自己的收尾逻辑。
   *
   * 注意这里**故意不复位 isPerformingBehavior**：该标志表达的不是「有没有回调在飞」，
   * 而是「宠物现在该不该被日常行为池打扰」。抢占方（startSpeaking）紧接着就会把它
   * 置为 true 来锁住行为池；若在这里清成 false，被抢占的动作虽已不再复位状态，
   * 行为池却会被解锁，日常行为就能在说话/跳跃中途插进来把动作掐断。
   */
  function cancelCurrent() {
    ++animationGen
  }

  function triggerBlink() {
    // 排期到触发之间状态可能已经变了（拖拽、日常行为、说话）。眨眼是纯装饰性动作，
    // 不该打断任何正在进行的动作——这里二次校验，不满足就放弃这一次眨眼
    // （日常调度仍在跑，下一个周期会重新眨）。
    if (currentAnimationState.value !== 'idle' || isPerformingBehavior.value || isMoving.value) return
    cancelCurrent()
    currentAnimationState.value = 'blink'
  }

  function onAnimationComplete() {
    if (currentAnimationState.value === 'blink') {
      currentAnimationState.value = 'idle'
    }
  }

  watch(currentAnimationState, (newState) => {
    if (newState === 'idle') {
      scheduleBlink()
    } else {
      stopBlink()
    }
  })

  function movePetTo(targetX: number, targetY: number): Promise<void> {
    return new Promise((resolve) => {
      isMoving.value = true
      const startX = petX.value
      const startY = petY.value
      const dx = targetX - startX
      const dy = targetY - startY
      const dist = Math.sqrt(dx * dx + dy * dy)

      if (dist < 2) {
        isMoving.value = false
        resolve()
        return
      }

      const style = movementStyle.value
      const baseDuration = style === 'teleport' ? 0 : Math.min(1200, Math.max(600, dist * 5))
      const easingFn = MOVEMENT_EASING[style]

      if (baseDuration === 0) {
        petX.value = targetX
        petY.value = targetY
        isMoving.value = false
        resolve()
        return
      }

      if (dx !== 0) {
        facingDirection.value = dx > 0 ? 'right' : 'left'
      }

      const startTime = performance.now()
      function animate(now: number) {
        const elapsed = now - startTime
        const progress = Math.min(1, elapsed / baseDuration)
        const eased = easingFn(progress)

        petX.value = startX + dx * eased
        petY.value = startY + dy * eased

        if (progress < 1) {
          requestAnimationFrame(animate)
        } else {
          petX.value = targetX
          petY.value = targetY
          isMoving.value = false
          resolve()
        }
      }
      requestAnimationFrame(animate)
    })
  }

  function resetPosition() {
    petX.value = 0
    petY.value = 0
    isMoving.value = false
  }

  const EMOTION_REACTION_MAP: Record<string, { state: AnimationState; duration: number }> = {
    UserPraisedPet: { state: 'happy', duration: 1500 },
    UserCelebratedTogether: { state: 'happy', duration: 2000 },
    UserCaredAboutPet: { state: 'content', duration: 1200 },
    UserSharedPersonalStory: { state: 'content', duration: 1200 },
    UserGotAngry: { state: 'cold', duration: 1500 },
    UserDismissedPet: { state: 'distant', duration: 1500 },
    UserIgnoredPet: { state: 'distant', duration: 1200 },
    NormalChat: { state: 'content', duration: 800 },
    FirstConversation: { state: 'happy', duration: 2000 },
    BirthdayCelebrated: { state: 'happy', duration: 2500 },
    CuriosityTriggered: { state: 'curious', duration: 2000 },
  }

  function playEmotionReaction(eventType: string, loveHateHint: number): Promise<void> {
    const reaction = EMOTION_REACTION_MAP[eventType] || (loveHateHint > 1 
      ? { state: 'happy' as AnimationState, duration: 1200 }
      : loveHateHint < -1
        ? { state: 'cold' as AnimationState, duration: 1000 }
        : { state: 'content' as AnimationState, duration: 800 }
    )
    return playOneShot(reaction.state, reaction.duration)
  }

  function startSpeaking() {
    isSpeakingActive.value = true
    speakingStartedAt = performance.now()
    clearSpeakTail()
    cancelCurrent()
    isPerformingBehavior.value = true
    currentAnimationState.value = 'speaking'
    stopDailyRoutine()
  }

  /**
   * 结束说话。
   *
   * 这里**不能**用 `currentAnimationState === 'speaking'` 作为守卫——那曾导致一个
   * 永久卡死的 bug：一旦说话期间状态被别的路径改掉（拖拽、tick 触发的 playOneShot 等），
   * 守卫为假 → isPerformingBehavior 永远停在 true、日常行为调度器永不重启，
   * 宠物从此除了眨眼不再做任何日常动作（startSpeaking 里的 stopDailyRoutine 停掉了它）。
   *
   * 改为「会话驱动」：只要还在说话会话里，就一定要释放标志并恢复调度；
   * 视觉状态只在「当前确实停在 speaking」时回 idle，避免踩掉拖拽或睡眠等持续状态。
   */
  function stopSpeaking() {
    if (!isSpeakingActive.value) return
    isSpeakingActive.value = false

    // 会话立刻结束（isSpeakingActive 立即置 false，让日常行为能正常恢复），
    // 但视觉上补足最短展示时长：只在「仍停在 speaking」时才延时回 idle，
    // 期间若别的动作（拖拽/日常行为）接管了状态，回调会因状态不符而自动放弃。
    const elapsed = performance.now() - speakingStartedAt
    const tail = MIN_SPEAKING_MS - elapsed
    if (tail > 0 && currentAnimationState.value === 'speaking') {
      clearSpeakTail()
      speakTailTimer = setTimeout(() => {
        speakTailTimer = null
        if (currentAnimationState.value === 'speaking') {
          currentAnimationState.value = 'idle'
        }
      }, tail)
    } else if (currentAnimationState.value === 'speaking') {
      currentAnimationState.value = 'idle'
    }

    isPerformingBehavior.value = false
    startDailyRoutine()
  }

  return {
    currentMood,
    currentBehavior,
    currentAnimationState,
    moodConfig,
    isMoving,
    isPerformingBehavior,
    isSpeakingActive,
    petX,
    petY,
    facingDirection,
    isFlipped,
    movementStyle,
    updateMood,
    setAnimationState,
    holdAnimation,
    setPersonality,
    playOneShot,
    startSpeaking,
    stopSpeaking,
    playEmotionReaction,
    triggerDailyBehavior,
    startDailyRoutine,
    stopDailyRoutine,
    stopBlink,
    movePetTo,
    resetPosition,
    onAnimationComplete,
  }
}
