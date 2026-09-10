<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, computed, nextTick } from 'vue'
import type { AnimationState } from '../composables/useAnimation'
import { STATE_TO_SPRITE_ROW, STATE_TO_FPS, STATE_TO_LOOP } from '../composables/useAnimation'
import type { SpriteConfig, TagFrameRange, FramePosition, HeadConfig } from '../composables/usePetRenderer'
import type { HeadDirection } from '../composables/useMouseTracking'

const props = defineProps<{
  animationState: AnimationState
  config: SpriteConfig
  /** 素材播放速率倍率（1 = 按 JSON 帧时长原速，<1 放慢）。由 moodConfig.bounceSpeed 驱动 */
  playbackRate: number
  tagRanges: Map<string, TagFrameRange>
  frameDurations: Map<number, number>
  framePositions: Map<number, FramePosition>
  styleOverride: Record<string, string>
  headDirection: HeadDirection
  headEnabled: boolean
  headConfig: HeadConfig
}>()

const emit = defineEmits<{
  animationComplete: []
}>()

// 9 宫格方向索引（与 headConfig.cols/rows 布局约定对应）
const DIRECTION_INDEX: Record<HeadDirection, number> = {
  'up-left': 0,
  up: 1,
  'up-right': 2,
  left: 3,
  center: 4,
  right: 5,
  'down-left': 6,
  down: 7,
  'down-right': 8,
}

const canvas = ref<HTMLCanvasElement | null>(null)
const image = ref<HTMLImageElement | null>(null)
const imageLoaded = ref(false)
const loading = ref(false)

let currentFrame = 0
let lastTimestamp = 0
let frameTimer = 0
let animFrameId: number | null = null
let transitioning = false
let transitionAlpha = 1
let previousSnapshot: HTMLCanvasElement | null = null
const CROSSFADE_MS = 0

function getTagRange(): TagFrameRange | null {
  return props.tagRanges.get(props.animationState) ?? null
}

function getFps(): number {
  const override = STATE_TO_FPS[props.animationState]
  // override 为 0 表示该 state 优先用 JSON duration 算 FPS（支持逐动画自定义速度）
  if (override !== undefined && override > 0) return override

  const rate = props.playbackRate > 0 ? props.playbackRate : 1
  const range = getTagRange()
  if (!range) return override !== undefined && override > 0 ? override : 4

  let sumMs = 0
  let count = 0
  for (let i = range.from; i <= range.to; i++) {
    const d = props.frameDurations.get(i)
    if (d !== undefined) {
      sumMs += d
      count++
    }
  }
  if (count > 0 && sumMs > 0) {
    const avgFps = 1000 / (sumMs / count)
    // 变速路径保持小数：rate<1 时 Math.round(avgFps) 的量化误差最大可达 6%，
    // 会让 useAnimation 按精确倍率算出的收尾时长与渲染端对不上，末帧被吞
    // （实测 curious 只能播 1.94 遍）。rate === 1 时仍走原来的 Math.round，
    // 原速动作的观感与本次改动前完全一致。
    return rate === 1 ? Math.round(avgFps) : avgFps * rate
  }
  return 4
}

function isLoop(): boolean {
  return STATE_TO_LOOP[props.animationState] ?? true
}

function getFrameDurationMs(): number {
  const fps = getFps()
  return fps > 0 ? 1000 / fps : 250
}

function getFrameCount(): number {
  const range = getTagRange()
  if (range) return range.frameCount
  return props.config.cols
}

function getSpriteX(_frameIndex: number): number {
  const range = getTagRange()
  if (range) {
    const absoluteIndex = range.from + currentFrame
    const pos = props.framePositions.get(absoluteIndex)
    if (pos) return pos.x
  }
  return (currentFrame % props.config.cols) * props.config.frameWidth
}

function getSpriteY(_frameIndex: number): number {
  const range = getTagRange()
  if (range) {
    const absoluteIndex = range.from + currentFrame
    const pos = props.framePositions.get(absoluteIndex)
    if (pos) return pos.y
  }
  const row = STATE_TO_SPRITE_ROW[props.animationState] ?? 0
  return row * props.config.frameHeight
}

// ===== 呼吸浮动检测 =====
// Idle/Blink 帧中"闭眼帧"会整体下移几个像素（呼吸感），头部叠层和挖洞
// 矩形必须跟随该偏移，否则浮动帧与叠层错位。这里自动检测相对 idle 首帧
// 的最佳平移（dx,dy ∈ [-4,4]），换素材后无需改代码。
let headShiftMap: Map<number, { x: number; y: number }> | null = null

