import { createApp } from 'vue'
import './style.css'
import { applyTheme } from './composables/useTheme'
import Settings from './Settings.vue'

applyTheme()

createApp(Settings).mount('#app')
