import { useDisclosure } from '@heroui/modal'
import { Code, UseDisclosureProps } from '@heroui/react'
import React from 'react'
import { snapshot, useSnapshot } from 'valtio'

import Button from '@/components/Button'
import Icon from '@/components/Icon'
import Tooltip from '@/components/Tooltip'
import AlertDialog, { AlertDialogButton } from '@/ui/Dialogs/AlertDialog'
import { appProxy } from '../-state'

// TODO: Rename component to ExitProcess
function FileName() {
  const {
    state: { videos, isProcessCompleted },
    resetProxy,
  } = useSnapshot(appProxy)
  const video = videos.length > 0 ? videos[0] : null
  const { fileName, compressedVideo } = video ?? {}

  const alertDiscloser = useDisclosure()

  const handleDiscard = async ({
    closeModal,
  }: {
    closeModal: UseDisclosureProps['onClose']
  }) => {
    try {
      // await Promise.allSettled([
      //   deleteFile(compressedVideo?.pathRaw as string),
      //   deleteFile(thumbnailPathRaw as string),
      // ])
      closeModal?.()
      resetProxy()
    } catch {
      //
    }
  }

  const handleCancelCompression = () => {
    const appSnapshot = snapshot(appProxy)
    if (appSnapshot.state.isProcessCompleted && !appSnapshot.state.isSaved) {
      alertDiscloser.onOpen()
    } else {
      resetProxy()
    }
  }

  const handleReconfigure = () => {
    appProxy.timeTravel('beforeCompressionStarted')
  }

  const fileNameDisplay =
    (isProcessCompleted ? compressedVideo?.fileNameToDisplay : fileName) ?? ''

  return videos.length ? (
    <>
      <div className="mx-auto w-fit flex justify-center items-center mb-2 gap-1">
        <Code
          size="sm"
          className="ml-auto mr-auto text-center rounded-xl px-4 text-xs xl:text-sm"
        >
          {fileNameDisplay?.length > 50
            ? `${fileNameDisplay?.slice(0, 20)}...${fileNameDisplay?.slice(
                -10,
              )}`
            : fileNameDisplay}
        </Code>
        {isProcessCompleted ? (
          <Button
            isIconOnly
            size="sm"
            onPress={handleReconfigure}
            className="bg-transparent"
          >
            <Tooltip content="Reconfigure" aria-label="Reconfigure">
              <Icon name="redo" size={22} />
            </Tooltip>
          </Button>
        ) : null}
        <Tooltip content="Cancel compression" aria-label="Cancel compression">
          <Button
            isIconOnly
            size="sm"
            onPress={handleCancelCompression}
            className="bg-transparent"
          >
            <Icon name="cross" size={22} />
          </Button>
        </Tooltip>
      </div>
      <AlertDialog
        title="Video not saved!"
        discloser={alertDiscloser}
        description="Your compressed video is not yet saved. Are you sure you want to discard it?"
        renderFooter={({ closeModal }) => (
          <>
            <AlertDialogButton onPress={closeModal}>Go Back</AlertDialogButton>
            <AlertDialogButton
              color="danger"
              onPress={() => handleDiscard({ closeModal })}
            >
              Yes
            </AlertDialogButton>
          </>
        )}
      />
    </>
  ) : null
}

export default React.memo(FileName)
