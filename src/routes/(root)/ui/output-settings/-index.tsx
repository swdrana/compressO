import { Divider, ScrollShadow, Tab } from '@heroui/react'
import { core } from '@tauri-apps/api'
import { TimelineAction } from '@xzdarcy/timeline-engine'
import { motion } from 'framer-motion'
import { useCallback, useEffect, useState } from 'react'
import { toast } from 'sonner'
import { snapshot, useSnapshot } from 'valtio'

import Button from '@/components/Button'
import Icon from '@/components/Icon'
import Tabs from '@/components/Tabs'
import { compressVideos } from '@/tauri/commands/ffmpeg'
import { getAudioStreams } from '@/tauri/commands/ffprobe'
import { VideoMetadataConfig } from '@/types/app'
import {
  CompressionResult,
  TrimSegment,
  VideoTransformsHistory,
} from '@/types/compression'
import { formatBytes } from '@/utils/fs'
import AudioBitrate from './AudioBitrate'
import AudioChannels from './AudioChannels'
import AudioVolume from './AudioVolume'
import CompressionPreset from './CompressionPreset'
import CompressionQuality from './CompressionQuality'
import CustomThumbnail from './CustomThumbnail'
import Metadata from './Metadata'
import TransformVideo from './TransformVideo'
import TrimVideo from './TrimVideo'
import VideoCodec from './VideoCodec'
import VideoDimensions from './VideoDimensions'
import VideoExtension from './VideoExtension'
import VideoFPS from './VideoFPS'
import { appProxy } from '../../-state'
import CancelCompression from '../CancelCompression'
import CompressionActions from '../CompressionActions'
import SaveVideo from '../SaveVideo'

type OutputSettingsProps = {
  videoIndex: number // if videoIndex < 0, we'll only show settings that applies to all videos
}

const TABS = {
  video: {
    id: 'video',
    title: 'Video',
  },
  audio: {
    id: 'audio',
    title: 'Audio',
  },
  metadata: {
    id: 'metadata',
    title: 'Metadata',
  },
} as const

