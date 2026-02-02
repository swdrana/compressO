import { AnimatePresence, motion } from 'framer-motion'
import React, { useEffect, useMemo } from 'react'
import { useSnapshot } from 'valtio'

import NumberInput from '@/components/NumberInput'
import Switch from '@/components/Switch'
import { slideDownTransition } from '@/utils/animation'
import { appProxy } from '../../-state'

type VideoDimensionsProps = {
  videoIndex: number
}

function VideoDimensions({ videoIndex }: VideoDimensionsProps) {
  if (videoIndex < 0) return

  const {
    state: { videos, isCompressing, isProcessCompleted, isLoadingFiles },
  } = useSnapshot(appProxy)
  const video = videos.length > 0 ? videos[videoIndex] : null
  const { config, dimensions: videoOriginalDimensions } = video ?? {}
  const {
    shouldEnableCustomDimensions,
    shouldTransformVideo,
    transformVideoConfig,
  } = config ?? {}

  const videoDimensions = useMemo(
    () =>
      shouldTransformVideo
        ? {
            width:
              transformVideoConfig?.transforms?.crop?.width ??
              videoOriginalDimensions?.width,
            height:
              transformVideoConfig?.transforms?.crop?.height ??
              videoOriginalDimensions?.height,
          }
        : videoOriginalDimensions,
    [
      shouldTransformVideo,
      transformVideoConfig?.transforms?.crop?.height,
      transformVideoConfig?.transforms?.crop?.width,
      videoOriginalDimensions,
    ],
  )

  const [dimensions, setDimensions] = React.useState({
    width: videoDimensions?.width ?? 0,
    height: videoDimensions?.height ?? 0,
  })

  useEffect(() => {
    if (
      videoIndex >= 0 &&
      !Number.isNaN(videoDimensions?.width) &&
      !Number.isNaN(videoDimensions)
    ) {
      const _dimensions: [number, number] = [
        videoDimensions?.width ?? 0,
        videoDimensions?.height ?? 0,
      ]
      setDimensions({
        width: _dimensions[0],
        height: _dimensions[1],
      })
      appProxy.state.videos[videoIndex].config.customDimensions = _dimensions
      appProxy.state.videos[videoIndex].isConfigDirty = true
    }
  }, [videoDimensions, videoIndex])

  useEffect(() => {
    if (shouldTransformVideo) {
      if (transformVideoConfig) {
        const transforms = transformVideoConfig?.transforms
        if (transforms?.crop) {
          setDimensions({
            width: transforms.crop.width,
            height: transforms.crop.height,
          })
        }
      }
    } else {
      if (videoDimensions) {
        setDimensions({
          width: videoDimensions.width!,
          height: videoDimensions.height!,
        })
      }
    }
  }, [videoDimensions, shouldTransformVideo, transformVideoConfig])

  const handleChange = (value: number, type: 'width' | 'height') => {
    if (
      appProxy.state.videos.length !== 1 ||
      !value ||
      value <= 0 ||
      videoDimensions == null ||
      Number.isNaN(videoDimensions?.width) ||
      Number.isNaN(videoDimensions?.height)
    ) {
      return
    }
    const aspectRatio = videoDimensions.width! / videoDimensions.height!
    const _dimensions: [number, number] =
      type === 'width'
        ? [value, Math.round(value / aspectRatio)]
        : [Math.round(value * aspectRatio), value]
    setDimensions((s) => ({
      ...s,
      width: _dimensions[0],
      height: _dimensions[1],
    }))
    appProxy.state.videos[videoIndex].config.customDimensions = _dimensions
    appProxy.state.videos[videoIndex].isConfigDirty = true
  }

  const shouldDisableInput =
    videos.length === 0 || isCompressing || isProcessCompleted || isLoadingFiles

  return (
    <>
      <Switch
        isSelected={shouldEnableCustomDimensions}
        onValueChange={() => {
          if (appProxy.state.videos[videoIndex]?.config) {
            appProxy.state.videos[
              videoIndex
            ].config.shouldEnableCustomDimensions =
              !shouldEnableCustomDimensions
            appProxy.state.videos[videoIndex].isConfigDirty = true
          }
        }}
        isDisabled={shouldDisableInput}
      >
        <p className="text-gray-600 dark:text-gray-400 text-sm mr-2 w-full">
          Dimensions
        </p>
      </Switch>
      <AnimatePresence mode="wait">
        {shouldEnableCustomDimensions ? (
          <motion.div
            {...slideDownTransition}
            className="mt-2 flex items-center space-x-2"
          >
            <NumberInput
              label="Width"
              className="max-w-[120px] xl:max-w-[150px]"
              value={dimensions?.width}
              onValueChange={(val) => handleChange(val, 'width')}
              labelPlacement="outside"
              classNames={{ label: '!text-gray-600 dark:!text-gray-400' }}
              isDisabled={!shouldEnableCustomDimensions || shouldDisableInput}
            />
            <NumberInput
              label="Height"
              className="max-w-[120px] xl:max-w-[150px]"
              value={dimensions?.height}
              onValueChange={(val) => handleChange(val, 'height')}
              labelPlacement="outside"
              classNames={{ label: '!text-gray-600 dark:!text-gray-400' }}
              isDisabled={
                videos.length === 0 ||
                isCompressing ||
                isProcessCompleted ||
                isLoadingFiles
              }
            />
          </motion.div>
        ) : null}
      </AnimatePresence>
    </>
  )
}

export default VideoDimensions
