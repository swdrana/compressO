import { AnimatePresence, motion } from 'framer-motion'
import React from 'react'
import { useSnapshot } from 'valtio'

import Button from '@/components/Button'
import Layout from '@/components/Layout'
import { zoomInTransition } from '@/utils/animation'
import { cn } from '@/utils/tailwind'
import CompressionProgress from './CompressionProgress'
import OutputSettings from './output-settings/-index'
import PreviewBatchVideos from './PreviewBatchVideos'
import PreviewSingleVideo from './PreviewSingleVideo'
import styles from './styles.module.css'
import { appProxy } from '../-state'

function VideoConfig() {
  const {
    state: { videos, isCompressing, selectedVideoIndexForCustomization },
  } = useSnapshot(appProxy)

  return (
    <Layout
      childrenProps={{
        className: 'h-full',
      }}
      hideLogo
    >
      <div className={cn(['h-full p-6', styles.videoConfigContainer])}>
        <section
          className={cn(
            'relative px-4 py-6 rounded-xl border-2 border-zinc-200 dark:border-zinc-800 overflow-hidden',
            videos.length === 1
              ? 'flex flex-col justify-center items-center'
              : '',
          )}
        >
          <AnimatePresence>
            {videos.length > 1 ? (
              <>
                <PreviewBatchVideos />
                {selectedVideoIndexForCustomization > -1 ? (
                  <>
                    <div className="absolute top-0 right-0 bottom-0 left-0 w-full h-full z-[10] flex flex-col justify-center items-center bg-black1">
                      <motion.div
                        className="flex flex-col justify-center items-center"
                        {...zoomInTransition}
                      >
                        <PreviewSingleVideo
                          videoIndex={selectedVideoIndexForCustomization}
                        />
                      </motion.div>
                      <Button
                        size="sm"
                        className="absolute top-4 right-4"
                        onPress={() => {
                          appProxy.state.selectedVideoIndexForCustomization = -1
                        }}
                      >
                        Apply
                      </Button>
                    </div>
                  </>
                ) : null}
              </>
            ) : (
              <motion.div
                className="flex flex-col justify-center items-center"
                {...zoomInTransition}
              >
                <PreviewSingleVideo videoIndex={0} />
              </motion.div>
            )}
          </AnimatePresence>
        </section>
        <OutputSettings
          videoIndex={
            videos.length === 1 ? 0 : selectedVideoIndexForCustomization
          }
        />
      </div>
      {isCompressing ? <CompressionProgress /> : null}
    </Layout>
  )
}

export default React.memo(VideoConfig)
