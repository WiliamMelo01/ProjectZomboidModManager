import { Check, ChevronRight, RotateCcw, Save, Search, Server, SlidersHorizontal, X, Puzzle, Settings2 } from "lucide-react"
import { useEffect, useState, useMemo } from "react"
import type { ReactNode } from "react"
import { useTranslation } from "react-i18next"

import { i18n } from "@/i18n"
import type { RemoteConnectionDraft } from "@/lib/commandRunner"
import { invokeTauri } from "@/lib/tauri"
import type { ServerConfigSetting, ServerIniSettings, ServerLuaSetting, ServerLuaSettings, ZomboidServer } from "@/types/server"

type ServerConfigurationModalProps = {
  isOpen: boolean
  server: ZomboidServer | null
  remoteConnection?: RemoteConnectionDraft | null
  onClose: () => void
  onSave: (settings: ServerIniSettings) => Promise<void> | void
}

const FALLBACK_SETTINGS: ServerIniSettings = {
  publicName: "",
  publicDescription: "",
  password: "",
  maxPlayers: 32,
  defaultPort: "16261",
  udpPort: "16262",
  isPublic: false,
  isOpen: true,
  pvp: true,
  pauseEmpty: true,
  globalChat: true,
  displayUserName: true,
  safetySystem: true,
  voiceEnable: true,
  steamVac: true,
  upnp: true,
  pingLimit: 400,
  saveWorldEveryMinutes: 0,
  hoursForLootRespawn: 0,
  playerSafehouse: false,
  adminSafehouse: false,
  backupsCount: 5,
  backupsOnStart: true,
  backupsPeriod: 0,
  settings: [],
}

const VANILLA_SECTIONS = [
  "Sandbox", "ZombieLore", "AdvancedZombieOptions", "Map", "Player", "Server",
  "Meta", "Chat", "Events", "Loot", "Time", "Vehicle", "Water", "Farming",
  "Nature", "SadisticAIDirector", "Multiplier", "Car", "DecayingCorpseHealthImpact",
  "WaterAndElectricity", "Fire"
]

