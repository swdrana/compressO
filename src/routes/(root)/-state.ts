import cloneDeep from 'lodash/cloneDeep'
import { proxy } from 'valtio'

import { App, VideoConfig } from '../../types/app'

export const videoConfigInitialState: VideoConfig = {
  convertToExtension: 'mp4',
  presetName: 'ironclad',
  shouldDisableCompression: false,
  shouldMuteVideo: false,
  quality: 50,
  shouldEnableQuality: false,
}

const appInitialState: App = {
  videos: [],
  isLoadingFiles: false,
  totalSelectedFilesCount: 0,
  currentVideoIndex: 0,
  totalDurationMs: 0,
  isCompressing: false,
  totalProgress: 0,
  isProcessCompleted: false,
  isBatchCompressionCancelled: false,
  isSaving: false,
  isSaved: false,
}

const snapshotMoment = {
  beforeCompressionStarted: 'beforeCompressionStarted',
  batchCompressionStep: 'batchCompressionStep',
} as const

type SnapshotMoment = keyof typeof snapshotMoment

type AppProxy = {
  state: App
  snapshots: Record<SnapshotMoment, App>
  takeSnapshot: (moment: SnapshotMoment) => void
  timeTravel: (to: SnapshotMoment) => void
  removeSnapshot: (moment: SnapshotMoment) => void
  resetProxy: () => void
}

const snapshotsInitialState = {
  [snapshotMoment.beforeCompressionStarted]: cloneDeep(appInitialState),
  [snapshotMoment.batchCompressionStep]: cloneDeep(appInitialState),
}

export const appProxy: AppProxy = proxy({
  state: appInitialState,
  snapshots: snapshotsInitialState,
  takeSnapshot(moment: SnapshotMoment) {
    if (moment in snapshotMoment) {
      appProxy.snapshots[moment] = cloneDeep(appProxy.state)
    }
  },
  timeTravel(to: SnapshotMoment) {
    if (to in snapshotMoment) {
      appProxy.state = cloneDeep(appProxy.snapshots[to])
    }
  },
  removeSnapshot(moment: SnapshotMoment) {
    if (moment in snapshotMoment) {
      delete appProxy.snapshots[moment]
    }
  },
  resetProxy() {
    appProxy.state = cloneDeep(appInitialState)
    appProxy.snapshots = cloneDeep(snapshotsInitialState)
  },
})
