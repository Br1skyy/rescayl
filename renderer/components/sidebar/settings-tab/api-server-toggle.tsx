"use client";

import { ELECTRON_COMMANDS } from "@common/electron-commands";
import { useEffect, useState } from "react";

export function ApiServerToggle() {
  const [enabled, setEnabled] = useState(false);
  const [port, setPort] = useState(7860);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    window.electron
      .invoke(ELECTRON_COMMANDS.GET_API_STATUS)
      .then((status) => {
        setEnabled(status.enabled);
        setPort(status.port);
        setLoaded(true);
      })
      .catch((error) => console.error("API status error:", error));
  }, []);

  const toggle = () => {
    window.electron.send(ELECTRON_COMMANDS.SET_API_ENABLED, {
      enabled: !enabled,
      port,
    });
    setEnabled(!enabled);
  };

  return (
    <div className="flex flex-col gap-2">
      <p className="text-sm font-medium">Local API Server</p>
      <p className="text-xs text-base-content/80">
        Exposes HTTP endpoints for scripting and automation. When disabled,
        requests return HTTP 503.
      </p>
      <p className="text-xs text-base-content/60">
        Endpoint:{" "}
        <span className="font-mono">http://127.0.0.1:{port}</span>
      </p>
      {loaded && (
        <input
          type="checkbox"
          className="toggle"
          checked={enabled}
          onClick={toggle}
        />
      )}
    </div>
  );
}