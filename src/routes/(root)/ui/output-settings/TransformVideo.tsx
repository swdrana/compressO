import { useSnapshot } from 'valtio'

import Switch from '@/components/Switch'
import { appProxy } from '../../-state'

type TransformVideoProps = {
  videoIndex: number
}

function TransformVideo({ videoIndex }: TransformVideoProps) {
  if (videoIndex < 0) return

  const {
    state: { videos, isCompressing, isProcessCompleted, isLoadingFiles },
  } = useSnapshot(appProxy)
  const video = videos.length > 0 ? videos[videoIndex] : null
  const { config } = video ?? {}
  const { shouldTransformVideo } = config ?? {}

  const shouldDisableInput =
    videos.length === 0 || isCompressing || isProcessCompleted || isLoadingFiles

  return (
    <>
      <Switch
        isSelected={shouldTransformVideo}
        onValueChange={() => {
          if (appProxy.state.videos[videoIndex]?.config) {
            appProxy.state.videos[videoIndex].config.shouldTransformVideo =
              !shouldTransformVideo
            appProxy.state.videos[videoIndex].isConfigDirty = true

            if (shouldTransformVideo) {
              appProxy.state.videos[videoIndex].config.transformVideoConfig =
                undefined
            }
          }
        }}
        isDisabled={shouldDisableInput}
      >
        <p className="text-gray-600 dark:text-gray-400 text-sm mr-2 w-full">
          Transform
        </p>
      </Switch>
    </>
  )
}

export default TransformVideo