function getFrameAlpha(idx: number, ctx: CanvasRenderingContext2D): Uint8ClampedArray | null {
  const fw = props.config.frameWidth
  const fh = props.config.frameHeight
  const pos = props.framePositions.get(idx)
  if (!pos) return null
  ctx.clearRect(0, 0, fw, fh)
  ctx.drawImage(image.value!, pos.x, pos.y, fw, fh, 0, 0, fw, fh)
  const data = ctx.getImageData(0, 0, fw, fh).data
  const alpha = new Uint8ClampedArray(fw * fh)
  for (let i = 0; i < alpha.length; i++) alpha[i] = data[i * 4 + 3]
  return alpha
}

function diffAlphaShift(a: Uint8ClampedArray, b: Uint8ClampedArray, dx: number, dy: number): number {
  const fw = props.config.frameWidth
  const fh = props.config.frameHeight
  let d = 0
  for (let y = 0; y < fh; y++) {
    for (let x = 0; x < fw; x++) {
      const aOn = a[y * fw + x] > 10
      const nx = x - dx
      const ny = y - dy
      const bOn = nx >= 0 && ny >= 0 && nx < fw && ny < fh && b[ny * fw + nx] > 10
      if (aOn !== bOn) d++
    }
  }
  return d
}

function detectHeadShifts() {
  if (!image.value) return
  const base = props.tagRanges.get('idle')
  if (!base) return
  const blink = props.tagRanges.get('blink')
  const fw = props.config.frameWidth
  const fh = props.config.frameHeight
  const off = document.createElement('canvas')
  off.width = fw
  off.height = fh
  const ctx = off.getContext('2d', { willReadFrequently: true })
  if (!ctx) return

  const baseAlpha = getFrameAlpha(base.from, ctx)
  if (!baseAlpha) return
  const map = new Map<number, { x: number; y: number }>()
  map.set(base.from, { x: 0, y: 0 })

  const idxs: number[] = []
  for (const r of [base, blink]) {
    if (r) for (let i = r.from; i <= r.to; i++) idxs.push(i)
  }
  for (const idx of idxs) {
    if (idx === base.from) continue
    const alpha = getFrameAlpha(idx, ctx)
    if (!alpha) continue
    let best = { x: 0, y: 0, diff: Infinity }
    for (let dy = -4; dy <= 4; dy++) {
      for (let dx = -4; dx <= 4; dx++) {
        const diff = diffAlphaShift(baseAlpha, alpha, dx, dy)
        if (diff < best.diff) best = { x: dx, y: dy, diff }
      }
    }
    // 平移后仍差异过大（>5%）视为无浮动，保持原位。
    // 注意：diffAlphaShift 的匹配方向与"帧相对基准的位移"反号，
    // 存储时取反，使 map 值 = 该帧相对基准的位移（供叠层/挖洞直接跟随）。
    if (best.diff <= baseAlpha.length * 0.05) map.set(idx, { x: -best.x, y: -best.y })
    else map.set(idx, { x: 0, y: 0 })
  }
  headShiftMap = map
  // 检测结果仅在开发期有用（且渲染循环每秒兜底重试时会重复触发），
  // 生产包里不该刷控制台，故用 DEV 护栏而非裸 console.log。
  if (import.meta.env.DEV) {
    console.log('[headShift] 呼吸浮动检测结果:', Object.fromEntries(map))
  }
}

function loadSprite() {
  if (!props.config.src) return
  loading.value = true

  const onSpriteLoaded = (img: HTMLImageElement) => {
    image.value = img
    imageLoaded.value = true
    loading.value = false
    detectHeadShiftsSafely()
    nextTick(() => {
      startRenderLoop()
    })
  }

  // 回退：直接 Image 加载（资源无 CORS 头时使用，画面正常但浮动检测会因
  // canvas 跨域污染而失败，已由 try/catch 兜底降级为不跟随）
  const loadDirect = () => {
    const img = new Image()
    img.onload = () => onSpriteLoaded(img)
    img.onerror = () => {
      loading.value = false
      console.error('Failed to load sprite:', props.config.src)
    }
    img.src = props.config.src
  }

  // 优先 fetch → blob URL：blob 视为同源数据，canvas 不会被跨域污染，
  // 浮动检测（getImageData）可正常读取像素。Tauri 生产环境资源经 asset
  // 协议跨域加载时，直接 Image + getImageData 会抛 SecurityError。
  fetch(props.config.src)
    .then((res) => {
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      return res.blob()
    })
    .then((blob) => {
      const url = URL.createObjectURL(blob)
      const img = new Image()
      img.onload = () => {
        URL.revokeObjectURL(url)
        onSpriteLoaded(img)
      }
      img.onerror = () => {
        URL.revokeObjectURL(url)
        loadDirect()
      }
      img.src = url
    })
    .catch(() => loadDirect())
}

