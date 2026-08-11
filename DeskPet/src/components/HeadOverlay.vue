<script setup lang="ts">
import { computed } from 'vue'
import type { HeadDirection } from '../composables/useMouseTracking'
import type { HeadConfig } from '../composables/usePetRenderer'

const props = defineProps<{
  direction: HeadDirection
  config: HeadConfig
}>()

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

// 背景定位由 Vue 响应式驱动（方向帧切换）；transform/display 由父组件
// 渲染循环同步控制（呼吸浮动必须与 canvas 挖洞严格同帧）。
const overlayStyle = computed(() => {
  const idx = DIRECTION_INDEX[props.direction]
  const col = idx % props.config.cols
  const row = Math.floor(idx / props.config.cols)
  return {
    backgroundImage: `url(${props.config.src})`,
    // 背景尺寸用百分比：背景图始终 = 容器宽 × cols，随容器自适应缩放
    backgroundSize: `${props.config.cols * 100}% ${props.config.rows * 100}%`,
    backgroundPosition: `${col / (props.config.cols - 1) * 100}% ${row / (props.config.rows - 1) * 100}%`,
  }
})
</script>

<template>
  <div class="head-overlay" :style="overlayStyle"></div>
</template>

<style scoped>
.head-overlay {
  position: absolute;
  inset: 0;
  pointer-events: none;
  background-repeat: no-repeat;
  image-rendering: pixelated;
  /* 无 transform 过渡：呼吸浮动跟随必须与 canvas 挖洞严格同帧，
     过渡会导致挖空与叠层错位露出白线 */
}
</style>
