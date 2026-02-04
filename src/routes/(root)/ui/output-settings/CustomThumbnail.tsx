import { core } from '@tauri-apps/api'
import { open } from '@tauri-apps/plugin-dialog'
import { motion } from 'framer-motion'
import { useCallback } from 'react'
import { toast } from 'sonner'
import { useSnapshot } from 'valtio'

import Button from '@/components/Button'
import Divider from '@/components/Divider'
import Icon from '@/components/Icon'
import Image from '@/components/Image'
import Switch from '@/components/Switch'
import TextInput from '@/components/TextInput'
import { slideDownTransition } from '@/utils/animation'
import { cn } from '@/utils/tailwind'
import { appProxy } from '../../-state'

type CustomThumbnailProps = {
  videoIndex: number
}

const THUMBNAIL_EXTENSIONS = ['png', 'jpg', 'jpeg', 'webp']

function CustomThumbnail({ videoIndex }: CustomThumbnailProps) {
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
  const {
    customThumbnailPath,
    shouldEnableCustomThumbnail,
    convertToExtension,
  } = config ?? commonConfigForBatchCompression ?? {}

  const shouldDisableInput =
    videos.length === 0 || isCompressing || isProcessCompleted || isLoadingFiles

  const thumbnailFileName = customThumbnailPath
    ? customThumbnailPath.split(/[/\\]/).pop()
    : null

  const handleThumbnailSelect = useCallback(async () => {
    try {
      const filePath = await open({
        directory: false,
        multiple: false,
        title: 'Select thumbnail image.',
        filters: [
          {
            name: 'image',
            extensions: THUMBNAIL_EXTENSIONS,
          },
        ],
      })
      if (typeof filePath === 'string') {
        if (videoIndex >= 0 && appProxy.state.videos[videoIndex]?.config) {
          appProxy.state.videos[videoIndex].config.customThumbnailPath =
            filePath
          appProxy.state.videos[videoIndex].isConfigDirty = true
        } else {
          if (appProxy.state.videos.length > 1) {
            appProxy.state.commonConfigForBatchCompression.customThumbnailPath =
              filePath
          }
        }
      }
    } catch (error: any) {
      toast.error(error?.message ?? 'Could not select thumbnail image.')
    }
  }, [videoIndex])

  const handleClearThumbnail = useCallback(() => {
    if (videoIndex >= 0 && appProxy.state.videos[videoIndex]?.config) {
      appProxy.state.videos[videoIndex].config.customThumbnailPath = ''
      appProxy.state.videos[videoIndex].isConfigDirty = true
    } else {
      if (appProxy.state.videos.length > 1) {
        appProxy.state.commonConfigForBatchCompression.customThumbnailPath = ''
      }
    }
  }, [videoIndex])

  const handleToggleChange = useCallback(
    (isSelected: boolean) => {
      if (videoIndex >= 0 && appProxy.state.videos[videoIndex]?.config) {
        appProxy.state.videos[videoIndex].config.shouldEnableCustomThumbnail =
          isSelected
        appProxy.state.videos[videoIndex].isConfigDirty = true
      } else {
        if (appProxy.state.videos.length > 1) {
          appProxy.state.commonConfigForBatchCompression.shouldEnableCustomThumbnail =
            isSelected
        }
      }
    },
    [videoIndex],
  )

  return (
    <>
      <div>
        <div className="flex items-center">
          <Switch
            isSelected={Boolean(shouldEnableCustomThumbnail)}
            onValueChange={handleToggleChange}
            className="flex justify-center items-center"
            isDisabled={shouldDisableInput}
            size="sm"
          >
            <div className="flex justify-center items-center">
              <span className="text-gray-600 dark:text-gray-400 block mr-2 text-sm">
                Custom Thumbnail{' '}
              </span>
            </div>
          </Switch>
        </div>
        {shouldEnableCustomThumbnail ? (
          <motion.div {...slideDownTransition} className="mt-2">
            <TextInput
              type="text"
              label=""
              placeholder="No thumbnail selected"
              value={thumbnailFileName ?? 'No thumbnail selected'}
              isDisabled={shouldDisableInput || convertToExtension === 'webm'}
              isReadOnly
              classNames={{
                input: 'text-xs',
                mainWrapper: 'my-3',
              }}
            />
            {customThumbnailPath ? (
              <div
                className={cn(
                  'flex justify-center items-center w-full',
                  shouldDisableInput || convertToExtension === 'webm'
                    ? 'opacity-50'
                    : '',
                )}
              >
                <Image
                  alt="custom thumbnail"
                  src={core.convertFileSrc(customThumbnailPath)}
                  className={
                    'max-w-[200px] max-h-[200px] mx-auto object-contain mb-4'
                  }
                />
              </div>
            ) : null}
            <div>
              {!thumbnailFileName ? (
                <Button
                  type="button"
                  onPress={handleThumbnailSelect}
                  fullWidth
                  size="sm"
                  isDisabled={
                    shouldDisableInput || convertToExtension === 'webm'
                  }
                >
                  Choose
                  <Icon name="fileExplorer" size={14} />
                </Button>
              ) : (
                <Button
                  type="button"
                  onPress={handleClearThumbnail}
                  fullWidth
                  size="sm"
                  isDisabled={
                    shouldDisableInput || convertToExtension === 'webm'
                  }
                  color="danger"
                >
                  Clear
                </Button>
              )}
              {convertToExtension === 'webm' ? (
                <p className="text-xs italic text-danger-300 mt-2">
                  webm does not support custom thumbnail
                </p>
              ) : null}
            </div>
          </motion.div>
        ) : null}
        <Divider className="mt-3 mb-6" />
      </div>
    </>
  )
}

export default CustomThumbnail