// 浮动检测依赖 framePositions（由 Aseprite JSON 解析而来），而图片加载与
// JSON 解析是两条独立异步链：图片先加载完时 JSON 可能未就绪，检测会静默跳过。
// JSON（tagRanges/framePositions 引用替换）就绪后重试，确保检测最终执行。
function detectHeadShiftsSafely() {
  try {
    detectHeadShifts()
  } catch (e) {
    console.warn('呼吸浮动检测失败:', e)
    headShiftMap = null
  }
}

watch(() => props.tagRanges, () => {
  if (image.value) detectHeadShiftsSafely()
})

function startRenderLoop() {
  if (animFrameId !== null) return
  lastTimestamp = performance.now()
  frameTimer = 0
  renderLoop(lastTimestamp)
}

function renderLoop(timestamp: number) {
  if (!canvas.value || !image.value) {
    animFrameId = null
    return
  }

  const ctx = canvas.value.getContext('2d')
  if (!ctx) {
    animFrameId = null
    return
  }

  const dt = Math.min(timestamp - lastTimestamp, 100)
  lastTimestamp = timestamp
  const isLooping = isLoop()
  const frameCount = getFrameCount()

  // 呼吸浮动检测兜底重试（每秒一次，直至成功）
  if (!headShiftMap && performance.now() - lastShiftAttempt > 1000) {
    lastShiftAttempt = performance.now()
    detectHeadShiftsSafely()
  }

  frameTimer += dt

  const frameDurationMs = getFrameDurationMs()

  if (frameTimer >= frameDurationMs) {
    const skippedFrames = Math.floor(frameTimer / frameDurationMs)
    currentFrame += Math.min(skippedFrames, 3)

    if (isLooping) {
      currentFrame %= frameCount
    } else {
      if (currentFrame >= frameCount) {
        currentFrame = frameCount - 1
        emit('animationComplete')
      }
    }

    frameTimer = frameTimer % frameDurationMs
  }

  const cw = canvas.value.width
  const ch = canvas.value.height
  ctx.clearRect(0, 0, cw, ch)

  const sx = getSpriteX(currentFrame)
  const sy = getSpriteY(currentFrame)
  const fw = props.config.frameWidth
  const fh = props.config.frameHeight

  ctx.imageSmoothingEnabled = false

  if (transitioning && previousSnapshot) {
    ctx.globalAlpha = transitionAlpha
    ctx.drawImage(previousSnapshot, 0, 0, cw, ch)
    ctx.globalAlpha = 1 - transitionAlpha
    ctx.drawImage(image.value, sx, sy, fw, fh, 0, 0, cw, ch)
    ctx.globalAlpha = 1
  } else {
    ctx.drawImage(image.value, sx, sy, fw, fh, 0, 0, cw, ch)
  }

  // 头部叠层：挖洞 + 绘制叠层在同一 canvas 同帧完成，与身体帧共享
  // image-rendering: pixelated（DOM 背景图不支持像素化渲染，会导致模糊），
  // 且不存在跨层同步问题（白线/空窗/错位全部消除）。
  // transition 过渡帧直接显示完整快照（旧姿势），此时挖洞/叠层会与快照
  // 姿势错位（如 curious 歪头姿势被挖洞+正脸头）产生怪异/空白帧，故跳过。
  if (!transitioning && shouldShowHead() && props.headConfig.headSlot && headImage.value) {
    const shift = getHeadShift()
    const slot = props.headConfig.headSlot
    ctx.globalCompositeOperation = 'destination-out'
    ctx.fillRect(
      ((slot.x + shift.x) / fw) * cw,
      ((slot.y + shift.y) / fh) * ch,
      (slot.w / fw) * cw,
      (slot.h / fh) * ch,
    )
    ctx.globalCompositeOperation = 'source-over'

    const hfw = props.headConfig.frameWidth
    const hfh = props.headConfig.frameHeight
    const idx = DIRECTION_INDEX[props.headDirection]
    const hcol = idx % props.headConfig.cols
    const hrow = Math.floor(idx / props.headConfig.cols)
    ctx.imageSmoothingEnabled = false
    ctx.drawImage(
      headImage.value,
      hcol * hfw, hrow * hfh, hfw, hfh,
      ((props.headConfig.offsetX + shift.x) / fw) * cw,
      ((props.headConfig.offsetY + shift.y) / fh) * ch,
      (hfw / fw) * cw,
      (hfh / fh) * ch,
    )
  }

  if (transitioning) {
    transitionAlpha -= dt / CROSSFADE_MS
    if (transitionAlpha <= 0) {
      transitioning = false
      previousSnapshot = null
      transitionAlpha = 1
    }
  }

  animFrameId = requestAnimationFrame(renderLoop)
}

let previousState: AnimationState | null = null

watch(() => props.animationState, (newState) => {
  if (previousState !== null && previousState !== newState) {
    crossfadeToState(newState)
  }
  previousState = newState
})

