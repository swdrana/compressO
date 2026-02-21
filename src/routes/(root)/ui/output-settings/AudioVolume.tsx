import React, { useCallback } from 'react'
import { snapshot, useSnapshot } from 'valtio'

import Slider from '@/components/Slider/Slider'
import { appProxy, normalizeBatchVideosConfig } from '../../-state'

type AudioVolumeProps = {
  videoIndex: number
}

function AudioVolume({ videoIndex }: AudioVolumeProps) {
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
  const { audioConfig } = config ?? commonConfigForBatchCompression ?? {}

  const [volume, setVolume] = React.useState<number>(audioConfig?.volume ?? 100)
  const debounceRef = React.useRef<NodeJS.Timeout>()
  const volumeRef = React.useRef<number>(volume)

  React.useEffect(() => {
    volumeRef.current = volume
  }, [volume])

  React.useEffect(() => {
    const appSnapshot = snapshot(appProxy)
    if (
      appSnapshot.state.videos.length &&
      volume !==
        (videoIndex >= 0
          ? appSnapshot.state.videos[videoIndex]?.config?.audioConfig?.volume
          : appSnapshot.state.videos.length > 1
            ? appSnapshot.state.commonConfigForBatchCompression?.audioConfig
                ?.volume
            : undefined)
    ) {
      if (debounceRef.current) {
        clearTimeout(debounceRef.current)
      }
      debounceRef.current = setTimeout(() => {
        if (videoIndex >= 0 && appProxy.state.videos[videoIndex]?.config) {
          if (!appProxy.state.videos[videoIndex].config.audioConfig) {
            appProxy.state.videos[videoIndex].config.audioConfig = {
              volume: 100,
            }
          }
          appProxy.state.videos[videoIndex].config.audioConfig.volume = volume
          appProxy.state.videos[videoIndex].isConfigDirty = true
        } else {
          if (appProxy.state.videos.length > 1) {
            if (!appProxy.state.commonConfigForBatchCompression.audioConfig) {
              appProxy.state.commonConfigForBatchCompression.audioConfig = {
                volume: 100,
              }
            }
            appProxy.state.commonConfigForBatchCompression.audioConfig.volume =
              volume
            normalizeBatchVideosConfig()
          }
        }
      }, 500)
    }
    return () => {
      clearTimeout(debounceRef.current)
    }
  }, [volume, videoIndex])

  React.useEffect(() => {
    if (audioConfig?.volume !== volumeRef.current) {
      if (
        typeof audioConfig?.volume === 'number' &&
        !Number.isNaN(+audioConfig.volume)
      ) {
        setVolume(audioConfig.volume)
      }
    }
  }, [audioConfig?.volume])

  const handleVolumeChange = useCallback((value: number | number[]) => {
    if (typeof value === 'number') {
      setVolume(value)
    }
  }, [])

  const hasNoAudio = videoInfoRaw?.audioStreams?.length === 0
  const shouldDisableInput =
    videos.length === 0 ||
    isCompressing ||
    isProcessCompleted ||
    isLoadingFiles ||
    hasNoAudio

  return (
    <>
      <Slider
        label="Volume:"
        aria-label="Audio Volume"
        marks={[
          {
            value: 0,
            label: 'Mute',
          },
          {
            value: 50,
            label: '50%',
          },
          {
            value: 100,
            label: 'Full',
          },
        ]}
        classNames={{
          mark: 'text-[11px] mt-3',
          label: 'text-sm text-gray-600 dark:text-gray-400',
        }}
        getValue={(value) => {
          const val = Array.isArray(value) ? value?.[0] : +value
          return `${Math.round(val)}%`
        }}
        renderValue={(props) => (
          <p className="text-primary text-xs font-bold">{props?.children}</p>
        )}
        value={volume}
        onChange={handleVolumeChange}
        isDisabled={shouldDisableInput}
      />
    </>
  )
}

export default AudioVolume
