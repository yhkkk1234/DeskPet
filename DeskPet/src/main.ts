import { createApp } from 'vue'
import './style.css'
import { applyTheme } from './composables/useTheme'
import App from './App.vue'

applyTheme()

createApp(App).mount('#app')