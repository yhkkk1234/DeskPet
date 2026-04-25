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

const DEFAULT_SPRITE_CONFIG: SpriteConfig = {
  src: '/pet/pet_spritesheet.png',
  jsonSrc: '/pet/pet_spritesheet.json',
  frameWidth: 64,
  frameHeight: 64,
  scale: 2,
  rows: 8,
  cols: 8,
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

    for (const tag of json.meta.frameTags) {
      const normalizedName = tag.name.toLowerCase()
      const frameCount = tag.to - tag.from + 1
      ranges.set(normalizedName, {
        from: tag.from,
        to: tag.to,
        frameCount,
      })
    }

    const frames = Object.values(json.frames)
    for (let i = 0; i < frames.length; i++) {
      durations.set(i, frames[i].duration)
    }

    tagRanges.value = ranges
    frameDurations.value = durations
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
    setRenderer,
    setSpriteConfig,
    setLottieConfig,
    onSpriteLoaded,
    parseAsepriteJson,
    getTagRange,
    getFrameDuration,
  }
}