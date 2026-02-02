import { Divider } from '@heroui/react'
import { core } from '@tauri-apps/api'
import { motion } from 'framer-motion'
import { useCallback } from 'react'
import { toast } from 'sonner'
import { snapshot, useSnapshot } from 'valtio'

import Button from '@/components/Button'
import Icon from '@/components/Icon'
import { compressVideos } from '@/tauri/commands/ffmpeg'
import { CompressionResult } from '@/types/compression'
import { formatBytes } from '@/utils/fs'
import CompressionPreset from './CompressionPreset'
import CompressionQuality from './CompressionQuality'
import MuteAudio from './MuteAudio'
import TransformVideo from './TransformVideo'
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
  const { dimensions } = video ?? {}

  const handleCompression = useCallback(async () => {
    const appSnapshot = snapshot(appProxy)
    if (appSnapshot.state.isCompressing) return

    appProxy.clearSnapshots()
    appProxy.state.isBatchCompressionCancelled = false
    appProxy.state.selectedVideoIndexForCustomization = -1
    appProxy.takeSnapshot('beforeCompressionStarted')

    const video = appSnapshot.state.videos[videoIndex]

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
      }

      const batchId = `${+new Date()}`
      appProxy.state.batchId = batchId

      const { results } = await compressVideos({
        batchId,
        videos: appSnapshot.state.videos.map((v) => ({
          videoId: v.id!,
          videoPath: v.pathRaw!,
        })),
        convertToExtension: video?.config?.convertToExtension ?? 'mp4',
        presetName: !video?.config?.shouldDisableCompression
          ? video.config.presetName
          : null,
        shouldMuteVideo: video.config.shouldMuteVideo,
        ...(video?.config?.shouldEnableQuality
          ? { quality: video?.config?.quality as number }
          : {}),
        ...(video.config.shouldEnableCustomDimensions
          ? { dimensions: video.config.customDimensions }
          : {}),
        ...(video.config.shouldEnableCustomFPS
          ? { fps: video.config.customFPS?.toString?.() }
          : {}),
        ...(video.config.shouldTransformVideo
          ? {
              transformsHistory:
                video.config.transformVideoConfig?.transformsHistory ??
                ([] as any),
            }
          : {}),
      })
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
  }, [videoIndex])

  return (
    <section className="p-4 rounded-xl border-2 border-zinc-200 dark:border-zinc-800 h-full">
      <div className="flex items-center justify-between w-full mb-4">
        <p className="text-xl font-bold">
          {videos.length === 1 || selectedVideoIndexForCustomization > -1
            ? 'Output'
            : 'Batch'}{' '}
          Settings
        </p>
        {!isCompressing ? <CompressionActions /> : null}
      </div>
      <>
        <CompressionPreset videoIndex={videoIndex} />
        <Divider className="my-3" />
      </>
      <>
        <MuteAudio videoIndex={videoIndex} />
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
        </>
      ) : null}
      <>
        <VideoFPS videoIndex={videoIndex} />
        <Divider className="my-3" />
      </>
      <>
        <div className="mt-8">
          <VideoExtension videoIndex={videoIndex} />
        </div>
      </>
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
            Compress <Icon name="logo" size={25} />
          </Button>
        )}
      </div>
    </section>
  )
}

export default OutputSettings
