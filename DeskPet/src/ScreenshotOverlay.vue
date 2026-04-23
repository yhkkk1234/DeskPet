<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { emit } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'

const loading = ref(true)
const screenshotDataUrl = ref('')
const winW = ref(window.innerWidth)
const winH = ref(window.innerHeight)
const dpr = window.devicePixelRatio || 1
const imgNatW = ref(0)
const imgNatH = ref(0)
let captured = false
let resizeHandler: (() => void) | null = null

const selecting = ref(false)
const startX = ref(0)
const startY = ref(0)
const endX = ref(0)
const endY = ref(0)

const selLeft = computed(() => Math.min(startX.value, endX.value))
const selTop = computed(() => Math.min(startY.value, endY.value))
const selWidth = computed(() => Math.abs(endX.value - startX.value))
const selHeight = computed(() => Math.abs(endY.value - startY.value))
const hasSelection = computed(() => selWidth.value > 2 || selHeight.value > 2)

const physWidth = computed(() => {
  const scaleX = imgNatW.value / winW.value || dpr
  return Math.round(selWidth.value * scaleX)
})
const physHeight = computed(() => {
  const scaleY = imgNatH.value / winH.value || dpr
  return Math.round(selHeight.value * scaleY)
})

const labelBelow = computed(() => selTop.value < 28)

function onMouseDown(e: MouseEvent) {
  if (e.button !== 0) return
  startX.value = e.clientX
  startY.value = e.clientY
  endX.value = e.clientX
  endY.value = e.clientY
  selecting.value = true
}

function onMouseMove(e: MouseEvent) {
  if (!selecting.value) return
  endX.value = Math.max(0, Math.min(e.clientX, winW.value))
  endY.value = Math.max(0, Math.min(e.clientY, winH.value))
}

function onMouseUp(e: MouseEvent) {
  if (!selecting.value) return
  selecting.value = false
  endX.value = Math.max(0, Math.min(e.clientX, winW.value))
  endY.value = Math.max(0, Math.min(e.clientY, winH.value))

  if (selWidth.value < 5 || selHeight.value < 5) {
    closeWindow()
    return
  }

  cropAndEmit()
}

function cropAndEmit() {
  const scaleX = imgNatW.value / winW.value
  const scaleY = imgNatH.value / winH.value

  const imgStartX = Math.round(selLeft.value * scaleX)
  const imgStartY = Math.round(selTop.value * scaleY)
  const imgW = Math.max(1, Math.round(selWidth.value * scaleX))
  const imgH = Math.max(1, Math.round(selHeight.value * scaleY))

  const canvas = document.createElement('canvas')
  canvas.width = imgW
  canvas.height = imgH
  const ctx = canvas.getContext('2d')
  if (!ctx) { closeWindow(); return }

  const imgEl = document.querySelector('.screenshot-bg') as HTMLImageElement | null
  if (!imgEl) { closeWindow(); return }

  ctx.drawImage(imgEl, imgStartX, imgStartY, imgW, imgH, 0, 0, imgW, imgH)
  const dataUrl = canvas.toDataURL('image/png')
  const base64 = dataUrl.split(',')[1]

  captured = true
  emit('screenshot-region-captured', {
    base64,
    region: { x: imgStartX, y: imgStartY, width: imgW, height: imgH },
  }).then(() => {
    closeWindow()
  }).catch(() => {
    closeWindow()
  })
}

function onKeyDown(e: KeyboardEvent) {
  if (e.key === 'Escape') closeWindow()
}

function onContextMenu(e: MouseEvent) {
  e.preventDefault()
  closeWindow()
}

async function closeWindow() {
  try {
    if (!captured) {
      await emit('screenshot-cancelled', {})
    }
    await getCurrentWindow().close()
  } catch (_) {}
}

