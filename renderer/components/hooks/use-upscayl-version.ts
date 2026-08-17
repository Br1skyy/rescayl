import { useState, useEffect } from "react";

const useUpscaylVersion = () => {
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    window.electron
      .getAppVersion()
      .then((appVersion) => setVersion(appVersion))
      .catch(() => setVersion(null));
  }, []);

  return version;
};

export default useUpscaylVersion;
