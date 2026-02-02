import { SelectItem } from '@heroui/select'
import { useCallback } from 'react'
import { useSnapshot } from 'valtio'

import Select from '@/components/Select'
import { extensions } from '@/types/compression'
import { appProxy, normalizeBatchVideosConfig } from '../../-state'

const videoExtensions = Object.keys(extensions?.video)

type VideoExtensionProps = {
  videoIndex: number
}

function VideoExtension({ videoIndex }: VideoExtensionProps) {
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
  const { convertToExtension } = config ?? commonConfigForBatchCompression ?? {}

  const handleValueChange = useCallback(
    (value: keyof typeof extensions.video) => {
      if (value?.length > 0) {
        if (videoIndex >= 0 && appProxy.state.videos[videoIndex]?.config) {
          appProxy.state.videos[videoIndex].config.convertToExtension = value
          appProxy.state.videos[videoIndex].isConfigDirty = true
        } else {
          if (appProxy.state.videos.length > 1) {
            appProxy.state.commonConfigForBatchCompression.convertToExtension =
              value
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
    <Select
      fullWidth
      label="Extension:"
      className="block flex-shrink-0 rounded-2xl"
      size="sm"
      value={convertToExtension}
      selectedKeys={[convertToExtension!]}
      onChange={(evt) => {
        const value = evt?.target
          ?.value as unknown as keyof typeof extensions.video
        handleValueChange(value)
      }}
      selectionMode="single"
      isDisabled={shouldDisableInput}
      classNames={{
        label: '!text-gray-600 dark:!text-gray-400 text-sm',
      }}
    >
      {videoExtensions?.map((ext) => (
        <SelectItem
          key={ext}
          value={ext}
          className="flex justify-center items-center"
        >
          {ext}
        </SelectItem>
      ))}
    </Select>
  )
}

export default VideoExtension