export function ServerConfigurationModal({ isOpen, server, remoteConnection = null, onClose, onSave }: ServerConfigurationModalProps) {
  const { t } = useTranslation()
  const [activeConfigTab, setActiveConfigTab] = useState<"server" | "sandbox">("server")
  const [settings, setSettings] = useState<ServerIniSettings>(FALLBACK_SETTINGS)
  const [luaSettings, setLuaSettings] = useState<ServerLuaSetting[]>([])
  const [luaFileName, setLuaFileName] = useState("")
  const [configSearch, setConfigSearch] = useState("")
  const [selectedIniCategory, setSelectedIniCategory] = useState<string | null>(null)
  const [selectedLuaSection, setSelectedLuaSection] = useState<string | null>(null)
  const [isLoading, setIsLoading] = useState(false)
  const [isSaving, setIsSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!server || !isOpen) return

    setIsLoading(true)
    setError(null)
    setActiveConfigTab("server")
    setLuaSettings([])
    setLuaFileName("")
    setConfigSearch("")
    setSelectedIniCategory(null)
    setSelectedLuaSection(null)
    setSettings({ ...FALLBACK_SETTINGS, publicName: server.name, maxPlayers: server.maxPlayers || 32, defaultPort: server.port || "16261" })

    Promise.allSettled([
      invokeTauri<ServerIniSettings>(remoteConnection ? "get_remote_zomboid_server_settings" : "get_zomboid_server_settings", {
        ...(remoteConnection ? { connection: remoteConnection } : {}),
        serverId: server.id,
      }),
      invokeTauri<ServerLuaSettings>(remoteConnection ? "get_remote_zomboid_server_lua_settings" : "get_zomboid_server_lua_settings", {
        ...(remoteConnection ? { connection: remoteConnection } : {}),
        serverId: server.id,
      }),
    ])
      .then(([iniResult, sandboxResult]) => {
        if (iniResult.status === "fulfilled") {
          setSettings(iniResult.value)
        } else {
          setError(getErrorMessage(iniResult.reason, i18n.t("serverConfig.loadError")))
        }

        if (sandboxResult.status === "fulfilled") {
          setLuaSettings(sandboxResult.value.settings)
          setLuaFileName(sandboxResult.value.fileName)
        } else {
          setLuaSettings([])
          setLuaFileName(`${server.id}_SandboxVars.lua`)
        }
      })
      .finally(() => setIsLoading(false))
  }, [isOpen, server, remoteConnection])

  const filteredLuaSettings = useMemo(() => {
    return luaSettings.filter((setting) => {
      const search = configSearch.trim().toLowerCase()
      if (!search) return true

      return (
        setting.path.toLowerCase().includes(search) ||
        setting.key.toLowerCase().includes(search) ||
        setting.section.toLowerCase().includes(search) ||
        setting.value.toLowerCase().includes(search) ||
        String(setting.defaultValue ?? "").toLowerCase().includes(search) ||
        setting.options.some((option) =>
          option.value.toLowerCase().includes(search) || option.label.toLowerCase().includes(search),
        )
      )
    })
  }, [luaSettings, configSearch])

  const filteredIniSettings = useMemo(() => {
    return settings.settings.filter((setting) => {
      const search = configSearch.trim().toLowerCase()
      if (!search) return true

      return (
        setting.key.toLowerCase().includes(search) ||
        setting.category.toLowerCase().includes(search) ||
        setting.value.toLowerCase().includes(search) ||
        setting.description.toLowerCase().includes(search) ||
        String(setting.defaultValue ?? "").toLowerCase().includes(search)
      )
    })
  }, [settings.settings, configSearch])

  const luaSections = useMemo(() => groupLuaSettings(filteredLuaSettings), [filteredLuaSettings])
  const iniCategories = useMemo(() => groupConfigSettings(filteredIniSettings), [filteredIniSettings])
  const allLuaSections = useMemo(() => Array.from(new Set(luaSettings.map((setting) => setting.section))), [luaSettings])
  const allIniCategories = useMemo(() => Array.from(new Set(settings.settings.map((setting) => setting.category))), [settings.settings])
  const visibleLuaSections = useMemo(
    () => allLuaSections.filter((section) => !configSearch || luaSections.some(([s]) => s === section)),
    [allLuaSections, configSearch, luaSections],
  )
  const visibleIniCategories = useMemo(
    () => allIniCategories.filter((category) => !configSearch || iniCategories.some(([current]) => current === category)),
    [allIniCategories, configSearch, iniCategories],
  )
  const vanillaSections = useMemo(
    () => visibleLuaSections.filter((section) => VANILLA_SECTIONS.includes(section)),
    [visibleLuaSections],
  )
  const modSections = useMemo(
    () => visibleLuaSections.filter((section) => !VANILLA_SECTIONS.includes(section)),
    [visibleLuaSections],
  )

  useEffect(() => {
    if (configSearch && luaSections.length > 0) {
      if (!luaSections.find(([section]) => section === selectedLuaSection)) {
        setSelectedLuaSection(null)
      }
    }
  }, [configSearch, luaSections, selectedLuaSection])

  useEffect(() => {
    if (configSearch && iniCategories.length > 0) {
      if (!iniCategories.find(([category]) => category === selectedIniCategory)) {
        setSelectedIniCategory(null)
      }
    }
  }, [configSearch, iniCategories, selectedIniCategory])

  if (!isOpen || !server) return null

  const normalizedIni = normalizeIniSettings(settings)
  const canSaveIni =
    normalizedIni.publicName.trim().length > 0 &&
    normalizedIni.maxPlayers >= 1 &&
    normalizedIni.maxPlayers <= 100 &&
    isValidPort(normalizedIni.defaultPort) &&
    isValidPort(normalizedIni.udpPort) &&
    normalizedIni.pingLimit >= 100 &&
    normalizedIni.backupsCount >= 1 &&
    normalizedIni.backupsCount <= 300 &&
    normalizedIni.backupsPeriod >= 0 &&
    normalizedIni.backupsPeriod <= 1500 &&
    normalizedIni.settings.every((setting) => isValidConfigSetting(setting))
  const canSaveLua = luaSettings.length > 0 && luaSettings.every((setting) => isValidLuaSetting(setting))
  const canSave = activeConfigTab === "server" ? canSaveIni : canSaveLua

  const updateIniSetting = (key: string, value: string) => {
    setSettings((current) => normalizeIniSettings({
      ...current,
      settings: current.settings.map((setting) => setting.key === key ? { ...setting, value } : setting),
    }))
  }

  const updateLuaSetting = (path: string, value: string) => {
    setLuaSettings((current) =>
      current.map((setting) => setting.path === path ? { ...setting, value } : setting),
    )
  }

  const handleSave = async () => {
    if (!canSave) return

    setIsSaving(true)
    setError(null)

    try {
      if (activeConfigTab === "server") {
        await onSave(normalizedIni)
      } else {
        const saved = await invokeTauri<ServerLuaSettings>(remoteConnection ? "update_remote_zomboid_server_lua_settings" : "update_zomboid_server_lua_settings", {
          ...(remoteConnection ? { connection: remoteConnection } : {}),
          serverId: server.id,
          settings: luaSettings,
        })
        setLuaSettings(saved.settings)
        setLuaFileName(saved.fileName)
      }
      onClose()
    } catch (saveError) {
      setError(getErrorMessage(saveError, i18n.t("serverConfig.saveError")))
    } finally {
      setIsSaving(false)
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 p-4 backdrop-blur-sm animate-in fade-in duration-300">
      <div className="flex h-full max-h-[92vh] w-full max-w-5xl flex-col overflow-hidden rounded-3xl border border-white/10 bg-[#161a1d] shadow-2xl animate-in zoom-in-95 duration-300">
        <div className="flex items-center justify-between border-b border-white/5 bg-[#1c2126] p-6">
          <div className="flex min-w-0 items-center gap-3">
            <div className="rounded-xl bg-orange-500/20 p-2.5 text-orange-500 ring-1 ring-orange-500/20">
              <Server size={24} />
            </div>
            <div className="min-w-0">
              <h3 className="text-xl font-black uppercase italic tracking-tight text-white">{t("serverConfig.title")}</h3>
              <p className="truncate text-xs font-medium text-gray-500">{server.fileName}</p>
            </div>
          </div>
          <button onClick={onClose} className="rounded-full bg-white/5 p-2 text-gray-400 transition-all hover:bg-white/10 hover:text-white">
            <X size={20} />
          </button>
        </div>

        <div className="flex gap-1 border-b border-white/5 bg-[#161a1d] px-6 pt-4">
          <TabButton
            isActive={activeConfigTab === "server"}
            onClick={() => setActiveConfigTab("server")}
            icon={<Server size={16} />}
            label={t("serverConfig.serverTab")}
          />
          <TabButton
            isActive={activeConfigTab === "sandbox"}
            onClick={() => setActiveConfigTab("sandbox")}
            icon={<SlidersHorizontal size={16} />}
            label={t("serverConfig.sandboxTab")}
          />
        </div>

        <div className="relative flex flex-1 overflow-hidden">
          <div className="w-72 shrink-0 overflow-y-auto border-r border-white/5 bg-[#1c2126]/50 p-4 custom-scrollbar">
            <div className="mb-4 space-y-3 px-2">
              <div className="relative">
                <Search className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-500" size={16} />
                <input
                  value={configSearch}
                  onChange={(event) => setConfigSearch(event.target.value)}
                  placeholder={activeConfigTab === "server" ? t("serverConfig.searchServer") : t("serverConfig.searchSandbox")}
                  className="w-full rounded-xl border border-white/5 bg-[#161a1d] py-3 pl-10 pr-3 text-sm text-gray-200 focus:border-orange-500/50 focus:outline-none focus:ring-1 focus:ring-orange-500/20"
                />
              </div>
            </div>

            {activeConfigTab === "server" ? (
              <div className="space-y-1">
                {visibleIniCategories.map((category) => (
                  <SidebarSectionButton
                    key={category}
                    label={`${category} (${settings.settings.filter((setting) => setting.category === category).length})`}
                    icon={<Settings2 size={18} />}
                    isSelected={selectedIniCategory === category}
                    onClick={() => setSelectedIniCategory(category)}
                  />
                ))}
              </div>
            ) : (
              <div className="space-y-4">
                {vanillaSections.length > 0 && (
                  <SidebarSectionGroup title={t("serverConfig.vanillaSections")}>
                    {vanillaSections.map((section) => (
                      <SidebarSectionButton
                        key={section}
                        label={`${section} (${luaSettings.filter((setting) => setting.section === section).length})`}
                        icon={<Settings2 size={18} />}
                        isSelected={selectedLuaSection === section}
                        onClick={() => setSelectedLuaSection(section)}
                      />
                    ))}
                  </SidebarSectionGroup>
                )}
                {modSections.length > 0 && (
                  <SidebarSectionGroup title={t("serverConfig.modSections")}>
                    {modSections.map((section) => (
                      <SidebarSectionButton
                        key={section}
                        label={`${section} (${luaSettings.filter((setting) => setting.section === section).length})`}
                        icon={<Puzzle size={18} />}
                        isSelected={selectedLuaSection === section}
                        onClick={() => setSelectedLuaSection(section)}
                        mutedIcon
                      />
                    ))}
                  </SidebarSectionGroup>
                )}
              </div>
            )}
          </div>

          <div className="flex-1 overflow-y-auto p-6 custom-scrollbar bg-[#161a1d]">
            {isLoading && (
              <div className="mb-6 flex items-center gap-3 rounded-2xl border border-white/5 bg-[#1c2126] px-5 py-4 text-sm font-medium text-gray-300 animate-pulse">
                <RotateCcw size={18} className="animate-spin text-orange-500" />
                {t("serverConfig.loading")}
              </div>
            )}

            {activeConfigTab === "server" ? (
              <div className="space-y-6">
                {!isLoading && settings.settings.length === 0 && (
                  <div className="rounded-2xl border border-white/5 bg-[#1c2126] p-12 text-center">
                    <Settings2 size={48} className="mx-auto mb-4 text-gray-700" />
                    <p className="text-gray-400">{t("serverConfig.noServerSettings")}</p>
                  </div>
                )}

                {iniCategories
                  .filter(([category]) => !selectedIniCategory || category === selectedIniCategory || configSearch)
                  .map(([category, items]) => (
                    <ConfigSection key={category} title={category} count={items.length} icon={<Settings2 size={12} />}>
                      {items.map((setting) => (
                        <ConfigSettingField
                          key={setting.key}
                          setting={setting}
                          onChange={(value) => updateIniSetting(setting.key, value)}
                        />
                      ))}
                    </ConfigSection>
                  ))}
              </div>
            ) : (
              <div className="space-y-6">
                {!isLoading && luaSettings.length === 0 && (
                  <div className="rounded-2xl border border-white/5 bg-[#1c2126] p-12 text-center">
                    <SlidersHorizontal size={48} className="mx-auto mb-4 text-gray-700" />
                    <p className="text-gray-400">{t("serverConfig.noSandboxSettings")}</p>
                  </div>
                )}

                {luaSections
                  .filter(([section]) => !selectedLuaSection || section === selectedLuaSection || configSearch)
                  .map(([section, items]) => {
                    const isVanilla = VANILLA_SECTIONS.includes(section)
                    return (
                      <div key={section} className="space-y-4">
                        <div className="flex items-center gap-3">
                          <div className="h-px flex-1 bg-gradient-to-r from-transparent via-white/5 to-transparent" />
                          <div className="flex items-center gap-2 text-[10px] font-black uppercase italic tracking-widest text-orange-500/50">
                            {isVanilla ? <Settings2 size={12} /> : <Puzzle size={12} />}
                            {section}
                          </div>
                          <div className="h-px flex-1 bg-gradient-to-r from-transparent via-white/5 to-transparent" />
                        </div>
                        <div className="grid gap-4 md:grid-cols-2">
                          {items.map((setting) => (
                            <LuaSettingField
                              key={setting.path}
                              setting={setting}
                              onChange={(value) => updateLuaSetting(setting.path, value)}
                            />
                          ))}
                        </div>
                      </div>
                    )
                  })}
              </div>
            )}

            {error && (
              <div className="mt-6 rounded-2xl border border-red-500/20 bg-red-500/10 px-6 py-4 text-sm font-medium text-red-400 ring-1 ring-red-500/10">
                {error}
              </div>
            )}
          </div>
        </div>

        <div className="flex items-center justify-between border-t border-white/5 bg-[#1c2126] p-6">
          <button onClick={onClose} className="px-6 py-3 text-sm font-black uppercase italic tracking-wider text-gray-500 transition-colors hover:text-white">
            {t("serverConfig.skip")}
          </button>
          <button
            disabled={!canSave || isSaving || isLoading}
            onClick={() => void handleSave()}
            className="group relative flex items-center gap-2 overflow-hidden rounded-2xl bg-orange-500 px-10 py-3.5 font-black uppercase italic tracking-widest text-white shadow-[0_0_20px_rgba(249,115,22,0.3)] transition-all hover:bg-orange-600 hover:shadow-[0_0_25px_rgba(249,115,22,0.4)] disabled:bg-gray-800 disabled:text-gray-600 disabled:shadow-none"
          >
            {isSaving ? <RotateCcw size={18} className="animate-spin" /> : <Save size={18} className="transition-transform group-hover:scale-110" />}
            <span>{isSaving ? t("serverConfig.saving") : t("serverConfig.save")}</span>
          </button>
        </div>
      </div>
    </div>
  )
}

