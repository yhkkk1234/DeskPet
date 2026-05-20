import { ref, computed, watch } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { LogicalPosition } from '@tauri-apps/api/dpi'

export type MoodState = 'love_high' | 'love_low' | 'neutral' | 'cold' | 'distant' | 'curious'
export type AnimationState = 'idle' | 'happy' | 'content' | 'curious' | 'cold' | 'distant' | 'speaking' | 'surprise' | 'blink' | 'dragged' | 'walk' | 'yawn' | 'sleep' | 'pout' | 'stretch'
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
}

export const STATE_TO_FPS: Record<AnimationState, number> = {
  idle: 4,
  happy: 6,
  content: 4,
  curious: 5,
  cold: 3,
  distant: 2,
  speaking: 6,
  surprise: 8,
  blink: 5,
  dragged: 6,
  walk: 8,
  yawn: 4,
  sleep: 3,
  pout: 4,
  stretch: 5,
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
  yawn: true,
  sleep: true,
  pout: false,
  stretch: false,
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
  bounce: 'happy',
  wave: 'content',
  look_around: 'curious',
  stretch: 'stretch',
  yawn: 'yawn',
  sleep: 'sleep',
  pout: 'pout',
  poke: 'curious',
  spin: 'happy',
  shiver: 'cold',
  wander: 'walk',
  face_left: 'idle',
  face_right: 'idle',
  teleport: 'surprise',
  peek: 'curious',
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

export function useAnimation() {
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

  function holdAnimation(state: AnimationState): () => void {
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

        win.outerPosition().then((pos) => {
          if (animationGen !== gen) { resolve(); return }

          const logicalX = pos.x / dpiScale
          const logicalY = pos.y / dpiScale
          const targetX = logicalX + direction * distance
          const screenW = window.screen.availWidth
          const screenH = window.screen.availHeight
          const winW = window.innerWidth
          const winH = window.innerHeight
          const clampedX = Math.max(0, Math.min(targetX, screenW - winW))
          const clampedY = Math.max(0, Math.min(logicalY, screenH - winH))

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

      const duration = action === 'sleep' ? 4000 : action === 'yawn' ? 3000 : action === 'pout' ? 2000 : action === 'spin' ? 1200 : 800
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

  function triggerBlink() {
    ++animationGen
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
    ++animationGen
    isPerformingBehavior.value = true
    currentAnimationState.value = 'speaking'
    stopDailyRoutine()
  }

  function stopSpeaking() {
    if (currentAnimationState.value === 'speaking') {
      currentAnimationState.value = 'idle'
      isPerformingBehavior.value = false
      startDailyRoutine()
    }
  }

  return {
    currentMood,
    currentBehavior,
    currentAnimationState,
    moodConfig,
    isMoving,
    isPerformingBehavior,
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
