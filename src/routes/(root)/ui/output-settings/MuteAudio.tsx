import { useCallback } from 'react'
import { useSnapshot } from 'valtio'

import Switch from '@/components/Switch'
import { appProxy, normalizeBatchVideosConfig } from '../../-state'

type MuteAudioProps = {
  videoIndex: number
}

function MuteAudio({ videoIndex }: MuteAudioProps) {
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
  const { shouldMuteVideo } = config ?? commonConfigForBatchCompression ?? {}

  const handleSwitchToggle = useCallback(() => {
    if (videoIndex >= 0 && appProxy.state.videos[videoIndex]?.config) {
      appProxy.state.videos[videoIndex].config.shouldMuteVideo =
        !shouldMuteVideo
      appProxy.state.videos[videoIndex].isConfigDirty = true
    } else {
      if (appProxy.state.videos.length > 1) {
        appProxy.state.commonConfigForBatchCompression.shouldMuteVideo =
          !shouldMuteVideo
        normalizeBatchVideosConfig()
      }
    }
  }, [videoIndex, shouldMuteVideo])

  const shouldDisableInput =
    videos.length === 0 || isCompressing || isProcessCompleted || isLoadingFiles

  return (
    <div className="flex items-center my-2">
      <Switch
        isSelected={shouldMuteVideo}
        onValueChange={handleSwitchToggle}
        className="flex justify-center items-center"
        isDisabled={shouldDisableInput}
      >
        <div className="flex justify-center items-center">
          <span className="text-gray-600 dark:text-gray-400 block mr-2 text-sm">
            Mute Audio
          </span>
        </div>
      </Switch>
    </div>
  )
}

export default MuteAudio
