import { VideoMetadataConfig } from './app'
import { FileMetadata } from './fs'

export const extensions = {
  video: { mp4: 'mp4', mov: 'mov', mkv: 'mkv', webm: 'webm', avi: 'avi' },
} as const

export const compressionPresets = {
  ironclad: 'ironclad',
  thunderbolt: 'thunderbolt',
} as const

export type CompressionResult = {
  videoId: string
  fileName: string
  filePath: string
  fileMetadata: FileMetadata
}

export enum CustomEvents {
  VideoCompressionProgress = 'VideoCompressionProgress',
  CancelInProgressCompression = 'CancelInProgressCompression',
  BatchCompressionProgress = 'BatchCompressionProgress',
  BatchCompressionIndividualCompressionCompletion = 'BatchCompressionIndividualCompressionCompletion',
}

export type VideoCompressionProgress = {
  videoId: string
  batchId: string
  fileName: string
  currentDuration: string
}
export type BatchCompressionIndividualCompressionResult = {
  batchId: string
  result: CompressionResult
}

export type VideoThumbnail = {
  id: string
  fileName: string
  filePath: string
}

export type VideoInfo = {
  duration: number
  dimensions: [number, number]
  fps: number
}

export type VideoTransforms = {
  crop: { top: number; left: number; width: number; height: number }
  rotate: number
  flip: { horizontal: boolean; vertical: boolean }
}

export type VideoTransformsHistory =
  | {
      type: 'crop'
      value: { top: number; left: number; width: number; height: number }
    }
  | { type: 'rotate'; value: number }
  | { type: 'flip'; value: { horizontal: boolean; vertical: boolean } }

export type BatchCompressionResult = {
  results: Record<string, CompressionResult>
}

export type BatchCompressionProgress = {
  batchId: string
  currentIndex: number
  totalCount: number
  videoProgress: VideoCompressionProgress
}

export type VideoFileMetadata = {
  id: string
  fileName: string
  path: string
  size: number
  thumbnailPath?: string
  duration?: string
  dimensions?: [number, number]
  fps?: number
}

export type TrimSegment = {
  start: number
  end: number
}

export type VideoCompressionConfig = {
  videoPath: string
  convertToExtension: string
  presetName?: string | null
  videoId: string
  batchId?: string | null
  shouldMuteVideo: boolean
  quality: number
  dimensions?: [number, number] | null
  fps?: string | null
  videoCodec?: string | null
  transformsHistory?: VideoTransformsHistory[] | null
  metadataConfig?: VideoMetadataConfig | null
  customThumbnailPath?: string | null
  trimSegments?: TrimSegment[] | null
}
