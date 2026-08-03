import { ref } from 'vue'

/** 主窗口宠物上方的临时气泡（主动搭话/成就解锁等提示），5 秒自动淡出 */
export function useSpeechBubble() {
  const speechBubble = ref('')
  let speechBubbleTimer: ReturnType<typeof setTimeout> | null = null

  function showSpeechBubble(text: string) {
    speechBubble.value = text
    if (speechBubbleTimer) clearTimeout(speechBubbleTimer)
    speechBubbleTimer = setTimeout(() => { speechBubble.value = '' }, 5000)
  }

  function clearSpeechBubble() {
    if (speechBubbleTimer) {
      clearTimeout(speechBubbleTimer)
      speechBubbleTimer = null
    }
    speechBubble.value = ''
  }

  return { speechBubble, showSpeechBubble, clearSpeechBubble }
}
