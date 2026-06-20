import { listen } from "@tauri-apps/api/event"
import { useEffect, useMemo, useRef, useState } from "react"

import { i18n } from "@/i18n"
import type { RemoteConnectionDraft } from "@/lib/commandRunner"
import { formatDuration } from "@/lib/serverTest"
import { invokeTauri } from "@/lib/tauri"
import type {
  DownloadItemStatus,
  DownloadListItem,
  DownloadType,
  WorkshopDownloadEvent,
  WorkshopDownloadLogEvent,
  WorkshopDownloadResult,
  WorkshopDownloadStatus,
} from "@/types/download"

type DownloadNotification = {
  title: string
  message: string
  tone: "success" | "warning" | "error"
  action: {
    type: "download-result"
    result: WorkshopDownloadResult
  }
}

type UseWorkshopDownloadManagerOptions = {
  isDownloadScreenActive: boolean
  remoteConnection?: RemoteConnectionDraft | null
  onDownloadFinished?: () => Promise<unknown>
  onNotification?: (notification: DownloadNotification) => void
}

type StartDownloadOptions = {
  downloadType: DownloadType
  workshopId: string
  forceValidate: boolean
}

export function useWorkshopDownloadManager({
  isDownloadScreenActive,
  remoteConnection = null,
  onDownloadFinished,
  onNotification,
}: UseWorkshopDownloadManagerOptions) {
  const [isDownloading, setIsDownloading] = useState(false)
  const [status, setStatus] = useState<WorkshopDownloadStatus | null>(null)
  const [downloadItems, setDownloadItems] = useState<DownloadListItem[]>([])
  const [steamCmdLogLines, setSteamCmdLogLines] = useState<WorkshopDownloadLogEvent[]>([])
  const [result, setResult] = useState<WorkshopDownloadResult | null>(null)
  const [isResultModalOpen, setIsResultModalOpen] = useState(false)
  const [forceValidate, setForceValidate] = useState(false)
  const [downloadStartedAt, setDownloadStartedAt] = useState<number | null>(null)
  const [elapsedSeconds, setElapsedSeconds] = useState(0)
  const isDownloadScreenActiveRef = useRef(isDownloadScreenActive)
  const remoteConnectionRef = useRef(remoteConnection)
  const onDownloadFinishedRef = useRef(onDownloadFinished)
  const onNotificationRef = useRef(onNotification)

  useEffect(() => {
    isDownloadScreenActiveRef.current = isDownloadScreenActive
  }, [isDownloadScreenActive])

  useEffect(() => {
    remoteConnectionRef.current = remoteConnection
  }, [remoteConnection])

  useEffect(() => {
    onDownloadFinishedRef.current = onDownloadFinished
  }, [onDownloadFinished])

  useEffect(() => {
    onNotificationRef.current = onNotification
  }, [onNotification])

  useEffect(() => {
    let dispose: (() => void) | null = null
    let disposeLog: (() => void) | null = null

    void listen<WorkshopDownloadEvent>("workshop-download-event", ({ payload }) => {
      setDownloadItems((items) => {
        const existing = items.findIndex((item) => item.workshopId === payload.workshopId)

        if (existing < 0) {
          return [...items, payload]
        }

        return items.map((item, index) => {
          if (index !== existing || !shouldApplyDownloadEvent(item.status, payload.status)) {
            return item
          }

          return { ...item, ...payload }
        })
      })
    }).then((unlisten) => {
      dispose = unlisten
    })

    void listen<WorkshopDownloadLogEvent>("workshop-download-log", ({ payload }) => {
      setSteamCmdLogLines((lines) => {
        const lastLine = lines.at(-1)

        if (lastLine?.instanceId === payload.instanceId && lastLine.line === payload.line) {
          return lines
        }

        return [...lines, payload].slice(-320)
      })
    }).then((unlisten) => {
      disposeLog = unlisten
    })

    return () => {
      dispose?.()
      disposeLog?.()
    }
  }, [])

  useEffect(() => {
    if (!isDownloading || downloadStartedAt === null) {
      return
    }

    const updateElapsed = () => {
      setElapsedSeconds(Math.max(0, Math.floor((Date.now() - downloadStartedAt) / 1000)))
    }

    updateElapsed()
    const interval = window.setInterval(updateElapsed, 1000)

    return () => window.clearInterval(interval)
  }, [downloadStartedAt, isDownloading])

  const progress = useMemo(() => {
    const totalItems = downloadItems.length
    const completedItems = downloadItems.filter((item) => item.status === "completed").length
    const skippedItems = downloadItems.filter((item) => item.status === "skipped").length
    const failedItems = downloadItems.filter((item) => item.status === "failed").length
    const queuedItems = downloadItems.filter((item) => item.status === "queued").length
    const percentage = totalItems > 0 ? Math.round(((completedItems + skippedItems) / totalItems) * 100) : 0

    return {
      totalItems,
      completedItems,
      skippedItems,
      failedItems,
      queuedItems,
      percentage,
      isPreparing: isDownloading && totalItems === 0,
    }
  }, [downloadItems, isDownloading])

  function startDownloadTimer() {
    setElapsedSeconds(0)
    setDownloadStartedAt(Date.now())
  }

  function stopDownloadTimer() {
    setDownloadStartedAt((startedAt) => {
      if (startedAt !== null) {
        setElapsedSeconds(Math.max(0, Math.floor((Date.now() - startedAt) / 1000)))
      }

      return null
    })
  }

  async function finishDownload(downloadResult: WorkshopDownloadResult) {
    stopDownloadTimer()
    setResult(downloadResult)
    setIsResultModalOpen(
      isDownloadScreenActiveRef.current &&
      (downloadResult.failedItems.length > 0 || downloadResult.wasCancelled),
    )

    if (downloadResult.wasCancelled) {
      setStatus({ type: "error", message: i18n.t("downloads.cancelledProgress", { count: downloadResult.downloadedItems, skipped: downloadResult.skippedItems }) })
    } else if (downloadResult.failedItems.length > 0) {
      setStatus({
        type: "error",
        message: i18n.t("downloads.progress", { downloaded: downloadResult.downloadedItems, skipped: downloadResult.skippedItems, total: downloadResult.totalItems, failed: downloadResult.failedItems.length }),
      })
    } else {
      setStatus({ type: "success", message: i18n.t("downloads.success", { downloaded: downloadResult.downloadedItems, skipped: downloadResult.skippedItems }) })
    }

    await onDownloadFinishedRef.current?.()
    onNotificationRef.current?.(buildDownloadNotification(downloadResult))
  }

  async function startDownload({ downloadType, workshopId, forceValidate: shouldValidate }: StartDownloadOptions) {
    if (isDownloading) {
      return
    }

    setDownloadItems([])
    setSteamCmdLogLines([])
    setResult(null)
    setIsResultModalOpen(false)
    startDownloadTimer()
    setIsDownloading(true)
    setForceValidate(shouldValidate)
    setStatus({
      type: "info",
      message: downloadType === "collection" ? i18n.t("downloads.checkingCollection", { id: workshopId }) : i18n.t("downloads.downloadingItem", { id: workshopId }),
    })

    try {
      const connection = remoteConnectionRef.current
      const downloadResult = downloadType === "collection"
        ? await invokeTauri<WorkshopDownloadResult>(connection ? "download_remote_steam_workshop_collection" : "download_steam_workshop_collection", {
            ...(connection ? { connection } : {}),
            collectionId: workshopId,
            forceValidate: shouldValidate,
          })
        : await invokeTauri<WorkshopDownloadResult>(connection ? "download_remote_steam_workshop_item" : "download_steam_workshop_item", {
            ...(connection ? { connection } : {}),
            workshopId,
            forceValidate: shouldValidate,
          })

      await finishDownload(downloadResult)
    } catch (error) {
      stopDownloadTimer()
      setStatus({ type: "error", message: getErrorMessage(error) })
    } finally {
      setIsDownloading(false)
    }
  }

  async function retryFailedItems() {
    const workshopIds = result?.failedItems.map((item) => item.workshopId) ?? []

    if (workshopIds.length === 0 || isDownloading) {
      return
    }

    setResult(null)
    setIsResultModalOpen(false)
    setDownloadItems([])
    setSteamCmdLogLines([])
    startDownloadTimer()
    setIsDownloading(true)
    setStatus({ type: "info", message: i18n.t("downloads.retrying", { count: workshopIds.length }) })

    try {
      const connection = remoteConnectionRef.current
      const downloadResult = await invokeTauri<WorkshopDownloadResult>(connection ? "download_remote_steam_workshop_items" : "download_steam_workshop_items", {
        ...(connection ? { connection } : {}),
        workshopIds,
        forceValidate,
      })
      await finishDownload(downloadResult)
    } catch (error) {
      stopDownloadTimer()
      setStatus({ type: "error", message: getErrorMessage(error) })
    } finally {
      setIsDownloading(false)
    }
  }

  async function cancelDownload() {
    setStatus({ type: "info", message: i18n.t("downloads.cancelling") })

    try {
      const connection = remoteConnectionRef.current
      await invokeTauri<void>(connection ? "cancel_remote_steam_workshop_download" : "cancel_steam_workshop_download", connection ? { connection } : undefined)
    } catch (error) {
      setStatus({ type: "error", message: getErrorMessage(error) })
    }
  }

  function openResultDetails(downloadResult = result) {
    if (!downloadResult) {
      return
    }

    setResult(downloadResult)
    setIsResultModalOpen(downloadResult.failedItems.length > 0 || downloadResult.wasCancelled)
    setStatus(
      downloadResult.wasCancelled
        ? { type: "error", message: i18n.t("downloads.cancelledProgress", { count: downloadResult.downloadedItems, skipped: downloadResult.skippedItems }) }
        : downloadResult.failedItems.length > 0
          ? { type: "error", message: i18n.t("downloads.progress", { downloaded: downloadResult.downloadedItems, skipped: downloadResult.skippedItems, total: downloadResult.totalItems, failed: downloadResult.failedItems.length }) }
          : { type: "success", message: i18n.t("downloads.success", { downloaded: downloadResult.downloadedItems, skipped: downloadResult.skippedItems }) },
    )
  }

  return {
    isDownloading,
    status,
    downloadItems,
    steamCmdLogLines,
    result,
    isResultModalOpen,
    progress,
    elapsedSeconds,
    elapsedLabel: formatDuration(elapsedSeconds),
    startDownload,
    retryFailedItems,
    cancelDownload,
    openResultDetails,
    closeResultModal: () => setIsResultModalOpen(false),
  }
}

