import { FEATURE_FLAGS } from "@common/feature-flags";
import React from "react";
import RescaylLogo from "@/components/icons/rescayl-logo";
import { useAtomValue } from "jotai";
import { translationAtom } from "@/atoms/translations-atom";

export default function Header({ version }: { version: string }) {
  const t = useAtomValue(translationAtom);

  return (
    <div className="outline-none">
      <div className="flex items-center gap-3 px-5 py-4">
        <RescaylLogo className="inline-block h-12 w-12" />
        <div className="flex flex-col justify-center">
          <h1 className="text-2xl font-bold tracking-tight text-red-100">
            {t("TITLE")}{" "}
            <span className="text-[0.65rem] font-normal text-base-content/50">
              {version} {FEATURE_FLAGS.APP_STORE_BUILD && "Mac"}
            </span>
          </h1>
          <p className="text-xs text-base-content/50">{t("HEADER.DESCRIPTION")}</p>
        </div>
      </div>
      <div className="mx-5 h-px bg-gradient-to-r from-red-900/60 via-red-800/30 to-transparent" />
    </div>
  );
}
