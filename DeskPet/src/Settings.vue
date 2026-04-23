<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { emit } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { ref, onMounted } from 'vue'
import type { RendererType } from './composables/usePetRenderer'

const aiEndpoint = ref('https://api.deepseek.com/v1')
const aiApiKey = ref('')
const aiModel = ref('deepseek-chat')

const rendererType = ref<RendererType>('spritesheet')
const spriteSrc = ref('/pet/pet_spritesheet.png')
const spriteJsonSrc = ref('/pet/pet_spritesheet.json')

const ghost = ref<any>(null)
const saveLoadPath = ref('')
const error = ref('')
const success = ref('')

function loadLocalStorage() {
  rendererType.value = (localStorage.getItem('deskpet_renderer_type') as RendererType) || 'spritesheet'
  spriteSrc.value = localStorage.getItem('deskpet_sprite_src') || '/pet/pet_spritesheet.png'
  spriteJsonSrc.value = localStorage.getItem('deskpet_sprite_json_src') || '/pet/pet_spritesheet.json'
  aiEndpoint.value = localStorage.getItem('deskpet_ai_endpoint') || 'https://api.deepseek.com/v1'
  aiModel.value = localStorage.getItem('deskpet_ai_model') || 'deepseek-chat'
}

async function fetchGhostStatus() {
  try {
    const status = await invoke<string>('get_ghost_status')
    ghost.value = JSON.parse(status)
  } catch (e) {
    // 没有 ghost 也没关系
  }
}

onMounted(() => {
  loadLocalStorage()
  fetchGhostStatus()
})

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

async function saveAIConfig() {
  if (!aiEndpoint.value.trim()) { showError('请填写 API Endpoint'); return }
  if (!aiApiKey.value.trim()) { showError('请填写 API Key'); return }
  if (!aiModel.value.trim()) { showError('请填写 Model 名称'); return }

  try {
    await invoke('configure_ai', {
      endpoint: aiEndpoint.value.trim(),
      apiKey: aiApiKey.value.trim(),
      model: aiModel.value.trim(),
    })
    localStorage.setItem('deskpet_ai_endpoint', aiEndpoint.value.trim())
    localStorage.setItem('deskpet_ai_model', aiModel.value.trim())
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
            <button @click="saveAIConfig" class="btn btn-generate btn-full">保存配置</button>
          </div>
        </div>

        <!-- Appearance -->
        <div class="settings-section">
          <div class="settings-section-title">🎨 外观</div>
          <div class="renderer-buttons">
            <button @click="switchRenderer('css')" :class="['btn', rendererType === 'css' ? 'btn-ren-active' : 'btn-ren-off']">表情</button>
            <button @click="switchRenderer('spritesheet')" :class="['btn', rendererType === 'spritesheet' ? 'btn-ren-active' : 'btn-ren-off']">精灵图</button>
            <button @click="switchRenderer('spine')" :class="['btn', rendererType === 'spine' ? 'btn-ren-active' : 'btn-ren-off']" disabled>Spine</button>
          </div>
          <div v-if="rendererType === 'spritesheet'" class="sprite-config">
            <label class="config-label">精灵图路径</label>
            <input v-model="spriteSrc" class="config-input" placeholder="spritesheet.png" @change="updateSpriteSrc" />
            <label class="config-label">JSON 描述路径</label>
            <input v-model="spriteJsonSrc" class="config-input" placeholder="spritesheet.json" @change="updateSpriteSrc" />
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
</style>
