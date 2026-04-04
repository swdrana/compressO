import { invoke } from '@tauri-apps/api/core'

import { getPlatform } from './fs'

/**
 * Get the video server URL (Linux only)
 * On other platforms, this returns null
 */
export async function getServerUrl(): Promise<string | null> {
  // TODO: Add back
  // const platform = getPlatform()

  // // Only use the server on Linux
  // if (!platform.isLinux) {
  //   return null
  // }

  // Return cached URL if available
  if (window.__serverUrl) {
    return window.__serverUrl
  }

  try {
    // Call the Tauri command to get the server URL
    const serverUrl = await invoke<string>('get_video_server_url')
    window.__serverUrl = serverUrl
    return serverUrl
  } catch (error) {
    // biome-ignore lint/suspicious/noConsole: <>
    console.error('Failed to get video server URL:', error)
    return null
  }
}

/**
 * Converts a file path to a video URL that works on the current platform
 *
 * On Linux: Returns HTTP server URL (http://127.0.0.1:PORT/video?path=...)
 * On macOS/Windows: Returns the path as-is (or converts to file:// if needed)
 *
 * @param filePath - The absolute path to the video file
 * @returns Promise resolving to the video URL
 */
export async function getVideoUrl(filePath: string): Promise<string> {
  const platform = getPlatform()

  // TODO: Remove for MacOS
  if (platform.isLinux || platform.isMacOS) {
    // Use the local HTTP server on Linux
    const serverUrl = await getServerUrl()

    if (!serverUrl) {
      // Fallback: if server isn't available, try using the path directly
      // This won't work due to the WebKit bug, but it's better than nothing
      // biome-ignore lint/suspicious/noConsole: <>
      console.warn('Video server not available, file may not play on Linux')
      return filePath
    }

    // Use the Tauri command to get the complete video URL
    try {
      return await invoke<string>('get_video_url', { filePath })
    } catch (error) {
      // biome-ignore lint/suspicious/noConsole: <>
      console.error('Failed to generate video URL:', error)
      // Fallback to constructing the URL manually
      const encodedPath = encodeURIComponent(filePath)
      return `${serverUrl}/video?path=${encodedPath}`
    }
  }

  // On macOS and Windows, return the path as-is
  // ReactPlayer and the browser will handle it correctly
  return filePath
}
