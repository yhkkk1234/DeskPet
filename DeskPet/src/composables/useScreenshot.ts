import { type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { emit, listen } from '@tauri-apps/api/event'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { LogicalPosition, PhysicalPosition, PhysicalSize } from '@tauri-apps/api/dpi'
import { clampToVirtualScreen } from './useScreenBounds'
import type { GhostStatus } from './useGhost'
import type { AnimationState } from './useAnimation'

export type AnsweringModeValue = 'Companion' | 'Assistant'

export interface ScreenshotRegion {
  x: number
  y: number
  width: number
  height: number
}

export interface ScreenshotDeps {
  ghost: Ref<GhostStatus | null>
  screenshotScreenRegion: Ref<ScreenshotRegion>
  screenshotAnalysisLoading: Ref<boolean>
  answeringMode: Ref<AnsweringModeValue>
  previousAnsweringMode: Ref<AnsweringModeValue | null>
  playOneShot: (state: AnimationState, duration: number) => Promise<void>
  openChatWindow: () => Promise<void>
  pushSystemMessage: (content: string, persist?: boolean) => void
  petX: Ref<number>
  petY: Ref<number>
}

/** 截图识图全流程：触发截图 → overlay 选区 → 数据传递 → AI 分析 → 桌宠移动 */
export function useScreenshot(deps: ScreenshotDeps) {
  async function triggerScreenshot() {
    if (deps.screenshotAnalysisLoading.value) return
    deps.screenshotAnalysisLoading.value = true

    try {
      const base64 = await invoke<string>('capture_screenshot')
      await invoke('store_screenshot_data', { data: base64 })
      await invoke('close_screenshot_window')

      // 从后端获取虚拟屏幕的物理边界（所有显示器合集），
      // 用物理坐标创建/定位 overlay 窗口，避免多屏 DPR 不一致时的坐标错位。
      let bounds: { x: number; y: number; width: number; height: number } | null = null
      try {
        bounds = await invoke<{ x: number; y: number; width: number; height: number }>('get_virtual_screen_bounds')
      } catch (e) {
        console.warn('Failed to get virtual screen bounds:', e)
      }

      const overlayOpts: Record<string, unknown> = {
        url: 'screenshot.html',
        title: 'DeskPet - 截图',
        decorations: false,
        transparent: true,
        alwaysOnTop: true,
        skipTaskbar: true,
        resizable: false,
        focus: true,
      }

      if (bounds && bounds.width > 0 && bounds.height > 0) {
        // 先用最小尺寸创建，再用物理像素精确定位和调整大小
        overlayOpts.width = 100
        overlayOpts.height = 100
      } else {
        // 兜底：获取失败时用 fullscreen（只在主屏）
        overlayOpts.fullscreen = true
      }

      const overlay = new WebviewWindow('screenshot-overlay', overlayOpts)

      // 创建后用物理像素精确定位和调整大小，覆盖整个虚拟屏幕
      if (bounds && bounds.width > 0 && bounds.height > 0) {
        overlay.once('tauri://created', async () => {
          try {
            await overlay.setPosition(new PhysicalPosition(bounds!.x, bounds!.y))
            await overlay.setSize(new PhysicalSize(bounds!.width, bounds!.height))
          } catch (e) {
            console.warn('Failed to position/size overlay:', e)
          }
        })
      }

      overlay.once('tauri://error', (e: any) => {
        console.error('Screenshot overlay creation error:', e)
        deps.pushSystemMessage(`截图窗口创建失败: ${e?.payload || e}`, false)
        deps.screenshotAnalysisLoading.value = false
      })
    } catch (e: any) {
      deps.screenshotAnalysisLoading.value = false
      deps.pushSystemMessage(`截图失败: ${e}`, false)
    }
  }

  async function analyzeScreenshot(base64: string) {
    if (!deps.ghost.value) {
      deps.pushSystemMessage('请先生成桌宠灵魂，再进行截图识别。', false)
      return
    }

    movePetToScreenshotRegion()
    deps.playOneShot('surprise', 400)

    if (deps.answeringMode.value === 'Companion') {
      deps.previousAnsweringMode.value = 'Companion'
      deps.answeringMode.value = 'Assistant'
      invoke('set_answering_mode', { mode: 'Assistant' }).catch(() => {})
    }

    await deps.openChatWindow()
    // 等待对话窗口就绪（监听器注册完成）再发截图数据，避免事件丢失。
    // 新建窗口：onMounted 末尾会 emit chat:ready；已存在窗口：监听器早已注册，直接发送。
    const chatWin = await WebviewWindow.getByLabel('chat')
    const alreadyVisible = chatWin ? await chatWin.isVisible() : false
    if (alreadyVisible) {
      // 窗口此前已存在且可见（openChatWindow 走的 else 分支直接 setFocus），
      // 监听器早就注册好了，直接发截图数据。
      try {
        await emit('chat:open-with-screenshot', { base64 })
      } catch {
        deps.pushSystemMessage('截图数据发送到对话窗口失败', false)
      }
    } else {
      // 新建窗口：等 chat:ready（带 2s 超时兜底，避免异常时永久卡住）
      let readyFired = false
      const readyUnlisten = await listen('chat:ready', () => { readyFired = true })
      const startWait = Date.now()
      const waitReady = () => {
        if (readyFired) {
          readyUnlisten()
          emit('chat:open-with-screenshot', { base64 }).catch(() => {
            deps.pushSystemMessage('截图数据发送到对话窗口失败', false)
          })
          return
        }
        if (Date.now() - startWait > 2000) {
          readyUnlisten()
          emit('chat:open-with-screenshot', { base64 }).catch(() => {})
          return
        }
        setTimeout(waitReady, 50)
      }
      waitReady()
    }

    deps.screenshotAnalysisLoading.value = false
  }

  function restoreAnsweringMode() {
    if (deps.previousAnsweringMode.value) {
      deps.answeringMode.value = deps.previousAnsweringMode.value
      invoke('set_answering_mode', { mode: deps.previousAnsweringMode.value }).catch(() => {})
      deps.previousAnsweringMode.value = null
    }
  }

  async function movePetToScreenshotRegion() {
    try {
      const win = getCurrentWindow()
      const region = deps.screenshotScreenRegion.value
      if (!region || (region.width === 0 && region.height === 0)) return

      const winSize = await win.innerSize()
      const scaleFactor = await win.scaleFactor()
      const winLogicalW = winSize.width / scaleFactor
      const winLogicalH = winSize.height / scaleFactor

      const targetScreenX = region.x / scaleFactor + region.width / scaleFactor + 20

      // 用虚拟屏幕（所有显示器合集）边界夹紧，替代 window.screen（仅主屏）
      const clamped = await clampToVirtualScreen(
        targetScreenX,
        region.y / scaleFactor - 40,
        winLogicalW,
        winLogicalH,
        scaleFactor,
      )

      await win.setPosition(new LogicalPosition(Math.round(clamped.x), Math.round(clamped.y)))
      deps.petX.value = 0
      deps.petY.value = 0
    } catch (e) {
      console.warn('Failed to move window to screenshot region:', e)
    }
  }

  return { triggerScreenshot, analyzeScreenshot, restoreAnsweringMode, movePetToScreenshotRegion }
}
