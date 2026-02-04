import { DateValue } from '@internationalized/date'

import {
  compressionPresets,
  extensions,
  VideoTransforms,
  VideoTransformsHistory,
} from '@/types/compression'

export type VideoMetadataConfig = {
  title?: string | null
  artist?: string | null
  album?: string | null
  year?: string | null
  comment?: string | null
  genre?: string | null
  creationTime?: string | null
  creationTimeRaw?: DateValue | null
  shouldEnableCreationTime?: boolean
}

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
  shouldPreserveMetadata?: boolean
  metadataConfig?: VideoMetadataConfig | null
  customThumbnailPath?: string | null
  shouldEnableCustomThumbnail?: boolean
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
  isConfigDirty?: boolean
  dimensions?: { width: number; height: number }
  fps?: number
}

export type App = {
  batchId?: string
  videos: Video[]
  isLoadingFiles: boolean
  totalSelectedFilesCount: number
  currentVideoIndex: number
  totalDurationMs: number
  isCompressing: boolean
  totalProgress: number
  isProcessCompleted: boolean
  isBatchCompressionCancelled: boolean
  isSaving: boolean
  isSaved: boolean
  savedPath?: string
  selectedVideoIndexForCustomization: number
  commonConfigForBatchCompression: VideoConfig
}
