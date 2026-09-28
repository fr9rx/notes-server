import clsx from "clsx";
import { m } from "motion/react";
import { useId, useRef, type KeyboardEvent } from "react";

import { spring } from "../motion/springs";

/** Segmented control with a sliding pill (DESIGN.md §6.17). */
export function Segmented<T extends string>({
  value,
  options,
  onChange,
  label,
  className,
}: {
  value: T;
  options: { value: T; label: string; icon?: React.ReactNode }[];
  onChange: (v: T) => void;
  label: string;
  className?: string;
}) {
  const id = useId();
  const refs = useRef<(HTMLButtonElement | null)[]>([]);
  const onKey = (e: KeyboardEvent) => {
    const i = options.findIndex((o) => o.value === value);
    const delta = e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
    if (!delta) return;
    e.preventDefault();
    const next = options[(i + delta + options.length) % options.length];
    if (next) {
      onChange(next.value);
      refs.current[options.indexOf(next)]?.focus();
    }
  };
  return (
    <div
      role="radiogroup"
      aria-label={label}
      onKeyDown={onKey}
      className={clsx("inline-flex h-9 rounded-[10px] border border-line-subtle bg-surface-2 p-[3px]", className)}
    >
      {options.map((o, i) => {
        const active = o.value === value;
        return (
          <button
            key={o.value}
            ref={(el) => {
              refs.current[i] = el;
            }}
            type="button"
            role="radio"
            aria-checked={active}
            tabIndex={active ? 0 : -1}
            onClick={() => onChange(o.value)}
            className={clsx(
              "relative flex items-center gap-1.5 rounded-[7px] px-3 text-[13px] font-[540] transition-colors duration-150",
              active ? "text-fg-1" : "text-fg-3 hover:text-fg-1",
            )}
          >
            {active && (
              <m.span
                layoutId={`seg-pill-${id}`}
                transition={spring.snappy}
                className="absolute inset-0 rounded-[7px] bg-white shadow-[var(--shadow-1)] dark:bg-surface-3"
              />
            )}
            <span className="relative flex items-center gap-1.5">
              {o.icon}
              {o.label}
            </span>
          </button>
        );
      })}
    </div>
  );
}
