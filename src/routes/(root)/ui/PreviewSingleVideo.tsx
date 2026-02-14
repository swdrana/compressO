import { motion } from 'framer-motion'
import { useMemo } from 'react'
import { useSnapshot } from 'valtio'

import Code from '@/components/Code'
import Divider from '@/components/Divider'
import Icon from '@/components/Icon'
import Image from '@/components/Image'
import { CircularProgress } from '@/components/Progress'
import { zoomInTransition } from '@/utils/animation'
import { formatDuration } from '@/utils/string'
import { cn } from '@/utils/tailwind'
import styles from './styles.module.css'
import Thumbnail from './Thumbnail'
import { appProxy } from '../-state'

type PreviewSingleVideoProps = {
  videoIndex: number
}

function PreviewSingleVideo({ videoIndex }: PreviewSingleVideoProps) {
  if (videoIndex < 0) return

  const {
    state: { videos, isCompressing, isProcessCompleted },
  } = useSnapshot(appProxy)
  const video = videos.length > 0 ? videos[videoIndex] : null
  const {
    config,
    size: videoSize,
    sizeInBytes,
    videoDuration,
    dimensions,
    fps,
    extension: videoExtension,
    thumbnailPath,
    compressedVideo,
    compressionProgress,
  } = video ?? {}
  const { isVideoTransformEditMode, isVideoTrimEditMode } = config ?? {}

  const compressedSizeDiff: number = useMemo(
    () =>
      typeof compressedVideo?.sizeInBytes === 'number' &&
      typeof sizeInBytes === 'number' &&
      !Number.isNaN(sizeInBytes)
        ? (((sizeInBytes ?? 0) - (compressedVideo?.sizeInBytes ?? 0)) * 100) /
          sizeInBytes
        : 0,
    [compressedVideo?.sizeInBytes, sizeInBytes],
  )

  const singleFileNameDisplay =
    (isProcessCompleted
      ? video?.compressedVideo?.fileNameToDisplay
      : video?.fileName) ?? ''

  return !isCompressing ? (
    <motion.div
      className="w-full h-full flex flex-col justify-center items-center"
      {...zoomInTransition}
    >
      {!(isVideoTransformEditMode || isVideoTrimEditMode) &&
      !isProcessCompleted ? (
        <Code size="sm" className="mb-3 text-center rounded-xl px-4 text-xs">
          {singleFileNameDisplay?.length > 50
            ? `${singleFileNameDisplay?.slice(0, 20)}...${singleFileNameDisplay?.slice(
                -10,
              )}`
            : singleFileNameDisplay}
        </Code>
      ) : null}

      {video ? <Thumbnail videoIndex={videoIndex} /> : null}

      {!(isVideoTransformEditMode || isVideoTrimEditMode) ? (
        isProcessCompleted ? (
          <section className="animate-appearance-in">
            <div className="flex justify-center items-center mt-3">
              <p className="text-2xl font-bold mx-4">{videoSize}</p>
              <Icon
                name="curvedArrow"
                className="text-black dark:text-white rotate-[-65deg] translate-y-[-8px]"
                size={100}
              />
              <p className="text-3xl  font-bold mx-4 text-primary">
                {compressedVideo?.size}
              </p>
            </div>
            {!(compressedSizeDiff <= 0) ? (
              <p className="block text-5xl text-center text-green-500">
                {compressedSizeDiff.toFixed(2)?.endsWith('.00')
                  ? compressedSizeDiff.toFixed(2)?.slice(0, -3)
                  : compressedSizeDiff.toFixed(2)}
                %<span className="text-large block">smaller</span>
              </p>
            ) : null}
          </section>
        ) : (
          <section className={cn(['my-4 mb-2', styles.videoMetadata])}>
            <>
              <div>
                <p className="italic text-gray-600 dark:text-gray-400">Size</p>
                <span className="block font-black">{videoSize}</span>
              </div>
              <Divider orientation="vertical" className="h-10" />
            </>
            <>
              <div>
                <p className="italic text-gray-600 dark:text-gray-400">
                  Extension
                </p>
                <span className="block font-black">
                  {videoExtension ?? '-'}
                </span>
              </div>
              <Divider orientation="vertical" className="h-10" />
            </>

            {videoDuration ? (
              <>
                <div>
                  <p className="italic text-gray-600 dark:text-gray-400">
                    Duration
                  </p>
                  <span className="block font-black">
                    {formatDuration(videoDuration) ?? '-'}
                  </span>
                </div>
              </>
            ) : null}
            <>
              {dimensions ? (
                <>
                  <Divider orientation="vertical" className="h-10" />{' '}
                  <div>
                    <p className="italic text-gray-600 dark:text-gray-400">
                      Dimensions
                    </p>
                    <span className="block font-black">
                      {dimensions.width ?? '-'} x {dimensions.height ?? '-'}
                    </span>
                  </div>
                </>
              ) : null}
            </>
            <>
              {fps ? (
                <>
                  <Divider orientation="vertical" className="h-10" />{' '}
                  <div>
                    <p className="italic text-gray-600 dark:text-gray-400">
                      FPS
                    </p>
                    <span className="block font-black">{fps ?? '-'}</span>
                  </div>
                </>
              ) : null}
            </>
          </section>
        )
      ) : null}
    </motion.div>
  ) : (
    <motion.div
      className="w-full h-full flex flex-col justify-center items-center flex-shrink-0"
      initial={{ scale: 0.9 }}
      animate={{ scale: 1 }}
      transition={{ type: 'spring', duration: 0.6 }}
    >
      <div className="relative">
        <CircularProgress
          {...(videoDuration == null
            ? { isIndeterminate: true }
            : { value: compressionProgress })}
          classNames={{
            base: 'absolute top-[50%] left-[50%] translate-x-[-50%] translate-y-[-50%]',
            svg: 'w-[480px] h-[480px] hlg:w-[540px] hlg:h-[540px] drop-shadow-md',
            indicator: 'stroke-primary stroke-1',
            track: 'stroke-transparent stroke-1',
            value: 'text-3xl font-semibold text-primary',
          }}
          strokeWidth={2}
          aria-label={`Progress-${compressionProgress}%`}
        />
        <Image
          alt="video to compress"
          src={thumbnailPath as string}
          className="z-0 w-[400px] h-[400px] min-w-[400px] min-h-[400px] hlg:w-[450px] hlg:h-[450px] hlg:min-w-[450px] hlg:min-h-[450px] object-cover rounded-full flex-shrink-0"
        />
        <div className="blur-2xl  z-[10] absolute top-0 right-0 bottom-0 left-0 rounded-full" />
      </div>
      <p className="italic text-sm mt-10 text-gray-600 dark:text-gray-400 text-center animate-pulse">
        Processing...
      </p>
      <p
        className={`not-italic text-2xl text-center font-bold text-primary my-4 opacity-${
          compressionProgress && compressionProgress > 0 ? 1 : 0
        }`}
      >
        {compressionProgress?.toFixed(2)}%
      </p>
    </motion.div>
  )
}

export default PreviewSingleVideo
