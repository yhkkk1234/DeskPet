import { ref } from 'vue'

export type RendererType = 'css' | 'spritesheet' | 'lottie' | 'spine'

export interface SpriteConfig {
  src: string
  jsonSrc: string
  frameWidth: number
  frameHeight: number
  scale: number
  rows: number
  cols: number
}

export interface LottieConfig {
  src: string  // path to directory containing lottie JSONs
}

/**
 * 头部视觉追踪素材配置（IDLE 状态下叠加的 9 宫格头部方向帧）。
 * 宫格布局约定（cols×rows，按行优先）：
 *   row0: up-left, up, up-right
 *   row1: left,   center, right
 *   row2: down-left, down, down-right
 *
 * 素材约定：每格 = Idle 帧从肩膀往下截掉后的上半部分（头+脖子），
 * 渲染时用 headSlot 挖掉身体帧对应区域再叠上素材，避免转头帧与
 * 原头部错位导致露出。
 */
export interface HeadSlot {
  /** 挖洞矩形左上角 x（相对帧坐标） */
  x: number
  /** 挖洞矩形左上角 y */
  y: number
  /** 挖洞矩形宽 */
  w: number
  /** 挖洞矩形高 */
  h: number
}

export interface HeadConfig {
  src: string
  /** 每格帧宽度（逻辑像素，与身体帧同尺寸） */
  frameWidth: number
  /** 每格帧高度 */
  frameHeight: number
  /** 列数，默认 3 */
  cols: number
  /** 行数，默认 3 */
  rows: number
  /** 对齐微调偏移（逻辑像素），用于修正头部素材与身体帧的错位 */
  offsetX: number
  offsetY: number
  /** 挖洞矩形：渲染身体帧时擦除该区域（相对帧坐标），默认挖掉上半 Y0..64 */
  headSlot: HeadSlot
}

const DEFAULT_HEAD_CONFIG: HeadConfig = {
  src: '',
  frameWidth: 128,
  frameHeight: 128,
  cols: 3,
  rows: 3,
  offsetX: 0,
  offsetY: 0,
  headSlot: { x: 0, y: 0, w: 128, h: 64 },
}

export interface FrameTag {
  name: string
  from: number
  to: number
}

export interface AsepriteFrame {
  frame: { x: number; y: number; w: number; h: number }
  duration: number
}

export interface AsepriteJson {
  frames: Record<string, AsepriteFrame>
  meta: {
    image: string
    size: { w: number; h: number }
    frameTags: FrameTag[]
  }
}

export type TagFrameRange = {
  from: number
  to: number
  frameCount: number
}

export interface FramePosition {
  x: number
  y: number
}

const TAG_ALIASES: Record<string, string[]> = {
  speak: ['speaking'],
  // Aseprite 标签名不含下划线（LookAround → lookaround），状态名含（look_around）
  lookaround: ['look_around'],
}

const DEFAULT_SPRITE_CONFIG: SpriteConfig = {
  src: '/pet/pet_spritesheet.png',
  jsonSrc: '/pet/pet_spritesheet.json',
  frameWidth: 128,
  frameHeight: 128,
  scale: 1,
  rows: 22,
  cols: 9,
}

const DEFAULT_LOTTIE_CONFIG: LottieConfig = {
  src: '/pet/lottie/',
}

export function usePetRenderer() {
  const rendererType = ref<RendererType>('spritesheet')
  const spriteConfig = ref<SpriteConfig>({ ...DEFAULT_SPRITE_CONFIG })
  const lottieConfig = ref<LottieConfig>({ ...DEFAULT_LOTTIE_CONFIG })
  const rendererReady = ref(false)
  const tagRanges = ref<Map<string, TagFrameRange>>(new Map())
  const frameDurations = ref<Map<number, number>>(new Map())
  const framePositions = ref<Map<number, FramePosition>>(new Map())
  const headConfig = ref<HeadConfig>({ ...DEFAULT_HEAD_CONFIG })
  const headEnabled = ref(false)

  function setRenderer(type: RendererType) {
    rendererType.value = type
    rendererReady.value = type === 'css' || type === 'lottie'
  }

  function setSpriteConfig(config: Partial<SpriteConfig>) {
    spriteConfig.value = { ...spriteConfig.value, ...config }
  }

  function setLottieConfig(config: Partial<LottieConfig>) {
    lottieConfig.value = { ...lottieConfig.value, ...config }
  }

  function setHeadConfig(config: Partial<HeadConfig>) {
    headConfig.value = { ...headConfig.value, ...config }
  }

  function setHeadEnabled(enabled: boolean) {
    headEnabled.value = enabled
  }

  function onSpriteLoaded() {
    rendererReady.value = true
  }

  function parseAsepriteJson(json: AsepriteJson) {
    const ranges = new Map<string, TagFrameRange>()
    const durations = new Map<number, number>()
    const positions = new Map<number, FramePosition>()

    for (const tag of json.meta.frameTags) {
      const normalizedName = tag.name.toLowerCase()
      const frameCount = tag.to - tag.from + 1
      const range = {
        from: tag.from,
        to: tag.to,
        frameCount,
      }
      ranges.set(normalizedName, range)
      const aliases = TAG_ALIASES[normalizedName]
      if (aliases) {
        for (const alias of aliases) {
          ranges.set(alias, range)
        }
      }
    }

    const frames = Object.values(json.frames)
    for (let i = 0; i < frames.length; i++) {
      durations.set(i, frames[i].duration)
      positions.set(i, {
        x: frames[i].frame.x,
        y: frames[i].frame.y,
      })
    }

    // 自动从第一帧提取帧尺寸，支持自设大小的精灵图（如 500×500）。
    // JSON 读取失败时保留默认 128 兜底。
    if (frames.length > 0) {
      const firstFrame = frames[0]
      const fw = firstFrame.frame.w
      const fh = firstFrame.frame.h
      if (fw > 0 && fh > 0 && (fw !== spriteConfig.value.frameWidth || fh !== spriteConfig.value.frameHeight)) {
        setSpriteConfig({ frameWidth: fw, frameHeight: fh })
      }
    }

    tagRanges.value = ranges
    frameDurations.value = durations
    framePositions.value = positions
  }

  function getTagRange(stateName: string): TagFrameRange | null {
    return tagRanges.value.get(stateName) ?? null
  }

  function getFrameDuration(frameIndex: number): number | null {
    return frameDurations.value.get(frameIndex) ?? null
  }

  return {
    rendererType,
    spriteConfig,
    lottieConfig,
    rendererReady,
    tagRanges,
    frameDurations,
    framePositions,
    headConfig,
    headEnabled,
    setRenderer,
    setSpriteConfig,
    setLottieConfig,
    setHeadConfig,
    setHeadEnabled,
    onSpriteLoaded,
    parseAsepriteJson,
    getTagRange,
    getFrameDuration,
  }
}