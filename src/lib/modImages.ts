import { convertFileSrc } from "@tauri-apps/api/core"

export function getModImageSrc(imageUrl?: string) {
  if (!imageUrl) {
    return undefined
  }

  if (/^(data:|https?:|blob:|asset:)/i.test(imageUrl)) {
    return imageUrl
  }

  return convertFileSrc(imageUrl)
}