function Section({ title, icon, children }: { title: string; icon: ReactNode; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-5 rounded-2xl border border-white/5 bg-[#1c2126] p-6 transition-colors hover:border-white/10">
      <div className="flex items-center gap-2.5 text-xs font-black uppercase italic tracking-widest text-orange-400">
        <div className="rounded-lg bg-orange-500/10 p-1.5 ring-1 ring-orange-500/20">
          {icon}
        </div>
        {title}
      </div>
      <div className="space-y-4">
        {children}
      </div>
    </section>
  )
}

function SidebarSectionGroup({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="space-y-1">
      <div className="px-4 pb-1 text-[9px] font-black uppercase tracking-[0.22em] text-gray-600">
        {title}
      </div>
      {children}
    </div>
  )
}

function SidebarSectionButton({
  label,
  icon,
  isSelected,
  onClick,
  mutedIcon = false,
}: {
  label: string
  icon: ReactNode
  isSelected: boolean
  onClick: () => void
  mutedIcon?: boolean
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`flex w-full items-center justify-between rounded-xl px-4 py-3.5 text-left transition-all group ${
        isSelected
          ? "bg-orange-500/10 ring-1 ring-orange-500/20"
          : "hover:bg-white/5"
      }`}
    >
      <div className="flex min-w-0 items-center gap-3">
        <span className={isSelected ? "text-orange-400" : mutedIcon ? "text-gray-500/50" : "text-gray-500"}>
          {icon}
        </span>
        <span className={`truncate text-sm font-black uppercase italic tracking-tight ${
          isSelected
            ? "text-orange-400"
            : "text-gray-400 group-hover:text-gray-200"
        }`}>
          {label}
        </span>
      </div>
      {isSelected && <ChevronRight size={16} className="text-orange-400" />}
    </button>
  )
}

