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
    setRenderer,
    setSpriteConfig,
    setLottieConfig,
    onSpriteLoaded,
    parseAsepriteJson,
    getTagRange,
    getFrameDuration,
  }
}