import { createApp } from 'vue'
import './style.css'
import { applyTheme } from './composables/useTheme'
import ScreenshotOverlay from './ScreenshotOverlay.vue'

applyTheme()

createApp(ScreenshotOverlay).mount('#app')