function TabButton({ isActive, onClick, icon, label }: { isActive: boolean; onClick: () => void; icon: ReactNode; label: string }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`relative flex items-center gap-2 px-6 py-4 text-xs font-black uppercase italic tracking-wider transition-all ${
        isActive
          ? "text-orange-400"
          : "text-gray-500 hover:text-gray-300"
      }`}
    >
      {icon}
      <span>{label}</span>
      {isActive && (
        <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-orange-500 shadow-[0_-4px_10px_rgba(249,115,22,0.5)]" />
      )}
    </button>
  )
}

function TextField({ label, value, onChange, icon, placeholder }: { label: string; value: string; onChange: (value: string) => void; icon?: React.ReactNode; placeholder?: string }) {
  return (
    <label className="block space-y-1.5">
      <span className="ml-1 text-[9px] font-black uppercase tracking-[0.2em] text-gray-500">{label}</span>
      <div className="relative group">
        {icon && <div className="absolute left-3.5 top-1/2 -translate-y-1/2 text-gray-500 transition-colors group-focus-within:text-orange-500">{icon}</div>}
        <input
          type="text"
          value={value}
          placeholder={placeholder}
          onChange={(event) => onChange(event.target.value)}
          className={`w-full rounded-xl border border-white/5 bg-[#161a1d] py-3 ${icon ? "pl-11" : "pl-4"} pr-4 text-sm text-gray-200 transition-all placeholder:text-gray-700 focus:border-orange-500/50 focus:outline-none focus:ring-1 focus:ring-orange-500/20`}
        />
      </div>
    </label>
  )
}

