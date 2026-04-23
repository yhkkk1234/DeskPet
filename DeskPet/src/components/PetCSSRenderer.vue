<script setup lang="ts">
import type { AnimationState, MoodAnimationConfig } from '../composables/useAnimation'
import { STATE_TO_CSS_CLASS } from '../composables/useAnimation'

const props = defineProps<{
  animationState: AnimationState
  moodConfig: MoodAnimationConfig | null
  emoji: string
  cssClass: string[]
  styleOverride: Record<string, string>
}>()

const computedClass = computed(() => {
  const stateClass = STATE_TO_CSS_CLASS[props.animationState] || 'idle-bounce'
  return [...props.cssClass, stateClass]
})

const computedStyle = computed(() => ({
  ...props.styleOverride,
}))

import { computed } from 'vue'
</script>

<template>
  <div class="pet-sprite-css" :class="computedClass" :style="computedStyle">{{ emoji }}</div>
</template>

<style scoped>
.pet-sprite-css {
  width: 128px;
  height: 128px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 80px;
  line-height: 1;
  flex-shrink: 0;
  transition: transform 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
}

.idle-bounce {
  animation: idleBounce 2.5s ease-in-out infinite;
}

.anim-happy {
  animation: animHappy 0.6s ease-in-out infinite;
}

.anim-content {
  animation: animContent 2s ease-in-out infinite;
}

.anim-curious {
  animation: animCurious 1.2s ease-in-out infinite;
}

.anim-cold {
  animation: animCold 0.8s ease-in-out infinite;
}

.anim-distant {
  animation: animDistant 3s ease-in-out infinite;
}

.anim-speaking {
  animation: animSpeaking 0.4s ease-in-out infinite;
}

.anim-surprise {
  animation: animSurprise 0.5s cubic-bezier(0.34, 1.56, 0.64, 1) 1;
}

@keyframes idleBounce {
  0%, 100% { transform: translateY(0); }
  50% { transform: translateY(-4px); }
}

@keyframes animHappy {
  0%, 100% { transform: translateY(0) scale(1); }
  30% { transform: translateY(-8px) scale(1.1); }
  60% { transform: translateY(-3px) scale(1.02); }
}

@keyframes animContent {
  0%, 100% { transform: translateY(0) rotate(0deg) scale(1); }
  25% { transform: translateY(-2px) rotate(3deg) scale(1.02); }
  75% { transform: translateY(-1px) rotate(-2deg) scale(1.01); }
}

@keyframes animCurious {
  0%, 100% { transform: translateX(0) rotate(0deg) scale(1); }
  20% { transform: translateX(-3px) rotate(-8deg) scale(1.04); }
  50% { transform: translateX(2px) rotate(5deg) scale(1.06); }
  80% { transform: translateX(-1px) rotate(-3deg) scale(1.02); }
}

@keyframes animCold {
  0%, 100% { transform: translateX(0) scale(1); }
  15% { transform: translateX(-2px) scale(0.98); }
  30% { transform: translateX(2px) scale(0.99); }
  45% { transform: translateX(-2px) scale(0.98); }
  60% { transform: translateX(1px) scale(0.99); }
}

@keyframes animDistant {
  0%, 100% { transform: scale(1) rotate(0deg); opacity: 1; }
  40% { transform: scale(0.96) rotate(-3deg); opacity: 0.85; }
  60% { transform: scale(0.97) rotate(2deg); opacity: 0.88; }
}

@keyframes animSpeaking {
  0%, 100% { transform: scale(1); }
  25% { transform: scale(1.06); }
  50% { transform: scale(0.98); }
  75% { transform: scale(1.04); }
}

@keyframes animSurprise {
  0% { transform: translateY(0) scale(1); }
  25% { transform: translateY(-14px) scale(1.2); }
  50% { transform: translateY(-8px) scale(1.05); }
  75% { transform: translateY(4px) scale(0.95); }
  100% { transform: translateY(0) scale(1); }
}
</style>