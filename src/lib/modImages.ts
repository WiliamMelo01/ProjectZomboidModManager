import { convertFileSrc } from "@tauri-apps/api/core"

export function getModImageSrc(imageUrl?: string) {
  if (!imageUrl) {
    return undefined
  }

  if (/^(data:|https?:|blob:|asset:)/i.test(imageUrl)) {
    return imageUrl
  }

  return convertFileSrc(normalizeFileImagePath(imageUrl))
}

function normalizeFileImagePath(imageUrl: string) {
  const trimmedUrl = imageUrl.trim()
  const fileUrlMatch = trimmedUrl.match(/^file:\/\/\/?(.*)$/i)
  const filePath = fileUrlMatch ? decodeURIComponent(fileUrlMatch[1]) : trimmedUrl

  return filePath
    .replace(/^\\\\\?\\/, "")
    .replace(/\\/g, "/")
}
