import { SelectItem } from '@heroui/react'
import { AnimatePresence, motion } from 'framer-motion'
import { useCallback } from 'react'
import { useSnapshot } from 'valtio'

import Icon from '@/components/Icon'
import Select from '@/components/Select'
import Switch from '@/components/Switch'
import Tooltip from '@/components/Tooltip'
import { compressionPresets } from '@/types/compression'
import { slideDownTransition } from '@/utils/animation'
import { appProxy, normalizeBatchVideosConfig } from '../../-state'

const PRESETS: {
  name: keyof typeof compressionPresets
  description: React.ReactNode
}[] = [
  {
    name: 'ironclad',
    description: <p>Optimal size but slightly slower processing</p>,
  },
  {
    name: 'thunderbolt',
    description: <p>Slightly larger size but faster processing</p>,
  },
]

type CompressionPresetProps = {
  videoIndex: number
}

function CompressionPreset({ videoIndex }: CompressionPresetProps) {
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
  const { presetName, shouldDisableCompression } =
    config ?? commonConfigForBatchCompression ?? {}

  const handleSwitchToggle = useCallback(() => {
    if (videoIndex >= 0 && appProxy.state.videos[videoIndex]?.config) {
      appProxy.state.videos[videoIndex].config.shouldDisableCompression =
        !shouldDisableCompression
      appProxy.state.videos[videoIndex].isConfigDirty = true
    } else {
      if (appProxy.state.videos.length > 1) {
        appProxy.state.commonConfigForBatchCompression.shouldDisableCompression =
          !shouldDisableCompression
        normalizeBatchVideosConfig()
      }
    }
  }, [videoIndex, shouldDisableCompression])

  const handleValueChange = useCallback(
    (value: keyof typeof compressionPresets) => {
      if (value?.length > 0) {
        if (videoIndex >= 0 && appProxy.state.videos[videoIndex]?.config) {
          appProxy.state.videos[videoIndex].config.presetName = value
          appProxy.state.videos[videoIndex].isConfigDirty = true
        } else {
          if (appProxy.state.videos.length > 1) {
            appProxy.state.commonConfigForBatchCompression.presetName = value
            normalizeBatchVideosConfig()
          }
        }
      }
    },
    [videoIndex],
  )

  const shouldDisableInput =
    videos.length === 0 || isCompressing || isProcessCompleted || isLoadingFiles

  return (
    <>
      <div className="flex items-center mb-4">
        <Switch
          isSelected={!shouldDisableCompression}
          onValueChange={handleSwitchToggle}
          className="flex justify-center items-center"
          isDisabled={shouldDisableInput}
          size="sm"
        >
          <div className="flex justify-center items-center">
            <span className="text-gray-600 dark:text-gray-400 block mr-2 text-sm">
              Compress
            </span>
          </div>
        </Switch>
      </div>
      <AnimatePresence mode="wait">
        {!shouldDisableCompression ? (
          <motion.div {...slideDownTransition} className="mt-2">
            <div className="mt-8">
              <Select
                fullWidth
                label="Compression Preset:"
                labelPlacement="outside"
                className="block flex-shrink-0 rounded-2xl"
                selectedKeys={[presetName!]}
                onChange={(evt) => {
                  const value = evt?.target
                    ?.value as unknown as keyof typeof compressionPresets
                  handleValueChange(value)
                }}
                selectionMode="single"
                isDisabled={shouldDisableCompression || shouldDisableInput}
                classNames={{
                  label: '!text-gray-600 dark:!text-gray-400 text-xs',
                }}
              >
                {PRESETS?.map((preset) => (
                  // Right now if we use SelectItem it breaks the code so opting for SelectItem from NextUI directly
                  <SelectItem
                    key={preset.name}
                    textValue={preset.name}
                    className="flex justify-center items-center"
                    endContent={
                      preset.name === compressionPresets.ironclad ? (
                        <Tooltip content="Recommended" aria-label="Recommended">
                          <Icon
                            name="star"
                            className="inline-block ml-1 text-yellow-500"
                            size={15}
                          />
                        </Tooltip>
                      ) : null
                    }
                    description={preset.description}
                  >
                    {preset.name}
                  </SelectItem>
                ))}
              </Select>
            </div>
          </motion.div>
        ) : null}
      </AnimatePresence>
    </>
  )
}

export default CompressionPreset
