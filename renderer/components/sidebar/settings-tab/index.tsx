import SelectTheme from "./select-theme";
import { SaveOutputFolderToggle } from "./save-output-folder-toggle";
import { InputGpuId } from "./input-gpu-id";
import { CustomModelsFolderSelect } from "./select-custom-models-folder";
import { LogArea } from "./log-area";
import { SelectImageScale } from "./select-image-scale";
import { SelectImageFormat } from "./select-image-format";
import React, { useState, useMemo } from "react";
import { useAtom, useAtomValue } from "jotai";
import { customModelsPathAtom, scaleAtom } from "@/atoms/user-settings-atom";
import { InputCompression } from "./input-compression";
import OverwriteToggle from "./overwrite-toggle";
import { ResetSettingsButton } from "./reset-settings-button";
import TurnOffNotificationsToggle from "./turn-off-notifications-toggle";
import { cn } from "@/lib/utils";
import { InputCustomResolution } from "./input-custom-resolution";
import { InputTileSize } from "./input-tile-size";
import LanguageSwitcher from "./language-switcher";
import { translationAtom } from "@/atoms/translations-atom";
import { ImageFormat } from "@/lib/valid-formats";
import AutoUpdateToggle from "./auto-update-toggle";
import TTAModeToggle from "./tta-mode-toggle";
import PreserveFilenameToggle from "./preserve-filename-toggle";
import SystemInfo from "./system-info";
import CopyMetadataToggle from "./copy-metadata-toggle";
import { Search, Settings, MonitorCog, Paintbrush, Cpu, FolderCog, Store } from "lucide-react";
import { ApiServerToggle } from "./api-server-toggle";
import { CustomModelsManager } from "./custom-models-manager";
import { Marketplace } from "./marketplace";

interface IProps {
  batchMode: boolean;
  saveImageAs: ImageFormat;
  setSaveImageAs: React.Dispatch<React.SetStateAction<ImageFormat>>;
  compression: number;
  setCompression: React.Dispatch<React.SetStateAction<number>>;
  gpuId: string;
  setGpuId: React.Dispatch<React.SetStateAction<string>>;
  logData: string[];
}

type SettingsTabId = "general" | "output" | "performance" | "system" | "models";

const SETTINGS_TABS: { id: SettingsTabId; label: string; icon: React.ElementType }[] = [
  { id: "general", label: "General", icon: Paintbrush },
  { id: "output", label: "Output", icon: FolderCog },
  { id: "performance", label: "Performance", icon: Cpu },
  { id: "system", label: "System", icon: MonitorCog },
  { id: "models", label: "Models", icon: Store },
];

