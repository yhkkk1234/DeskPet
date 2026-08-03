import { type Ref } from 'vue'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'

export interface WindowManagerDeps {
  asleep: Ref<boolean>
  wakeUpPet: () => Promise<void>
  pushSystemMessage: (content: string, persist?: boolean) => void
}

/** 子窗口管理：对话窗口 / 设置窗口的创建、复用与聚焦 */
export function useWindowManager(deps: WindowManagerDeps) {
  // 防止 openChatWindow 重入：getByLabel 与 new WebviewWindow 非原子，
  // 连按热键/双击宠物/热键+点击叠加时会双触发，产生"一个能用一个僵尸"的双窗口。
  let chatOpening = false

  async function openSettingsWindow() {
    let settingsWin = await WebviewWindow.getByLabel('settings')
    if (!settingsWin) {
      settingsWin = new WebviewWindow('settings', {
        url: 'settings.html',
        title: 'DeskPet - 设置',
        width: 380,
        height: 600,
        minWidth: 320,
        minHeight: 400,
        resizable: true,
        transparent: true,
        decorations: false,
        alwaysOnTop: true,
        skipTaskbar: false,
        visible: false,
      })
      // 窗口创建为不可见，由 Settings.vue 在 onMounted 加载完成后自己 show()，避免默认位置闪现。
      settingsWin.once('tauri://error', (e: any) => {
        console.error('Settings window creation error:', e)
        settingsWin?.close().catch(() => {})
      })
    } else {
      // 已存在的窗口：直接显示并聚焦
      const visible = await settingsWin.isVisible()
      if (visible) {
        await settingsWin.setFocus()
      } else {
        await settingsWin.show()
        await settingsWin.setFocus()
      }
    }
  }

  async function openChatWindow() {
    // 重入保护：连按热键/双击宠物等并发触发时，第二次直接返回，
    // 避免两次 getByLabel 都返回 null 导致 new WebviewWindow 两次产生僵尸窗口。
    if (chatOpening) return
    chatOpening = true
    try {
      if (deps.asleep.value) {
        await deps.wakeUpPet()
      }
      let chatWin = await WebviewWindow.getByLabel('chat')
      if (!chatWin) {
      chatWin = new WebviewWindow('chat', {
        url: 'chat.html',
        title: 'DeskPet - 对话',
        width: 360,
        height: 500,
        minWidth: 300,
        minHeight: 300,
        // resizable: false —— 禁用系统边缘拖拽（透明窗口看不见边缘，且系统 resize 与
        // WebView 内容可能不同步导致气泡不跟随）。resize 由 ChatWindow 内的
        // 透明热区（气泡边缘）通过 setSize API 驱动，该路径已验证气泡同步跟随。
        resizable: false,
        transparent: true,
        decorations: false,
        alwaysOnTop: true,
        skipTaskbar: false,
        visible: false,
      })
        // 兜底：若 label 冲突（理论上已被 chatOpening 拦住，此处为双保险）
        // 导致 Rust 侧创建失败，捕获 tauri://error 并清理可能已显示的僵尸窗口。
        chatWin.once('tauri://error', (e: any) => {
          console.error('Chat window creation error:', e)
          deps.pushSystemMessage(`对话窗口创建失败: ${e?.payload || e}`, false)
          chatWin?.close().catch(() => {})
        })
        // 窗口创建为不可见，由 ChatWindow.vue 在定位完成后自己 show()，避免先在默认位置闪现再移到目标位置。
      } else {
        // 已存在的窗口：直接显示并聚焦
        const visible = await chatWin.isVisible()
        if (visible) {
          await chatWin.setFocus()
        } else {
          await chatWin.show()
          await chatWin.setFocus()
        }
      }
    } finally {
      chatOpening = false
    }
  }

  async function wakeUpAndOpenChat() {
    if (deps.asleep.value) {
      await deps.wakeUpPet()
    }
    await openChatWindow()
  }

  return { openChatWindow, openSettingsWindow, wakeUpAndOpenChat }
}
