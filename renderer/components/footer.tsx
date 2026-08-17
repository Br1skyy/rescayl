import { translationAtom } from "@/atoms/translations-atom";
import { useAtomValue } from "jotai";
import React from "react";

function Footer() {
  const t = useAtomValue(translationAtom);

  return (
    <div className="p-2 text-center text-[0.65rem] text-base-content/40">
      <p>
        {t("FOOTER.COPYRIGHT")} {new Date().getFullYear()} -{" "}
        <span className="font-semibold text-red-400/70">{t("TITLE")}</span>
      </p>
    </div>
  );
}

export default Footer;
