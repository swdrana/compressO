import { save } from '@tauri-apps/plugin-dialog'
import React from 'react'
import { snapshot, useSnapshot } from 'valtio'

import Button from '@/components/Button'
import Icon from '@/components/Icon'
import { toast } from '@/components/Toast'
import Tooltip from '@/components/Tooltip'
import { moveFile, showItemInFileManager } from '@/tauri/commands/fs'
import { appProxy } from '../-state'

function SaveVideo() {
  const {
    state: { videos, isProcessCompleted },
  } = useSnapshot(appProxy)
  const video = videos.length > 0 ? videos[0] : null
  const { compressedVideo, fileName } = video ?? {}

  const fileNameDisplay =
    (isProcessCompleted ? compressedVideo?.fileNameToDisplay : fileName) ?? ''

  const handleCompressedVideoSave = async () => {
    if (appProxy.state.videos.length) {
      try {
        const pathToSave = await save({
          title: 'Choose location to save the compressed video(s).',
          defaultPath: `compressO-${fileNameDisplay}`,
        })
        if (pathToSave) {
          appProxy.state.videos[0].compressedVideo = {
            ...(snapshot(appProxy).state.videos[0].compressedVideo ?? {}),
            isSaving: true,
            isSaved: false,
          }
          await moveFile(compressedVideo?.pathRaw as string, pathToSave)
          appProxy.state.videos[0].compressedVideo = {
            ...(snapshot(appProxy).state.videos[0].compressedVideo ?? {}),
            savedPath: pathToSave,
            isSaving: false,
            isSaved: true,
          }
        }
      } catch (_) {
        toast.error('Could not save video to the given path.')
        appProxy.state.videos[0].compressedVideo = {
          ...(snapshot(appProxy).state.videos[0].compressedVideo ?? {}),
          isSaving: false,
          isSaved: false,
        }
      }
    }
  }

  const openInFileManager = async () => {
    if (!compressedVideo?.savedPath) return
    try {
      await showItemInFileManager(compressedVideo?.savedPath)
    } catch {
      //
    }
  }

  return (
    <div className="flex items-center">
      <Button
        className="flex justify-center items-center"
        color="success"
        onPress={handleCompressedVideoSave}
        isLoading={compressedVideo?.isSaving}
        isDisabled={compressedVideo?.isSaving || compressedVideo?.isSaved}
        fullWidth
        size="lg"
      >
        {compressedVideo?.isSaved ? 'Saved' : 'Save Video'}
        <Icon
          name={compressedVideo?.isSaved ? 'tick' : 'save'}
          className="text-green-300"
        />
      </Button>
      {compressedVideo?.isSaved && compressedVideo?.savedPath ? (
        <Tooltip
          content="Show in File Explorer"
          aria-label="Show in File Explorer"
        >
          <Button
            isIconOnly
            className="ml-2 text-green-500"
            onPress={openInFileManager}
          >
            <Icon name="fileExplorer" />
          </Button>
        </Tooltip>
      ) : null}
    </div>
  )
}

export default React.memo(SaveVideo)
