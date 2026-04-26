<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { emit } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { ref, computed, onMounted } from 'vue'
import type { RendererType } from './composables/usePetRenderer'
import EmotionTimeline from './components/EmotionTimeline.vue'

const aiEndpoint = ref('https://api.deepseek.com/v1')
const aiApiKey = ref('')
const aiModel = ref('deepseek-chat')
const aiVisionModel = ref('')
const aiImageModel = ref('')
const aiImageGenEndpoint = ref('')
const aiImageGenApiKey = ref('')

const rendererType = ref<RendererType>('spritesheet')
const spriteSrc = ref('/pet/pet_spritesheet.png')
const spriteJsonSrc = ref('/pet/pet_spritesheet.json')
const lottieSrc = ref('/pet/lottie/')

const ghost = ref<any>(null)
const saveLoadPath = ref('')
const error = ref('')
const success = ref('')

const ttsEnabled = ref(false)
const ttsRate = ref(1.0)
const ttsPitch = ref(1.1)
const ttsEngine = ref<'system' | 'edge'>('system')
const ttsVoice = ref('zh-CN-XiaoxiaoNeural')
const answeringMode = ref<'Companion' | 'Assistant'>('Companion')

const EDGE_VOICE_OPTIONS = [
  { id: 'zh-CN-XiaoxiaoNeural', label: '晓晓 (女/活泼)' },
  { id: 'zh-CN-XiaoyiNeural', label: '晓伊 (女/温柔)' },
  { id: 'zh-CN-YunjianNeural', label: '云健 (男/阳光)' },
  { id: 'zh-CN-YunxiNeural', label: '云希 (男/沉稳)' },
  { id: 'zh-CN-YunxiaNeural', label: '云霞 (女/亲切)' },
  { id: 'zh-CN-YunyangNeural', label: '云扬 (男/新闻)' },
  { id: 'zh-CN-XiaochenNeural', label: '晓晨 (女/自然)' },
  { id: 'zh-CN-XiaohanNeural', label: '晓涵 (女/甜美)' },
]

async function loadLocalStorage() {
  rendererType.value = (localStorage.getItem('deskpet_renderer_type') as RendererType) || 'spritesheet'
  spriteSrc.value = localStorage.getItem('deskpet_sprite_src') || '/pet/pet_spritesheet.png'
  spriteJsonSrc.value = localStorage.getItem('deskpet_sprite_json_src') || '/pet/pet_spritesheet.json'
  lottieSrc.value = localStorage.getItem('deskpet_lottie_src') || '/pet/lottie/'
  aiEndpoint.value = localStorage.getItem('deskpet_ai_endpoint') || 'https://api.deepseek.com/v1'
  aiModel.value = localStorage.getItem('deskpet_ai_model') || 'deepseek-chat'
  aiVisionModel.value = localStorage.getItem('deskpet_ai_vision_model') || ''
  aiImageModel.value = localStorage.getItem('deskpet_ai_image_model') || ''
  aiImageGenEndpoint.value = localStorage.getItem('deskpet_ai_image_gen_endpoint') || ''
  aiImageGenApiKey.value = localStorage.getItem('deskpet_ai_image_gen_api_key') || ''
  ttsEnabled.value = localStorage.getItem('deskpet_tts_enabled') === 'true'
  ttsRate.value = parseFloat(localStorage.getItem('deskpet_tts_rate') || '1.0')
  ttsPitch.value = parseFloat(localStorage.getItem('deskpet_tts_pitch') || '1.1')
  ttsEngine.value = (localStorage.getItem('deskpet_tts_engine') as 'system' | 'edge') || 'system'
  ttsVoice.value = localStorage.getItem('deskpet_tts_voice') || 'zh-CN-XiaoxiaoNeural'
  answeringMode.value = (localStorage.getItem('deskpet_answering_mode') as 'Companion' | 'Assistant') || 'Companion'
  await syncTTSToMain()
}

onMounted(async () => {
  await loadLocalStorage()
  fetchGhostStatus()
})

async function syncTTSToMain() {
  await emit('tts-updated', { enabled: ttsEnabled.value, rate: ttsRate.value, pitch: ttsPitch.value, engine: ttsEngine.value, voice: ttsVoice.value })
}

