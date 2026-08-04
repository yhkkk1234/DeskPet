import { ref } from 'vue'
import { emit } from '@tauri-apps/api/event'

export type ThemeKey = 'classic' | 'midnight' | 'vivid' | 'neon' | 'cream'

export interface ThemeInfo {
  key: ThemeKey
  name: string
  desc: string
  /** 主题卡片预览主色 */
  primary: string
  secondary: string
  /** 深色主题（文字/气泡对比调整） */
  dark: boolean
  /** 卡片预览背景 */
  previewBg: string
  /** 卡片预览文字色 */
  previewText: string
}

export const THEMES: ThemeInfo[] = [
  { key: 'classic', name: '经典漫画', desc: '白底黑边，可爱手绘风', primary: '#ff6b9d', secondary: '#c084fc', dark: false, previewBg: '#ffffff', previewText: '#333333' },
  { key: 'midnight', name: '暗夜流光', desc: '深色玻璃拟态，霓虹渐变', primary: '#6e8efb', secondary: '#a777e3', dark: true, previewBg: '#161928', previewText: '#e8eaf2' },
  { key: 'vivid', name: '元气跃动', desc: '糖果高饱和，弹跳动效', primary: '#ff9a56', secondary: '#ff2ec4', dark: false, previewBg: '#fffcf7', previewText: '#4a3f6b' },
  { key: 'neon', name: '赛博霓虹', desc: '青品霓虹，扫描线光效', primary: '#00f0ff', secondary: '#ff2ec4', dark: true, previewBg: '#070b13', previewText: '#d8f6ff' },
  { key: 'cream', name: '清新奶油', desc: '低饱和奶油色，柔和治愈', primary: '#7fc8a9', secondary: '#ffb894', dark: false, previewBg: '#fdf8f0', previewText: '#5b5148' },
]

const STORAGE_KEY = 'deskpet_theme'

export function loadThemeKey(): ThemeKey {
  const saved = localStorage.getItem(STORAGE_KEY)
  if (saved && THEMES.some(t => t.key === saved)) return saved as ThemeKey
  return 'classic'
}

/** 将主题应用到当前窗口（写 <html data-theme> + localStorage），返回生效主题 */
export function applyTheme(): ThemeKey {
  const key = loadThemeKey()
  document.documentElement.dataset.theme = key
  localStorage.setItem(STORAGE_KEY, key)
  return key
}

export function useTheme() {
  const theme = ref<ThemeKey>(applyTheme())

  function setTheme(key: ThemeKey) {
    theme.value = key
    localStorage.setItem(STORAGE_KEY, key)
    document.documentElement.dataset.theme = key
    // 广播到其他窗口（聊天窗口/桌宠主窗口实时换肤）
    emit('settings-updated', { section: 'theme' }).catch(() => {})
  }

  return { theme, setTheme }
}
