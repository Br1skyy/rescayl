import { themeAtom } from "@/atoms/user-settings-atom";
import { translationAtom } from "@/atoms/translations-atom";
import { useAtom, useAtomValue } from "jotai";
import React, { useEffect } from "react";

const availableThemes = [
  "rescayl",
  "ocean",
  "forest",
  "violet",
  "ember",
  "mocha",
  "alpine",
  "dawn",
  "rose",
  "sage",
  "sky",
  "synthwave",
  "dark",
  "halloween",
];

const SelectTheme = ({ hideLabel }: { hideLabel?: boolean }) => {
  const t = useAtomValue(translationAtom);
  const [theme, setTheme] = useAtom(themeAtom);

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
  }, [theme]);

  return (
    <div className="flex w-full flex-col gap-2">
      {!hideLabel && (
        <p className="text-sm font-medium">{t("SETTINGS.THEME.TITLE")}</p>
      )}
      <select
        className="select select-primary"
        value={theme}
        onChange={(e) => setTheme(e.target.value)}
      >
        {availableThemes.map((theme) => {
          return (
            <option value={theme} key={theme}>
              {theme.toLocaleUpperCase()}
            </option>
          );
        })}
      </select>
    </div>
  );
};

export default SelectTheme;