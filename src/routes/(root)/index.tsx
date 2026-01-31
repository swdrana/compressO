import { Spinner } from '@heroui/react'
import { createFileRoute } from '@tanstack/react-router'
import { core } from '@tauri-apps/api'
import { motion } from 'framer-motion'
import cloneDeep from 'lodash/cloneDeep'
import React from 'react'
import { useSnapshot } from 'valtio'

import Icon from '@/components/Icon'
import Layout from '@/components/Layout'
import { toast } from '@/components/Toast'
import { generateVideoThumbnail, getVideoInfo } from '@/tauri/commands/ffmpeg'
import { getFileMetadata } from '@/tauri/commands/fs'
import VideoPicker from '@/tauri/components/VideoPicker'
import { extensions } from '@/types/compression'
import { formatBytes } from '@/utils/fs'
import { convertDurationToMilliseconds } from '@/utils/string'
import { Video } from '../../types/app'
import { appProxy, videoConfigInitialState } from './-state'
import DragAndDrop from './ui/DragAndDrop'
import Setting from './ui/Setting'
import VideoConfig from './ui/VideoConfig'

export const Route = createFileRoute('/(root)/')({
  component: Root,
})

function Root() {
  const { state, resetProxy } = useSnapshot(appProxy)

  const { videos, isLoadingFiles } = state

  const handleVideoSelected = React.useCallback(
    async (path: string | string[]) => {
      if (appProxy.state.isCompressing) return

      appProxy.state.isLoadingFiles = true
      const videoPaths = Array.isArray(path) ? path : [path]
      if (videoPaths.length === 0) {
        toast.error('Invalid video(s) selected.')
        return
      }
      let corruptedFilesCount = 0
      for (const index in videoPaths) {
        const path = videoPaths[index]
        try {
          const [fileMetadata, videoInfo, videoThumbnail] = await Promise.all([
            getFileMetadata(path),
            getVideoInfo(path),
            generateVideoThumbnail(path),
          ])

          if (
            !fileMetadata ||
            (typeof fileMetadata?.size === 'number' &&
              fileMetadata?.size <= 1000)
          ) {
            corruptedFilesCount++
            continue
          }

          const videoState: Video = {
            id: videoThumbnail?.id ?? `${index}-${+new Date()}`,
            pathRaw: path,
            path: core.convertFileSrc(path),
            fileName: fileMetadata?.fileName,
            mimeType: fileMetadata?.mimeType,
            sizeInBytes: fileMetadata?.size,
            size: formatBytes(fileMetadata?.size ?? 0),
            extension: fileMetadata?.extension?.toLowerCase?.(),
            config: cloneDeep(videoConfigInitialState),
          }

          if (videoPaths.length === 1 && fileMetadata?.extension) {
            videoState.config.convertToExtension =
              fileMetadata?.extension as keyof (typeof extensions)['video']
          }

          if (videoInfo) {
            const dimensions = videoInfo.dimensions
            if (
              !Number.isNaN(videoInfo.dimensions?.[0]) &&
              !Number.isNaN(videoInfo.dimensions[1])
            ) {
              videoState.dimensions = {
                width: dimensions[0],
                height: dimensions[1],
              }
            }
            const duration = videoInfo.duration
            const durationInMilliseconds =
              convertDurationToMilliseconds(duration)
            if (durationInMilliseconds > 0) {
              videoState.videDurationRaw = duration
              videoState.videoDurationMilliseconds = durationInMilliseconds
            }
            if (videoInfo.fps) {
              videoState.fps = Math.ceil(videoInfo.fps)
            }
          }

          if (videoThumbnail) {
            videoState.id = videoThumbnail?.id
            videoState.thumbnailPathRaw = videoThumbnail?.filePath
            videoState.thumbnailPath = core.convertFileSrc(
              videoThumbnail?.filePath,
            )
          }
          appProxy.state.videos.push(videoState)
        } catch {
          corruptedFilesCount++
        }
      }
      appProxy.state.isLoadingFiles = false
      if (corruptedFilesCount > 0) {
        toast.error(
          `${videoPaths.length > 1 ? 'Some files seem' : 'File seems'} to be corrupted/invalid ${videoPaths.length > 1 ? 'and are filtered out' : ''}.`,
        )
        if (corruptedFilesCount === videoPaths.length) {
          resetProxy()
        }
      }
    },
    [resetProxy],
  )

  return videos.length ? (
    isLoadingFiles ? (
      <div className="w-screen h-screen flex justify-center items-center dark:bg-black1 bg-white1">
        <Spinner size="lg" />
      </div>
    ) : (
      <VideoConfig />
    )
  ) : (
    <Layout
      containerProps={{ className: 'relative' }}
      childrenProps={{ className: 'm-auto' }}
    >
      <VideoPicker
        multiple
        onSuccess={({ filePath }) => handleVideoSelected(filePath)}
      >
        {({ onClick }) => (
          <motion.div
            role="button"
            tabIndex={0}
            className="h-full w-full flex flex-col justify-center items-center z-0"
            initial={{ scale: 0.9, opacity: 0 }}
            animate={{
              scale: 1,
              opacity: 1,
              transition: {
                duration: 0.6,
                bounce: 0.3,
                type: 'spring',
              },
            }}
            onClick={onClick}
            onKeyDown={(evt) => {
              if (evt?.key === 'Enter') {
                onClick()
              }
            }}
          >
            <div className="flex flex-col justify-center items-center py-16 px-20 border-2 border-dashed border-zinc-200 dark:border-zinc-800 rounded-3xl">
              <Icon name="videoFile" className="text-primary" size={60} />
              <p className="italic text-sm mt-4 text-gray-600 dark:text-gray-400 text-center">
                Drag & Drop
                <span className="block">Or</span>
                Click to select video(s).
              </p>
            </div>
          </motion.div>
        )}
      </VideoPicker>
      <DragAndDrop
        multiple
        disable={videos.length > 0}
        onFile={handleVideoSelected}
      />
      <Setting />
    </Layout>
  )
}

export default Root
