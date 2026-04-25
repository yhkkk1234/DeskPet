<script setup lang="ts">
import type { AnimationState, MoodAnimationConfig } from '../composables/useAnimation'
import type { RendererType, SpriteConfig, LottieConfig, TagFrameRange } from '../composables/usePetRenderer'
import PetCSSRenderer from './PetCSSRenderer.vue'
import PetSpriteRenderer from './PetSpriteRenderer.vue'
import PetLottieRenderer from './PetLottieRenderer.vue'

const props = defineProps<{
  animationState: AnimationState
  moodConfig: MoodAnimationConfig | null
  emoji: string
  cssClass: string[]
  styleOverride: Record<string, string>
  rendererType: RendererType
  spriteConfig: SpriteConfig
  lottieConfig: LottieConfig
  tagRanges: Map<string, TagFrameRange>
  frameDurations: Map<number, number>
  isFlipped: boolean
}>()

const emit = defineEmits<{
  animationComplete: []
}>()
</script>

<template>
  <div class="pet-renderer-wrapper" :class="{ 'pet-flipped': isFlipped }">
    <PetCSSRenderer
      v-if="rendererType === 'css'"
      :animation-state="animationState"
      :mood-config="moodConfig"
      :emoji="emoji"
      :css-class="cssClass"
      :style-override="styleOverride"
    />
    <PetSpriteRenderer
      v-else-if="rendererType === 'spritesheet'"
      :animation-state="animationState"
      :config="spriteConfig"
      :tag-ranges="tagRanges"
      :frame-durations="frameDurations"
      @animation-complete="emit('animationComplete')"
    />
    <PetLottieRenderer
      v-else-if="rendererType === 'lottie'"
      :animation-state="animationState"
      :is-flipped="isFlipped"
      :config-src="lottieConfig.src"
    />
    <div v-else class="pet-sprite-placeholder">
      <span class="placeholder-text">?</span>
    </div>
  </div>
</template>

<style scoped>
.pet-renderer-wrapper {
  transition: transform 0.3s ease;
}

.pet-flipped {
  transform: scaleX(-1);
}

.pet-sprite-placeholder {
  width: 128px;
  height: 128px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.placeholder-text {
  font-size: 24px;
  color: #ccc;
}
</style>