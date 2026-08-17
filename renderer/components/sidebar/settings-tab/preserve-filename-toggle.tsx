import { translationAtom } from "@/atoms/translations-atom";
import { preserveFilenameAtom } from "@/atoms/user-settings-atom";
import { useAtom, useAtomValue } from "jotai";

const PreserveFilenameToggle = () => {
  const [preserveFilename, setPreserveFilename] = useAtom(
    preserveFilenameAtom,
  );
  const t = useAtomValue(translationAtom);

  return (
    <div className="flex flex-col gap-2">
      <p className="text-sm font-medium">
        {t("SETTINGS.PRESERVE_FILENAME.TITLE")}
      </p>
      <p className="text-xs text-base-content/80">
        {t("SETTINGS.PRESERVE_FILENAME.DESCRIPTION")}
      </p>
      <input
        type="checkbox"
        className="toggle"
        checked={preserveFilename}
        onClick={() => {
          setPreserveFilename(!preserveFilename);
        }}
      />
    </div>
  );
};

export default PreserveFilenameToggle;