async function fetchGhostStatus() {
  try {
    const status = await invoke<string>('get_ghost_status')
    ghost.value = JSON.parse(status)
  } catch (e) {
    // 没有 ghost 也没关系
  }
}

function showError(msg: string) {
  error.value = msg
  success.value = ''
  setTimeout(() => { error.value = '' }, 4000)
}

function showSuccess(msg: string) {
  success.value = msg
  error.value = ''
  setTimeout(() => { success.value = '' }, 3000)
}

async function saveTTSSettings() {
  localStorage.setItem('deskpet_tts_enabled', ttsEnabled.value.toString())
  localStorage.setItem('deskpet_tts_rate', ttsRate.value.toString())
  localStorage.setItem('deskpet_tts_pitch', ttsPitch.value.toString())
  localStorage.setItem('deskpet_tts_engine', ttsEngine.value)
  localStorage.setItem('deskpet_tts_voice', ttsVoice.value)
  await syncTTSToMain()
  showSuccess('语音设置已保存')
  emit('settings-updated', { section: 'tts' })
}

async function clearHistory() {
  if (!ghost.value) return
  try {
    await invoke('clear_chat_history', { ghostId: ghost.value.ghostId })
    showSuccess('聊天记录已清除')
  } catch (e: any) {
    showError('清除失败: ' + (e as string))
  }
}

async function saveAIConfig() {
  if (!aiEndpoint.value.trim()) { showError('请填写 API Endpoint'); return }
  if (!aiApiKey.value.trim()) { showError('请填写 API Key'); return }
  if (!aiModel.value.trim()) { showError('请填写 Model 名称'); return }

  try {
    await invoke('configure_ai', {
      endpoint: aiEndpoint.value.trim(),
      apiKey: aiApiKey.value.trim(),
      model: aiModel.value.trim(),
      visionModel: aiVisionModel.value.trim() || null,
      imageModel: aiImageModel.value.trim() || null,
      imageGenEndpoint: aiImageGenEndpoint.value.trim() || null,
      imageGenApiKey: aiImageGenApiKey.value.trim() || null,
    })
    localStorage.setItem('deskpet_ai_endpoint', aiEndpoint.value.trim())
    localStorage.setItem('deskpet_ai_model', aiModel.value.trim())
    localStorage.setItem('deskpet_ai_vision_model', aiVisionModel.value.trim())
    localStorage.setItem('deskpet_ai_image_model', aiImageModel.value.trim())
    localStorage.setItem('deskpet_ai_image_gen_endpoint', aiImageGenEndpoint.value.trim())
    localStorage.setItem('deskpet_ai_image_gen_api_key', aiImageGenApiKey.value.trim())
    showSuccess('AI 配置已保存')
    await emit('settings-updated', { section: 'ai' })
  } catch (e: any) {
    showError('配置失败: ' + (e as string))
  }
}

function switchRenderer(type: RendererType) {
  rendererType.value = type
  localStorage.setItem('deskpet_renderer_type', type)
  emit('settings-updated', { section: 'renderer' })
}

function updateSpriteSrc() {
  localStorage.setItem('deskpet_sprite_src', spriteSrc.value)
  localStorage.setItem('deskpet_sprite_json_src', spriteJsonSrc.value)
  emit('settings-updated', { section: 'renderer' })
}

function updateLottieSrc() {
  localStorage.setItem('deskpet_lottie_src', lottieSrc.value)
  emit('settings-updated', { section: 'renderer' })
}

async function setCuriosityLevel(level: string) {
  if (!ghost.value) return
  try {
    await invoke('set_curiosity_level', { level })
    ghost.value.curiosityLevel = level
    showSuccess('好奇心已设置为 ' + level)
    await emit('settings-updated', { section: 'curiosity' })
  } catch (e: any) {
    showError('设置失败: ' + e)
  }
}

