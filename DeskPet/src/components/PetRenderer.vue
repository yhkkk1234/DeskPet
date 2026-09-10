<script setup lang="ts">
import { computed } from 'vue'
import type { AnimationState, MoodAnimationConfig } from '../composables/useAnimation'
import { computeBouncePlaybackRate } from '../composables/useAnimation'
import type { RendererType, SpriteConfig, LottieConfig, TagFrameRange, FramePosition, HeadConfig } from '../composables/usePetRenderer'
import type { HeadDirection } from '../composables/useMouseTracking'
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
  framePositions: Map<number, FramePosition>
  isFlipped: boolean
  headDirection: HeadDirection
  headEnabled: boolean
  headConfig: HeadConfig
}>()

const emit = defineEmits<{
  animationComplete: []
}>()

// 宠物面向左时整体被 scaleX(-1) 镜像，头部方向帧需同步左右互换，
// 否则鼠标向右移动会显示为看向屏幕左。
const MIRROR_DIRECTION: Record<HeadDirection, HeadDirection> = {
  center: 'center',
  up: 'up',
  down: 'down',
  left: 'right',
  right: 'left',
  'up-left': 'up-right',
  'up-right': 'up-left',
  'down-left': 'down-right',
  'down-right': 'down-left',
}

const spriteHeadDirection = computed(() =>
  props.isFlipped ? MIRROR_DIRECTION[props.headDirection] : props.headDirection,
)

// 素材播放速率：bounce 由心情驱动（moodConfig.bounceSpeed），其余动作为原速。
// 与 useAnimation 的收尾时长共用 computeBouncePlaybackRate，公式不会两处漂移。
const playbackRate = computed(() => computeBouncePlaybackRate(props.moodConfig?.bounceSpeed))
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
      :playback-rate="playbackRate"
      :tag-ranges="tagRanges"
      :frame-durations="frameDurations"
      :frame-positions="framePositions"
      :style-override="styleOverride"
      :head-direction="spriteHeadDirection"
      :head-enabled="headEnabled"
      :head-config="headConfig"
      @animation-complete="emit('animationComplete')"
    />
    <PetLottieRenderer
      v-else-if="rendererType === 'lottie'"
      :animation-state="animationState"
      :is-flipped="isFlipped"
      :config-src="lottieConfig.src"
      :style-override="styleOverride"
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