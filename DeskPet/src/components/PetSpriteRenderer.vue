<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, computed, nextTick } from 'vue'
import type { AnimationState } from '../composables/useAnimation'
import { STATE_TO_SPRITE_ROW, STATE_TO_FPS, STATE_TO_LOOP } from '../composables/useAnimation'
import type { SpriteConfig, TagFrameRange } from '../composables/usePetRenderer'

const props = defineProps<{
  animationState: AnimationState
  config: SpriteConfig
  tagRanges: Map<string, TagFrameRange>
  frameDurations: Map<number, number>
}>()

const emit = defineEmits<{
  animationComplete: []
}>()

const canvas = ref<HTMLCanvasElement | null>(null)
const image = ref<HTMLImageElement | null>(null)
const imageLoaded = ref(false)

let currentFrame = 0
let lastTimestamp = 0
let frameTimer = 0
let animFrameId: number | null = null
let transitioning = false
let transitionAlpha = 1
let previousSnapshot: HTMLCanvasElement | null = null
const CROSSFADE_MS = 200

function getTagRange(): TagFrameRange | null {
  return props.tagRanges.get(props.animationState) ?? null
}

function getFps(): number {
  const override = STATE_TO_FPS[props.animationState]
  if (override !== undefined) return override

  const range = getTagRange()
  if (!range) return 4

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
    return Math.round(1000 / (sumMs / count))
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
    return (absoluteIndex % props.config.cols) * props.config.frameWidth
  }
  return (currentFrame % props.config.cols) * props.config.frameWidth
}

function getSpriteY(_frameIndex: number): number {
  const range = getTagRange()
  if (range) {
    const absoluteIndex = range.from + currentFrame
    return Math.floor(absoluteIndex / props.config.cols) * props.config.frameHeight
  }
  const row = STATE_TO_SPRITE_ROW[props.animationState] ?? 0
  return row * props.config.frameHeight
}

function loadSprite() {
  if (!props.config.src) return
  const img = new Image()
  img.onload = () => {
    image.value = img
    imageLoaded.value = true
    nextTick(() => {
      startRenderLoop()
    })
  }
  img.onerror = () => {
    console.error('Failed to load sprite:', props.config.src)
  }
  img.src = props.config.src
}

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
}

onMounted(() => {
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
  currentFrame = 0
  if (props.config.src) {
    loadSprite()
  }
})

const canvasWidth = computed(() => props.config.frameWidth * props.config.scale)
const canvasHeight = computed(() => props.config.frameHeight * props.config.scale)
</script>

<template>
  <div class="pet-sprite-canvas-wrapper">
    <canvas
      v-if="imageLoaded"
      ref="canvas"
      :width="canvasWidth"
      :height="canvasHeight"
      class="pet-sprite-canvas"
    />
    <div v-else class="pet-sprite-placeholder">?</div>
  </div>
</template>

<style scoped>
.pet-sprite-canvas-wrapper {
  width: 128px;
  height: 128px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.pet-sprite-canvas {
  image-rendering: pixelated;
  image-rendering: crisp-edges;
  max-width: 100%;
  max-height: 100%;
}

.pet-sprite-placeholder {
  width: 128px;
  height: 128px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 48px;
  color: #ccc;
  background: rgba(255, 255, 255, 0.3);
  border-radius: 16px;
  border: 2px dashed #ddd;
}
</style>