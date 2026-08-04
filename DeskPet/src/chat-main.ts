import { createApp } from 'vue'
import './style.css'
import { applyTheme } from './composables/useTheme'
import ChatWindow from './ChatWindow.vue'

applyTheme()

createApp(ChatWindow).mount('#app')