function SettingsTab({
  batchMode,
  compression,
  setCompression,
  gpuId,
  setGpuId,
  saveImageAs,
  setSaveImageAs,
  logData,
}: IProps) {
  const [isCopied, setIsCopied] = useState(false);
  const [customModelsPath, setCustomModelsPath] = useAtom(customModelsPathAtom);
  const [scale, setScale] = useAtom(scaleAtom);
  const t = useAtomValue(translationAtom);

  const [activeTab, setActiveTab] = useState<SettingsTabId>("general");
  const [searchQuery, setSearchQuery] = useState("");

  const setExportType = (format: ImageFormat) => {
    setSaveImageAs(format);
  };

  const handleCompressionChange = (e) => {
    setCompression(e.target.value);
  };

  const handleGpuIdChange = (e) => {
    setGpuId(e.target.value);
    localStorage.setItem("gpuId", e.target.value);
  };

  const copyOnClickHandler = () => {
    navigator.clipboard.writeText(logData.join("\n"));
    setIsCopied(true);
    setTimeout(() => setIsCopied(false), 2000);
  };

  // Build searchable labels for each setting
  const searchableSettings = useMemo(() => ({
    theme: "theme language appearance dark light color",
    language: "language locale translation english spanish",
    format: "image format png jpg webp save output type",
    metadata: "metadata exif copy keep original",
    scale: "scale upscale multiplier 2x 3x 4x resize magnification",
    resolution: "resolution custom width output size dimension",
    compression: "compression quality lossy lossless jpg webp reduce",
    overwrite: "overwrite replace reprocess reload cache",
    saveFolder: "output folder save remember path directory",
    preserveFilename: "filename name keep original rename",
    notifications: "notifications alerts sound system popup",
    autoUpdate: "auto update upgrade version check",
    gpu: "gpu device id nvidia amd graphics card hardware",
    tileSize: "tile size segments processing memory vram",
    customModels: "custom models folder import add",
    marketplace: "marketplace store models download install",
    api: "api server http endpoint scripting port",
    tta: "tta test time augmentation quality speed",
    logs: "logs debug copy diagnostic console",
    reset: "reset settings clear restore default",
    systemInfo: "system info os platform hardware",
  }), []);

  const isSearchMatch = (keywords: string) => {
    if (!searchQuery.trim()) return true;
    const q = searchQuery.toLowerCase();
    return keywords.toLowerCase().includes(q);
  };

  const matchesSearch = (id: string) => {
    const key = id as keyof typeof searchableSettings;
    return isSearchMatch(searchableSettings[key] || id);
  };

  // When searching, show all matching settings across tabs
  const isSearching = searchQuery.trim().length > 0;

  const generalContent = (
    <div className="flex flex-col gap-5">
      <SelectTheme />
      <LanguageSwitcher />
      <TurnOffNotificationsToggle />
      <AutoUpdateToggle />
    </div>
  );

  const outputContent = (
    <div className="flex flex-col gap-5">
      <SelectImageFormat
        batchMode={batchMode}
        saveImageAs={saveImageAs}
        setExportType={setExportType}
      />
      <CopyMetadataToggle saveImageAs={saveImageAs} setExportType={setExportType} />
      <SelectImageScale scale={scale} setScale={setScale} />
      <InputCustomResolution />
      <InputCompression
        compression={compression}
        handleCompressionChange={handleCompressionChange}
      />
      <SaveOutputFolderToggle />
      <OverwriteToggle />
      <PreserveFilenameToggle />
    </div>
  );

  const performanceContent = (
    <div className="flex flex-col gap-5">
      <InputGpuId gpuId={gpuId} handleGpuIdChange={handleGpuIdChange} />
      <InputTileSize />
      <TTAModeToggle />
    </div>
  );

  const systemContent = (
    <div className="flex flex-col gap-5">
      <LogArea
        copyOnClickHandler={copyOnClickHandler}
        isCopied={isCopied}
        logData={logData}
      />
      <CustomModelsFolderSelect
        customModelsPath={customModelsPath}
        setCustomModelsPath={setCustomModelsPath}
      />
      <ApiServerToggle />
      <ResetSettingsButton />
      <SystemInfo />
    </div>
  );

  const modelsContent = (
    <div className="flex flex-col gap-5">
      <Marketplace />
      <CustomModelsManager />
    </div>
  );

  const tabContent: Record<SettingsTabId, React.ReactNode> = {
    general: generalContent,
    output: outputContent,
    performance: performanceContent,
    system: systemContent,
    models: modelsContent,
  };

  return (
    <div className="animate-step-in animate z-50 flex h-screen flex-col overflow-hidden">
      {/* Search Bar */}
      <div className="px-5 pt-4 pb-2">
        <div className="relative">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-base-content/40" />
          <input
            type="text"
            placeholder="Search settings..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="input input-sm w-full rounded-xl border border-base-300 bg-base-200 pl-9 pr-3 text-sm text-base-content placeholder-base-content/40 focus:border-red-800 focus:outline-none focus:ring-1 focus:ring-red-800/50"
          />
        </div>
      </div>

      {/* Sub-tabs */}
      {!isSearching && (
        <div className="flex gap-1 overflow-x-auto px-5 pb-3 scrollbar-thin scrollbar-thumb-base-300">
          {SETTINGS_TABS.map((tab) => {
            const Icon = tab.icon;
            return (
              <button
                key={tab.id}
                onClick={() => setActiveTab(tab.id)}
                className={cn(
                  "flex shrink-0 items-center justify-center gap-1.5 rounded-lg px-2 py-1.5 text-[0.65rem] font-medium transition-all duration-200",
                  activeTab === tab.id
                    ? "bg-red-900/40 text-red-100 shadow-sm shadow-red-900/20"
                    : "text-base-content/50 hover:bg-base-200 hover:text-base-content/70",
                )}
              >
                <Icon className="h-3.5 w-3.5" />
                {tab.label}
              </button>
            );
          })}
        </div>
      )}

      {/* Content */}
      <div className="flex-1 overflow-y-auto overflow-x-hidden px-5 pb-5">
        {isSearching ? (
          <div className="flex flex-col gap-3">
            <p className="text-xs text-base-content/40">
              Results for "{searchQuery}"
            </p>
            {matchesSearch("theme") && <SelectTheme />}
            {matchesSearch("language") && <LanguageSwitcher />}
            {matchesSearch("format") && (
              <SelectImageFormat
                batchMode={batchMode}
                saveImageAs={saveImageAs}
                setExportType={setExportType}
              />
            )}
            {matchesSearch("metadata") && (
              <CopyMetadataToggle saveImageAs={saveImageAs} setExportType={setExportType} />
            )}
            {matchesSearch("scale") && <SelectImageScale scale={scale} setScale={setScale} />}
            {matchesSearch("resolution") && <InputCustomResolution />}
            {matchesSearch("compression") && (
              <InputCompression
                compression={compression}
                handleCompressionChange={handleCompressionChange}
              />
            )}
            {matchesSearch("saveFolder") && <SaveOutputFolderToggle />}
            {matchesSearch("overwrite") && <OverwriteToggle />}
            {matchesSearch("preserveFilename") && <PreserveFilenameToggle />}
            {matchesSearch("notifications") && <TurnOffNotificationsToggle />}
            {matchesSearch("autoUpdate") && <AutoUpdateToggle />}
            {matchesSearch("gpu") && (
              <InputGpuId gpuId={gpuId} handleGpuIdChange={handleGpuIdChange} />
            )}
            {matchesSearch("tileSize") && <InputTileSize />}
            {matchesSearch("tta") && <TTAModeToggle />}
            {matchesSearch("customModels") && (
              <CustomModelsFolderSelect
                customModelsPath={customModelsPath}
                setCustomModelsPath={setCustomModelsPath}
              />
            )}
            {matchesSearch("marketplace") && <Marketplace />}
            {matchesSearch("customModels") && <CustomModelsManager />}
            {matchesSearch("api") && <ApiServerToggle />}
            {matchesSearch("logs") && (
              <LogArea
                copyOnClickHandler={copyOnClickHandler}
                isCopied={isCopied}
                logData={logData}
              />
            )}
            {matchesSearch("reset") && <ResetSettingsButton />}
            {matchesSearch("systemInfo") && <SystemInfo />}
          </div>
        ) : (
          tabContent[activeTab]
        )}
      </div>
    </div>
  );
}

export default SettingsTab;