export type WorkshopDownloadManager = ReturnType<typeof useWorkshopDownloadManager>

function buildDownloadNotification(result: WorkshopDownloadResult): DownloadNotification {
  const failedCount = result.failedItems.length
  const title = result.wasCancelled
    ? i18n.t("downloads.cancelledTitle")
    : failedCount > 0
      ? i18n.t("downloads.failedTitle")
      : i18n.t("downloads.finishedTitle")
  const message = result.wasCancelled
    ? i18n.t("downloads.cancelled", { downloaded: result.downloadedItems, skipped: result.skippedItems, cancelled: result.cancelledItems })
    : failedCount > 0
      ? i18n.t("downloads.progress", { downloaded: result.downloadedItems, skipped: result.skippedItems, total: result.totalItems, failed: failedCount })
      : i18n.t("downloads.success", { downloaded: result.downloadedItems, skipped: result.skippedItems })

  return {
    title,
    message,
    tone: result.wasCancelled
      ? "warning"
      : failedCount > 0
        ? result.downloadedItems > 0
          ? "warning"
          : "error"
        : "success",
    action: { type: "download-result", result },
  }
}

function shouldApplyDownloadEvent(currentStatus: DownloadItemStatus, nextStatus: DownloadItemStatus) {
  if (currentStatus === nextStatus) {
    return true
  }

  if (currentStatus === "completed" || currentStatus === "skipped") {
    return false
  }

  if (currentStatus === "cancelled") {
    return nextStatus === "completed"
  }

  if (currentStatus === "failed") {
    return nextStatus === "retrying" || nextStatus === "downloading" || nextStatus === "completed"
  }

  if (nextStatus === "queued") {
    return false
  }

  return true
}

function getErrorMessage(error: unknown) {
  if (error instanceof Error) return error.message
  if (typeof error === "string") return error
  return i18n.t("downloads.fallbackError")
}
