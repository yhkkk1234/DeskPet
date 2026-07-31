import { invoke } from '@tauri-apps/api/core'

/** 虚拟屏幕物理边界（覆盖所有显示器，副屏在左侧时 x/y 可能为负） */
interface VirtualScreenBounds {
  x: number
  y: number
  width: number
  height: number
}

let cachedBounds: VirtualScreenBounds | null = null

/**
 * 获取所有显示器合集（虚拟屏幕）的物理像素边界。
 * 后端用 Win32 虚拟屏幕 API（SM_XVIRTUALSCREEN 等），比 window.screen
 * 只返回主屏更准确，多屏环境下窗口定位不会跑到屏幕外或错误位置。
 */
export async function getVirtualScreenBounds(): Promise<VirtualScreenBounds> {
  if (cachedBounds) return cachedBounds
  cachedBounds = await invoke<VirtualScreenBounds>('get_virtual_screen_bounds')
  return cachedBounds
}

/**
 * 将逻辑像素坐标夹紧到虚拟屏幕范围内（防止窗口被放到屏幕外）。
 * @param x / y 目标逻辑坐标（含负值场景，副屏在左侧时允许为负）
 * @param winLogW / winLogH 窗口逻辑尺寸
 * @param scale 设备像素比（用于物理 → 逻辑换算）
 */
export async function clampToVirtualScreen(
  x: number,
  y: number,
  winLogW: number,
  winLogH: number,
  scale: number,
): Promise<{ x: number; y: number }> {
  const bounds = await getVirtualScreenBounds()
  const bx = bounds.x / scale
  const by = bounds.y / scale
  const bw = bounds.width / scale
  const bh = bounds.height / scale
  return {
    x: Math.max(bx, Math.min(x, bx + bw - winLogW)),
    y: Math.max(by, Math.min(y, by + bh - winLogH)),
  }
}
