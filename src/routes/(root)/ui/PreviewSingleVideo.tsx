import { useSnapshot } from 'valtio'

import Divider from '@/components/Divider'
import { cn } from '@/utils/tailwind'
import { appProxy } from '../-state'
import VideoTransformer from './compression-options/VideoTransformer'
import styles from './styles.module.css'
import VideoThumbnail from './VideoThumbnail'

function PreviewSingleVideo() {
  const {
    state: { videos },
  } = useSnapshot(appProxy)
  const video = videos.length > 0 ? videos[0] : null
  const {
    config,
    size: videoSize,
    videDurationRaw,
    dimensions,
    fps,
    extension: videoExtension,
  } = video ?? {}
  const { shouldTransformVideo } = config ?? {}

  return videos.length === 1 ? (
    <>
      {shouldTransformVideo ? <VideoTransformer /> : <VideoThumbnail />}
      <section className={cn(['my-4', styles.videoMetadata])}>
        <>
          <div>
            <p className="italic text-gray-600 dark:text-gray-400">Size</p>
            <span className="block font-black">{videoSize}</span>
          </div>
          <Divider orientation="vertical" className="h-10" />
        </>
        <>
          <div>
            <p className="italic text-gray-600 dark:text-gray-400">Extension</p>
            <span className="block font-black">{videoExtension ?? '-'}</span>
          </div>
          <Divider orientation="vertical" className="h-10" />
        </>

        <>
          <div>
            <p className="italic text-gray-600 dark:text-gray-400">Duration</p>
            <span className="block font-black">{videDurationRaw ?? '-'}</span>
          </div>
        </>
        <>
          {dimensions ? (
            <>
              <Divider orientation="vertical" className="h-10" />{' '}
              <div>
                <p className="italic text-gray-600 dark:text-gray-400">
                  Dimensions
                </p>
                <span className="block font-black">
                  {dimensions.width ?? '-'} x {dimensions.height ?? '-'}
                </span>
              </div>
            </>
          ) : null}
        </>
        <>
          {fps ? (
            <>
              <Divider orientation="vertical" className="h-10" />{' '}
              <div>
                <p className="italic text-gray-600 dark:text-gray-400">FPS</p>
                <span className="block font-black">{fps ?? '-'}</span>
              </div>
            </>
          ) : null}
        </>
      </section>
    </>
  ) : null
}

export default PreviewSingleVideo
