import { m } from "motion/react";
import type { ReactNode } from "react";

import { LEDMatrix, type LEDPattern } from "../led/LEDMatrix";
import { spring } from "../motion/springs";

/** Empty / error / not-found panel with an LED plate (DESIGN.md §6.18). */
export function StatePanel({
  pattern,
  title,
  children,
  actions,
  titleAs: Title = "h2",
}: {
  pattern: LEDPattern;
  title: string;
  children?: ReactNode;
  actions?: ReactNode;
  titleAs?: "h1" | "h2";
}) {
  return (
    <div className="mx-auto flex max-w-[420px] flex-col items-center px-4 py-16 text-center">
      <m.div initial={{ scale: 0.94, opacity: 0 }} animate={{ scale: 1, opacity: 1 }} transition={spring.soft}>
        <LEDMatrix size="md" pattern={pattern} />
      </m.div>
      <m.div
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ ...spring.soft, delay: 0.12 }}
        className="mt-8"
      >
        <Title className="t-title-l text-fg-1">{title}</Title>
        {children && <div className="t-body mt-3 text-fg-2">{children}</div>}
        {actions && <div className="mt-6 flex flex-wrap justify-center gap-3">{actions}</div>}
      </m.div>
    </div>
  );
}
