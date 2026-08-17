"use client";

import { useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Eraser, Info } from "lucide-react";
import { useAtom } from "jotai";
import {
  bgRemovalModeAtom,
  BgRemovalMode,
} from "@/atoms/user-settings-atom";
import useTranslation from "@/components/hooks/use-translation";

const BG_REMOVAL_MODES: BgRemovalMode[] = ["off", "before", "after"];

const SelectBgRemovalDialog = () => {
  const t = useTranslation();
  const [bgRemovalMode, setBgRemovalMode] = useAtom(bgRemovalModeAtom);
  const [open, setOpen] = useState(false);

  const modeLabel = t(
    `APP.BG_REMOVAL.OPTIONS.${bgRemovalMode.toUpperCase()}` as any,
  );

  const handleModeSelect = (mode: BgRemovalMode) => {
    setBgRemovalMode(mode);
    if (mode !== "off") {
      setOpen(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <button className="btn btn-primary justify-start border-border w-full min-w-0 overflow-hidden">
          <Eraser className="mr-2 h-5 w-5 shrink-0" />
          <span className="min-w-0 truncate text-xs whitespace-nowrap">
            {t("APP.BG_REMOVAL.TITLE")}: {modeLabel}
          </span>
        </button>
      </DialogTrigger>
      <DialogContent className="z-50 sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{t("APP.BG_REMOVAL.DESCRIPTION")}</DialogTitle>
        </DialogHeader>
        <ScrollArea className="max-h-[500px] pr-4">
          <div className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <p className="text-sm font-medium">{t("APP.BG_REMOVAL.TITLE")}</p>
              <div className="flex flex-col gap-1">
                {BG_REMOVAL_MODES.map((mode) => (
                  <button
                    key={mode}
                    className={`btn btn-sm btn-outline justify-start ${
                      bgRemovalMode === mode ? "btn-primary" : ""
                    }`}
                    onClick={() => handleModeSelect(mode)}
                  >
                    {t(`APP.BG_REMOVAL.OPTIONS.${mode.toUpperCase()}` as any)}
                  </button>
                ))}
              </div>
            </div>

            {bgRemovalMode !== "off" && (
              <>
                <div className="border-t border-base-300" />
                <div className="flex items-start gap-2 rounded-sm bg-base-200 p-3 text-xs text-base-content/80">
                  <Info className="mt-0.5 h-4 w-4 shrink-0" />
                  <p>{t("APP.BG_REMOVAL.REQUIRES_PYTHON")}</p>
                </div>
              </>
            )}
          </div>
        </ScrollArea>
      </DialogContent>
    </Dialog>
  );
};

export default SelectBgRemovalDialog;
