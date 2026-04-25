<script setup lang="ts">
import { ref, watch, nextTick, computed } from 'vue'
import BubbleMessage from './BubbleMessage.vue'
import type { ChatMessage } from '../composables/useChat'

const props = defineProps<{
  messages: ChatMessage[]
  petName: string
  chatLoading: boolean
  streamingContent: string
  responseComplete: boolean
  loveHate: number
  placeholder?: string
  allowEmptySend?: boolean
  answeringMode?: 'Companion' | 'Assistant'
}>()

const emit = defineEmits<{
  send: [message: string]
  inputFocus: []
  inputBlur: []
  toggleMode: []
}>()

const inputValue = ref('')
const bubbleArea = ref<HTMLElement | null>(null)
const inputRef = ref<HTMLInputElement | null>(null)
const typingDots = ref(['●', '●', '●'])
let typingFrame = 0
let typingInterval: ReturnType<typeof setInterval> | null = null

function handleSend() {
  if (props.chatLoading) return
  const msg = inputValue.value.trim()
  if (!msg && !props.allowEmptySend) return
  inputValue.value = ''
  emit('send', msg)
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    handleSend()
  }
  if (e.key === 'Escape') {
    inputValue.value = ''
    ;(e.target as HTMLElement)?.blur()
  }
}

function handleFocus() {
  emit('inputFocus')
}

function handleBlur() {
  emit('inputBlur')
}

function scrollToBottom() {
  nextTick(() => {
    if (bubbleArea.value) {
      bubbleArea.value.scrollTop = bubbleArea.value.scrollHeight
    }
  })
}

watch(() => props.messages.length, scrollToBottom)

watch(() => props.streamingContent, scrollToBottom)

watch(() => props.chatLoading, (loading) => {
  if (loading) {
    if (typingInterval) clearInterval(typingInterval)
    typingInterval = setInterval(() => {
      typingFrame = (typingFrame + 1) % 3
      const dots = ['●', '●', '●']
      dots[typingFrame] = '◉'
      typingDots.value = dots
    }, 400)
  } else {
    if (typingInterval) {
      clearInterval(typingInterval)
      typingInterval = null
    }
    typingDots.value = ['●', '●', '●']
  }
})

function focusInput() {
  nextTick(() => {
    inputRef.value?.focus()
  })
}

defineExpose({ focusInput })

const typingText = computed(() => typingDots.value.join(' '))
</script>

<template>
  <div class="bubble-dialogue">
    <div class="bubble-area" ref="bubbleArea">
      <transition-group name="bubble-fade">
        <BubbleMessage
          v-for="(msg, i) in messages"
          :key="i"
          :role="msg.role"
          :content="msg.content"
          :pet-name="petName"
          :image-base64="msg.imageBase64"
          :index="i"
        />
      </transition-group>
      <!-- 流式回复：正在生成中 -->
      <div v-if="streamingContent" class="bubble-wrapper bubble-pet bubble-enter">
        <span class="bubble-sender sender-pet">{{ petName }}</span>
        <div class="bubble-content bubble-streaming">
          <span class="bubble-text">{{ streamingContent }}<span class="streaming-cursor">|</span></span>
        </div>
        <div class="bubble-tail bubble-tail-left"></div>
      </div>
      <!-- 打字动画：加载中但尚未收到 token -->
      <div v-if="chatLoading && !streamingContent && !responseComplete" class="bubble-wrapper bubble-pet bubble-enter">
        <span class="bubble-sender sender-pet">{{ petName }}</span>
        <div class="bubble-content pet-typing">
          <span class="typing-indicator">{{ typingText }}</span>
        </div>
        <div class="bubble-tail bubble-tail-left"></div>
      </div>
    </div>
      <div class="bubble-input-area">
        <button class="mode-toggle-btn" :class="'mode-' + (answeringMode || 'Companion').toLowerCase()" @click="$emit('toggleMode')" :title="(answeringMode || 'Companion') === 'Companion' ? '陪伴模式' : '助力模式'">
          {{ (answeringMode || 'Companion') === 'Companion' ? '♥' : '✦' }}
        </button>
        <input
        ref="inputRef"
        v-model="inputValue"
        class="bubble-input"
        :placeholder="placeholder || '跟桌宠说说...'"
        @keydown="handleKeydown"
        @focus="handleFocus"
        @blur="handleBlur"
      />
      <button class="bubble-send-btn" :disabled="chatLoading || !inputValue.trim()" @click="handleSend">
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
          <path d="M2 8L14 2L10 8L14 14L2 8Z" fill="currentColor" stroke="currentColor" stroke-width="0.5" stroke-linejoin="round"/>
        </svg>
      </button>
    </div>
  </div>
</template>

