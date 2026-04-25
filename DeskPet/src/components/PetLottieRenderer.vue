<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch, nextTick } from 'vue'
import lottie, { type AnimationItem } from 'lottie-web'

const props = defineProps<{
  animationState: string
  isFlipped: boolean
  configSrc: string
}>()

const canvasRef = ref<HTMLDivElement | null>(null)
const moodRequest = ref('')
let animInstance: AnimationItem | null = null
let currentMood = 'idle'

const MOOD_TO_FILE: Record<string, string> = {
  idle: 'idle.json',
  happy: 'happy.json',
  content: 'content.json',
  curious: 'curious.json',
  cold: 'cold.json',
  distant: 'distant.json',
  speaking: 'speaking.json',
  surprise: 'surprise.json',
}

function getAnimPath(mood: string): string {
  const file = MOOD_TO_FILE[mood] || 'idle.json'
  return `${props.configSrc}${file}`
}

async function loadMood(mood: string) {
  if (mood === currentMood && animInstance) return
  currentMood = mood

  if (animInstance) {
    animInstance.destroy()
    animInstance = null
  }

  if (!canvasRef.value) return

  const path = getAnimPath(mood)

  try {
    const response = await fetch(path)
    if (!response.ok) {
      const fallback = getAnimPath('idle')
      if (path !== fallback) {
        const fallbackResp = await fetch(fallback)
        if (fallbackResp.ok) {
          const data = await fallbackResp.json()
          animInstance = lottie.loadAnimation({
            container: canvasRef.value,
            renderer: 'canvas',
            loop: mood !== 'surprise',
            autoplay: true,
            animationData: data,
          })
          return
        }
      }
      throw new Error(`HTTP ${response.status}`)
    }
    const data = await response.json()
    animInstance = lottie.loadAnimation({
      container: canvasRef.value,
      renderer: 'canvas',
      loop: mood !== 'surprise',
      autoplay: true,
      animationData: data,
    })
  } catch {
    // 加载失败时显示占位指示，不做崩溃处理
    console.warn(`[Lottie] 无法加载动画: ${path}`)
  }
}

watch(() => props.animationState, (newVal) => {
  if (newVal) {
    moodRequest.value = newVal
    nextTick(() => loadMood(newVal))
  }
})

onMounted(() => {
  if (props.animationState) {
    loadMood(props.animationState)
  }
})

onUnmounted(() => {
  if (animInstance) {
    animInstance.destroy()
    animInstance = null
  }
})

defineExpose({ loadMood })
</script>

<template>
  <div
    ref="canvasRef"
    class="pet-lottie-container"
    :class="{ 'pet-flipped': isFlipped }"
  />
</template>

<style scoped>
.pet-lottie-container {
  width: 128px;
  height: 128px;
}

.pet-flipped {
  transform: scaleX(-1);
}
</style>
