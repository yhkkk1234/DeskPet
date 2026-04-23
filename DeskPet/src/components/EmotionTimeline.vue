<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core'
import { ref, onMounted, onUnmounted } from 'vue'

interface EmotionEvent {
  eventType: string
  intensity: number
  loveHateDelta: number
  baselineDelta: number
  timestamp: string
  description: string
}

const events = ref<EmotionEvent[]>([])
const loading = ref(false)
let refreshTimer: ReturnType<typeof setInterval> | null = null

const eventTypeLabels: Record<string, string> = {
  UserInitiatedChat: '主动聊天',
  UserCaredAboutPet: '关心',
  UserPraisedPet: '夸奖',
  UserSharedPersonalStory: '分享',
  UserCelebratedTogether: '庆祝',
  FirstConversation: '初次对话',
  BirthdayCelebrated: '生日',
  UserIgnoredPet: '被忽略',
  UserGotAngry: '主人生气',
  UserDismissedPet: '被敷衍',
  NormalChat: '闲聊',
  CuriosityTriggered: '好奇',
}

const eventTypeColors: Record<string, string> = {
  UserInitiatedChat: '#4caf50',
  UserCaredAboutPet: '#e91e63',
  UserPraisedPet: '#ff9800',
  UserSharedPersonalStory: '#9c27b0',
  UserCelebratedTogether: '#f44336',
  FirstConversation: '#ff6b9d',
  BirthdayCelebrated: '#e91e63',
  UserIgnoredPet: '#9e9e9e',
  UserGotAngry: '#f44336',
  UserDismissedPet: '#795548',
  NormalChat: '#2196f3',
  CuriosityTriggered: '#00bcd4',
}

async function fetchHistory() {
  loading.value = true
  try {
    const result = await invoke<{ events: EmotionEvent[]; total: number }>('get_emotion_history')
    events.value = result.events || []
  } catch (e) {
    console.warn('Failed to fetch emotion history:', e)
  } finally {
    loading.value = false
  }
}

function formatTime(ts: string): string {
  try {
    const d = new Date(ts)
    const now = new Date()
    const diffMs = now.getTime() - d.getTime()
    const diffMin = Math.floor(diffMs / 60000)
    if (diffMin < 1) return '刚刚'
    if (diffMin < 60) return `${diffMin}分钟前`
    const diffHr = Math.floor(diffMin / 60)
    if (diffHr < 24) return `${diffHr}小时前`
    const diffDay = Math.floor(diffHr / 24)
    return `${diffDay}天前`
  } catch {
    return ts
  }
}

function getLabel(type: string): string {
  return eventTypeLabels[type] || type
}

function getColor(type: string): string {
  return eventTypeColors[type] || '#999'
}

onMounted(() => {
  fetchHistory()
  refreshTimer = setInterval(fetchHistory, 30000)
})

onUnmounted(() => {
  if (refreshTimer) clearInterval(refreshTimer)
})
</script>

<template>
  <div class="emotion-timeline">
    <div class="timeline-header">
      <span class="timeline-title">情感时间线</span>
      <button class="timeline-refresh" @click="fetchHistory" :disabled="loading">
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
          <path d="M1 6C1 3.24 3.24 1 6 1C8.76 1 11 3.24 11 6C11 8.76 8.76 11 6 11" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
          <path d="M1 9V6H4" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
      </button>
    </div>
    <div v-if="loading && events.length === 0" class="timeline-empty">加载中...</div>
    <div v-else-if="events.length === 0" class="timeline-empty">暂无情感事件记录</div>
    <div v-else class="timeline-list">
      <div v-for="(event, i) in events.slice().reverse()" :key="i" class="timeline-item">
        <div class="timeline-dot" :style="{ background: getColor(event.eventType) }" />
        <div class="timeline-content">
          <div class="timeline-event-row">
            <span class="timeline-event-label" :style="{ color: getColor(event.eventType) }">{{ getLabel(event.eventType) }}</span>
            <span class="timeline-time">{{ formatTime(event.timestamp) }}</span>
          </div>
          <div v-if="event.description" class="timeline-desc">{{ event.description }}</div>
          <div class="timeline-delta">
            <span v-if="event.loveHateDelta !== 0" class="delta-lh" :class="event.loveHateDelta > 0 ? 'delta-pos' : 'delta-neg'">
              好感 {{ event.loveHateDelta > 0 ? '+' : '' }}{{ event.loveHateDelta.toFixed(1) }}
            </span>
            <span v-if="event.baselineDelta !== 0" class="delta-bl">
              基线 {{ event.baselineDelta > 0 ? '+' : '' }}{{ event.baselineDelta.toFixed(1) }}
            </span>
          </div>
        </div>
      </div>
    </div>
    <div v-if="events.length > 0" class="timeline-footer">
      共 {{ events.length }} 条记录
    </div>
  </div>
</template>

<style scoped>
.emotion-timeline {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.timeline-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.timeline-title {
  font-size: 11px;
  font-weight: 600;
  color: #888;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.timeline-refresh {
  background: none;
  border: none;
  color: #999;
  cursor: pointer;
  padding: 2px;
  border-radius: 4px;
  transition: all 0.15s;
  display: flex;
  align-items: center;
}

.timeline-refresh:hover {
  color: #666;
  background: rgba(0,0,0,0.04);
}

.timeline-refresh:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.timeline-empty {
  font-size: 11px;
  color: #aaa;
  text-align: center;
  padding: 12px 0;
}

.timeline-list {
  display: flex;
  flex-direction: column;
  gap: 0;
  max-height: 200px;
  overflow-y: auto;
  padding-right: 2px;
}

.timeline-list::-webkit-scrollbar {
  width: 3px;
}

.timeline-list::-webkit-scrollbar-thumb {
  background: rgba(0,0,0,0.12);
  border-radius: 2px;
}

.timeline-item {
  display: flex;
  gap: 8px;
  padding: 4px 0;
  position: relative;
}

.timeline-item:not(:last-child)::after {
  content: '';
  position: absolute;
  left: 4px;
  top: 16px;
  bottom: -4px;
  width: 1px;
  background: rgba(0,0,0,0.06);
}

.timeline-dot {
  width: 9px;
  height: 9px;
  border-radius: 50%;
  flex-shrink: 0;
  margin-top: 3px;
}

.timeline-content {
  flex: 1;
  min-width: 0;
}

.timeline-event-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
}

.timeline-event-label {
  font-size: 11px;
  font-weight: 600;
}

.timeline-time {
  font-size: 10px;
  color: #bbb;
  flex-shrink: 0;
}

.timeline-desc {
  font-size: 10px;
  color: #777;
  line-height: 1.4;
  margin-top: 1px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.timeline-delta {
  display: flex;
  gap: 8px;
  margin-top: 1px;
}

.delta-lh, .delta-bl {
  font-size: 10px;
  font-weight: 500;
}

.delta-pos { color: #4caf50; }
.delta-neg { color: #f44336; }
.delta-bl { color: #999; }

.timeline-footer {
  font-size: 10px;
  color: #bbb;
  text-align: center;
  padding-top: 2px;
}
</style>
