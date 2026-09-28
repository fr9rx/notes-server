import clsx from "clsx";
import { m } from "motion/react";
import type { ReactNode } from "react";

import { spring } from "../motion/springs";

export type BadgeVariant = "neutral" | "accent" | "new" | "glass" | "cover";

const styles: Record<BadgeVariant, string> = {
  neutral: "bg-surface-3 text-fg-2",
  accent: "bg-accent-soft text-accent",
  new: "bg-hi text-[#1a1400]",
  cover: "bg-hi text-[#1a1400]",
  glass: "bg-[rgba(5,7,13,0.55)] text-white backdrop-blur-md",
};

export function Badge({
  variant = "neutral",
  icon,
  children,
  className,
  pop,
}: {
  variant?: BadgeVariant;
  icon?: ReactNode;
  children: ReactNode;
  className?: string;
  pop?: boolean;
}) {
  return (
    <m.span
      initial={pop ? { scale: 0.6, opacity: 0 } : false}
      animate={{ scale: 1, opacity: 1 }}
      transition={spring.pop}
      className={clsx(
        "t-caption inline-flex h-[22px] items-center gap-1 rounded-[6px] px-2 leading-none",
        styles[variant],
        className,
      )}
    >
      {icon}
      {children}
    </m.span>
  );
}

/** A small LED status dot (§6.3 `status` badge). */
export function StatusDot({ tone, pulse }: { tone: "ok" | "warn" | "err" | "off"; pulse?: boolean }) {
  const color = { ok: "var(--accent)", warn: "var(--warning)", err: "var(--danger)", off: "var(--text-4)" }[tone];
  return (
    <span className="relative inline-flex h-1.5 w-1.5" aria-hidden>
      {pulse && (
        <m.span
          key={String(pulse)}
          className="absolute inset-0 rounded-full"
          style={{ background: color }}
          initial={{ scale: 1, opacity: 0.8 }}
          animate={{ scale: 3, opacity: 0 }}
          transition={{ duration: 0.8, ease: [0.16, 1, 0.3, 1] }}
        />
      )}
      <span className="relative h-1.5 w-1.5 rounded-full" style={{ background: color, boxShadow: `0 0 8px ${color}` }} />
    </span>
  );
}
