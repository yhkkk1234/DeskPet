import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export interface ChatMessage {
  role: 'pet' | 'user' | 'system'
  content: string
  imageBase64?: string
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
  generatedImage?: string
}

export function useChat() {
  const chatMessages = ref<ChatMessage[]>([])
  const chatInput = ref('')
  const chatLoading = ref(false)
  const ghostId = ref('')
  const ttsEnabled = ref(false)
  const ttsRate = ref(1.0)
  const ttsPitch = ref(1.1)
  const ttsEngine = ref<'system' | 'edge'>('system')
  const ttsVoice = ref('zh-CN-XiaoxiaoNeural')

  async function persistMessage(role: string, content: string) {
    if (!ghostId.value) return
    try {
      await invoke('save_chat_message', {
        ghostId: ghostId.value,
        role,
        content,
      })
    } catch (e) {
      console.warn('持久化消息失败:', e)
    }
  }

  async function loadHistory() {
    if (!ghostId.value) return
    try {
      const messages = await invoke<Array<{ id: string; role: string; content: string; createdAt: string }>>(
        'load_chat_history',
        { ghostId: ghostId.value, limit: 100 },
      )
      chatMessages.value = messages.map(m => ({
        role: m.role === 'assistant' ? 'pet' : m.role as 'pet' | 'user' | 'system',
        content: m.content,
      }))
    } catch (e) {
      console.warn('加载聊天记录失败:', e)
    }
  }

  async function clearAllMessages() {
    chatMessages.value = []
    window.speechSynthesis?.cancel()
    if (!ghostId.value) return
    try {
      await invoke('clear_chat_history', { ghostId: ghostId.value })
    } catch (e) {
      console.warn('清除聊天记录失败:', e)
    }
  }

  function speak(text: string) {
    if (!ttsEnabled.value) return
    if (ttsEngine.value === 'edge') {
      speakEdge(text)
    } else {
      speakSystem(text)
    }
  }

  function speakSystem(text: string) {
    if (!window.speechSynthesis) return
    window.speechSynthesis.cancel()
    const utterance = new SpeechSynthesisUtterance(text)
    utterance.lang = 'zh-CN'
    utterance.rate = ttsRate.value
    utterance.pitch = ttsPitch.value
    utterance.volume = 0.9
    window.speechSynthesis.speak(utterance)
  }

  let currentAudio: HTMLAudioElement | null = null

  async function speakEdge(text: string) {
    try {
      if (currentAudio) {
        currentAudio.pause()
        currentAudio = null
      }
      const base64 = await invoke<string>('speak_edge_tts', {
        text,
        voice: ttsVoice.value,
      })
      const blob = new Blob(
        [Uint8Array.from(atob(base64), c => c.charCodeAt(0))],
        { type: 'audio/mp3' },
      )
      const url = URL.createObjectURL(blob)
      const audio = new Audio(url)
      audio.volume = 0.9
      currentAudio = audio
      audio.onended = () => {
        URL.revokeObjectURL(url)
        if (currentAudio === audio) currentAudio = null
      }
      audio.onerror = () => {
        URL.revokeObjectURL(url)
        if (currentAudio === audio) currentAudio = null
      }
      await audio.play()
    } catch (e) {
      console.warn('Edge TTS 失败，降级到系统语音:', e)
      speakSystem(text)
    }
  }

  async function sendMessage(): Promise<ChatResult | null> {
    if (!chatInput.value.trim() || chatLoading.value) return null
    const msg = chatInput.value.trim()
    chatInput.value = ''
    chatMessages.value.push({ role: 'user', content: msg })
    persistMessage('user', msg)
    chatLoading.value = true

    try {
      const result = await invoke<ChatResult>('chat_with_pet', { message: msg })
      chatMessages.value.push({ role: 'pet', content: result.response })
      persistMessage('pet', result.response)

      if (result.sentiment && result.sentiment.loveHateHint !== 0) {
        const hint = result.sentiment.loveHateHint > 0
          ? `(感到${result.sentiment.loveHateHint > 2 ? '很开心' : '有些开心'})`
          : `(感到${result.sentiment.loveHateHint < -2 ? '很不高兴' : '有点不高兴'})`
        const sysMsg = `情感: ${result.sentiment.eventType} ${hint}`
        chatMessages.value.push({ role: 'system', content: sysMsg })
        persistMessage('system', sysMsg)
      }

      speak(result.response)

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

  function pushUserImageMessage(content: string, imageBase64: string) {
    chatMessages.value.push({ role: 'user', content, imageBase64 })
    persistMessage('user', content)
  }

  function pushSystemMessage(content: string) {
    chatMessages.value.push({ role: 'system', content })
    persistMessage('system', content)
  }

  function pushPetMessage(content: string) {
    chatMessages.value.push({ role: 'pet', content })
    speak(content)
  }

  function pushPetImageMessage(content: string, imageBase64: string) {
    chatMessages.value.push({ role: 'pet', content, imageBase64 })
    persistMessage('pet', content)
  }

  function clearMessages() {
    chatMessages.value = []
    window.speechSynthesis?.cancel()
  }

  return {
    chatMessages,
    chatInput,
    chatLoading,
    ghostId,
    ttsEnabled,
    ttsRate,
    ttsPitch,
    ttsEngine,
    ttsVoice,
    sendMessage,
    pushUserMessage,
    pushUserImageMessage,
    pushSystemMessage,
    pushPetMessage,
    pushPetImageMessage,
    clearMessages,
    clearAllMessages,
    loadHistory,
    speak,
  }
}