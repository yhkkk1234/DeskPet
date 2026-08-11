export type HeadDirection =
  | 'center'
  | 'up-left' | 'up' | 'up-right'
  | 'left' | 'right'
  | 'down-left' | 'down' | 'down-right'

export interface MouseTrackingOptions {
  /** 追踪半径（逻辑像素）：鼠标距宠物中心超过该值则视为“远处活动”，头部回正 */
  radius?: number
  /** 滞回角（度）：与当前方向中心角的夹角超过该值才切换方向，防方向边界抖动 */
  hysteresisDeg?: number
  /** 鼠标静止回正延迟（ms）：范围内鼠标停止移动超过该时长后头部回正，0 = 不启用 */
  settleMs?: number
  /** 移动判定阈值（逻辑像素）：低于该位移视为静止 */
  moveThresholdPx?: number
}

const DIRECTION_INDEX: Record<number, HeadDirection> = {
  0: 'right',
  1: 'down-right',
  2: 'down',
  3: 'down-left',
  4: 'left',
  [-1]: 'up-right',
  [-2]: 'up',
  [-3]: 'up-left',
  [-4]: 'left',
}

// 各方向帧的中心角（弧度），滞回比较以方向中心角为基准
const DIRECTION_ANGLE: Record<HeadDirection, number> = {
  right: 0,
  'down-right': Math.PI / 4,
  down: Math.PI / 2,
  'down-left': (3 * Math.PI) / 4,
  left: Math.PI,
  'up-left': (-3 * Math.PI) / 4,
  up: -Math.PI / 2,
  'up-right': -Math.PI / 4,
  center: 0,
}

/**
 * 头部视觉追踪核心逻辑（纯计算，无 DOM/定时器，由外部 rAF 循环驱动）。
 * 语义：追踪鼠标“位置”相对宠物的方位——鼠标在宠物哪个方向，头部就看向哪边。
 * 鼠标静止超过 settleMs → 回正；鼠标距目标超过 radius → 回正。
 */
export function useMouseTracking(opts: MouseTrackingOptions = {}) {
  const radius = opts.radius ?? 400
  const hysteresisDeg = opts.hysteresisDeg ?? 22.5
  const settleMs = opts.settleMs ?? 2000
  const moveThresholdPx = opts.moveThresholdPx ?? 5

  let lastX = 0
  let lastY = 0
  let hasLast = false
  let lastMoveTime = 0
  let currentAngle: number | null = null
  let direction: HeadDirection = 'center'

  function update(cursorX: number, cursorY: number, targetX: number, targetY: number): HeadDirection {
    const now = performance.now()
    const moved = hasLast
      && (cursorX - lastX) * (cursorX - lastX) + (cursorY - lastY) * (cursorY - lastY) >= moveThresholdPx * moveThresholdPx
    if (moved) lastMoveTime = now
    hasLast = true
    lastX = cursorX
    lastY = cursorY

    const rx = cursorX - targetX
    const ry = cursorY - targetY
    const dist2 = rx * rx + ry * ry
    const inRange = dist2 <= radius * radius

    // 静止或范围外：超过 settleMs 未移动则回正（保持最后方向直到超时）
    if (!moved || !inRange) {
      if (direction !== 'center' && now - lastMoveTime >= settleMs) {
        direction = 'center'
        currentAngle = null
      }
      return direction
    }

    const angle = Math.atan2(ry, rx)
    const dirIndex = Math.round(angle / (Math.PI / 4))

    if (currentAngle !== null) {
      let diff = Math.abs(angle - currentAngle)
      if (diff > Math.PI) diff = Math.PI * 2 - diff
      if (diff < (hysteresisDeg * Math.PI) / 180) {
        return direction
      }
    }

    const newDir = DIRECTION_INDEX[dirIndex]
    if (newDir && newDir !== direction) {
      direction = newDir
      currentAngle = DIRECTION_ANGLE[newDir]
    }
    return direction
  }

  function reset() {
    hasLast = false
    currentAngle = null
    direction = 'center'
  }

  return { update, reset }
}