function CustomSelect({ 
  value, 
  options, 
  defaultValue, 
  onChange 
}: { 
  value: string; 
  options: { value: string; label: string }[]; 
  defaultValue?: string | null; 
  onChange: (val: string) => void;
}) {
  const [isOpen, setIsOpen] = useState(false);
  const selectedOption = options.find(o => o.value === value) || { value, label: value };
  const defaultOptionValue = resolveDefaultOptionValue(options, defaultValue);
  const defaultOption = defaultOptionValue ? options.find((option) => option.value === defaultOptionValue) : null;
  const isSelectedDefault = value === defaultOptionValue;

  return (
    <div className="space-y-1.5">
      <div className="relative group">
        <button
          type="button"
          onClick={() => setIsOpen(!isOpen)}
          onBlur={() => setTimeout(() => setIsOpen(false), 200)}
          className={`flex w-full items-center justify-between rounded-xl border bg-[#161a1d] px-4 py-3.5 text-sm font-bold transition-all hover:border-white/10 focus:border-orange-500/50 focus:outline-none focus:ring-1 focus:ring-orange-500/20 shadow-inner ${isOpen ? "border-orange-500/50 ring-1 ring-orange-500/20 text-white" : "border-white/5 text-gray-100"}`}
        >
          <div className="flex min-w-0 items-center gap-2">
            <span className="whitespace-normal text-left">{selectedOption.label}</span>
            {isSelectedDefault && <DefaultBadge tone="muted" />}
          </div>
          <ChevronRight size={18} className={`text-orange-500 transition-transform duration-200 shrink-0 ${isOpen ? "-rotate-90" : "rotate-90"}`} strokeWidth={3} />
        </button>

        {isOpen && (
          <div className="absolute z-50 mt-2 w-full overflow-hidden rounded-xl border border-white/10 bg-[#2b3238] py-1 shadow-2xl shadow-black/60 animate-in fade-in slide-in-from-top-2">
            <div className="max-h-60 overflow-y-auto custom-scrollbar">
              {options.map((option) => {
                const isDefault = option.value === defaultOptionValue;
                const isSelected = option.value === value;
                return (
                  <button
                    key={option.value}
                    type="button"
                    onClick={() => {
                      onChange(option.value);
                      setIsOpen(false);
                    }}
                    className={`flex w-full items-center justify-between gap-3 px-4 py-3 text-sm transition-colors hover:bg-white/10 ${
                      isDefault
                        ? "border-l-2 border-orange-400 bg-orange-500/10 text-orange-200 font-black"
                        : isSelected
                          ? "bg-orange-500/10 text-orange-400 font-bold"
                          : "text-gray-300 font-medium"
                    }`}
                  >
                    <span className="whitespace-normal text-left">{option.label}</span>
                    {isDefault && <DefaultBadge tone="strong" />}
                  </button>
                );
              })}
            </div>
          </div>
        )}
      </div>

      {defaultOption && (
        <div className="ml-1 flex items-center gap-1.5 text-[10px] font-medium text-gray-500">
          <span className="text-[9px] font-black uppercase tracking-wider text-gray-600">{i18n.t("serverConfig.defaultOption")}</span>
          <span className="rounded bg-orange-500/10 px-1.5 py-0.5 text-[9px] font-black text-orange-300 ring-1 ring-orange-500/20">
            {defaultOption.label}
          </span>
        </div>
      )}
    </div>
  );
}

function DefaultBadge({ tone }: { tone: "muted" | "strong" }) {
  return (
    <span
      className={`shrink-0 rounded px-2 py-0.5 text-[9px] font-black uppercase tracking-wider ring-1 ${
        tone === "strong"
          ? "bg-orange-500/20 text-orange-300 ring-orange-500/30"
          : "bg-white/5 text-gray-500 ring-white/10"
      }`}
    >
      {i18n.t("serverConfig.defaultLabel")}
    </span>
  )
}

function ConfigSection({ title, count, icon, children }: { title: string; count: number; icon: ReactNode; children: ReactNode }) {
  return (
    <div className="space-y-4">
      <div className="flex items-center gap-3">
        <div className="h-px flex-1 bg-gradient-to-r from-transparent via-white/5 to-transparent" />
        <div className="flex items-center gap-2 text-[10px] font-black uppercase italic tracking-widest text-orange-500/50">
          {icon}
          {title}
          <span className="rounded bg-white/5 px-1.5 py-0.5 text-[9px] text-gray-500">{count}</span>
        </div>
        <div className="h-px flex-1 bg-gradient-to-r from-transparent via-white/5 to-transparent" />
      </div>
      <div className="grid gap-4 md:grid-cols-2">
        {children}
      </div>
    </div>
  )
}

function ConfigSettingField({ setting, onChange }: { setting: ServerConfigSetting; onChange: (value: string) => void }) {
  const defaultValue = resolveConfigDefaultValue(setting)
  const isDefault = defaultValue !== null && valuesMatch(setting.value, defaultValue, setting.valueKind)
  const hasRange = setting.minValue || setting.maxValue

  return (
    <div className="flex flex-col gap-3 rounded-xl border border-white/5 bg-[#1c2126] p-4 transition-all hover:border-white/10 hover:bg-[#1f252a]">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <div className="break-all text-[10px] font-black uppercase tracking-wider text-gray-300" title={setting.key}>
            {setting.key}
          </div>
          {setting.description && (
            <p className="mt-1 line-clamp-3 whitespace-pre-line text-xs leading-relaxed text-gray-500">
              {setting.description}
            </p>
          )}
        </div>
        {defaultValue !== null && !isDefault && (
          <button
            type="button"
            onClick={() => onChange(defaultValue)}
            className="flex shrink-0 items-center gap-1 rounded-md bg-white/5 px-2 py-1 text-[9px] font-black uppercase text-orange-400 transition-colors hover:bg-white/10"
          >
            <RotateCcw size={10} />
            {i18n.t("serverConfig.resetToDefault")}
          </button>
        )}
      </div>

      {setting.valueKind === "boolean" ? (
        <div className="flex items-center justify-between gap-4 py-1">
          <div className="flex flex-col">
            <span className="text-sm font-black uppercase italic text-gray-200">
              {isTrueValue(setting.value) ? i18n.t("serverConfig.enabled") : i18n.t("serverConfig.disabled")}
            </span>
            {defaultValue !== null && (
              <span className="text-[10px] font-medium text-gray-600">
                {i18n.t("serverConfig.defaultValue")} {isTrueValue(defaultValue) ? i18n.t("serverConfig.enabled") : i18n.t("serverConfig.disabled")}
              </span>
            )}
          </div>
          <button
            type="button"
            onClick={() => onChange(isTrueValue(setting.value) ? "false" : "true")}
            className={`h-7 w-12 shrink-0 rounded-full p-1 transition-all ${isTrueValue(setting.value) ? "bg-orange-500 shadow-[0_0_10px_rgba(249,115,22,0.3)]" : "bg-gray-700"}`}
          >
            <div className={`h-5 w-5 rounded-full bg-white transition-transform ${isTrueValue(setting.value) ? "translate-x-5" : ""}`} />
          </button>
        </div>
      ) : (
        <div className="space-y-1.5">
          <input
            type={setting.valueKind === "number" ? "number" : "text"}
            min={setting.minValue ?? undefined}
            max={setting.maxValue ?? undefined}
            value={setting.value}
            onChange={(event) => onChange(event.target.value)}
            className={`w-full rounded-lg border bg-[#161a1d] px-4 py-3 text-sm font-medium text-gray-200 transition-all focus:border-orange-500/50 focus:outline-none focus:ring-1 focus:ring-orange-500/20 ${
              isValidConfigSetting(setting) ? "border-white/5" : "border-red-500/50 focus:border-red-500/50 focus:ring-red-500/20"
            }`}
          />
          <div className="flex flex-wrap gap-2">
            {defaultValue !== null && (
              <MetaPill label={i18n.t("serverConfig.defaultValue")} value={defaultValue} strong={isDefault} />
            )}
            {hasRange && (
              <MetaPill
                label={i18n.t("serverConfig.range")}
                value={`${setting.minValue ?? "-"} - ${setting.maxValue ?? "-"}`}
              />
            )}
          </div>
        </div>
      )}
    </div>
  )
}

