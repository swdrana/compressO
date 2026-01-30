import { AnimatePresence, motion } from 'framer-motion'
import { useCallback } from 'react'
import { useSnapshot } from 'valtio'

import Card from '@/components/Card'
import Divider from '@/components/Divider'
import Icon from '@/components/Icon'
import Image from '@/components/Image'
import { zoomInStaggerAnimation } from '@/utils/animation'
import { cn } from '@/utils/tailwind'
import { appProxy } from '../-state'

function PreviewBatchVideos() {
  const {
    state: { videos, isCompressing, currentVideoIndex },
  } = useSnapshot(appProxy)

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
    <AnimatePresence mode="wait">
      <motion.div
        variants={zoomInStaggerAnimation.container}
        initial="hidden"
        animate="show"
        exit="hidden"
        className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4 gap-4"
      >
        {videos.map((video, index) => (
          <motion.div
            key={video.id}
            layout
            variants={zoomInStaggerAnimation.item}
            className={cn([
              'relative rounded-xl border-zinc-300 dark:border-zinc-800 overflow-hidden',
              isCompressing ? 'opacity-50' : '',
            ])}
          >
            <Card
              className="border-2 border-primary bg-zinc-100 dark:bg-zinc-900"
              radius="lg"
            >
              {video.thumbnailPath ? (
                <Image
                  src={video.thumbnailPath as string}
                  alt={video.fileName ?? ''}
                  className="w-full aspect-video object-cover"
                />
              ) : (
                <div className="w-full aspect-video bg-zinc-200 dark:bg-zinc-800 flex items-center justify-center">
                  <Icon name="videoFile" size={40} className="text-zinc-400" />
                </div>
              )}
              {!isCompressing && (
                <button
                  onClick={() => handleRemoveVideo(index)}
                  className="absolute top-2 right-2 z-10 p-2 rounded-full bg-zinc-800/80 text-white hover:bg-zinc-700 transition-colors"
                >
                  <Icon name="cross" size={20} />
                </button>
              )}
              <div className="px-3 py-2">
                <p
                  className={cn([
                    'font-medium text-sm truncate block',
                    isCompressing ? 'text-gray-500' : '',
                  ])}
                >
                  {video.fileName ?? ''}
                </p>
                <div className="flex justify-around items-center gap-2 mt-2 text-xs">
                  <div>
                    <p className="italic text-gray-600 dark:text-gray-400 mb-1">
                      Size
                    </p>
                    <span className="block font-black">{video.size}</span>
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
                      <Divider orientation="vertical" className="h-5" />
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
                      <Divider orientation="vertical" className="h-5" />
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
                      <Divider orientation="vertical" className="h-5" />
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
                </div>
                {currentVideoIndex === index + 1 && isCompressing && (
                  <div className="mt-2">
                    <div className="h-1 bg-zinc-200 dark:bg-zinc-700 rounded-full overflow-hidden">
                      <motion.div
                        className="h-full bg-primary"
                        initial={{ width: 0 }}
                        animate={{
                          width: `${video.compressionProgress ?? 0}%`,
                        }}
                      />
                    </div>
                    <p className="text-xs text-primary mt-1 font-medium">
                      Compressing...
                    </p>
                  </div>
                )}
                {currentVideoIndex > index && video.isProcessCompleted && (
                  <div className="mt-2 flex items-center gap-1 text-xs text-green-600 dark:text-green-400 font-medium">
                    <Icon name="tick" size={12} />
                    <span>Compressed</span>
                  </div>
                )}
              </div>
            </Card>
          </motion.div>
        ))}
      </motion.div>
    </AnimatePresence>
  )
}

export default PreviewBatchVideos
