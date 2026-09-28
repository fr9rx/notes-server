import clsx from "clsx";
import { m, useInView, useMotionValueEvent, useReducedMotion, useSpring } from "motion/react";
import { useEffect, useRef, useState } from "react";

import { formatNumber } from "../lib/format";
import { counterSpring, spring } from "../motion/springs";

/** One rolling digit: a 0-9 strip in a 1em window. */
function Digit({ d }: { d: number }) {
  return (
    <span className="relative inline-block h-[1em] w-[0.62em] overflow-hidden align-top">
      <m.span
        className="absolute left-0 top-0 flex flex-col"
        animate={{ y: `${-d}em` }}
        transition={spring.snappy}
        aria-hidden
      >
        {Array.from({ length: 10 }, (_, i) => (
          <span key={i} className="block h-[1em] leading-none">
            {i}
          </span>
        ))}
      </m.span>
    </span>
  );
}

/** A number that counts up once in view, then rolls its digits on change. */
export function Counter({ value, label, className }: { value: number; label: string; className?: string }) {
  const ref = useRef<HTMLDivElement>(null);
  const inView = useInView(ref, { once: true, amount: 0.4 });
  const reduced = useReducedMotion();
  const mv = useSpring(0, counterSpring);
  const [shown, setShown] = useState(reduced ? value : 0);
  const [flash, setFlash] = useState(0);
  const first = useRef(true);

  useMotionValueEvent(mv, "change", (v) => setShown(Math.round(v)));
  useEffect(() => {
    if (reduced) {
      setShown(value);
      return;
    }
    if (!inView) return;
    if (!first.current) setFlash((f) => f + 1);
    first.current = false;
    mv.set(value);
  }, [value, inView, reduced, mv]);

  const text = formatNumber(shown);
  return (
    <div ref={ref} className={clsx("flex flex-col gap-2", className)}>
      <span className="t-counter text-fg-1" aria-hidden>
        {[...text].map((ch, i) =>
          /\d/.test(ch) ? <Digit key={text.length - i} d={Number(ch)} /> : <span key={`s${i}`}>{ch}</span>,
        )}
      </span>
      <span className="sr-only">{formatNumber(value)}</span>
      <m.span
        key={flash}
        className="t-caption"
        initial={flash ? { color: "var(--accent)" } : false}
        animate={{ color: "var(--text-3)" }}
        transition={{ duration: 0.6, ease: "linear" }}
      >
        {label}
      </m.span>
    </div>
  );
}
