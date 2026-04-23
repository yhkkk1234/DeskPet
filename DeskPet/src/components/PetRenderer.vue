<script setup lang="ts">
import type { AnimationState, MoodAnimationConfig } from '../composables/useAnimation'
import { STATE_TO_CSS_CLASS } from '../composables/useAnimation'
import type { RendererType, SpriteConfig, TagFrameRange } from '../composables/usePetRenderer'
import PetCSSRenderer from './PetCSSRenderer.vue'
import PetSpriteRenderer from './PetSpriteRenderer.vue'

const props = defineProps<{
  animationState: AnimationState
  moodConfig: MoodAnimationConfig | null
  emoji: string
  cssClass: string[]
  styleOverride: Record<string, string>
  rendererType: RendererType
  spriteConfig: SpriteConfig
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
    <div v-else class="pet-sprite-spine-placeholder">
      <div class="spine-coming-soon">Spine</div>
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

.pet-sprite-spine-placeholder {
  width: 128px;
  height: 128px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.spine-coming-soon {
  font-size: 12px;
  color: #aaa;
  background: rgba(255, 255, 255, 0.3);
  border-radius: 50%;
  width: 80px;
  height: 80px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px dashed #ccc;
}
</style>