async function triggerCuriosity() {
  if (!ghost.value) return
  try {
    const result = await invoke<{
      triggered: boolean
      reason?: string
      interests?: Array<{ topic: string; weight: number }>
      loveHate?: number
    }>('trigger_curiosity')
    if (!result.triggered) {
      showError(result.reason || '好奇心不足，无法触发')
      return
    }
    showSuccess('好奇心已触发')
    await emit('settings-updated', { section: 'curiosity' })
  } catch (e: any) {
    showError('触发失败: ' + e)
  }
}

async function saveGhost() {
  if (!ghost.value) return
  try {
    const path = saveLoadPath.value.trim() || 'deskpet.ghost'
    await invoke('save_ghost', { path })
    showSuccess('灵魂已保存到 ' + path)
    await emit('settings-updated', { section: 'ghost' })
  } catch (e: any) {
    showError('保存失败: ' + e)
  }
}

async function loadGhost() {
  const path = saveLoadPath.value.trim() || 'deskpet.ghost'
  try {
    await invoke('load_ghost', { path })
    await fetchGhostStatus()
    showSuccess('灵魂已从 ' + path + ' 加载')
    await emit('settings-updated', { section: 'ghost' })
  } catch (e: any) {
    showError('加载失败: ' + e)
  }
}

async function requestTransfer() {
  await emit('request-transfer')
  await closeWindow()
}

async function setAnsweringMode(mode: 'Companion' | 'Assistant') {
  answeringMode.value = mode
  localStorage.setItem('deskpet_answering_mode', mode)
  try {
    await invoke('set_answering_mode', { mode })
    showSuccess('回应风格已设置为 ' + (mode === 'Companion' ? '陪伴' : '助力'))
    await emit('settings-updated', { section: 'answering' })
  } catch (e: any) {
    showError('设置失败: ' + e)
  }
}

const personalityLabels: Record<string, string> = {
  openness: '开放',
  conscientiousness: '尽责',
  extraversion: '外向',
  agreeableness: '宜人',
  neuroticism: '情绪',
  creativity: '创造',
}

const eventGroups = [
  { label: '正向', events: [
    { type: 'UserCaredAboutPet', intensity: 0.8, name: '关心', cls: 'btn-positive' },
    { type: 'UserPraisedPet', intensity: 0.6, name: '夸奖', cls: 'btn-positive' },
    { type: 'UserSharedPersonalStory', intensity: 0.7, name: '分享', cls: 'btn-positive' },
    { type: 'FirstConversation', intensity: 1.0, name: '初次', cls: 'btn-positive' },
    { type: 'BirthdayCelebrated', intensity: 1.0, name: '生日', cls: 'btn-positive' },
  ]},
  { label: '中性', events: [
    { type: 'NormalChat', intensity: 0.3, name: '聊天', cls: 'btn-neutral' },
  ]},
  { label: '负向', events: [
    { type: 'UserGotAngry', intensity: 0.5, name: '生气', cls: 'btn-negative' },
    { type: 'UserIgnoredPet', intensity: 0.8, name: '忽略', cls: 'btn-negative' },
    { type: 'UserDismissedPet', intensity: 0.5, name: '敷衍', cls: 'btn-negative' },
  ]},
]

async function applyEvent(eventType: string, intensity: number) {
  if (!ghost.value) return
  try {
    const result = await invoke<string>('apply_event', { eventType, intensity, description: eventType })
    const parsed = JSON.parse(result)
    if (ghost.value) {
      ghost.value.loveHate = parsed.loveHate
      ghost.value.baseline = parsed.baseline
      ghost.value.impression.overallAffinity = parsed.overallAffinity
    }
    showSuccess(`${eventType} 已应用`)
    await emit('settings-updated', { section: 'ghost' })
  } catch (e: any) {
    showError('事件失败: ' + e)
  }
}

const affinityColor = computed(() => {
  if (!ghost.value) return '#888'
  const lh = ghost.value.loveHate
  if (lh > 50) return '#ff6b9d'
  if (lh > 20) return '#8bc34a'
  if (lh > -20) return '#ffb74d'
  if (lh > -50) return '#ff9800'
  return '#f44336'
})

async function closeWindow() {
  const win = getCurrentWindow()
  await win.hide()
}
</script>

