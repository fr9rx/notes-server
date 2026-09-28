// Shared page-transition plumbing (DESIGN.md §3.6).
//
// Each route renders <Page>, whose root owns the variant *state*
// (hidden → show → exit). Children marked <PageItem> only declare variants, so
// they inherit the state and stagger in by index. The page root itself never
// fades, so shared elements (layoutId) are never dimmed mid-morph.
import { m, useReducedMotion, type Variants } from "motion/react";
import { createContext, useContext, type ReactNode } from "react";

import { dur, ease, spring } from "../motion/springs";

/** +1 when navigating deeper, -1 when going back up, 0 for unrelated jumps. */
export const DirectionContext = createContext(1);
export const useDirection = () => useContext(DirectionContext);

interface Custom {
  dir: number;
  i: number;
}

const item: Variants = {
  hidden: ({ dir }: Custom) => ({ opacity: 0, y: dir >= 0 ? 16 : -10, filter: "blur(4px)" }),
  show: ({ i }: Custom) => ({
    opacity: 1,
    y: 0,
    filter: "blur(0px)",
    transition: {
      ...spring.soft,
      delay: 0.08 + Math.min(i, 10) * 0.04,
      filter: { duration: dur.slow, ease: ease.outExpo, delay: 0.08 + Math.min(i, 10) * 0.04 },
    },
  }),
  exit: ({ dir }: Custom) => ({ opacity: 0, y: dir >= 0 ? -8 : 12, transition: { duration: 0.16, ease: ease.in } }),
};

const fade: Variants = {
  hidden: { opacity: 0 },
  show: { opacity: 1, transition: { duration: dur.base, ease: ease.inOut } },
  exit: { opacity: 0, transition: { duration: dur.fast } },
};

const root: Variants = { hidden: {}, show: {}, exit: {} };

export function Page({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <m.div className={className} variants={root} initial="hidden" animate="show" exit="exit">
      {children}
    </m.div>
  );
}

export function PageItem({
  i = 0,
  children,
  className,
  as = "div",
}: {
  i?: number;
  children: ReactNode;
  className?: string;
  as?: "div" | "section" | "header" | "ol" | "li";
}) {
  const dir = useDirection();
  const reduced = useReducedMotion();
  const C = m[as];
  return (
    <C className={className} custom={{ dir, i }} variants={reduced ? fade : item}>
      {children}
    </C>
  );
}

export function usePageItemVariants() {
  const reduced = useReducedMotion();
  return reduced ? fade : item;
}
