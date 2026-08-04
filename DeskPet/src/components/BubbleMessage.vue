<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  role: 'pet' | 'user' | 'system'
  content: string
  petName: string
  imageBase64?: string
}>()

const bubbleClass = computed(() => `bubble-${props.role}`)
const hasImage = computed(() => !!props.imageBase64)
const imageSrc = computed(() => {
  if (!props.imageBase64) return ''
  // 支持 url: 前缀（表示是URL）或纯 base64
  if (props.imageBase64.startsWith('url:')) {
    return props.imageBase64.slice(4)
  }
  if (props.imageBase64.startsWith('data:')) {
    return props.imageBase64
  }
  return `data:image/png;base64,${props.imageBase64}`
})

const senderLabel = computed(() => {
  if (props.role === 'pet') return props.petName
  if (props.role === 'user') return '你'
  return ''
})
</script>

<template>
  <div class="bubble-wrapper" :class="[bubbleClass, { 'has-image': hasImage }]">
    <span v-if="senderLabel" class="bubble-sender" :class="{ 'sender-pet': role === 'pet', 'sender-user': role === 'user' }">
      {{ senderLabel }}
    </span>
    <div class="bubble-content">
      <img v-if="hasImage" class="bubble-image" :src="imageSrc" alt="截图" />
      <span v-if="content" class="bubble-text">{{ content }}</span>
    </div>
    <div v-if="role === 'pet'" class="bubble-tail bubble-tail-left"></div>
    <div v-if="role === 'user'" class="bubble-tail bubble-tail-right"></div>
  </div>
</template>

<style scoped>
.bubble-wrapper {
  position: relative;
  max-width: 220px;
}

.bubble-pet {
  align-self: flex-start;
  margin-right: auto;
}

.bubble-user {
  align-self: flex-end;
  margin-left: auto;
}

.bubble-system {
  align-self: center;
  margin-left: auto;
  margin-right: auto;
}

.bubble-sender {
  display: block;
  font-size: 10px;
  font-weight: 700;
  margin-bottom: 1px;
  letter-spacing: 0.3px;
}

.sender-pet {
  color: var(--t-sender-pet, #ff6b9d);
}

.sender-user {
  color: var(--t-sender-user, #4caf50);
  text-align: right;
}

.bubble-content {
  padding: 8px 12px;
  font-size: 13px;
  line-height: 1.6;
  word-break: break-word;
}

.bubble-text {
  white-space: pre-wrap;
}

.bubble-pet .bubble-content {
  background: var(--t-bp-bg, #fff);
  border: var(--t-bp-border, 2.5px solid #333);
  border-radius: var(--t-bp-radius, 16px 16px 16px 4px);
  box-shadow: var(--t-bp-shadow, 3px 3px 0 #333);
  color: var(--t-bp-text, #333);
}

.bubble-user .bubble-content {
  background: var(--t-bu-bg, #f0faf0);
  border: var(--t-bu-border, 2px solid #a5d6a7);
  border-radius: var(--t-bu-radius, 16px 16px 4px 16px);
  box-shadow: var(--t-bu-shadow, 2px 2px 0 #c8e6c9);
  color: var(--t-bu-text, #2e7d32);
  max-width: 200px;
}

.bubble-system .bubble-content {
  background: var(--t-bs-bg, rgba(255, 248, 240, 0.92));
  border: var(--t-bs-border, 1px dashed #e0c8a0);
  border-radius: 8px;
  font-size: 11px;
  color: var(--t-bs-text, #a08050);
  padding: 4px 10px;
  max-width: 260px;
  text-align: center;
}

.bubble-tail {
  position: absolute;
  width: 0;
  height: 0;
}

.bubble-tail-left {
  bottom: -10px;
  left: 16px;
  border-left: 8px solid transparent;
  border-right: 8px solid transparent;
  border-top: 10px solid var(--t-bp-tail, #333);
}

.bubble-tail-left::after {
  content: '';
  position: absolute;
  top: -12px;
  left: -7px;
  border-left: 7px solid transparent;
  border-right: 7px solid transparent;
  border-top: 9px solid var(--t-bp-bg, #fff);
}

.bubble-tail-right {
  bottom: -10px;
  right: 16px;
  width: 0;
  height: 0;
  border-left: 8px solid transparent;
  border-right: 8px solid transparent;
  border-top: 10px solid var(--t-bu-tail, #a5d6a7);
}

.bubble-image {
  display: block;
  max-width: 100%;
  max-height: 140px;
  width: auto;
  height: auto;
  border-radius: 6px;
  border: 1px solid rgba(0, 0, 0, 0.1);
  object-fit: contain;
}

.has-image .bubble-content {
  padding: 5px;
  overflow: hidden;
}

.has-image .bubble-text {
  display: block;
  margin-top: 4px;
  font-size: 11px;
  color: var(--t-text-2, #888);
}
</style>