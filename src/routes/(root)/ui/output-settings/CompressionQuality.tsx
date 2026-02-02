import { AnimatePresence, motion } from 'framer-motion'
import React, { useCallback } from 'react'
import { snapshot, useSnapshot } from 'valtio'

import Slider from '@/components/Slider/Slider'
import Switch from '@/components/Switch'
import { slideDownTransition } from '@/utils/animation'
import { appProxy, normalizeBatchVideosConfig } from '../../-state'

type CompressionQualityProps = {
  videoIndex: number
}

function CompressionQuality({ videoIndex }: CompressionQualityProps) {
  const {
    state: {
      isCompressing,
      isProcessCompleted,
      videos,
      commonConfigForBatchCompression,
      isLoadingFiles,
    },
  } = useSnapshot(appProxy)
  const video = videos.length > 0 && videoIndex >= 0 ? videos[videoIndex] : null
  const { config } = video ?? {}
  const { quality: compressionQuality, shouldEnableQuality } =
    config ?? commonConfigForBatchCompression ?? {}

  const [quality, setQuality] = React.useState<number>(
    compressionQuality ?? 100,
  )
  const debounceRef = React.useRef<NodeJS.Timeout>()
  const qualityRef = React.useRef<number>(quality)

  React.useEffect(() => {
    qualityRef.current = quality
  }, [quality])

  React.useEffect(() => {
    const appSnapshot = snapshot(appProxy)
    if (
      appSnapshot.state.videos.length &&
      quality !==
        (videoIndex >= 0
          ? appSnapshot.state.videos[videoIndex]?.config?.quality
          : appSnapshot.state.videos.length > 1
            ? appSnapshot.state.commonConfigForBatchCompression?.quality
            : undefined)
    ) {
      if (debounceRef.current) {
        clearTimeout(debounceRef.current)
      }
      debounceRef.current = setTimeout(() => {
        if (videoIndex >= 0 && appProxy.state.videos[videoIndex]?.config) {
          appProxy.state.videos[videoIndex].config.quality = quality
          appProxy.state.videos[videoIndex].isConfigDirty = true
        } else {
          if (appProxy.state.videos.length > 1) {
            appProxy.state.commonConfigForBatchCompression.quality = quality
            normalizeBatchVideosConfig()
          }
        }
      }, 500)
    }
    return () => {
      clearTimeout(debounceRef.current)
    }
  }, [quality, videoIndex])

  React.useEffect(() => {
    if (compressionQuality !== qualityRef.current) {
      if (
        typeof compressionQuality === 'number' &&
        !Number.isNaN(+compressionQuality)
      )
        setQuality(compressionQuality)
    }
  }, [compressionQuality])

  const handleQualityChange = React.useCallback((value: number | number[]) => {
    if (typeof value === 'number') {
      setQuality(value)
    }
  }, [])

  const handleSwitchToggle = useCallback(() => {
    if (videoIndex >= 0 && appProxy.state.videos[videoIndex]?.config) {
      appProxy.state.videos[videoIndex].config.shouldEnableQuality =
        !shouldEnableQuality
      appProxy.state.videos[videoIndex].isConfigDirty = true
    } else {
      if (appProxy.state.videos.length > 1) {
        appProxy.state.commonConfigForBatchCompression.shouldEnableQuality =
          !shouldEnableQuality
        normalizeBatchVideosConfig()
      }
    }
  }, [videoIndex, shouldEnableQuality])

  const shouldDisableInput =
    videos.length === 0 || isCompressing || isProcessCompleted || isLoadingFiles

  return (
    <>
      <Switch
        isSelected={shouldEnableQuality}
        onValueChange={handleSwitchToggle}
        isDisabled={shouldDisableInput}
      >
        <p className="text-gray-600 dark:text-gray-400 text-sm mr-2 w-full">
          Quality
        </p>
      </Switch>
      <AnimatePresence mode="wait">
        {shouldEnableQuality ? (
          <motion.div {...slideDownTransition}>
            <Slider
              label
              aria-label="Quality"
              size="sm"
              marks={[
                {
                  value: 0,
                  label: 'Low',
                },
                {
                  value: 50,
                  label: 'Medium',
                },
                {
                  value: 99,
                  label: 'High',
                },
              ]}
              className="mb-8"
              classNames={{ mark: 'text-xs' }}
              getValue={(value) => {
                const val = Array.isArray(value) ? value?.[0] : +value
                return val < 50
                  ? 'Low'
                  : val >= 50 && val < 100
                    ? 'Medium'
                    : 'High'
              }}
              renderValue={(props) => (
                <p className="text-primary text-sm font-bold">
                  {props?.children}
                </p>
              )}
              value={quality}
              onChange={handleQualityChange}
              isDisabled={!shouldEnableQuality || shouldDisableInput}
            />
          </motion.div>
        ) : null}
      </AnimatePresence>
    </>
  )
}

export default CompressionQuality
