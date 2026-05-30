import { listen } from "@tauri-apps/api/event"
import { useEffect, useMemo, useRef, useState } from "react"

import { invokeTauri } from "@/lib/tauri"
import type {
  DownloadListItem,
  DownloadType,
  WorkshopDownloadEvent,
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
  onDownloadFinished,
  onNotification,
}: UseWorkshopDownloadManagerOptions) {
  const [isDownloading, setIsDownloading] = useState(false)
  const [status, setStatus] = useState<WorkshopDownloadStatus | null>(null)
  const [downloadItems, setDownloadItems] = useState<DownloadListItem[]>([])
  const [result, setResult] = useState<WorkshopDownloadResult | null>(null)
  const [isResultModalOpen, setIsResultModalOpen] = useState(false)
  const [forceValidate, setForceValidate] = useState(false)
  const isDownloadScreenActiveRef = useRef(isDownloadScreenActive)
  const onDownloadFinishedRef = useRef(onDownloadFinished)
  const onNotificationRef = useRef(onNotification)

  useEffect(() => {
    isDownloadScreenActiveRef.current = isDownloadScreenActive
  }, [isDownloadScreenActive])

  useEffect(() => {
    onDownloadFinishedRef.current = onDownloadFinished
  }, [onDownloadFinished])

  useEffect(() => {
    onNotificationRef.current = onNotification
  }, [onNotification])

  useEffect(() => {
    let dispose: (() => void) | null = null

    void listen<WorkshopDownloadEvent>("workshop-download-event", ({ payload }) => {
      setDownloadItems((items) => {
        const existing = items.findIndex((item) => item.workshopId === payload.workshopId)

        if (existing < 0) {
          return [...items, payload]
        }

        return items.map((item, index) => index === existing ? { ...item, ...payload } : item)
      })
    }).then((unlisten) => {
      dispose = unlisten
    })

    return () => dispose?.()
  }, [])

  const progress = useMemo(() => {
    const totalItems = downloadItems.length
    const completedItems = downloadItems.filter((item) => item.status === "completed").length
    const failedItems = downloadItems.filter((item) => item.status === "failed").length
    const queuedItems = downloadItems.filter((item) => item.status === "queued").length
    const percentage = totalItems > 0 ? Math.round((completedItems / totalItems) * 100) : 0

    return {
      totalItems,
      completedItems,
      failedItems,
      queuedItems,
      percentage,
      isPreparing: isDownloading && totalItems === 0,
    }
  }, [downloadItems, isDownloading])

  async function finishDownload(downloadResult: WorkshopDownloadResult) {
    setResult(downloadResult)
    setIsResultModalOpen(
      isDownloadScreenActiveRef.current &&
      (downloadResult.failedItems.length > 0 || downloadResult.wasCancelled),
    )

    if (downloadResult.wasCancelled) {
      setStatus({ type: "error", message: `Download cancelado. ${downloadResult.downloadedItems} itens foram concluídos.` })
    } else if (downloadResult.failedItems.length > 0) {
      setStatus({
        type: "error",
        message: `${downloadResult.downloadedItems} de ${downloadResult.totalItems} itens foram baixados. ${downloadResult.failedItems.length} falharam.`,
      })
    } else {
      setStatus({ type: "success", message: `${downloadResult.downloadedItems} itens baixados com sucesso.` })
    }

    await onDownloadFinishedRef.current?.()
    onNotificationRef.current?.(buildDownloadNotification(downloadResult))
  }

  async function startDownload({ downloadType, workshopId, forceValidate: shouldValidate }: StartDownloadOptions) {
    if (isDownloading) {
      return
    }

    setDownloadItems([])
    setResult(null)
    setIsResultModalOpen(false)
    setIsDownloading(true)
    setForceValidate(shouldValidate)
    setStatus({
      type: "info",
      message: downloadType === "collection" ? `Consultando a coleção ${workshopId}...` : `Baixando item ${workshopId}...`,
    })

    try {
      const downloadResult = downloadType === "collection"
        ? await invokeTauri<WorkshopDownloadResult>("download_steam_workshop_collection", {
            collectionId: workshopId,
            forceValidate: shouldValidate,
          })
        : await invokeTauri<WorkshopDownloadResult>("download_steam_workshop_item", {
            workshopId,
            forceValidate: shouldValidate,
          })

      await finishDownload(downloadResult)
    } catch (error) {
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
    setIsDownloading(true)
    setStatus({ type: "info", message: `Tentando baixar novamente ${workshopIds.length} itens...` })

    try {
      const downloadResult = await invokeTauri<WorkshopDownloadResult>("download_steam_workshop_items", {
        workshopIds,
        forceValidate,
      })
      await finishDownload(downloadResult)
    } catch (error) {
      setStatus({ type: "error", message: getErrorMessage(error) })
    } finally {
      setIsDownloading(false)
    }
  }

  async function cancelDownload() {
    setStatus({ type: "info", message: "Cancelando download..." })

    try {
      await invokeTauri<void>("cancel_steam_workshop_download")
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
        ? { type: "error", message: `Download cancelado. ${downloadResult.downloadedItems} itens foram concluídos.` }
        : downloadResult.failedItems.length > 0
          ? { type: "error", message: `${downloadResult.downloadedItems} de ${downloadResult.totalItems} itens foram baixados. ${downloadResult.failedItems.length} falharam.` }
          : { type: "success", message: `${downloadResult.downloadedItems} itens baixados com sucesso.` },
    )
  }

  return {
    isDownloading,
    status,
    downloadItems,
    result,
    isResultModalOpen,
    progress,
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
    ? "Download da Workshop cancelado"
    : failedCount > 0
      ? "Download concluído com falhas"
      : "Download da Workshop finalizado"
  const message = result.wasCancelled
    ? `${result.downloadedItems} baixados e ${result.cancelledItems} cancelados.`
    : `${result.downloadedItems} de ${result.totalItems} itens baixados${failedCount > 0 ? `; ${failedCount} falharam.` : "."}`

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

function getErrorMessage(error: unknown) {
  if (error instanceof Error) return error.message
  if (typeof error === "string") return error
  return "Não foi possível baixar o mod."
}
