import { AnimatePresence, motion } from 'framer-motion'
import { useCallback, useMemo } from 'react'
import { useSnapshot } from 'valtio'

import Card from '@/components/Card'
import Divider from '@/components/Divider'
import Icon from '@/components/Icon'
import Image from '@/components/Image'
import Progress, { CircularProgress } from '@/components/Progress'
import ScrollShadow from '@/components/ScrollShadow'
import { zoomInStaggerAnimation } from '@/utils/animation'
import { formatBytes } from '@/utils/fs'
import { cn } from '@/utils/tailwind'
import { appProxy } from '../-state'

function PreviewBatchVideos() {
  const {
    state: {
      videos,
      isCompressing,
      isProcessCompleted,
      currentVideoIndex,
      totalSelectedFilesCount,
      isLoadingFiles,
      isBatchCompressionCancelled,
    },
  } = useSnapshot(appProxy)

  const compressionStats = useMemo(() => {
    const totalVideos = videos.length
    const totalSize = videos.reduce((sum, v) => sum + (v.sizeInBytes ?? 0), 0)

    const completedVideos = videos.filter(
      (v) => v.isProcessCompleted && v.compressedVideo?.sizeInBytes,
    )
    const compressedCount = completedVideos.length
    const cancelledCount = isBatchCompressionCancelled
      ? totalVideos - compressedCount
      : 0

    const originalSize = completedVideos.reduce(
      (sum, v) => sum + (v.sizeInBytes ?? 0),
      0,
    )
    const compressedSize = completedVideos.reduce((sum, v) => {
      const cs = v.compressedVideo?.sizeInBytes
      const os = v.sizeInBytes ?? 0

      return sum + (cs != null && cs < os ? cs : 0)
    }, 0)
    const sizeSaved = originalSize - compressedSize
    const percentageSaved =
      originalSize > 0 ? (sizeSaved / originalSize) * 100 : 0

    const totalProgress = videos.reduce(
      (a, c) => a + (c?.compressionProgress ?? 0) / videos.length,
      0,
    )

    const displayTotalVideos = isBatchCompressionCancelled
      ? compressedCount
      : totalVideos
    const displayTotalSize = isBatchCompressionCancelled
      ? completedVideos.reduce((sum, v) => sum + (v.sizeInBytes ?? 0), 0)
      : totalSize

    return {
      totalVideos,
      totalSize,
      compressedCount,
      compressedSize,
      sizeSaved,
      percentageSaved,
      totalProgress,
      cancelledCount,
      displayTotalVideos,
      displayTotalSize,
      isPositiveCompression:
        (compressedSize ?? Number.MAX_SAFE_INTEGER) < (totalSize ?? 0),
    }
  }, [videos, isBatchCompressionCancelled])

  const handleRemoveVideo = useCallback(
    (index: number) => {
      if (isCompressing) return
      appProxy.state.videos = appProxy.state.videos.filter(
        (_, i) => i !== index,
      )
    },
    [isCompressing],
  )

  return (
    <>
      <ScrollShadow
        className="h-[80vh] overflow-hidden overflow-y-auto"
        hideScrollBar
      >
        <AnimatePresence mode="wait">
          <motion.div
            variants={zoomInStaggerAnimation.container}
            initial="hidden"
            animate="show"
            exit="hidden"
            className="grid grid-cols-1 md:grid-cols-2 2xl:grid-cols-3 3xl:grid-cols-4 gap-4"
          >
            {videos.map((video, index) => {
              const compressedSizeDiff =
                typeof video?.compressedVideo?.sizeInBytes === 'number' &&
                typeof video?.sizeInBytes === 'number' &&
                !Number.isNaN(video?.sizeInBytes)
                  ? (((video?.sizeInBytes ?? 0) -
                      (video?.compressedVideo?.sizeInBytes ?? 0)) *
                      100) /
                    video?.sizeInBytes
                  : 0

              return (
                <motion.div
                  key={video.id}
                  layout
                  variants={zoomInStaggerAnimation.item}
                  className={cn([
                    'relative rounded-xl border-zinc-300 dark:border-zinc-800 overflow-hidden',
                  ])}
                >
                  <Card
                    className={cn(
                      'border-2 bg-zinc-100 dark:bg-zinc-900',
                      currentVideoIndex > index || video.isProcessCompleted
                        ? 'border-green-400'
                        : 'border-primary',
                    )}
                    radius="lg"
                  >
                    <div className="relative w-full">
                      {video.thumbnailPath ? (
                        <Image
                          src={video.thumbnailPath as string}
                          alt={video.fileName ?? ''}
                          className="w-full max-w-[unset] h-[220px] object-cover drop-shadow-xl"
                          removeWrapper
                        />
                      ) : (
                        <div className="w-full aspect-video bg-zinc-200 dark:bg-zinc-800 flex items-center justify-center">
                          <Icon
                            name="videoFile"
                            size={40}
                            className="text-zinc-400"
                          />
                        </div>
                      )}
                      {!isCompressing &&
                      !isProcessCompleted &&
                      !isLoadingFiles ? (
                        <button
                          onClick={() => handleRemoveVideo(index)}
                          className="absolute top-2 right-2 z-10 p-2 rounded-full bg-zinc-800/80 text-white hover:bg-zinc-700 transition-colors"
                        >
                          <Icon name="cross" size={20} />
                        </button>
                      ) : null}
                      {isCompressing && currentVideoIndex === index ? (
                        <>
                          <CircularProgress
                            size="lg"
                            showValueLabel
                            className="absolute left-1/2 top-1/2 z-20 -translate-x-1/2 -translate-y-1/2"
                            value={video.compressionProgress ?? 0}
                            strokeWidth={3}
                            classNames={{
                              svg: 'w-20 h-20 drop-shadow-md',
                              track: 'dark:stroke-white/50',
                              value: 'font-bold text-sm text-white1',
                            }}
                            aria-label="Compressing"
                          />
                          <div className="absolute inset-0 bg-black/70 z-10 rounded-lg"></div>
                        </>
                      ) : null}
                    </div>
                    <section className="px-3 py-2">
                      <p className={cn(['font-medium text-sm truncate block'])}>
                        {video.fileName ?? ''}
                      </p>
                      <section
                        className={cn(
                          'flex items-center gap-2 text-[12px] min-h-[55px]',
                          !video?.compressedVideo?.isSuccessful
                            ? 'justify-between'
                            : '',
                        )}
                      >
                        {video.isProcessCompleted &&
                        video?.compressedVideo?.isSuccessful ? (
                          <section className="animate-appearance-in w-full flex items-center justify-center gap-4">
                            <div className="flex justify-center items-center">
                              <p className="text-[16px] font-bold">
                                {video.size ?? ''}
                              </p>
                              <Icon
                                name="curvedArrow"
                                className="text-black dark:text-white rotate-[-65deg] translate-y-[-2px] mx-2"
                                size={40}
                              />
                              <p className="text-[16px] font-bold text-primary">
                                {video?.compressedVideo?.size ?? ''}
                              </p>
                            </div>
                            {!(compressedSizeDiff <= 0) ? (
                              <>
                                <Divider
                                  orientation="vertical"
                                  className="h-5"
                                />
                                <p className="block text-center text-[16px] text-green-500">
                                  {compressedSizeDiff
                                    .toFixed(2)
                                    ?.endsWith('.00')
                                    ? compressedSizeDiff
                                        .toFixed(2)
                                        ?.slice(0, -3)
                                    : compressedSizeDiff.toFixed(2)}
                                  %<span> smaller</span>
                                </p>
                              </>
                            ) : null}
                          </section>
                        ) : (
                          <>
                            <div>
                              <p className="italic text-gray-600 dark:text-gray-400 mb-1">
                                Size
                              </p>
                              <span className="block font-black">
                                {video.size}
                              </span>
                            </div>{' '}
                            <Divider orientation="vertical" className="h-5" />
                            <div>
                              <p className="italic text-gray-600 dark:text-gray-400 mb-1">
                                Extension
                              </p>
                              <span className="block font-black">
                                {video.extension ?? '-'}
                              </span>
                            </div>
                            {video.videDurationRaw ? (
                              <>
                                <Divider
                                  orientation="vertical"
                                  className="h-5"
                                />
                                <div>
                                  <p className="italic text-gray-600 dark:text-gray-400 mb-1">
                                    Duration
                                  </p>
                                  <span className="block font-black">
                                    {video.videDurationRaw ?? '-'}
                                  </span>
                                </div>
                              </>
                            ) : null}
                            {video.dimensions ? (
                              <>
                                <Divider
                                  orientation="vertical"
                                  className="h-5"
                                />
                                <div>
                                  <p className="italic text-gray-600 dark:text-gray-400 mb-1">
                                    Dimensions
                                  </p>
                                  <span className="block font-black">
                                    {video.dimensions.width ?? '-'} x{' '}
                                    {video.dimensions.height ?? '-'}
                                  </span>
                                </div>
                              </>
                            ) : null}
                            {video.fps ? (
                              <>
                                <Divider
                                  orientation="vertical"
                                  className="h-5"
                                />
                                <div>
                                  <div>
                                    <p className="italic text-gray-600 dark:text-gray-400 mb-1">
                                      FPS
                                    </p>
                                    <span className="block font-black">
                                      {video.fps ?? '-'}
                                    </span>
                                  </div>
                                </div>
                              </>
                            ) : null}
                          </>
                        )}
                      </section>
                    </section>
                  </Card>
                </motion.div>
              )
            })}
          </motion.div>
        </AnimatePresence>
      </ScrollShadow>
      <section className="relative px-4 py-6 flex flex-col items-center">
        {isLoadingFiles ? (
          <Progress
            size="sm"
            isIndeterminate={totalSelectedFilesCount == null}
            className="w-[100px] mb-2 absolute top-2 right-1/2 translate-x-1/2"
            value={(videos.length * 100) / (totalSelectedFilesCount || 1)}
          />
        ) : null}
        <div className="max-w-7xl mx-auto">
          {isCompressing ? (
            <div className="flex flex-wrap items-center justify-between gap-4">
              <div className="flex items-center gap-6">
                <div>
                  <p className="italic text-gray-600 dark:text-gray-400">
                    Compressed
                  </p>
                  <p className="font-black text-lg">
                    {compressionStats?.compressedCount ?? 0} /{' '}
                    {compressionStats?.totalVideos}
                  </p>
                </div>
                <Divider orientation="vertical" className="h-8" />
                <div>
                  <p className="italic text-gray-600 dark:text-gray-400">
                    Saved
                  </p>
                  <p className="font-black text-lg text-green-600 dark:text-green-400">
                    {formatBytes(compressionStats.sizeSaved ?? 0) || '...'}
                    {compressionStats.percentageSaved
                      ? `(${(compressionStats.percentageSaved ?? 0).toFixed(0)}%)`
                      : null}
                  </p>
                </div>
              </div>
              <Divider orientation="vertical" className="h-8" />
              <div className="flex-1">
                <CircularProgress
                  showValueLabel
                  size="lg"
                  value={compressionStats.totalProgress ?? 0}
                  strokeWidth={4}
                />
              </div>
            </div>
          ) : (
            <div className="flex items-center gap-6">
              <div>
                <p className="italic text-gray-600 dark:text-gray-400">
                  Videos
                </p>
                <p
                  className={cn(
                    'font-black text-lg',
                    isLoadingFiles ? 'animate-pulse' : '',
                  )}
                >
                  {compressionStats.displayTotalVideos}
                  {compressionStats.cancelledCount > 0 ? (
                    <span className="text-xs italic text-warning-400 ml-2">
                      ({compressionStats.cancelledCount} cancelled)
                    </span>
                  ) : null}
                </p>
              </div>
              <Divider orientation="vertical" className="h-8" />
              <div>
                <p className="italic text-gray-600 dark:text-gray-400">Size</p>
                <p
                  className={cn(
                    'font-black text-lg',
                    isLoadingFiles ? 'animate-pulse' : '',
                  )}
                >
                  {formatBytes(compressionStats.displayTotalSize)}
                </p>
              </div>
              {isProcessCompleted ? (
                <>
                  <Divider orientation="vertical" className="h-8" />
                  <div>
                    <p className="italic text-gray-600 dark:text-gray-400">
                      Output Size
                    </p>
                    <p
                      className={cn(
                        'font-black text-lg',
                        compressionStats.isPositiveCompression
                          ? 'text-green-600 dark:text-green-400'
                          : '',
                      )}
                    >
                      {formatBytes(compressionStats.compressedSize ?? 0) || '-'}
                    </p>
                  </div>
                  <Divider orientation="vertical" className="h-8" />
                  <div>
                    <p className="italic text-gray-600 dark:text-gray-400">
                      Saved
                    </p>
                    <p
                      className={cn(
                        'font-black text-lg',
                        compressionStats.isPositiveCompression
                          ? 'text-green-600 dark:text-green-400'
                          : '',
                      )}
                    >
                      {formatBytes(compressionStats.sizeSaved ?? 0) || '-'} (
                      {(compressionStats.percentageSaved ?? 0).toFixed(2)}%)
                    </p>
                  </div>
                </>
              ) : null}
            </div>
          )}
        </div>
      </section>
    </>
  )
}

export default PreviewBatchVideos
