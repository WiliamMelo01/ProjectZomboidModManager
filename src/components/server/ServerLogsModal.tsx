import { AlertCircle, ArrowDown, Check, ChevronDown, Copy, FileText, RefreshCw, Search, X } from "lucide-react"
import { useEffect, useMemo, useRef, useState } from "react"
import { useTranslation } from "react-i18next"

import { invokeTauri } from "@/lib/tauri"
import type { RemoteConnectionDraft } from "@/lib/commandRunner"
import type { ZomboidServer } from "@/types/server"

type AvailableLogFile = {
  name: string
  path: string
  sizeBytes: number
  lastModified: number
}

type LogFilePreview = {
  serverId: string
  fileName: string
  path: string
  content: string
}

type LogCategory = "ALL" | "ERROR" | "WARN" | "LOG" | "LUA" | "BOOT"

type ServerLogsModalProps = {
  server: ZomboidServer
  remoteConnection?: RemoteConnectionDraft | null
  onClose: () => void
}

export function ServerLogsModal({
  server,
  remoteConnection,
  onClose,
}: ServerLogsModalProps) {
  const { t } = useTranslation()
  const [logFiles, setLogFiles] = useState<AvailableLogFile[]>([])
  const [selectedLogName, setSelectedLogName] = useState<string>("")
  const [logContent, setLogContent] = useState<string>("")
  const [logPath, setLogPath] = useState<string>("")
  const [isLoadingList, setIsLoadingList] = useState(true)
  const [isLoadingContent, setIsLoadingContent] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [search, setSearch] = useState("")
  const [selectedCategory, setSelectedCategory] = useState<LogCategory>("ALL")
  const [isCopied, setIsCopied] = useState(false)
  const logContainerRef = useRef<HTMLDivElement>(null)

  // 1. Carrega lista de arquivos de log disponíveis
  const loadLogFilesList = async () => {
    setIsLoadingList(true)
    setError(null)
    try {
      if (remoteConnection) {
        setLogFiles([
          { name: "DebugLog-server.txt", path: "/home/ubuntu/Zomboid/Logs/DebugLog-server.txt", sizeBytes: 0, lastModified: Date.now() / 1000 },
          { name: "coop-console.txt", path: "/home/ubuntu/Zomboid/Logs/coop-console.txt", sizeBytes: 0, lastModified: Date.now() / 1000 },
        ])
        setSelectedLogName("DebugLog-server.txt")
      } else {
        const list = await invokeTauri<AvailableLogFile[]>("list_zomboid_server_logs", {
          serverId: server.id,
        })
        setLogFiles(list || [])
        if (list && list.length > 0) {
          setSelectedLogName(list[0].name)
        }
      }
    } catch (err) {
      console.error("Erro ao listar logs:", err)
      setError("Não foi possível carregar a lista de arquivos de log.")
    } finally {
      setIsLoadingList(false)
    }
  }

  // 2. Carrega o conteúdo do log selecionado
  const loadLogContent = async (logName: string) => {
    if (!logName) return
    setIsLoadingContent(true)
    setError(null)
    try {
      if (remoteConnection) {
        const preview = await invokeTauri<LogFilePreview>("read_remote_zomboid_server_file", {
          connection: remoteConnection,
          serverId: server.id,
        })
        setLogContent(preview.content || "")
        setLogPath(preview.path || "")
      } else {
        const preview = await invokeTauri<LogFilePreview>("read_zomboid_server_log_file", {
          serverId: server.id,
          logName,
        })
        setLogContent(preview.content || "")
        setLogPath(preview.path || "")
      }
    } catch (err) {
      console.error("Erro ao ler log:", err)
      setError("Não foi possível ler o arquivo de log selecionado.")
    } finally {
      setIsLoadingContent(false)
    }
  }

  useEffect(() => {
    void loadLogFilesList()
  }, [server.id])

  useEffect(() => {
    if (selectedLogName) {
      void loadLogContent(selectedLogName)
    }
  }, [selectedLogName])

  // Contagem e filtragem de categorias
  const lines = useMemo(() => (logContent ?? "").split("\n"), [logContent])

  const categoryCounts = useMemo(() => {
    let errorCount = 0
    let warnCount = 0
    let logCount = 0
    let luaCount = 0
    let bootCount = 0

    for (const line of lines) {
      const lower = line.toLowerCase()
      if (lower.includes("error") || lower.includes("err") || lower.includes("exception") || lower.includes("failed") || lower.includes("fatal")) {
        errorCount++
      } else if (lower.includes("warn") || lower.includes("warning")) {
        warnCount++
      } else if (lower.includes("server started") || lower.includes("listening") || lower.includes("raknet.startup")) {
        bootCount++
      } else if (lower.includes("luanet") || lower.includes("lua") || lower.includes("mod")) {
        luaCount++
      } else if (lower.includes("log") || lower.includes("info")) {
        logCount++
      }
    }

    return {
      ALL: lines.length,
      ERROR: errorCount,
      WARN: warnCount,
      LOG: logCount,
      LUA: luaCount,
      BOOT: bootCount,
    }
  }, [lines])

  const categoryFilteredLines = useMemo(() => {
    if (selectedCategory === "ALL") return lines
    return lines.filter((line) => {
      const lower = line.toLowerCase()
      switch (selectedCategory) {
        case "ERROR":
          return lower.includes("error") || lower.includes("err") || lower.includes("exception") || lower.includes("failed") || lower.includes("fatal")
        case "WARN":
          return lower.includes("warn") || lower.includes("warning")
        case "LOG":
          return lower.includes("log") || lower.includes("info")
        case "LUA":
          return lower.includes("luanet") || lower.includes("lua") || lower.includes("mod")
        case "BOOT":
          return lower.includes("server started") || lower.includes("listening") || lower.includes("raknet.startup")
        default:
          return true
      }
    })
  }, [lines, selectedCategory])

  const filteredLines = useMemo(() => {
    const q = search.trim().toLowerCase()
    if (!q) return categoryFilteredLines
    return categoryFilteredLines.filter((line) => line.toLowerCase().includes(q))
  }, [categoryFilteredLines, search])

  const scrollToBottom = () => {
    if (logContainerRef.current) {
      logContainerRef.current.scrollTop = logContainerRef.current.scrollHeight
    }
  }

  const handleCopyLogs = async () => {
    try {
      await navigator.clipboard.writeText(filteredLines.join("\n"))
      setIsCopied(true)
      setTimeout(() => setIsCopied(false), 2000)
    } catch (err) {
      console.error("Falha ao copiar logs:", err)
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 p-4 backdrop-blur-md animate-in fade-in duration-200"
      onClick={onClose}
    >
      <div
        role="dialog"
        aria-modal="true"
        className="flex h-[85vh] max-h-[90vh] w-full max-w-5xl flex-col overflow-hidden rounded-3xl border border-white/10 bg-[#22272b] shadow-2xl animate-in zoom-in-95 duration-200"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex flex-wrap items-center justify-between gap-4 border-b border-white/10 bg-[#2b3238] px-6 py-4">
          <div className="flex items-center gap-3 min-w-0">
            <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-cyan-500/20 text-cyan-400 border border-cyan-500/30">
              <FileText size={20} />
            </div>
            <div className="min-w-0">
              <h3 className="truncate text-lg font-bold text-white">Logs do Servidor</h3>
              <p className="truncate font-mono text-[10px] text-gray-400">{logPath || server.name}</p>
            </div>
          </div>

          <div className="flex items-center gap-2">
            {/* Seletor de Arquivos de Log */}
            <div className="relative">
              <select
                value={selectedLogName}
                onChange={(e) => setSelectedLogName(e.target.value)}
                disabled={isLoadingList || logFiles.length === 0}
                className="appearance-none rounded-xl border border-white/10 bg-black/40 py-2 pl-3 pr-8 text-xs font-mono text-cyan-300 focus:border-cyan-500/50 focus:outline-none disabled:opacity-50 cursor-pointer"
              >
                {logFiles.length === 0 ? (
                  <option value="">Sem arquivos de log</option>
                ) : (
                  logFiles.map((file) => (
                    <option key={file.name} value={file.name} className="bg-[#22272b] text-white">
                      {file.name} ({Math.round(file.sizeBytes / 1024)} KB)
                    </option>
                  ))
                )}
              </select>
              <ChevronDown size={14} className="pointer-events-none absolute right-2.5 top-3 text-gray-400" />
            </div>

            <button
              type="button"
              onClick={() => loadLogContent(selectedLogName)}
              disabled={isLoadingContent}
              title="Atualizar Logs"
              className="rounded-xl border border-white/10 bg-[#1e2327] p-2 text-gray-300 hover:text-white transition-colors disabled:opacity-50"
            >
              <RefreshCw size={16} className={isLoadingContent ? "animate-spin text-cyan-400" : ""} />
            </button>

            <button
              type="button"
              onClick={onClose}
              className="rounded-full border border-white/5 p-2 text-gray-400 hover:bg-white/5 hover:text-white transition-colors"
            >
              <X size={18} />
            </button>
          </div>
        </div>

        {/* Category Pills Bar */}
        <div className="flex flex-wrap items-center gap-2 border-b border-white/5 bg-[#1a1e21] px-6 py-2">
          <button
            type="button"
            onClick={() => setSelectedCategory("ALL")}
            className={`rounded-lg px-2.5 py-1 text-[11px] font-bold transition-all ${
              selectedCategory === "ALL"
                ? "bg-white/20 text-white shadow-sm"
                : "text-gray-400 hover:text-white hover:bg-white/5"
            }`}
          >
            Todos ({categoryCounts.ALL})
          </button>

          <button
            type="button"
            onClick={() => setSelectedCategory("ERROR")}
            className={`rounded-lg px-2.5 py-1 text-[11px] font-bold transition-all ${
              selectedCategory === "ERROR"
                ? "bg-red-500/30 text-red-300 border border-red-500/50 shadow-sm"
                : "text-red-400/70 hover:text-red-300 hover:bg-red-500/10"
            }`}
          >
            Errors ({categoryCounts.ERROR})
          </button>

          <button
            type="button"
            onClick={() => setSelectedCategory("WARN")}
            className={`rounded-lg px-2.5 py-1 text-[11px] font-bold transition-all ${
              selectedCategory === "WARN"
                ? "bg-amber-500/30 text-amber-300 border border-amber-500/50 shadow-sm"
                : "text-amber-400/70 hover:text-amber-300 hover:bg-amber-500/10"
            }`}
          >
            Warnings ({categoryCounts.WARN})
          </button>

          <button
            type="button"
            onClick={() => setSelectedCategory("LUA")}
            className={`rounded-lg px-2.5 py-1 text-[11px] font-bold transition-all ${
              selectedCategory === "LUA"
                ? "bg-sky-500/30 text-sky-300 border border-sky-500/50 shadow-sm"
                : "text-sky-400/70 hover:text-sky-300 hover:bg-sky-500/10"
            }`}
          >
            Lua & Mods ({categoryCounts.LUA})
          </button>

          <button
            type="button"
            onClick={() => setSelectedCategory("BOOT")}
            className={`rounded-lg px-2.5 py-1 text-[11px] font-bold transition-all ${
              selectedCategory === "BOOT"
                ? "bg-green-500/30 text-green-300 border border-green-500/50 shadow-sm"
                : "text-green-400/70 hover:text-green-300 hover:bg-green-500/10"
            }`}
          >
            Boot / Servidor ({categoryCounts.BOOT})
          </button>

          <button
            type="button"
            onClick={() => setSelectedCategory("LOG")}
            className={`rounded-lg px-2.5 py-1 text-[11px] font-bold transition-all ${
              selectedCategory === "LOG"
                ? "bg-gray-500/30 text-gray-200 border border-gray-500/50 shadow-sm"
                : "text-gray-400 hover:text-gray-200 hover:bg-white/5"
            }`}
          >
            Logs / Info ({categoryCounts.LOG})
          </button>
        </div>

        {/* Sub-Header Toolbar (Filtro, Copiar, Rolar para Baixo) */}
        <div className="flex items-center justify-between gap-3 border-b border-white/5 bg-[#1e2327] px-6 py-2.5">
          <div className="relative flex-1 max-w-xs">
            <Search className="absolute left-3 top-2.5 text-gray-500" size={14} />
            <input
              type="text"
              placeholder="Filtrar por texto (ex: WARN, ERROR, mod...)"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="w-full rounded-xl border border-white/10 bg-black/40 py-1.5 pl-8 pr-3 text-xs font-mono text-white placeholder-gray-600 focus:border-cyan-500/50 focus:outline-none"
            />
          </div>

          <div className="flex items-center gap-2">
            <span className="text-[10px] font-mono text-gray-500">
              Exibindo {filteredLines.length} de {lines.length} linha(s)
            </span>

            <button
              type="button"
              onClick={scrollToBottom}
              className="flex items-center gap-1 rounded-xl border border-white/10 bg-[#2b3238] px-3 py-1.5 text-xs font-bold text-gray-300 hover:bg-white/5 hover:text-white transition-colors"
            >
              <ArrowDown size={14} />
              <span>Rolar para Fim</span>
            </button>

            <button
              type="button"
              onClick={handleCopyLogs}
              className="flex items-center gap-1 rounded-xl border border-cyan-500/30 bg-cyan-500/10 px-3 py-1.5 text-xs font-bold text-cyan-200 hover:bg-cyan-500/20 transition-all shadow-sm"
            >
              {isCopied ? <Check size={14} className="text-green-400" /> : <Copy size={14} />}
              <span>{isCopied ? "Copiado!" : "Copiar"}</span>
            </button>
          </div>
        </div>

        {/* Content Viewer */}
        <div
          ref={logContainerRef}
          className="min-h-0 flex-1 overflow-auto p-6 font-mono text-xs leading-relaxed custom-scrollbar bg-[#181c20]"
        >
          {error ? (
            <div className="flex flex-col items-center justify-center p-12 text-center">
              <AlertCircle size={36} className="text-red-400 mb-2" />
              <p className="text-xs text-red-300 font-bold">{error}</p>
            </div>
          ) : isLoadingContent ? (
            <div className="flex flex-col items-center justify-center p-12 text-center text-gray-500">
              <RefreshCw size={24} className="animate-spin text-cyan-400 mb-2" />
              <p className="text-xs font-mono">Carregando arquivo de log...</p>
            </div>
          ) : filteredLines.length === 0 ? (
            <div className="flex items-center justify-center p-12 text-gray-500 italic text-xs font-mono">
              Nenhuma linha encontrada para o filtro ou categoria selecionada.
            </div>
          ) : (
            filteredLines.map((line, index) => {
              const lower = line.toLowerCase()
              let lineStyle = "text-gray-300"

              if (lower.includes("error") || lower.includes("err") || lower.includes("exception") || lower.includes("failed") || lower.includes("fatal")) {
                lineStyle = "text-red-400 font-bold bg-red-500/10 px-1 rounded"
              } else if (lower.includes("warn") || lower.includes("warning")) {
                lineStyle = "text-amber-300 font-semibold bg-amber-500/10 px-1 rounded"
              } else if (lower.includes("luanet") || lower.includes("lua") || lower.includes("mod")) {
                lineStyle = "text-sky-300 font-medium"
              } else if (lower.includes("server started") || lower.includes("listening") || lower.includes("raknet.startup")) {
                lineStyle = "text-green-300 font-bold bg-green-500/10 px-1 rounded"
              }

              return (
                <div key={index} className="flex items-start py-0.5 hover:bg-white/[0.02] rounded px-1 transition-colors">
                  <span className="w-12 text-[10px] text-gray-600 select-none text-right pr-3 shrink-0 pt-0.5 font-mono">
                    {index + 1}
                  </span>
                  <span className={`flex-1 min-w-0 break-all ${lineStyle}`}>{line}</span>
                </div>
              )
            })
          )}
        </div>
      </div>
    </div>
  )
}
