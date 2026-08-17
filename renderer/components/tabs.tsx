import { translationAtom } from "@/atoms/translations-atom";
import { useAtomValue } from "jotai";
import React from "react";

type TabsProps = {
  selectedTab: number;
  setSelectedTab: (tab: number) => void;
};

const Tabs = ({ selectedTab, setSelectedTab }: TabsProps) => {
  const t = useAtomValue(translationAtom);

  return (
    <div className="tabs-boxed tabs mx-auto mb-2 bg-base-200/50 p-1">
      <a
        className={`tab flex-1 transition-all duration-200 ${
          selectedTab === 0 ? "tab-active bg-red-900/40 text-red-100" : "text-base-content/60 hover:text-base-content"
        }`}
        onClick={() => setSelectedTab(0)}
      >
        {t("TITLE")}
      </a>
      <a
        className={`tab flex-1 transition-all duration-200 ${
          selectedTab === 1 ? "tab-active bg-red-900/40 text-red-100" : "text-base-content/60 hover:text-base-content"
        }`}
        onClick={() => setSelectedTab(1)}
      >
        {t("SETTINGS.TITLE")}
      </a>
    </div>
  );
};

export default Tabs;
