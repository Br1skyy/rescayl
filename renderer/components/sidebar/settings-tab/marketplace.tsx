"use client";

import { ELECTRON_COMMANDS } from "@common/electron-commands";
import { useEffect, useState } from "react";

type MarketplaceModel = {
  id: string;
  name: string;
  author: string;
  scale: number;
  description: string;
  size: string;
  downloadUrl: string;
  previewUrl: string;
  tags: string[];
  downloads: number;
  rating: number;
};

type ProgressPayload = {
  modelId: string;
  file: string;
  progress: number;
  downloaded?: number;
  total?: number;
};

type CustomModelEntry = {
  id: string;
  directory: string;
  metadata: {
    name: string;
    scale: number;
  };
};

export function Marketplace() {
  const [models, setModels] = useState<MarketplaceModel[]>([]);
  const [progress, setProgress] = useState<Record<string, number>>({});
  const [installing, setInstalling] = useState<Record<string, boolean>>({});
  const [installed, setInstalled] = useState<Record<string, boolean>>({});
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  const loadInstalled = () => {
    window.electron
      .invoke(ELECTRON_COMMANDS.GET_CUSTOM_MODELS)
      .then((list) => {
        const installedMap: Record<string, boolean> = {};
        (list as CustomModelEntry[] | null | undefined)?.forEach((m) => {
          if (m?.id) installedMap[m.id] = true;
        });
        setInstalled(installedMap);
      })
      .catch(() => setInstalled({}));
  };

  const fetchManifest = () => {
    setLoading(true);
    setError("");
    window.electron
      .invoke(ELECTRON_COMMANDS.FETCH_MARKETPLACE)
      .then((manifest) => setModels(manifest?.models ?? []))
      .catch((err) => setError(String(err)))
      .finally(() => setLoading(false));
    loadInstalled();
  };

  useEffect(() => {
    fetchManifest();
    const progressHandler = (_: unknown, data: ProgressPayload) => {
      setProgress((prev) => ({ ...prev, [data.modelId]: data.progress }));
    };
    const doneHandler = (_: unknown, modelId: string) => {
      setInstalling((prev) => ({ ...prev, [modelId]: false }));
      setProgress((prev) => ({ ...prev, [modelId]: 100 }));
    };
    const errorHandler = (_: unknown, data: ProgressPayload) => {
      setInstalling((prev) => ({ ...prev, [data.modelId]: false }));
      setProgress((prev) => ({ ...prev, [data.modelId]: 0 }));
    };
    window.electron.on(
      ELECTRON_COMMANDS.MARKETPLACE_DOWNLOAD_PROGRESS,
      progressHandler,
    );
    window.electron.on(ELECTRON_COMMANDS.MARKETPLACE_DOWNLOAD_DONE, doneHandler);
    window.electron.on(
      ELECTRON_COMMANDS.MARKETPLACE_DOWNLOAD_ERROR,
      errorHandler,
    );
    return () => {
      window.electron.off(
        ELECTRON_COMMANDS.MARKETPLACE_DOWNLOAD_PROGRESS,
        progressHandler,
      );
      window.electron.off(ELECTRON_COMMANDS.MARKETPLACE_DOWNLOAD_DONE, doneHandler);
      window.electron.off(
        ELECTRON_COMMANDS.MARKETPLACE_DOWNLOAD_ERROR,
        errorHandler,
      );
    };
  }, []);

  const install = async (modelId: string) => {
    setInstalling((prev) => ({ ...prev, [modelId]: true }));
    setProgress((prev) => ({ ...prev, [modelId]: 0 }));
    try {
      await window.electron.invoke(
        ELECTRON_COMMANDS.DOWNLOAD_MARKETPLACE_MODEL,
        modelId,
      );
      window.electron.send(ELECTRON_COMMANDS.SCAN_CUSTOM_MODELS);
      loadInstalled();
    } catch (err) {
      console.error("Marketplace install error:", err);
      setProgress((prev) => ({ ...prev, [modelId]: 0 }));
    }
    setInstalling((prev) => ({ ...prev, [modelId]: false }));
  };

  const uninstall = async (modelId: string) => {
    if (
      !window.confirm(
        `Uninstall "${modelId}"? This deletes its downloaded files.`,
      )
    ) {
      return;
    }
    setInstalling((prev) => ({ ...prev, [modelId]: true }));
    try {
      await window.electron.invoke(
        ELECTRON_COMMANDS.UNINSTALL_CUSTOM_MODEL,
        modelId,
      );
      loadInstalled();
    } catch (err) {
      console.error("Marketplace uninstall error:", err);
    }
    setInstalling((prev) => ({ ...prev, [modelId]: false }));
  };

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-2">
        <p className="text-sm font-medium">Model Marketplace</p>
        <button className="btn btn-sm btn-secondary" onClick={fetchManifest}>
          Refresh
        </button>
      </div>
      <p className="text-xs text-base-content/80">
        Install community models with a single click. Downloads go into your
        custom models directory and appear in the model picker automatically.
      </p>

      {loading && (
        <p className="text-xs text-base-content/50">Loading models...</p>
      )}
      {error && <p className="text-xs text-red-400">{error}</p>}

      {!loading && models.length === 0 && !error && (
        <p className="text-xs text-base-content/50">
          No models available.
        </p>
      )}

      <div className="flex flex-col gap-3">
        {models.map((model) => {
          const isInstalling = installing[model.id];
          const modelProgress = progress[model.id] ?? 0;
          return (
            <div
              key={model.id}
              className="flex flex-col gap-2 rounded-lg border border-base-300 bg-base-200 p-3"
            >
              <div className="flex items-start gap-3">
                {model.previewUrl && (
                  <img
                    src={model.previewUrl}
                    alt={model.name}
                    className="h-16 w-16 shrink-0 rounded object-cover"
                  />
                )}
                <div className="flex min-w-0 flex-col">
                  <p className="truncate text-sm font-semibold">{model.name}</p>
                  <p className="truncate text-xs text-base-content/60">
                    {model.author}
                  </p>
                  <div className="mt-1 flex gap-1">
                    <span className="rounded bg-red-900/30 px-1.5 py-0.5 text-[0.65rem] font-semibold text-red-100">
                      {model.scale}x
                    </span>
                    <span className="rounded bg-base-300 px-1.5 py-0.5 text-[0.65rem] text-base-content/70">
                      {model.size}
                    </span>
                    <span className="rounded bg-base-300 px-1.5 py-0.5 text-[0.65rem] text-base-content/70">
                      {model.downloads.toLocaleString()} downloads
                    </span>
                  </div>
                </div>
              </div>
              <p className="text-xs leading-normal text-base-content/70">
                {model.description}
              </p>
              <div className="flex items-center gap-2">
                {installed[model.id] ? (
                  <button
                    className="btn btn-sm btn-secondary self-start"
                    disabled={isInstalling}
                    onClick={() => uninstall(model.id)}
                  >
                    {isInstalling ? "Removing..." : "Uninstall"}
                  </button>
                ) : (
                  <button
                    className="btn btn-sm btn-primary self-start"
                    disabled={isInstalling}
                    onClick={() => install(model.id)}
                  >
                    {isInstalling ? "Installing..." : "Install"}
                  </button>
                )}
                {installed[model.id] && (
                  <span className="text-xs font-medium text-green-400">
                    Installed
                  </span>
                )}
              </div>
              {isInstalling && modelProgress > 0 && (
                <progress
                  className="progress progress-primary h-2 w-full"
                  value={modelProgress}
                  max={100}
                />
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}