<template>
  <div class="settings-app">
    <div class="settings-card">
      <div class="settings-header">
        <span class="settings-title">设置</span>
        <button class="settings-close" @click.stop="closeWindow">✕</button>
      </div>

      <div class="settings-body">
        <!-- AI Config -->
        <div class="settings-section">
          <div class="settings-section-title">🤖 AI 配置</div>
          <div class="ai-config">
            <label class="config-label">API Endpoint</label>
            <input v-model="aiEndpoint" class="config-input" placeholder="https://api.deepseek.com/v1" />
            <label class="config-label">API Key</label>
            <input v-model="aiApiKey" class="config-input" type="password" placeholder="sk-..." />
            <label class="config-label">Model</label>
            <input v-model="aiModel" class="config-input" placeholder="deepseek-chat" />
            <div class="config-divider"></div>
            <label class="config-subtitle">扩展模型 (可选)</label>
            <label class="config-label">Vision Model</label>
            <input v-model="aiVisionModel" class="config-input" placeholder="gpt-4o / 留空则用主模型" />
            <label class="config-label">Image Model</label>
            <input v-model="aiImageModel" class="config-input" placeholder="dall-e-3 / 留空则不生图" />
            <label class="config-label">生图 Endpoint</label>
            <input v-model="aiImageGenEndpoint" class="config-input" placeholder="留空则用主Endpoint" />
            <label class="config-label">生图 API Key</label>
            <input v-model="aiImageGenApiKey" class="config-input" type="password" placeholder="留空则用主API Key" />
            <p class="field-hint" v-if="aiImageGenEndpoint.trim()">独立生图路径已配置，将使用独立endpoint</p>
            <button @click="saveAIConfig" class="btn btn-generate btn-full">保存配置</button>
          </div>
        </div>

        <!-- Appearance -->
        <div class="settings-section">
          <div class="settings-section-title">🎨 外观</div>
          <div class="renderer-buttons">
            <button @click="switchRenderer('css')" :class="['btn', rendererType === 'css' ? 'btn-ren-active' : 'btn-ren-off']">表情</button>
            <button @click="switchRenderer('spritesheet')" :class="['btn', rendererType === 'spritesheet' ? 'btn-ren-active' : 'btn-ren-off']">精灵图</button>
            <button @click="switchRenderer('lottie')" :class="['btn', rendererType === 'lottie' ? 'btn-ren-active' : 'btn-ren-off']">Lottie</button>
          </div>
          <div v-if="rendererType === 'spritesheet'" class="sprite-config">
            <label class="config-label">精灵图路径</label>
            <input v-model="spriteSrc" class="config-input" placeholder="spritesheet.png" @change="updateSpriteSrc" />
            <label class="config-label">JSON 描述路径</label>
            <input v-model="spriteJsonSrc" class="config-input" placeholder="spritesheet.json" @change="updateSpriteSrc" />
          </div>
          <div v-if="rendererType === 'lottie'" class="sprite-config">
            <label class="config-label">Lottie 动画目录</label>
            <input v-model="lottieSrc" class="config-input" placeholder="/pet/lottie/" @change="updateLottieSrc" />
            <p class="field-hint">目录下应包含: idle.json / happy.json / content.json / curious.json / cold.json / distant.json / speaking.json / surprise.json</p>
          </div>
        </div>

        <!-- Behavior -->
        <div v-if="ghost" class="settings-section">
          <div class="settings-section-title">🔮 行为</div>
          <div class="curiosity-panel">
            <div class="curiosity-status">
              当前状态：<span :class="'curiosity-' + ghost.curiosityLevel.toLowerCase()">{{ ghost.curiosityLevel }}</span>
            </div>
            <div class="curiosity-levels">
              <button @click="setCuriosityLevel('Off')" :class="['btn', ghost.curiosityLevel === 'Off' ? 'btn-cur-active' : 'btn-cur-off']">关闭</button>
              <button @click="setCuriosityLevel('Normal')" :class="['btn', ghost.curiosityLevel === 'Normal' ? 'btn-cur-active' : 'btn-cur-off']">普通</button>
              <button @click="setCuriosityLevel('Enhanced')" :class="['btn', ghost.curiosityLevel === 'Enhanced' ? 'btn-cur-active' : 'btn-cur-off']">增强</button>
            </div>
            <button v-if="ghost.curiosityLevel !== 'Off'" @click="triggerCuriosity" class="btn btn-curiosity-trigger">
              主动探索一下
            </button>
            <p class="curiosity-hint" v-if="ghost.curiosityLevel === 'Enhanced'">增强模式会使用更多AI调用，请留意。</p>
          </div>
          <div class="answering-panel">
            <div class="curiosity-status">
              回应风格：<span :class="answeringMode === 'Companion' ? 'answering-companion' : 'answering-assistant'">{{ answeringMode === 'Companion' ? '陪伴' : '助力' }}</span>
            </div>
            <div class="curiosity-levels">
              <button @click="setAnsweringMode('Companion')" :class="['btn', answeringMode === 'Companion' ? 'btn-cur-active' : 'btn-cur-off']">陪伴</button>
              <button @click="setAnsweringMode('Assistant')" :class="['btn', answeringMode === 'Assistant' ? 'btn-cur-active' : 'btn-cur-off']">助力</button>
            </div>
            <p class="curiosity-hint">{{ answeringMode === 'Companion' ? '人格驱动，心情决定回答意愿' : '优先回答，截图对话自动启用' }}</p>
          </div>
        </div>

        <!-- Personality -->
        <div v-if="ghost" class="settings-section">
          <div class="settings-section-title">🧠 人格数据</div>
          <div class="stat-row">
            <span>好感度</span>
            <span class="stat-value" :style="{ color: affinityColor }">{{ ghost.loveHate.toFixed(1) }}</span>
            <span class="stat-dim">基线 {{ ghost.baseline.toFixed(1) }}</span>
          </div>
          <div class="stat-row">
            <span>印象</span>
            <span class="stat-value">{{ ghost.impression.overallAffinity.toFixed(1) }}</span>
          </div>
          <div class="personality-grid">
            <div class="personality-item" v-for="(val, key) in ghost.personality" :key="key">
              <span class="p-label">{{ personalityLabels[key as string] || key }}</span>
              <div class="p-bar"><div class="p-fill" :style="{width: (val*100)+'%'}"></div></div>
              <span class="p-val">{{ (val*100).toFixed(0) }}</span>
            </div>
          </div>
        </div>

        <!-- Emotional Events -->
        <div v-if="ghost" class="settings-section">
          <div class="settings-section-title">💗 情感事件</div>
          <div class="event-groups">
            <div v-for="group in eventGroups" :key="group.label" class="event-group">
              <div class="event-group-label">{{ group.label }}</div>
              <div class="event-buttons">
                <button
                  v-for="ev in group.events"
                  :key="ev.type"
                  @click.stop="applyEvent(ev.type, ev.intensity)"
                  :class="['btn', ev.cls]"
                >{{ ev.name }}</button>
              </div>
            </div>
          </div>
          <EmotionTimeline />
        </div>

        <!-- Data -->
        <div v-if="ghost" class="settings-section">
          <div class="settings-section-title">💾 数据</div>
          <div class="save-load-panel">
            <input v-model="saveLoadPath" class="config-input" placeholder="文件名 (默认: deskpet.ghost)" />
            <div class="save-load-buttons">
              <button @click="saveGhost" class="btn btn-save">保存灵魂</button>
              <button @click="loadGhost" class="btn btn-load">加载灵魂</button>
            </div>
          </div>
        </div>

        <!-- TTS -->
        <div v-if="ghost" class="settings-section">
          <div class="settings-section-title">🔊 语音</div>
          <div class="tts-panel">
            <label class="tts-toggle">
              <input type="checkbox" v-model="ttsEnabled" @change="saveTTSSettings" />
              <span>开启语音朗读</span>
            </label>
            <div v-if="ttsEnabled" class="tts-rate-panel">
              <label class="config-label">语音引擎</label>
              <select v-model="ttsEngine" @change="saveTTSSettings" class="tts-select">
                <option value="system">系统语音 (离线)</option>
                <option value="edge">Edge TTS (在线/自然)</option>
              </select>
              <div v-if="ttsEngine === 'edge'" style="margin-top: 8px;">
                <label class="config-label">语音选择</label>
                <select v-model="ttsVoice" @change="saveTTSSettings" class="tts-select">
                  <option v-for="v in EDGE_VOICE_OPTIONS" :key="v.id" :value="v.id">{{ v.label }}</option>
                </select>
              </div>
              <label class="config-label" style="margin-top: 12px;">语速</label>
              <div class="slider-row">
                <input type="range" min="0.5" max="2.0" step="0.1" v-model.number="ttsRate" @change="saveTTSSettings" class="tts-slider" />
                <span class="tts-rate-value">{{ ttsRate.toFixed(1) }}x</span>
              </div>
              <label class="config-label" style="margin-top: 12px;">音调</label>
              <div class="slider-row">
                <input type="range" min="0.5" max="2.0" step="0.1" v-model.number="ttsPitch" @change="saveTTSSettings" class="tts-slider" />
                <span class="tts-rate-value">{{ ttsPitch.toFixed(1) }}</span>
              </div>
            </div>
            <p class="tts-hint" v-if="ttsEnabled">
              {{ ttsEngine === 'edge' ? '使用微软 Edge 免费在线语音，质量自然流畅' : '使用系统语音合成' }}
            </p>
          </div>
        </div>

        <!-- Chat History -->
        <div v-if="ghost" class="settings-section">
          <div class="settings-section-title">💬 聊天记录</div>
          <div class="history-panel">
            <button @click="clearHistory" class="btn btn-danger">清除所有聊天记录</button>
            <p class="history-hint">清除后无法恢复，但不会影响记忆系统。</p>
          </div>
        </div>

        <!-- Advanced -->
        <div v-if="ghost" class="settings-section settings-danger">
          <div class="settings-section-title">⚠️ 高级</div>
          <div class="transfer-intro">
            <p class="transfer-desc">传送会将灵魂转移到新文件。原体会消逝，新体会带有微弱的人格偏移——如同生命传递中不可避免的痕迹。</p>
            <p class="transfer-warning">此操作不可逆，请确认。</p>
            <button @click="requestTransfer" class="btn btn-transfer">开始传送</button>
          </div>
        </div>

        <div v-if="error" class="error-msg">{{ error }}</div>
        <div v-if="success" class="success-msg">{{ success }}</div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings-app {
  width: 100vw;
  height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
  box-sizing: border-box;
  font-family: 'Segoe UI', system-ui, -apple-system, sans-serif;
  font-size: 13px;
  color: #333;
  user-select: none;
}

