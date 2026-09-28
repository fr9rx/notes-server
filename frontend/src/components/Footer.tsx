import { m } from "motion/react";
import { useState } from "react";

import { LEDMatrix } from "../led/LEDMatrix";
import { formatNumber, formatUptime } from "../lib/format";
import { useLive } from "../stats/store";
import { StatusDot } from "./Badge";
import { STATUS_TEXT, statusTone } from "./Header";
import { ThemeSelect } from "./ThemeToggle";

export function Footer() {
  const live = useLive();
  const s = live.stats;
  const [intro, setIntro] = useState<string | undefined>("NOTES SERVER");
  const [seen, setSeen] = useState(false);
  return (
    <footer className="mt-24 border-t border-line-subtle bg-bg-1 pb-[calc(32px+env(safe-area-inset-bottom))] pt-12">
      <div className="mx-auto flex max-w-[1240px] flex-col gap-8 px-4 sm:px-6 md:flex-row md:items-center md:justify-between lg:px-8">
        <m.div
          className="flex flex-col gap-5 sm:flex-row sm:items-center"
          onViewportEnter={() => setSeen(true)}
          viewport={{ once: true }}
        >
          <LEDMatrix
            source="stats"
            size="sm"
            text={seen ? intro : undefined}
            onTextDone={() => setIntro(undefined)}
            className="self-start"
          />
          <div className="t-meta space-y-1.5 text-fg-3">
            <p className="text-fg-2">Served from an Arduino UNO Q on your network</p>
            <p>
              {s
                ? `Up ${formatUptime(s.uptime_secs)} · ${formatNumber(s.courses)} courses · ${formatNumber(s.images)} photos`
                : "Connecting…"}
            </p>
            <p className="flex items-center gap-2">
              <StatusDot tone={statusTone(live.state)} />
              {STATUS_TEXT[live.state]}
            </p>
          </div>
        </m.div>
        <div className="flex flex-col items-start gap-3 md:items-end">
          <ThemeSelect />
          <p className="t-body-s text-fg-3">No accounts. No cloud. Just notes.</p>
        </div>
      </div>
    </footer>
  );
}