<style scoped>
.bubble-dialogue {
  display: flex;
  flex-direction: column;
  gap: 0;
  margin: 0 8px;
  animation: slideDown 0.3s ease both;
}

@keyframes slideDown {
  from {
    opacity: 0;
    transform: translateY(-8px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.bubble-area {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 8px 8px 6px;
  max-height: 260px;
  overflow-y: auto;
  overflow-x: hidden;
  background: rgba(255, 255, 255, 0.88);
  backdrop-filter: blur(8px);
  border-radius: 16px 16px 4px 4px;
  box-shadow: 0 -2px 12px rgba(0, 0, 0, 0.06);
  border: 1.5px solid rgba(0, 0, 0, 0.08);
  border-bottom: none;
}

.bubble-area::-webkit-scrollbar {
  width: 3px;
}

.bubble-area::-webkit-scrollbar-track {
  background: transparent;
}

.bubble-area::-webkit-scrollbar-thumb {
  background: rgba(0, 0, 0, 0.12);
  border-radius: 2px;
}

.bubble-input-area {
  display: flex;
  gap: 4px;
  padding: 6px 8px 8px;
  background: rgba(255, 255, 255, 0.92);
  backdrop-filter: blur(8px);
  border-radius: 0 0 16px 16px;
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.08);
  border: 1.5px solid rgba(0, 0, 0, 0.08);
  border-top: 1px solid rgba(0, 0, 0, 0.04);
}

.mode-toggle-btn {
  width: 32px;
  height: 32px;
  border: 2px solid #ddd;
  border-radius: 10px;
  background: #fafafa;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 14px;
  transition: all 0.15s;
  flex-shrink: 0;
  padding: 0;
}

.mode-toggle-btn:hover {
  transform: scale(1.1);
}

.mode-companion {
  color: #ff9800;
  border-color: rgba(255, 152, 0, 0.3);
  background: rgba(255, 152, 0, 0.05);
}

.mode-assistant {
  color: #2196f3;
  border-color: rgba(33, 150, 243, 0.3);
  background: rgba(33, 150, 243, 0.05);
}

.mode-companion:hover {
  background: rgba(255, 152, 0, 0.12);
}

.mode-assistant:hover {
  background: rgba(33, 150, 243, 0.12);
}

.bubble-input {
  flex: 1;
  padding: 7px 12px;
  border: 2px solid #ddd;
  border-radius: 12px;
  font-size: 13px;
  outline: none;
  transition: border-color 0.2s, box-shadow 0.2s;
  background: #fafafa;
  font-family: inherit;
}

.bubble-input:focus {
  border-color: #ff6b9d;
  box-shadow: 0 0 0 3px rgba(255, 107, 157, 0.12);
}

.bubble-send-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border: none;
  border-radius: 12px;
  background: linear-gradient(135deg, #ff6b9d, #c084fc);
  color: white;
  cursor: pointer;
  transition: opacity 0.15s, transform 0.15s;
  flex-shrink: 0;
}

.bubble-send-btn:hover:not(:disabled) {
  opacity: 0.9;
  transform: scale(1.05);
}

.bubble-send-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
  transform: none;
}

.pet-typing {
  background: #fff !important;
  border: 2.5px solid #333 !important;
  border-radius: 16px 16px 16px 4px !important;
  box-shadow: 3px 3px 0 #333 !important;
  padding: 8px 14px !important;
}

.typing-indicator {
  font-size: 14px;
  letter-spacing: 3px;
  color: #ff6b9d;
  animation: typingPulse 1s ease-in-out infinite;
}

@keyframes typingPulse {
  0%, 100% { opacity: 0.6; }
  50% { opacity: 1; }
}

.bubble-streaming {
  background: #fff !important;
  border: 2.5px solid #333 !important;
  border-radius: 16px 16px 16px 4px !important;
  box-shadow: 3px 3px 0 #333 !important;
  padding: 8px 14px !important;
  animation: streamPulse 2s ease-in-out infinite;
}

@keyframes streamPulse {
  0%, 100% { box-shadow: 3px 3px 0 #333; }
  50% { box-shadow: 3px 3px 0 #ff6b9d; }
}

.streaming-cursor {
  animation: cursorBlink 0.6s step-end infinite;
  color: #ff6b9d;
  font-weight: bold;
  margin-left: 1px;
}

@keyframes cursorBlink {
  50% { opacity: 0; }
}

.bubble-fade-enter-active {
  transition: all 0.3s ease;
}

.bubble-fade-leave-active {
  transition: all 0.25s ease;
}

.bubble-fade-enter-from {
  opacity: 0;
  transform: translateY(8px) scale(0.9);
}

.bubble-fade-leave-to {
  opacity: 0;
  transform: translateY(-4px) scale(0.95);
}
</style>