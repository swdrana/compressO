import {
  compressionPresets,
  extensions,
  VideoTransforms,
  VideoTransformsHistory,
} from '@/types/compression'

export type VideoConfig = {
  convertToExtension: keyof typeof extensions.video
  presetName: keyof typeof compressionPresets
  shouldDisableCompression: boolean
  shouldMuteVideo: boolean
  shouldEnableQuality?: boolean
  quality?: number | null
  shouldEnableCustomDimensions?: boolean
  customDimensions?: [number, number]
  shouldEnableCustomFPS?: boolean
  customFPS?: number
  shouldTransformVideo?: boolean
  transformVideoConfig?: {
    transforms: VideoTransforms
    transformsHistory: VideoTransformsHistory[]
    previewUrl?: string
  }
}

export type Video = {
  id: string | null
  pathRaw?: string | null
  path?: string | null
  fileName?: string | null
  mimeType?: string | null
  sizeInBytes?: number | null
  size?: string | null
  extension?: null | string
  thumbnailPathRaw?: string | null
  thumbnailPath?: string | null
  videoDurationMilliseconds?: number | null
  videDurationRaw?: string | null
  isCompressing?: boolean
  isProcessCompleted?: boolean
  compressedVideo?: {
    isSuccessful?: boolean
    pathRaw?: string | null
    path?: string | null
    fileName?: string | null
    fileNameToDisplay?: string | null
    mimeType?: string | null
    sizeInBytes?: number | null
    size?: string | null
    extension?: null | string
    isSaving?: boolean
    isSaved?: boolean
    savedPath?: string
  } | null
  compressionProgress?: number
  config: VideoConfig
  dimensions?: { width: number; height: number }
  fps?: number
}

export type App = {
  batchId?: string
  videos: Video[]
  currentVideoIndex: number
  totalDurationMs: number
  isCompressing: boolean
  totalProgress: number
  isProcessCompleted: boolean
  isSaving: boolean
  isSaved: boolean
  isLoadingFiles: boolean
}
