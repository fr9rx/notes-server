import clsx from "clsx";
import { useReducedMotion } from "motion/react";
import { useEffect, useState } from "react";

import { LED_COLORS } from "../led/sprites";
import { loaderLevels } from "../led/views";

/** A row of LED dots running the firmware comet in 1D (DESIGN.md §6.13). */
export function LEDLoader({
  dots = 13,
  size = 5,
  gap = 4,
  className,
  label,
}: {
  dots?: number;
  size?: number;
  gap?: number;
  className?: string;
  label?: string;
}) {
  const reduced = useReducedMotion();
  const [now, setNow] = useState(0);
  useEffect(() => {
    if (reduced) return;
    let raf = 0;
    const start = performance.now();
    const loop = (t: number) => {
      setNow(t - start);
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  }, [reduced]);
  const levels = reduced ? new Array<number>(dots).fill(1) : loaderLevels(dots, now);
  return (
    <span
      className={clsx("inline-flex items-center", className)}
      style={{ gap }}
      role={label ? "status" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
    >
      {levels.map((lv, i) => (
        <span
          key={i}
          className="rounded-full"
          style={{
            width: size,
            height: size,
            background: LED_COLORS[lv],
            boxShadow: lv >= 5 ? `0 0 ${size}px rgba(76,154,255,0.5)` : undefined,
          }}
        />
      ))}
    </span>
  );
}
