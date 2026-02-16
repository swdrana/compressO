import { SelectItem } from '@heroui/react'
import { AnimatePresence, motion } from 'framer-motion'
import { useCallback } from 'react'
import { useSnapshot } from 'valtio'

import Checkbox from '@/components/Checkbox'
import Divider from '@/components/Divider'
import Select from '@/components/Select'
import Switch from '@/components/Switch'
import { slideDownTransition } from '@/utils/animation'
import { appProxy, normalizeBatchVideosConfig } from '../../-state'

type AudioChannelsProps = {
  videoIndex: number
}

function AudioChannels({ videoIndex }: AudioChannelsProps) {
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
  const { config, videoInfoRaw } = video ?? {}
  const { audioConfig, shouldEnableCustomChannel } =
    config ?? commonConfigForBatchCompression ?? {}

  const handleSwitchToggle = useCallback(() => {
    if (videoIndex >= 0 && appProxy.state.videos[videoIndex]?.config) {
      const videoConfig = appProxy.state.videos[videoIndex].config
      videoConfig.shouldEnableCustomChannel = !shouldEnableCustomChannel
      if (!shouldEnableCustomChannel) {
        if (!videoConfig.audioConfig) {
          videoConfig.audioConfig = { volume: 100 }
        }
        videoConfig.audioConfig.audioChannelConfig = {
          channelLayout: 'stereo',
        }
      } else {
        if (videoConfig.audioConfig) {
          videoConfig.audioConfig.audioChannelConfig = null
        }
      }
      appProxy.state.videos[videoIndex].isConfigDirty = true
    } else {
      if (appProxy.state.videos.length > 1) {
        const commonConfig = appProxy.state.commonConfigForBatchCompression
        commonConfig.shouldEnableCustomChannel = !shouldEnableCustomChannel
        if (!commonConfig.audioConfig) {
          commonConfig.audioConfig = { volume: 100 }
        }
        if (!shouldEnableCustomChannel) {
          commonConfig.audioConfig.audioChannelConfig = {
            channelLayout: 'stereo',
          }
        } else {
          commonConfig.audioConfig.audioChannelConfig = null
        }
        normalizeBatchVideosConfig()
      }
    }
  }, [videoIndex, shouldEnableCustomChannel])

  const handleChannelLayoutChange = useCallback(
    (value: string) => {
      const newLayout = value as 'mono' | 'stereo'

      if (videoIndex >= 0 && appProxy.state.videos[videoIndex]?.config) {
        const videoConfig = appProxy.state.videos[videoIndex].config
        if (!videoConfig.audioConfig) {
          videoConfig.audioConfig = { volume: 100 }
        }
        if (newLayout === 'mono') {
          videoConfig.audioConfig.audioChannelConfig = {
            channelLayout: newLayout,
            monoSource: { left: true, right: true },
          }
        } else {
          videoConfig.audioConfig.audioChannelConfig = {
            channelLayout: newLayout,
            stereoSwapChannels: false,
          }
        }
        appProxy.state.videos[videoIndex].isConfigDirty = true
      } else {
        if (appProxy.state.videos.length > 1) {
          if (!appProxy.state.commonConfigForBatchCompression.audioConfig) {
            appProxy.state.commonConfigForBatchCompression.audioConfig = {
              volume: 100,
            }
          }
          if (newLayout === 'mono') {
            appProxy.state.commonConfigForBatchCompression.audioConfig.audioChannelConfig =
              {
                channelLayout: newLayout,
                monoSource: { left: true, right: true },
              }
          } else {
            appProxy.state.commonConfigForBatchCompression.audioConfig.audioChannelConfig =
              {
                channelLayout: newLayout,
                stereoSwapChannels: false,
              }
          }
          normalizeBatchVideosConfig()
        }
      }
    },
    [videoIndex],
  )

  const handleMonoLeftChange = useCallback(
    (isSelected: boolean) => {
      if (videoIndex >= 0 && appProxy.state.videos[videoIndex]?.config) {
        const videoConfig = appProxy.state.videos[videoIndex].config
        if (!videoConfig.audioConfig.audioChannelConfig) {
          videoConfig.audioConfig.audioChannelConfig = {
            channelLayout: 'mono',
          }
        }
        if (!videoConfig.audioConfig.audioChannelConfig.monoSource) {
          videoConfig.audioConfig.audioChannelConfig.monoSource = {
            left: true,
            right: true,
          }
        }
        videoConfig.audioConfig.audioChannelConfig.monoSource!.left = isSelected
        appProxy.state.videos[videoIndex].isConfigDirty = true
      } else {
        if (appProxy.state.videos.length > 1) {
          if (
            !appProxy.state.commonConfigForBatchCompression.audioConfig
              .audioChannelConfig
          ) {
            appProxy.state.commonConfigForBatchCompression.audioConfig.audioChannelConfig =
              {
                channelLayout: 'mono',
              }
          }
          if (
            !appProxy.state.commonConfigForBatchCompression.audioConfig
              .audioChannelConfig!.monoSource
          ) {
            appProxy.state.commonConfigForBatchCompression.audioConfig
              .audioChannelConfig!.monoSource = {
              left: true,
              right: true,
            }
          }
          appProxy.state.commonConfigForBatchCompression.audioConfig
            .audioChannelConfig!.monoSource!.left = isSelected
          normalizeBatchVideosConfig()
        }
      }
    },
    [videoIndex],
  )

  const handleMonoRightChange = useCallback(
    (isSelected: boolean) => {
      if (videoIndex >= 0 && appProxy.state.videos[videoIndex]?.config) {
        const videoConfig = appProxy.state.videos[videoIndex].config
        if (!videoConfig.audioConfig.audioChannelConfig) {
          videoConfig.audioConfig.audioChannelConfig = {
            channelLayout: 'mono',
          }
        }
        if (!videoConfig.audioConfig.audioChannelConfig.monoSource) {
          videoConfig.audioConfig.audioChannelConfig.monoSource = {
            left: true,
            right: true,
          }
        }
        videoConfig.audioConfig.audioChannelConfig.monoSource!.right =
          isSelected
        appProxy.state.videos[videoIndex].isConfigDirty = true
      } else {
        if (appProxy.state.videos.length > 1) {
          if (
            !appProxy.state.commonConfigForBatchCompression.audioConfig
              .audioChannelConfig
          ) {
            appProxy.state.commonConfigForBatchCompression.audioConfig.audioChannelConfig =
              {
                channelLayout: 'mono',
              }
          }
          if (
            !appProxy.state.commonConfigForBatchCompression.audioConfig
              .audioChannelConfig!.monoSource
          ) {
            appProxy.state.commonConfigForBatchCompression.audioConfig
              .audioChannelConfig!.monoSource = {
              left: true,
              right: true,
            }
          }
          appProxy.state.commonConfigForBatchCompression.audioConfig
            .audioChannelConfig!.monoSource!.right = isSelected
          normalizeBatchVideosConfig()
        }
      }
    },
    [videoIndex],
  )

  const handleStereoSwapChange = useCallback(() => {
    const currentConfig =
      videoIndex >= 0 && appProxy.state.videos[videoIndex]?.config
        ? appProxy.state.videos[videoIndex].config.audioConfig
            ?.audioChannelConfig
        : appProxy.state.commonConfigForBatchCompression.audioConfig
            ?.audioChannelConfig

    const newValue = !currentConfig?.stereoSwapChannels

    if (videoIndex >= 0 && appProxy.state.videos[videoIndex]?.config) {
      const videoConfig = appProxy.state.videos[videoIndex].config
      if (!videoConfig.audioConfig.audioChannelConfig) {
        videoConfig.audioConfig.audioChannelConfig = {
          channelLayout: 'stereo',
        }
      }
      videoConfig.audioConfig.audioChannelConfig.stereoSwapChannels = newValue
      appProxy.state.videos[videoIndex].isConfigDirty = true
    } else {
      if (appProxy.state.videos.length > 1) {
        if (
          !appProxy.state.commonConfigForBatchCompression.audioConfig
            .audioChannelConfig
        ) {
          appProxy.state.commonConfigForBatchCompression.audioConfig.audioChannelConfig =
            {
              channelLayout: 'stereo',
            }
        }
        appProxy.state.commonConfigForBatchCompression.audioConfig
          .audioChannelConfig!.stereoSwapChannels = newValue
        normalizeBatchVideosConfig()
      }
    }
  }, [videoIndex])

  const shouldDisableInput =
    videos.length === 0 ||
    isCompressing ||
    isProcessCompleted ||
    isLoadingFiles ||
    audioConfig?.volume === 0

  const hasNoAudio = videoInfoRaw?.audioStreams?.length === 0

  return (
    <div>
      <Switch
        isSelected={shouldEnableCustomChannel}
        onValueChange={handleSwitchToggle}
        isDisabled={shouldDisableInput || hasNoAudio}
      >
        <p className="text-gray-600 dark:text-gray-400 text-sm mr-2 w-full">
          Channel
        </p>
      </Switch>
      <AnimatePresence mode="wait">
        {shouldEnableCustomChannel ? (
          <motion.div {...slideDownTransition}>
            <Select
              fullWidth
              label="Layout:"
              className="block flex-shrink-0 rounded-2xl !mt-8"
              size="sm"
              value={audioConfig?.audioChannelConfig?.channelLayout ?? 'stereo'}
              selectedKeys={[
                audioConfig?.audioChannelConfig?.channelLayout ?? 'stereo',
              ]}
              onChange={(evt) => {
                const value = evt?.target?.value
                if (value) {
                  handleChannelLayoutChange(value)
                }
              }}
              selectionMode="single"
              isDisabled={!shouldEnableCustomChannel || shouldDisableInput}
              classNames={{
                label: '!text-gray-600 dark:!text-gray-400 text-xs',
              }}
            >
              <SelectItem key="mono" textValue="Mono">
                Mono
              </SelectItem>
              <SelectItem key="stereo" textValue="Stereo">
                Stereo
              </SelectItem>
            </Select>
            <AnimatePresence mode="wait">
              {audioConfig?.audioChannelConfig?.channelLayout === 'mono' ? (
                <motion.div {...slideDownTransition} className="mt-4">
                  <p className="text-xs text-gray-600 dark:text-gray-400 mb-2">
                    Mono Source:
                  </p>
                  <div className="flex gap-4">
                    <Checkbox
                      isSelected={
                        audioConfig?.audioChannelConfig?.monoSource?.left ??
                        true
                      }
                      onValueChange={handleMonoLeftChange}
                      isDisabled={shouldDisableInput}
                    >
                      <span className="text-sm">Left</span>
                    </Checkbox>
                    <Divider orientation="vertical" className="h-5" />
                    <Checkbox
                      isSelected={
                        audioConfig?.audioChannelConfig?.monoSource?.right ??
                        true
                      }
                      onValueChange={handleMonoRightChange}
                      isDisabled={shouldDisableInput}
                    >
                      <span className="text-sm">Right</span>
                    </Checkbox>
                  </div>
                </motion.div>
              ) : null}
              {audioConfig?.audioChannelConfig?.channelLayout === 'stereo' ? (
                <motion.div {...slideDownTransition} className="mt-4">
                  <Switch
                    isSelected={
                      audioConfig?.audioChannelConfig?.stereoSwapChannels ??
                      false
                    }
                    onValueChange={handleStereoSwapChange}
                    isDisabled={shouldDisableInput}
                  >
                    <p className="text-gray-600 dark:text-gray-400 text-sm mr-2 w-full">
                      Swap left and right channels
                    </p>
                  </Switch>
                </motion.div>
              ) : null}
            </AnimatePresence>
          </motion.div>
        ) : null}
      </AnimatePresence>
    </div>
  )
}

export default AudioChannels