function MetaPill({ label, value, strong = false }: { label: string; value: string; strong?: boolean }) {
  return (
    <span className={`rounded px-1.5 py-0.5 text-[9px] font-bold ring-1 ${
      strong
        ? "bg-orange-500/10 text-orange-300 ring-orange-500/20"
        : "bg-white/5 text-gray-500 ring-white/5"
    }`}>
      <span className="font-black uppercase tracking-wider">{label}</span> {value}
    </span>
  )
}

function LuaSettingField({ setting, onChange }: { setting: ServerLuaSetting; onChange: (value: string) => void }) {
  const resolvedDefaultValue = resolveDefaultSettingValue(setting)
  const isDefault = resolvedDefaultValue !== null ? isLuaSettingDefault(setting, resolvedDefaultValue) : setting.value === ""

  const resetToDefault = () => {
    if (resolvedDefaultValue !== null) {
      onChange(resolvedDefaultValue)
    }
  }

  return (
    <div className="flex flex-col gap-3 rounded-xl border border-white/5 bg-[#1c2126] p-4 transition-all hover:border-white/10 hover:bg-[#1f252a]">
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-[10px] font-black uppercase tracking-wider whitespace-normal break-all text-gray-400" title={setting.key}>
            {setting.key}
          </span>
        </div>
        {resolvedDefaultValue !== null && setting.defaultValue && !isDefault && (
          <button
            onClick={resetToDefault}
            className="flex items-center gap-1 rounded-md bg-white/5 px-2 py-1 text-[9px] font-black uppercase text-orange-400 transition-colors hover:bg-white/10"
          >
            <RotateCcw size={10} />
            {i18n.t("serverConfig.resetToDefault", { defaultValue: "Resetar" })}
          </button>
        )}
      </div>

      <div className="relative">
        {setting.valueKind === "boolean" ? (
          <div className="flex items-center justify-between gap-4 py-1">
             <div className="flex flex-col">
               <span className="text-sm font-black uppercase italic text-gray-200">
                 {setting.value === "true" ? i18n.t("serverConfig.enabled") : i18n.t("serverConfig.disabled")}
               </span>
               {setting.defaultValue && (
                 <span className="text-[10px] font-medium text-gray-600">
                   {i18n.t("serverConfig.defaultValue")} {setting.defaultValue === "true" ? i18n.t("serverConfig.enabled") : i18n.t("serverConfig.disabled")}
                 </span>
               )}
             </div>
             <button
              type="button"
              onClick={() => onChange(setting.value === "true" ? "false" : "true")}
              className={`h-7 w-12 shrink-0 rounded-full p-1 transition-all ${setting.value === "true" ? "bg-orange-500 shadow-[0_0_10px_rgba(249,115,22,0.3)]" : "bg-gray-700"}`}
            >
              <div className={`h-5 w-5 rounded-full bg-white transition-transform ${setting.value === "true" ? "translate-x-5" : ""}`} />
            </button>
          </div>
        ) : setting.options.length > 0 ? (
          <div className="space-y-1.5">
            <CustomSelect
              value={setting.value}
              options={setting.options}
              defaultValue={setting.defaultValue}
              onChange={(value) => onChange(value)}
            />
          </div>
        ) : (
          <div className="space-y-1.5">
            <div className="relative group">
              <input
                type={setting.valueKind === "number" ? "number" : "text"}
                value={setting.value}
                onChange={(event) => onChange(event.target.value)}
                className={`w-full rounded-lg border bg-[#161a1d] px-4 py-3 text-sm font-medium text-gray-200 transition-all focus:border-orange-500/50 focus:outline-none focus:ring-1 focus:ring-orange-500/20 ${
                  isValidLuaSetting(setting) ? "border-white/5" : "border-red-500/50 focus:border-red-500/50 focus:ring-red-500/20"
                }`}
              />
              {isValidLuaSetting(setting) && !isDefault && (
                <div className="absolute right-4 top-1/2 -translate-y-1/2 text-orange-400">
                   <Check size={16} strokeWidth={3} />
                </div>
              )}
            </div>
            {setting.defaultValue && (
              <div className="ml-1 flex items-center gap-1.5">
                <span className="text-[9px] font-black uppercase tracking-wider text-gray-600">{i18n.t("serverConfig.defaultValue")}</span>
                <span className="rounded bg-white/5 px-1.5 py-0.5 text-[9px] font-bold text-gray-500 ring-1 ring-white/5">
                  {setting.defaultValue}
                </span>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  )
}

function resolveDefaultSettingValue(setting: ServerLuaSetting) {
  if (setting.defaultValue === undefined || setting.defaultValue === null) {
    return null
  }

  const optionValue = resolveDefaultOptionValue(setting.options, setting.defaultValue)

  if (optionValue !== null) {
    return optionValue
  }

  if (setting.valueKind === "number") {
    return normalizeLuaNumberDefault(setting.defaultValue)
  }

  return setting.defaultValue
}

function isLuaSettingDefault(setting: ServerLuaSetting, resolvedDefaultValue: string) {
  if (setting.valueKind === "number") {
    return Number(setting.value) === Number(resolvedDefaultValue)
  }

  return setting.value === resolvedDefaultValue
}

function resolveDefaultOptionValue(options: { value: string; label: string }[], defaultValue?: string | null) {
  if (defaultValue === undefined || defaultValue === null) {
    return null
  }

  const normalizedDefault = normalizeDefaultText(defaultValue)
  const defaultOption = options.find((option) =>
    normalizeDefaultText(option.value) === normalizedDefault ||
    normalizeDefaultText(option.label) === normalizedDefault ||
    normalizeDefaultText(option.label).startsWith(`${normalizedDefault} `) ||
    normalizedDefault.startsWith(`${normalizeDefaultText(option.label)} `) ||
    normalizeTimeLabel(option.label) === normalizeTimeLabel(defaultValue),
  )

  return defaultOption?.value ?? null
}

function normalizeDefaultText(value: string) {
  return value
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .trim()
    .toLowerCase()
}

function normalizeTimeLabel(value: string) {
  const trimmedValue = value.trim().toUpperCase()
  const twentyFourHour = trimmedValue.match(/^(\d{1,2}):(\d{2})$/)

  if (twentyFourHour) {
    const hour = Number(twentyFourHour[1])
    const minute = twentyFourHour[2]

    if (hour >= 0 && hour <= 23) {
      return `${hour.toString().padStart(2, "0")}:${minute}`
    }
  }

  const twelveHour = trimmedValue.match(/^(\d{1,2})(?::(\d{2}))?\s*(AM|PM)$/)

  if (twelveHour) {
    let hour = Number(twelveHour[1])
    const minute = twelveHour[2] ?? "00"
    const period = twelveHour[3]

    if (period === "AM" && hour === 12) {
      hour = 0
    } else if (period === "PM" && hour !== 12) {
      hour += 12
    }

    if (hour >= 0 && hour <= 23) {
      return `${hour.toString().padStart(2, "0")}:${minute}`
    }
  }

  return normalizeDefaultText(value)
}

function normalizeLuaNumberDefault(value: string) {
  const normalizedValue = value.trim().replace(",", ".")

  return Number.isFinite(Number(normalizedValue)) ? normalizedValue : null
}

function TextArea({ label, value, onChange }: { label: string; value: string; onChange: (value: string) => void }) {
  return (
    <label className="block space-y-1.5">
      <span className="ml-1 text-[9px] font-black uppercase tracking-[0.2em] text-gray-500">{label}</span>
      <textarea
        value={value}
        rows={3}
        onChange={(event) => onChange(event.target.value)}
        className="w-full resize-none rounded-xl border border-white/5 bg-[#161a1d] px-4 py-3 text-sm text-gray-200 transition-all placeholder:text-gray-700 focus:border-orange-500/50 focus:outline-none focus:ring-1 focus:ring-orange-500/20"
      />
    </label>
  )
}

function NumberField({ label, value, min, max, onChange, icon }: { label: string; value: number; min: number; max: number; onChange: (value: number) => void; icon?: React.ReactNode }) {
  return (
    <label className="block space-y-1.5">
      <span className="ml-1 text-[9px] font-black uppercase tracking-[0.2em] text-gray-500">{label}</span>
      <div className="relative group">
        {icon && <div className="absolute left-3.5 top-1/2 -translate-y-1/2 text-gray-500 transition-colors group-focus-within:text-orange-500">{icon}</div>}
        <input
          type="number"
          min={min}
          max={max}
          value={value}
          onChange={(event) => onChange(clampNumber(event.target.valueAsNumber, min, max, min))}
          className={`w-full rounded-xl border border-white/5 bg-[#161a1d] py-3 ${icon ? "pl-11" : "pl-4"} pr-4 text-sm text-gray-200 transition-all focus:border-orange-500/50 focus:outline-none focus:ring-1 focus:ring-orange-500/20`}
        />
      </div>
    </label>
  )
}

function Toggle({ label, checked, onChange, icon }: { label: string; checked: boolean; onChange: (value: boolean) => void; icon?: React.ReactNode }) {
  return (
    <button
      type="button"
      onClick={() => onChange(!checked)}
      className="flex w-full items-center justify-between gap-4 rounded-xl border border-white/5 bg-[#1c2126] px-4 py-3 text-left text-sm transition-all hover:border-orange-500/30 hover:bg-[#1f252a] group"
    >
      <span className="flex min-w-0 items-center gap-2.5 text-xs font-bold uppercase italic text-gray-400 group-hover:text-gray-200">
        {icon && <span className="text-gray-600 transition-colors group-hover:text-orange-500 shrink-0">{icon}</span>}
        <span className="whitespace-normal leading-tight text-left">{label}</span>
      </span>
      <span className={`h-6 w-11 shrink-0 rounded-full p-1 transition-all ${checked ? "bg-orange-500 shadow-[0_0_10px_rgba(249,115,22,0.3)]" : "bg-gray-700"}`}>
        <span className={`block h-4 w-4 rounded-full bg-white transition-transform ${checked ? "translate-x-5" : ""}`} />
      </span>
    </button>
  )
}

function isValidPort(value: string) {
  const port = Number(value)
  return Number.isInteger(port) && port >= 1 && port <= 65535
}

function isValidLuaSetting(setting: ServerLuaSetting) {
  if (setting.valueKind === "number") {
    return setting.value.trim() !== "" && Number.isFinite(Number(setting.value))
  }

  if (setting.valueKind === "boolean") {
    return setting.value === "true" || setting.value === "false"
  }

  return true
}

function isValidConfigSetting(setting: ServerConfigSetting) {
  if (setting.valueKind === "number") {
    const value = Number(setting.value)
    if (setting.value.trim() === "" || !Number.isFinite(value)) {
      return false
    }

    const min = setting.minValue ? Number(setting.minValue.replace(",", ".")) : null
    const max = setting.maxValue ? Number(setting.maxValue.replace(",", ".")) : null

    return (min === null || !Number.isFinite(min) || value >= min) && (max === null || !Number.isFinite(max) || value <= max)
  }

  if (setting.valueKind === "boolean") {
    return ["true", "false", "1", "0", "yes", "no", "on", "off"].includes(setting.value.trim().toLowerCase())
  }

  return true
}

function normalizeIniSettings(settings: ServerIniSettings): ServerIniSettings {
  const values = new Map(settings.settings.map((setting) => [setting.key, setting.value]))
  const boolValue = (key: string, fallback: boolean) => {
    const value = values.get(key)
    if (value === undefined) return fallback
    return isTrueValue(value)
  }
  const numberValue = (key: string, fallback: number) => {
    const value = Number(values.get(key))
    return Number.isFinite(value) ? value : fallback
  }

  return {
    ...settings,
    publicName: values.get("PublicName") ?? settings.publicName,
    publicDescription: values.get("PublicDescription") ?? settings.publicDescription,
    password: values.get("Password") ?? settings.password,
    maxPlayers: numberValue("MaxPlayers", settings.maxPlayers),
    defaultPort: values.get("DefaultPort") ?? settings.defaultPort,
    udpPort: values.get("UDPPort") ?? settings.udpPort,
    isPublic: boolValue("Public", settings.isPublic),
    isOpen: boolValue("Open", settings.isOpen),
    pvp: boolValue("PVP", settings.pvp),
    pauseEmpty: boolValue("PauseEmpty", settings.pauseEmpty),
    globalChat: boolValue("GlobalChat", settings.globalChat),
    displayUserName: boolValue("DisplayUserName", settings.displayUserName),
    safetySystem: boolValue("SafetySystem", settings.safetySystem),
    voiceEnable: boolValue("VoiceEnable", settings.voiceEnable),
    steamVac: boolValue("SteamVAC", settings.steamVac),
    upnp: boolValue("UPnP", settings.upnp),
    pingLimit: numberValue("PingLimit", settings.pingLimit),
    saveWorldEveryMinutes: numberValue("SaveWorldEveryMinutes", settings.saveWorldEveryMinutes),
    hoursForLootRespawn: numberValue("HoursForLootRespawn", settings.hoursForLootRespawn),
    playerSafehouse: boolValue("PlayerSafehouse", settings.playerSafehouse),
    adminSafehouse: boolValue("AdminSafehouse", settings.adminSafehouse),
    backupsCount: numberValue("BackupsCount", settings.backupsCount),
    backupsOnStart: boolValue("BackupsOnStart", settings.backupsOnStart),
    backupsPeriod: numberValue("BackupsPeriod", settings.backupsPeriod),
  }
}

function resolveConfigDefaultValue(setting: ServerConfigSetting) {
  if (setting.defaultValue === undefined || setting.defaultValue === null) {
    return null
  }

  if (setting.valueKind === "number") {
    return normalizeLuaNumberDefault(setting.defaultValue)
  }

  if (setting.valueKind === "boolean") {
    return isTrueValue(setting.defaultValue) ? "true" : "false"
  }

  return setting.defaultValue
}

function valuesMatch(value: string, defaultValue: string, valueKind: ServerConfigSetting["valueKind"]) {
  if (valueKind === "number") {
    return Number(value) === Number(defaultValue)
  }

  if (valueKind === "boolean") {
    return isTrueValue(value) === isTrueValue(defaultValue)
  }

  return value === defaultValue
}

function isTrueValue(value: string) {
  return ["true", "1", "yes", "on"].includes(value.trim().toLowerCase())
}

function groupConfigSettings(settings: ServerConfigSetting[]) {
  const sections = new Map<string, ServerConfigSetting[]>()

  for (const setting of settings) {
    sections.set(setting.category, [...(sections.get(setting.category) ?? []), setting])
  }

  return Array.from(sections.entries())
}

function groupLuaSettings(settings: ServerLuaSetting[]) {
  const sections = new Map<string, ServerLuaSetting[]>()

  for (const setting of settings) {
    sections.set(setting.section, [...(sections.get(setting.section) ?? []), setting])
  }

  return Array.from(sections.entries())
}

function clampNumber(value: number, min: number, max: number, fallback: number) {
  if (!Number.isFinite(value)) {
    return fallback
  }

  return Math.min(max, Math.max(min, Math.trunc(value)))
}

function getErrorMessage(error: unknown, fallback: string) {
  if (error instanceof Error) {
    return error.message
  }

  if (typeof error === "string") {
    return error
  }

  return fallback
}
