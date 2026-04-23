import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export interface ChatMessage {
  role: 'pet' | 'user' | 'system'
  content: string
}

export interface ChatResult {
  response: string
  loveHate: number
  baseline: number
  personality: Record<string, number>
  sentiment: {
    eventType: string
    loveHateHint: number
  }
  impression: {
    overallAffinity: number
    latestSnippet: string | null
  }
}

export function useChat() {
  const chatMessages = ref<ChatMessage[]>([])
  const chatInput = ref('')
  const chatLoading = ref(false)

  async function sendMessage(): Promise<ChatResult | null> {
    if (!chatInput.value.trim() || chatLoading.value) return null
    const msg = chatInput.value.trim()
    chatInput.value = ''
    chatMessages.value.push({ role: 'user', content: msg })
    chatLoading.value = true

    try {
      const result = await invoke<ChatResult>('chat_with_pet', { message: msg })
      chatMessages.value.push({ role: 'pet', content: result.response })

      if (result.sentiment && result.sentiment.loveHateHint !== 0) {
        const hint = result.sentiment.loveHateHint > 0
          ? `(感到${result.sentiment.loveHateHint > 2 ? '很开心' : '有些开心'})`
          : `(感到${result.sentiment.loveHateHint < -2 ? '很不高兴' : '有点不高兴'})`
        chatMessages.value.push({
          role: 'system',
          content: `情感: ${result.sentiment.eventType} ${hint}`,
        })
      }

      return result
    } catch (e: any) {
      chatMessages.value.push({ role: 'system', content: `错误: ${e}` })
      return null
    } finally {
      chatLoading.value = false
    }
  }

  function pushUserMessage(content: string) {
    chatMessages.value.push({ role: 'user', content })
  }

  function pushSystemMessage(content: string) {
    chatMessages.value.push({ role: 'system', content })
  }

  function pushPetMessage(content: string) {
    chatMessages.value.push({ role: 'pet', content })
  }

  function clearMessages() {
    chatMessages.value = []
  }

  return {
    chatMessages,
    chatInput,
    chatLoading,
    sendMessage,
    pushUserMessage,
    pushSystemMessage,
    pushPetMessage,
    clearMessages,
  }
}