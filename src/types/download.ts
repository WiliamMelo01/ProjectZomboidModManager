export type DownloadType = "item" | "collection"

export type DownloadItemStatus = "queued" | "downloading" | "completed" | "retrying" | "failed" | "cancelled"

export type WorkshopDownloadFailedItem = {
  workshopId: string
  name: string
  error: string
}

export type WorkshopDownloadResult = {
  totalItems: number
  downloadedItems: number
  failedItems: WorkshopDownloadFailedItem[]
  cancelledItems: number
  wasCancelled: boolean
}

export type WorkshopDownloadEvent = {
  workshopId: string
  name: string
  status: DownloadItemStatus
  error?: string | null
}

export type DownloadListItem = WorkshopDownloadEvent

export type WorkshopDownloadStatus = {
  type: "success" | "error" | "info"
  message: string
}