function OutputSettings({ videoIndex }: OutputSettingsProps) {
  const {
    state: {
      videos,
      isCompressing,
      isProcessCompleted,
      isLoadingFiles,
      selectedVideoIndexForCustomization,
    },
  } = useSnapshot(appProxy)
  const video = videos.length && videoIndex >= 0 ? videos[videoIndex] : null
  const { dimensions, pathRaw: videoPathRaw, videoInfoRaw } = video ?? {}

  const [tab, setTab] = useState<keyof typeof TABS>('video')

  const handleCompression = useCallback(async () => {
    const appSnapshot = snapshot(appProxy)
    if (appSnapshot.state.isCompressing) return

    // Resets
    appProxy.clearSnapshots()
    appProxy.state.isBatchCompressionCancelled = false
    appProxy.state.selectedVideoIndexForCustomization = -1
    appProxy.state.showVideoInfo = false
    for (const index in appProxy.state.videos) {
      appProxy.state.videos[index].config.isVideoTransformEditMode = false
      appProxy.state.videos[index].config.isVideoTrimEditMode = false
    }

    appProxy.takeSnapshot('beforeCompressionStarted')

    try {
      appProxy.state.isCompressing = true

      for (const index in appProxy.state.videos) {
        if (
          appProxy.state.videos[index]?.config?.shouldTransformVideo &&
          appProxy.state.videos[index].config?.transformVideoConfig?.previewUrl
        ) {
          appProxy.state.videos[index].thumbnailPath =
            appProxy.state.videos[
              index
            ]?.config?.transformVideoConfig?.previewUrl
        }
        appProxy.state.videos[index].config.isVideoTransformEditMode = false
      }

      const batchId = `${+new Date()}`
      appProxy.state.batchId = batchId

      const { results } = await compressVideos(
        batchId,
        appSnapshot.state.videos.map((v) => ({
          videoId: v.id!,
          videoPath: v.pathRaw!,
          convertToExtension: v.config?.convertToExtension ?? 'mp4',
          presetName: !v.config?.shouldDisableCompression
            ? v.config.presetName
            : null,
          audioConfig: {
            volume: v.config?.audioConfig?.volume ?? 100,
            audioChannelConfig:
              (v.config?.audioConfig?.volume ?? 100) !== 0
                ? (v.config?.audioConfig?.audioChannelConfig ?? null)
                : null,
            bitrate:
              (v.config?.audioConfig?.volume ?? 100) !== 0
                ? (v.config?.audioConfig?.bitrate ?? null)
                : null,
          },
          quality: v.config?.shouldEnableQuality
            ? (v.config?.quality as number)
            : 101,
          dimensions:
            v.config?.shouldEnableCustomDimensions && v.config.customDimensions
              ? ([
                  Math.round(v.config.customDimensions[0]),
                  Math.round(v.config.customDimensions[1]),
                ] as [number, number])
              : null,
          fps: v.config?.shouldEnableCustomFPS
            ? v.config.customFPS?.toString?.()
            : null,
          videoCodec: v.config?.shouldEnableCustomVideoCodec
            ? v.config.customVideoCodec
            : null,
          transformsHistory: v.config?.shouldTransformVideo
            ? ((v.config.transformVideoConfig?.transformsHistory ??
                []) as VideoTransformsHistory[])
            : null,
          metadataConfig:
            !v.config?.shouldPreserveMetadata && v.config?.metadataConfig
              ? Object.entries(
                  v.config?.metadataConfig as VideoMetadataConfig,
                ).reduce(
                  (a, [key, value]: [string, any]) => {
                    a[key] = value?.length > 0 ? value : null
                    return a
                  },
                  {} as Record<string, string>,
                )
              : null,
          customThumbnailPath:
            v.config?.shouldEnableCustomThumbnail &&
            v.config?.customThumbnailPath?.length
              ? v.config.customThumbnailPath
              : null,
          trimSegments:
            v.config?.shouldTrimVideo && Array.isArray(v.config?.trimConfig)
              ? (v.config.trimConfig
                  .filter((a) => a.end >= a.start)
                  .map(
                    (action: TimelineAction): TrimSegment => ({
                      start: action.start,
                      end: action.end,
                    }),
                  ) as TrimSegment[])
              : null,
        })),
      )
      if (Object.keys(results).length === 0) {
        throw new Error()
      }

      appProxy.state.isCompressing = false
      appProxy.state.isProcessCompleted = true

      const videosSnapShot = snapshot(appProxy.state.videos)
      for (const index in videosSnapShot) {
        const video = videosSnapShot[index]
        const videoResult: CompressionResult | null = results[video.id!] || null

        appProxy.state.videos[index].isProcessCompleted = true
        appProxy.state.videos[index].compressedVideo = {
          isSuccessful: !(videoResult == null),
          fileName: videoResult?.fileMetadata?.fileName ?? video.fileName,
          fileNameToDisplay: `${video?.fileName?.slice(
            0,
            -((video?.extension?.length ?? 0) + 1),
          )}.${videoResult?.fileMetadata?.extension}`,
          pathRaw: videoResult?.fileMetadata?.path,
          path: core.convertFileSrc(videoResult?.fileMetadata?.path ?? ''),
          mimeType: videoResult?.fileMetadata?.mimeType,
          sizeInBytes: videoResult?.fileMetadata?.size,
          size: formatBytes(videoResult?.fileMetadata?.size ?? 0),
          extension: videoResult?.fileMetadata?.extension,
        }
      }
    } catch (error) {
      if (error !== 'CANCELLED') {
        toast.error('Something went wrong during compression.')
        appProxy.timeTravel('beforeCompressionStarted')
      }
    }
  }, [])

  useEffect(() => {
    tab === 'audio' &&
      videoIndex > -1 &&
      appProxy.state.videos[videoIndex] &&
      !appProxy.state.videos[videoIndex]?.videoInfoRaw?.audioStreams &&
      videoPathRaw &&
      (async () => {
        const streams = await getAudioStreams(videoPathRaw)
        if (streams) {
          const targetVideo = appProxy.state.videos[videoIndex]
          if (!targetVideo?.videoInfoRaw) {
            appProxy.state.videos[videoIndex].videoInfoRaw = {}
          }
          if (targetVideo.videoInfoRaw) {
            targetVideo.videoInfoRaw.audioStreams = streams
          }
        }
      })()
  }, [videoPathRaw, videoIndex, tab])

  const hasNoAudio = videoInfoRaw?.audioStreams?.length === 0

  return (
    <>
      <div className="flex items-center justify-between w-full mb-2">
        <p className="text-xl font-bold">
          {videos.length === 1 || selectedVideoIndexForCustomization > -1
            ? 'Output'
            : 'Batch'}{' '}
          Settings
        </p>
        {!isCompressing ? <CompressionActions /> : null}
      </div>
      <section>
        <Tabs
          aria-label="Compression Settings"
          size="sm"
          selectedKey={tab}
          onSelectionChange={(t) => setTab(t as keyof typeof TABS)}
          className="w-full"
          fullWidth
        >
          {Object.values(TABS).map((t) => (
            <Tab key={t.id} value={t.id} title={t.title} />
          ))}
        </Tabs>
        <ScrollShadow
          className="max-h-[70vh] hxl:max-h-[80vh] my-6"
          hideScrollBar
        >
          {tab === 'video' ? (
            <div>
              <>
                <CompressionPreset videoIndex={videoIndex} />
                <Divider className="my-3" />
              </>
              <>
                <VideoCodec videoIndex={videoIndex} />
                <Divider className="my-3" />
              </>
              <>
                <CompressionQuality videoIndex={videoIndex} />
                <Divider className="my-3" />
              </>
              {videoIndex >= 0 && dimensions ? (
                <>
                  <VideoDimensions videoIndex={videoIndex} />
                  <Divider className="my-3" />
                  <TransformVideo videoIndex={videoIndex} />
                  <Divider className="my-3" />
                  <TrimVideo videoIndex={videoIndex} />
                  <Divider className="my-3" />
                </>
              ) : null}
              <>
                <VideoFPS videoIndex={videoIndex} />
                <Divider className="my-3" />
              </>
              <CustomThumbnail videoIndex={videoIndex} />
              <>
                <div className="mt-8">
                  <VideoExtension videoIndex={videoIndex} />
                </div>
              </>
            </div>
          ) : null}
          {tab === 'audio' ? (
            <>
              <div className="mb-4">
                <AudioVolume videoIndex={videoIndex} />
                <Divider className="mt-8" />
              </div>
              <>
                <AudioChannels videoIndex={videoIndex} />
                <Divider className="my-3" />
              </>
              <>
                <AudioBitrate videoIndex={videoIndex} />
              </>
              {hasNoAudio ? <p className="text-xs">No audio found</p> : null}
            </>
          ) : null}
          {tab === 'metadata' ? (
            <>
              <Metadata videoIndex={videoIndex} />
            </>
          ) : null}
        </ScrollShadow>
      </section>

      {selectedVideoIndexForCustomization < 0 ? (
        <div className="mt-4">
          {isCompressing ? (
            <CancelCompression />
          ) : isProcessCompleted ? (
            <SaveVideo />
          ) : (
            <Button
              as={motion.button}
              color="primary"
              onPress={handleCompression}
              fullWidth
              className="text-primary"
              isDisabled={isLoadingFiles}
            >
              Process <Icon name="logo" size={25} />
            </Button>
          )}
        </div>
      ) : null}
    </>
  )
}

export default OutputSettings
