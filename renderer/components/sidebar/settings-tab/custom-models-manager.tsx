"use client";

import { ELECTRON_COMMANDS } from "@common/electron-commands";
import { useEffect, useState } from "react";

export type CustomModel = {
  id: string;
  directory: string;
  metadata: {
    name: string;
    description: string;
    scale: number;
    tags: string[];
  };
};

export function CustomModelsManager() {
  const [models, setModels] = useState<CustomModel[]>([]);
  const [directory, setDirectory] = useState("");

  const load = () => {
    window.electron
      .invoke(ELECTRON_COMMANDS.GET_CUSTOM_MODELS_DIR)
      .then((dir) => setDirectory(dir))
      .catch(() => setDirectory(""));
    window.electron
      .invoke(ELECTRON_COMMANDS.GET_CUSTOM_MODELS)
      .then((list) => setModels(list ?? []))
      .catch(() => setModels([]));
  };

  useEffect(() => {
    load();
    const handler = (_: unknown, data: CustomModel[]) => {
      setModels(data ?? []);
    };
    window.electron.on(ELECTRON_COMMANDS.CUSTOM_MODELS_UPDATED, handler);
    return () => {
      window.electron.off(ELECTRON_COMMANDS.CUSTOM_MODELS_UPDATED, handler);
    };
  }, []);

  const rescan = () => {
    window.electron.send(ELECTRON_COMMANDS.SCAN_CUSTOM_MODELS);
  };

  const openFolder = () => {
    window.electron.send(ELECTRON_COMMANDS.OPEN_CUSTOM_MODELS_FOLDER);
  };

  return (
    <div className="flex flex-col gap-3">
      <p className="text-sm font-medium">Installed Custom Models</p>
      <p className="text-xs text-base-content/80">
        Models are discovered automatically from the folder below. Use Rescan
        after adding files manually. Models installed from the marketplace
        appear here automatically.
      </p>
      <p className="break-all text-xs text-base-content/60">{directory}</p>
      <div className="flex gap-2">
        <button className="btn btn-sm btn-primary" onClick={rescan}>
          Rescan
        </button>
        <button className="btn btn-sm btn-secondary" onClick={openFolder}>
          Open Folder
        </button>
      </div>
      {models.length > 0 && (
        <div className="flex flex-col gap-2">
          {models.map((model) => (
            <div
              key={model.id}
              className="flex items-center justify-between rounded-lg border border-base-300 bg-base-200 px-3 py-2"
            >
              <div className="flex min-w-0 flex-col">
                <p className="truncate text-sm font-medium">
                  {model.metadata.name || model.id}
                </p>
                <p className="truncate text-xs text-base-content/60">
                  {model.id}
                </p>
              </div>
              <span className="ml-2 shrink-0 rounded bg-red-900/30 px-2 py-0.5 text-xs font-semibold text-red-100">
                {model.metadata.scale}x
              </span>
            </div>
          ))}
        </div>
      )}
      {models.length === 0 && (
        <p className="text-xs text-base-content/50">
          No custom models found yet.
        </p>
      )}
    </div>
  );
}