.settings-card {
  background: rgba(255, 255, 255, 0.97);
  backdrop-filter: blur(16px);
  border-radius: 20px;
  box-shadow: 0 2px 16px rgba(0, 0, 0, 0.1), 0 0 0 1px rgba(0, 0, 0, 0.04);
  width: 100%;
  max-width: 360px;
  max-height: calc(100vh - 32px);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.settings-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 18px;
  border-bottom: 1px solid rgba(0, 0, 0, 0.06);
  -webkit-app-region: drag;
}

.settings-header button {
  -webkit-app-region: no-drag;
}

.settings-title {
  font-size: 15px;
  font-weight: 700;
  color: #333;
}

.settings-close {
  width: 28px;
  height: 28px;
  border: none;
  background: transparent;
  border-radius: 8px;
  color: #999;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 14px;
  transition: all 0.15s;
}

.settings-close:hover {
  background: rgba(0, 0, 0, 0.06);
  color: #333;
}

.settings-body {
  padding: 12px 18px 18px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.settings-body::-webkit-scrollbar {
  width: 4px;
}

.settings-body::-webkit-scrollbar-track {
  background: transparent;
}

.settings-body::-webkit-scrollbar-thumb {
  background: rgba(0, 0, 0, 0.15);
  border-radius: 2px;
}

.settings-section {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.settings-section-title {
  font-size: 11px;
  font-weight: 700;
  color: #888;
  text-transform: uppercase;
  letter-spacing: 0.6px;
}

.settings-danger {
  padding: 10px;
  background: rgba(244, 67, 54, 0.04);
  border: 1px solid rgba(244, 67, 54, 0.12);
  border-radius: 12px;
}

.btn {
  padding: 4px 10px;
  border: none;
  border-radius: 8px;
  font-size: 11px;
  cursor: pointer;
  transition: all 0.15s;
  font-weight: 500;
  line-height: 1.4;
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-generate {
  background: linear-gradient(135deg, #ff6b9d, #c084fc);
  color: white;
  border: none;
  padding: 8px 16px;
  font-size: 13px;
  width: 100%;
}

.btn-generate:hover:not(:disabled) {
  opacity: 0.9;
}

.btn-full {
  width: 100%;
  margin-top: 4px;
}

.config-label {
  font-size: 10px;
  font-weight: 600;
  color: #888;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.config-input {
  padding: 5px 10px;
  border: 1.5px solid rgba(192, 132, 252, 0.2);
  border-radius: 6px;
  font-size: 12px;
  outline: none;
  transition: border-color 0.2s;
  background: #fff;
  width: 100%;
  box-sizing: border-box;
}

.config-input:focus {
  border-color: #c084fc;
}

.ai-config {
  background: rgba(192, 132, 252, 0.06);
  border: 1px solid rgba(192, 132, 252, 0.15);
  border-radius: 10px;
  padding: 10px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.field-hint {
  font-size: 10px;
  color: #4caf50;
  margin: 0;
}

.config-divider {
  height: 1px;
  background: rgba(192, 132, 252, 0.15);
  margin: 4px 0;
}

.config-subtitle {
  font-size: 11px;
  font-weight: 600;
  color: #c084fc;
}

.renderer-buttons {
  display: flex;
  gap: 4px;
}

.btn-ren-active {
  font-size: 11px;
  padding: 4px 10px;
  border-radius: 8px;
  cursor: pointer;
  border: none;
  color: white;
  background: linear-gradient(135deg, #ff6b9d, #c084fc);
}

.btn-ren-off {
  background: #f0f0f0;
  color: #666;
  border: 1px solid #ddd;
  font-size: 11px;
  padding: 4px 10px;
  border-radius: 8px;
  cursor: pointer;
  flex: 1;
}

.btn-ren-off:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.sprite-config {
  margin-top: 6px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.curiosity-panel {
  padding: 6px 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.curiosity-status {
  font-size: 12px;
  color: #555;
}

.curiosity-off { color: #aaa; }
.curiosity-normal { color: #4caf50; }
.curiosity-enhanced { color: #7c4dff; font-weight: 700; }
.answering-companion { color: #ff9800; font-weight: 600; }
.answering-assistant { color: #2196f3; font-weight: 600; }
.answering-panel {
  padding: 6px 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-top: 4px;
  padding-top: 10px;
  border-top: 1px solid rgba(0, 0, 0, 0.06);
}

.curiosity-levels {
  display: flex;
  gap: 4px;
}

.btn-cur-off {
  background: #f0f0f0;
  color: #666;
  border: 1px solid #ddd;
  font-size: 11px;
  padding: 4px 10px;
  border-radius: 8px;
  cursor: pointer;
  flex: 1;
}

.btn-cur-active {
  font-size: 11px;
  padding: 4px 10px;
  border-radius: 8px;
  cursor: pointer;
  border: none;
  flex: 1;
  color: white;
  background: #4caf50;
}

.btn-cur-off:hover { background: #e5e5e5; }

.btn-curiosity-trigger {
  background: linear-gradient(135deg, #4caf50, #8bc34a);
  color: white;
  border: none;
  padding: 6px 14px;
  border-radius: 8px;
  font-size: 12px;
  cursor: pointer;
  width: 100%;
}

.btn-curiosity-trigger:hover {
  opacity: 0.9;
}

.curiosity-hint {
  font-size: 10px;
  color: #ff9800;
  margin: 0;
}

.save-load-panel {
  padding: 6px 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.save-load-buttons {
  display: flex;
  gap: 6px;
}

.btn-save {
  flex: 1;
  background: #e3f2fd;
  color: #1565c0;
  border: 1px solid #90caf9;
  font-size: 11px;
  padding: 6px 12px;
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.15s;
}

.btn-save:hover {
  background: #bbdefb;
}

.btn-load {
  flex: 1;
  background: #f3e5f5;
  color: #7b1fa2;
  border: 1px solid #ce93d8;
  font-size: 11px;
  padding: 6px 12px;
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.15s;
}

.btn-load:hover {
  background: #e1bee7;
}

.transfer-intro {
  padding: 6px 0;
}

.transfer-desc {
  font-size: 11px;
  color: #777;
  line-height: 1.6;
  margin: 4px 0;
}

.transfer-warning {
  font-size: 10px;
  color: #f44336;
  margin: 2px 0 8px;
}

.btn-transfer {
  background: linear-gradient(135deg, #7c4dff, #448aff);
  color: white;
  border: none;
  padding: 8px 16px;
  border-radius: 10px;
  font-size: 13px;
  width: 100%;
  cursor: pointer;
  font-weight: 600;
  transition: opacity 0.2s;
}

.btn-transfer:hover {
  opacity: 0.9;
}

.error-msg {
  color: #f44336;
  font-size: 11px;
  padding: 6px 8px;
  background: #ffebee;
  border-radius: 8px;
}

.success-msg {
  color: #4caf50;
  font-size: 11px;
  padding: 6px 8px;
  background: #e8f5e9;
  border-radius: 8px;
}

.tts-panel {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.tts-toggle {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  cursor: pointer;
}

.tts-toggle input[type="checkbox"] {
  width: 16px;
  height: 16px;
  accent-color: #ff6b9d;
}

.tts-rate-panel {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.slider-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.tts-slider {
  flex: 1;
  accent-color: #ff6b9d;
}

.tts-rate-value {
  font-size: 12px;
  color: #666;
  min-width: 28px;
}

.tts-select {
  width: 100%;
  padding: 6px 8px;
  border: 1px solid #ccc;
  border-radius: 6px;
  font-size: 13px;
  background: #fff;
  color: #333;
}

.tts-hint {
  font-size: 10px;
  color: #999;
  margin: 0;
}

.history-panel {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.history-hint {
  font-size: 10px;
  color: #999;
  margin: 0;
}

.btn-danger {
  background: #ffebee;
  color: #c62828;
  border: 1px solid #ef9a9a;
  padding: 6px 14px;
  border-radius: 8px;
  font-size: 12px;
  cursor: pointer;
  width: 100%;
}

.btn-danger:hover {
  background: #ffcdd2;
}

.stat-row {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: #666;
  padding: 2px 0;
}

.stat-value {
  font-weight: 700;
  font-size: 13px;
}

.stat-dim {
  color: #aaa;
  font-size: 11px;
}

.personality-grid {
  display: flex;
  flex-direction: column;
  gap: 3px;
  padding: 4px 0;
}

.personality-item {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
}

.p-label {
  width: 32px;
  text-align: right;
  color: #888;
  flex-shrink: 0;
}

.p-bar {
  flex: 1;
  height: 5px;
  background: #eee;
  border-radius: 3px;
  overflow: hidden;
}

.p-fill {
  height: 100%;
  background: linear-gradient(90deg, #ff6b9d, #c084fc);
  border-radius: 3px;
  transition: width 0.5s ease;
}

.p-val {
  width: 26px;
  text-align: right;
  color: #888;
  font-size: 10px;
}

.event-groups {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 4px 0;
}

.event-group {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.event-group-label {
  font-size: 10px;
  font-weight: 600;
  color: #aaa;
  text-transform: uppercase;
  letter-spacing: 0.3px;
}

.event-buttons {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.event-buttons .btn {
  font-size: 11px;
  padding: 4px 10px;
  border-radius: 8px;
  cursor: pointer;
  border: none;
  transition: all 0.15s;
}

.btn-positive {
  background: #e8f5e9;
  color: #2e7d32;
  border: 1px solid #a5d6a7;
}

.btn-positive:hover {
  background: #c8e6c9;
}

.btn-neutral {
  background: #f5f5f5;
  color: #666;
  border: 1px solid #ddd;
}

.btn-neutral:hover {
  background: #eee;
}

.btn-negative {
  background: #ffebee;
  color: #c62828;
  border: 1px solid #ef9a9a;
}

.btn-negative:hover {
  background: #ffcdd2;
}
</style>