onMounted(async () => {
  document.addEventListener('keydown', onKeyDown)

  winW.value = window.innerWidth
  winH.value = window.innerHeight

  resizeHandler = () => {
    winW.value = window.innerWidth
    winH.value = window.innerHeight
  }
  window.addEventListener('resize', resizeHandler)

  try {
    const win = getCurrentWindow()
    await win.setFocus()
  } catch (_) {}

  try {
    const base64 = await invoke<string>('get_screenshot_data')
    screenshotDataUrl.value = `data:image/png;base64,${base64}`

    const img = new Image()
    img.onload = () => {
      imgNatW.value = img.naturalWidth
      imgNatH.value = img.naturalHeight
      loading.value = false
    }
    img.onerror = () => {
      closeWindow()
    }
    img.src = screenshotDataUrl.value
  } catch (e) {
    console.error('Failed to get screenshot data:', e)
    closeWindow()
  }
})

onUnmounted(() => {
  document.removeEventListener('keydown', onKeyDown)
  if (resizeHandler) {
    window.removeEventListener('resize', resizeHandler)
  }
})
</script>

<template>
  <div
    class="screenshot-root"
    @mousedown="onMouseDown"
    @mousemove="onMouseMove"
    @mouseup="onMouseUp"
    @contextmenu="onContextMenu"
  >
    <div v-if="loading" class="loading-container">
      <div class="loading-spinner"></div>
      <span>正在截取屏幕...</span>
    </div>

    <template v-else>
      <img :src="screenshotDataUrl" class="screenshot-bg" draggable="false" />

      <div
        v-if="hasSelection"
        class="selection-cutout"
        :style="{
          left: selLeft + 'px',
          top: selTop + 'px',
          width: selWidth + 'px',
          height: selHeight + 'px',
          backgroundImage: `url(${screenshotDataUrl})`,
          backgroundSize: `${winW}px ${winH}px`,
          backgroundPosition: `-${selLeft}px -${selTop}px`,
        }"
      >
        <div class="corner tl"></div>
        <div class="corner tr"></div>
        <div class="corner bl"></div>
        <div class="corner br"></div>
        <div class="size-label" :class="{ below: labelBelow }">
          {{ physWidth }} × {{ physHeight }}
        </div>
      </div>

      <div v-if="!hasSelection" class="dark-overlay"></div>
      <div v-if="!hasSelection" class="hint">拖动选择区域 · 右键或ESC取消</div>
    </template>
  </div>
</template>

<style scoped>
.screenshot-root {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  cursor: crosshair;
  user-select: none;
  overflow: hidden;
}

.loading-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
  gap: 12px;
  color: white;
  font-size: 14px;
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
  to {
    transform: rotate(360deg);
  }
}

.screenshot-bg {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
}

.dark-overlay {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background: rgba(0, 0, 0, 0.5);
  pointer-events: none;
}

.selection-cutout {
  position: absolute;
  border: 1px solid rgba(255, 107, 157, 0.8);
  pointer-events: none;
  z-index: 10;
  box-shadow: 0 0 0 9999px rgba(0, 0, 0, 0.5);
}

.corner {
  position: absolute;
  width: 12px;
  height: 12px;
  pointer-events: none;
}

.corner.tl {
  top: -1px;
  left: -1px;
  border-top: 2px solid #ff6b9d;
  border-left: 2px solid #ff6b9d;
}

.corner.tr {
  top: -1px;
  right: -1px;
  border-top: 2px solid #ff6b9d;
  border-right: 2px solid #ff6b9d;
}

.corner.bl {
  bottom: -1px;
  left: -1px;
  border-bottom: 2px solid #ff6b9d;
  border-left: 2px solid #ff6b9d;
}

.corner.br {
  bottom: -1px;
  right: -1px;
  border-bottom: 2px solid #ff6b9d;
  border-right: 2px solid #ff6b9d;
}

.size-label {
  position: absolute;
  top: -22px;
  left: 0;
  background: rgba(0, 0, 0, 0.75);
  color: #fff;
  padding: 2px 8px;
  border-radius: 4px;
  font-size: 12px;
  white-space: nowrap;
  font-family: 'Consolas', 'Monaco', monospace;
  line-height: 18px;
}

.size-label.below {
  top: auto;
  bottom: -22px;
}

.hint {
  position: fixed;
  bottom: 40px;
  left: 50%;
  transform: translateX(-50%);
  background: rgba(0, 0, 0, 0.7);
  color: white;
  padding: 8px 16px;
  border-radius: 8px;
  font-size: 12px;
  z-index: 20;
  pointer-events: none;
}
</style>