function crossfadeToState(_state: AnimationState) {
  if (!canvas.value || !image.value) return

  const snapshotCanvas = document.createElement('canvas')
  snapshotCanvas.width = canvas.value.width
  snapshotCanvas.height = canvas.value.height
  const snapCtx = snapshotCanvas.getContext('2d')
  if (snapCtx) {
    snapCtx.drawImage(canvas.value, 0, 0)
  }
  previousSnapshot = snapshotCanvas

  transitioning = true
  transitionAlpha = 1
  currentFrame = 0
  // 重置帧计时，避免旧状态的帧计时残留导致新状态首帧被跳过（跳帧）
  frameTimer = 0
}

onMounted(() => {
  loadHeadImage()
  if (props.config.src) {
    loadSprite()
  }
})

onUnmounted(() => {
  if (animFrameId !== null) {
    cancelAnimationFrame(animFrameId)
    animFrameId = null
  }
})

watch(() => props.config.src, () => {
  if (animFrameId !== null) {
    cancelAnimationFrame(animFrameId)
    animFrameId = null
  }
  imageLoaded.value = false
  loading.value = false
  currentFrame = 0
  if (props.config.src) {
    loadSprite()
  }
})

watch(() => props.headConfig.src, () => {
  loadHeadImage()
})

const canvasWidth = computed(() => props.config.frameWidth * props.config.scale)
const canvasHeight = computed(() => props.config.frameHeight * props.config.scale)

// wrapper 显示尺寸：跟随 canvas 尺寸，但限制在窗口余量内（400px 窗口留 20px 边距）。
// 大图（如 500×500）自动缩到 380px 显示，不超出窗口；小图（128×128）不受限。
const MAX_DISPLAY = 380
const wrapperStyle = computed(() => {
  const w = Math.min(canvasWidth.value, MAX_DISPLAY)
  const h = Math.min(canvasHeight.value, MAX_DISPLAY)
  return {
    ...props.styleOverride,
    width: w + 'px',
    height: h + 'px',
  }
})

// 头部叠层激活条件：仅 IDLE 状态（含正转头时保持方向的眨眼瞬间）。
// blink 且头部正偏离时保持叠层显示，避免头部方向跳变；blink 且朝正面时
// 隐藏叠层露出原始闭眼帧，保留眨眼动画。
function shouldShowHead(): boolean {
  if (!props.headEnabled || !props.headConfig.src) return false
  return props.animationState === 'idle'
    || (props.animationState === 'blink' && props.headDirection !== 'center')
}

// 头部素材（9 宫格），fetch→blob 加载（与 sprite 相同策略，避免跨域污染）
const headImage = ref<HTMLImageElement | null>(null)

function loadHeadImage() {
  if (!props.headConfig.src) {
    headImage.value = null
    return
  }
  fetch(props.headConfig.src)
    .then((res) => {
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      return res.blob()
    })
    .then((blob) => {
      const url = URL.createObjectURL(blob)
      const img = new Image()
      img.onload = () => {
        URL.revokeObjectURL(url)
        headImage.value = img
      }
      img.onerror = () => {
        URL.revokeObjectURL(url)
        headImage.value = null
      }
      img.src = url
    })
    .catch(() => {
      headImage.value = null
    })
}

// 初始检测可能因 JSON/图片时序未就绪而失败，渲染循环里每秒兜底重试，
// 直到检测成功一次。
let lastShiftAttempt = 0

function getHeadShift(): { x: number; y: number } {
  const range = getTagRange()
  if (!range || !headShiftMap) return { x: 0, y: 0 }
  return headShiftMap.get(range.from + currentFrame) ?? { x: 0, y: 0 }
}
</script>

<template>
  <div class="pet-sprite-canvas-wrapper" :style="wrapperStyle">
    <canvas
      v-if="imageLoaded"
      ref="canvas"
      :width="canvasWidth"
      :height="canvasHeight"
      class="pet-sprite-canvas"
    />
    <div v-else-if="loading" class="pet-sprite-loading">
      <div class="sprite-loading-spinner"></div>
    </div>
    <div v-else class="pet-sprite-placeholder">?</div>
  </div>
</template>

<style scoped>
.pet-sprite-canvas-wrapper {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  overflow: hidden;
}

.pet-sprite-canvas {
  image-rendering: pixelated;
  image-rendering: crisp-edges;
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
}

.pet-sprite-placeholder {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 48px;
  color: #ccc;
  background: rgba(255, 255, 255, 0.3);
  border-radius: 16px;
  border: 2px dashed #ddd;
}

.pet-sprite-loading {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}

.sprite-loading-spinner {
  width: 24px;
  height: 24px;
  border: 3px solid rgba(255, 107, 157, 0.2);
  border-top: 3px solid #ff6b9d;
  border-radius: 50%;
  animation: sprite-spin 0.8s linear infinite;
}

@keyframes sprite-spin {
  to { transform: rotate(360deg); }
}
</style>