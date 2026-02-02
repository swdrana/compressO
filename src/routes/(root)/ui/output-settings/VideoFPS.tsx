import { SelectItem } from '@heroui/select'
import { AnimatePresence, motion } from 'framer-motion'
import { useCallback } from 'react'
import { useSnapshot } from 'valtio'

import Select from '@/components/Select'
import Switch from '@/components/Switch'
import { slideDownTransition } from '@/utils/animation'
import { appProxy, normalizeBatchVideosConfig } from '../../-state'

const FPS = [24, 25, 30, 50, 60] as const

type VideoFPSProps = {
  videoIndex: number
}

function VideoFPS({ videoIndex }: VideoFPSProps) {
  const {
    state: {
      videos,
      isCompressing,
      isProcessCompleted,
      commonConfigForBatchCompression,
      isLoadingFiles,
    },
  } = useSnapshot(appProxy)
  const video = videos.length > 0 && videoIndex >= 0 ? videos[videoIndex] : null
  const { config, fps } = video ?? {}
  const { shouldEnableCustomFPS, customFPS } =
    config ?? commonConfigForBatchCompression ?? {}

  const handleSwitchToggle = useCallback(() => {
    if (videoIndex >= 0 && appProxy.state.videos[videoIndex]?.config) {
      appProxy.state.videos[videoIndex].config.shouldEnableCustomFPS =
        !shouldEnableCustomFPS
      appProxy.state.videos[videoIndex].isConfigDirty = true
    } else {
      if (appProxy.state.videos.length > 1) {
        appProxy.state.commonConfigForBatchCompression.shouldEnableCustomFPS =
          !shouldEnableCustomFPS
        normalizeBatchVideosConfig()
      }
    }
  }, [videoIndex, shouldEnableCustomFPS])

  const handleValueChange = useCallback(
    (value: number) => {
      if (videoIndex >= 0 && appProxy.state.videos[videoIndex]?.config) {
        appProxy.state.videos[videoIndex].config.customFPS = +value
        appProxy.state.videos[videoIndex].isConfigDirty = true
      } else {
        if (appProxy.state.videos.length > 1) {
          appProxy.state.commonConfigForBatchCompression.customFPS = +value
          normalizeBatchVideosConfig()
        }
      }
    },
    [videoIndex],
  )

  const shouldDisableInput =
    videos.length === 0 || isCompressing || isProcessCompleted || isLoadingFiles

  return (
    <>
      <Switch
        isSelected={shouldEnableCustomFPS}
        onValueChange={handleSwitchToggle}
        isDisabled={shouldDisableInput}
      >
        <p className="text-gray-600 dark:text-gray-400 text-sm mr-2 w-full">
          FPS
        </p>
      </Switch>
      <AnimatePresence mode="wait">
        {shouldEnableCustomFPS ? (
          <motion.div {...slideDownTransition}>
            <Select
              fullWidth
              label="Frames Per Second:"
              className="block flex-shrink-0 rounded-2xl !mt-8"
              selectedKeys={[String(customFPS ?? fps)!]}
              size="sm"
              value={String(customFPS ?? fps)}
              onChange={(evt) => {
                const value = evt?.target?.value
                if (value && !Number.isNaN(+value)) {
                  handleValueChange(+value)
                }
              }}
              selectionMode="single"
              isDisabled={!shouldEnableCustomFPS || shouldDisableInput}
              classNames={{
                label: '!text-gray-600 dark:!text-gray-400 text-xs',
              }}
            >
              {FPS?.map((f) => (
                <SelectItem
                  key={String(f)}
                  value={String(f)}
                  className="flex justify-center items-center"
                >
                  {String(f)}
                </SelectItem>
              ))}
            </Select>
          </motion.div>
        ) : null}
      </AnimatePresence>
    </>
  )
}

export default VideoFPS
