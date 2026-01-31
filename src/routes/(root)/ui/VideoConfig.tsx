import { core } from '@tauri-apps/api'
import { AnimatePresence, motion } from 'framer-motion'
import React from 'react'
import { snapshot, useSnapshot } from 'valtio'

import Button from '@/components/Button'
import Divider from '@/components/Divider'
import Icon from '@/components/Icon'
import Layout from '@/components/Layout'
import { toast } from '@/components/Toast'
import { compressVideos } from '@/tauri/commands/ffmpeg'
import { CompressionResult } from '@/types/compression'
import { zoomInTransition } from '@/utils/animation'
import { formatBytes } from '@/utils/fs'
import { cn } from '@/utils/tailwind'
import { appProxy } from '../-state'
import CancelCompression from './CancelCompression'
import CompressionPreset from './compression-options/CompressionPreset'
import CompressionQuality from './compression-options/CompressionQuality'
import MuteAudio from './compression-options/MuteAudio'
import TransformVideo from './compression-options/TransformVideo'
import VideoDimensions from './compression-options/VideoDimensions'
import VideoExtension from './compression-options/VideoExtension'
import VideoFPS from './compression-options/VideoFPS'
import FileName from './FileName'
import PreviewBatchVideos from './PreviewBatchVideos'
import PreviewSingleVideo from './PreviewSingleVideo'
import SaveVideo from './SaveVideo'
import styles from './styles.module.css'
import Success from './Success'
import VideoThumbnail from './VideoThumbnail'

function VideoConfig() {
  const {
    state: { videos, isCompressing, isProcessCompleted },
  } = useSnapshot(appProxy)
  const video = videos.length > 0 ? videos[0] : null
  const { config, thumbnailPath, fileName, dimensions, fps } = video ?? {}

  const { presetName, shouldMuteVideo } = config ?? {}
  const isThumbnailGenerating = thumbnailPath == null
  const isSingleVideoMode = videos.length === 1

  const handleCompression = async () => {
    const appSnapshot = snapshot(appProxy)
    if (appSnapshot.state.isCompressing) return

    const video = appSnapshot.state.videos[0]
    try {
      appProxy.takeSnapshot('beforeCompressionStarted')
      appProxy.state.isCompressing = true

      if (
        appProxy.state.videos[0].config.shouldTransformVideo &&
        appProxy.state.videos[0].config.transformVideoConfig?.previewUrl
      ) {
        appProxy.state.videos[0].thumbnailPath =
          appProxy.state.videos[0].config.transformVideoConfig.previewUrl
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
          ? presetName
          : null,
        shouldMuteVideo,
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
      // console.log('Compression results', results)

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
  }

  return (
    <Layout
      childrenProps={{
        className: cn(isThumbnailGenerating ? 'm-auto' : 'h-full'),
      }}
      hideLogo
    >
      <div className={cn(['h-full p-6', styles.videoConfigContainer])}>
        <AnimatePresence>
          <section
            className={cn(
              'px-4 py-6 hlg:py-10 rounded-xl border-2 border-zinc-200 dark:border-zinc-800',
              videos.length === 1
                ? 'flex flex-col justify-center items-center'
                : '',
            )}
          >
            {fileName && !isCompressing ? <FileName /> : null}
            {isCompressing ? (
              // <Compressing />
              <PreviewBatchVideos />
            ) : isProcessCompleted ? (
              <>
                <VideoThumbnail />
                <Success />
              </>
            ) : videos.length > 1 ? (
              <PreviewBatchVideos />
            ) : (
              <motion.div
                className="flex flex-col justify-center items-center"
                {...zoomInTransition}
              >
                <PreviewSingleVideo />
              </motion.div>
            )}
          </section>
        </AnimatePresence>
        <section
          className="px-4 py-6 hlg:py-10 rounded-xl border-2 border-zinc-200 dark:border-zinc-800"
          {...zoomInTransition}
        >
          <p className="text-xl mb-6 font-bold">Output Settings</p>
          <>
            <CompressionPreset />
            <Divider className="my-3" />
          </>
          <>
            <MuteAudio />
            <Divider className="my-3" />
          </>

          <>
            <CompressionQuality />
            <Divider className="my-3" />
          </>
          {isSingleVideoMode && dimensions ? (
            <>
              <VideoDimensions />
              <Divider className="my-3" />
              <TransformVideo />
              <Divider className="my-3" />
            </>
          ) : null}
          {fps ? (
            <>
              <VideoFPS />
              <Divider className="my-3" />
            </>
          ) : null}
          <>
            <div className="mt-8">
              <VideoExtension />
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
              >
                Compress <Icon name="logo" size={25} />
              </Button>
            )}
          </div>
        </section>
      </div>
    </Layout>
  )
}

export default React.memo(VideoConfig)
