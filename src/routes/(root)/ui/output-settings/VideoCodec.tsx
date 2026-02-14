import { SelectItem } from '@heroui/react'
import { AnimatePresence, motion } from 'framer-motion'
import { useCallback, useEffect } from 'react'
import { useSnapshot } from 'valtio'

import Select from '@/components/Select'
import Switch from '@/components/Switch'
import { extensions } from '@/types/compression'
import { slideDownTransition } from '@/utils/animation'
import { appProxy, normalizeBatchVideosConfig } from '../../-state'

type VideoExtension = keyof typeof extensions.video

type VideoCodecOption = {
  value: string
  name: string
  description: string
  compatible_containers: VideoExtension[]
}

const VIDEO_CODECS: readonly VideoCodecOption[] = [
  {
    value: 'libx264',
    name: 'H.264 (AVC)',
    description: 'Most compatible, good quality',
    compatible_containers: ['mp4', 'mov', 'mkv', 'avi'] as VideoExtension[],
  },
  {
    value: 'libx265',
    name: 'H.265 (HEVC)',
    description: 'Better compression, newer standard',
    compatible_containers: ['mp4', 'mov', 'mkv'] as VideoExtension[],
  },
  {
    value: 'libvpx-vp9',
    name: 'VP9',
    description: 'Open-source, great for web',
    compatible_containers: ['webm', 'mkv'] as VideoExtension[],
  },
  {
    value: 'libaom-av1',
    name: 'AV1',
    description: 'Best compression, very slow',
    compatible_containers: ['mp4', 'mkv', 'webm'] as VideoExtension[],
  },
  {
    value: 'mpeg4',
    name: 'MPEG-4',
    description: 'Legacy codec, wide support',
    compatible_containers: ['mp4', 'mov', 'mkv', 'avi'] as VideoExtension[],
  },
]

type VideoCodecProps = {
  videoIndex: number
}

function VideoCodec({ videoIndex }: VideoCodecProps) {
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
  const { config } = video ?? {}
  const { shouldEnableCustomVideoCodec, customVideoCodec, convertToExtension } =
    config ?? commonConfigForBatchCompression ?? {}

  const currentExtension = convertToExtension ?? 'mp4'

  // Reset codec if it's not compatible with the current extension
  useEffect(() => {
    if (shouldEnableCustomVideoCodec && customVideoCodec) {
      const currentCodec = VIDEO_CODECS.find(
        (c) => c.value === customVideoCodec,
      )
      if (
        currentCodec &&
        !currentCodec.compatible_containers.includes(currentExtension)
      ) {
        // Codec is incompatible with current extension, reset it
        if (videoIndex >= 0 && appProxy.state.videos[videoIndex]?.config) {
          appProxy.state.videos[videoIndex].config.customVideoCodec = undefined
        } else {
          if (appProxy.state.videos.length > 1) {
            appProxy.state.commonConfigForBatchCompression.customVideoCodec =
              undefined
          }
        }
      }
    }
  }, [
    currentExtension,
    shouldEnableCustomVideoCodec,
    customVideoCodec,
    videoIndex,
  ])

  const handleSwitchToggle = useCallback(() => {
    if (videoIndex >= 0 && appProxy.state.videos[videoIndex]?.config) {
      appProxy.state.videos[videoIndex].config.shouldEnableCustomVideoCodec =
        !shouldEnableCustomVideoCodec
      appProxy.state.videos[videoIndex].isConfigDirty = true
    } else {
      if (appProxy.state.videos.length > 1) {
        appProxy.state.commonConfigForBatchCompression.shouldEnableCustomVideoCodec =
          !shouldEnableCustomVideoCodec
        normalizeBatchVideosConfig()
      }
    }
  }, [videoIndex, shouldEnableCustomVideoCodec])

  const handleValueChange = useCallback(
    (value: string) => {
      if (videoIndex >= 0 && appProxy.state.videos[videoIndex]?.config) {
        appProxy.state.videos[videoIndex].config.customVideoCodec = value
        appProxy.state.videos[videoIndex].isConfigDirty = true
      } else {
        if (appProxy.state.videos.length > 1) {
          appProxy.state.commonConfigForBatchCompression.customVideoCodec =
            value
          normalizeBatchVideosConfig()
        }
      }
    },
    [videoIndex],
  )

  const shouldDisableInput =
    videos.length === 0 || isCompressing || isProcessCompleted || isLoadingFiles

  const initialCodecValue = customVideoCodec ?? 'libx264'

  const compatibleCodecs = VIDEO_CODECS.filter((codec) =>
    codec.compatible_containers.includes(currentExtension),
  )

  return (
    <>
      <Switch
        isSelected={shouldEnableCustomVideoCodec}
        onValueChange={handleSwitchToggle}
        isDisabled={shouldDisableInput}
      >
        <p className="text-gray-600 dark:text-gray-400 text-sm mr-2 w-full">
          Codec
        </p>
      </Switch>
      <AnimatePresence mode="wait">
        {shouldEnableCustomVideoCodec ? (
          <motion.div {...slideDownTransition}>
            <Select
              fullWidth
              label="Select Codec:"
              className="block flex-shrink-0 rounded-2xl !mt-8"
              selectedKeys={[initialCodecValue]}
              size="sm"
              value={initialCodecValue}
              onChange={(evt) => {
                const value = evt?.target?.value
                if (value) {
                  handleValueChange(value)
                }
              }}
              selectionMode="single"
              isDisabled={!shouldEnableCustomVideoCodec || shouldDisableInput}
              classNames={{
                label: '!text-gray-600 dark:!text-gray-400 text-xs',
              }}
            >
              {compatibleCodecs?.map((codec) => (
                <SelectItem
                  key={codec.value}
                  textValue={codec.name}
                  className="flex justify-center items-center"
                >
                  <div className="flex flex-col">
                    <span className="text-sm">{codec.name}</span>
                    <span className="text-xs text-gray-500">
                      {codec.description}
                    </span>
                  </div>
                </SelectItem>
              ))}
            </Select>
          </motion.div>
        ) : null}
      </AnimatePresence>
    </>
  )
}

export default VideoCodec
