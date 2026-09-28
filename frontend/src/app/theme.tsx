import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { flushSync } from "react-dom";

export type ThemePref = "system" | "light" | "dark";
export type Theme = "light" | "dark";

interface ThemeCtx {
  pref: ThemePref;
  theme: Theme;
  /** Change the preference; `origin` (client coords) centres the circular reveal. */
  setPref: (pref: ThemePref, origin?: { x: number; y: number }) => void;
}

const Ctx = createContext<ThemeCtx | null>(null);
const media = () => window.matchMedia("(prefers-color-scheme: dark)");
const reducedMotion = () => window.matchMedia("(prefers-reduced-motion: reduce)").matches;

function readPref(): ThemePref {
  try {
    const v = localStorage.getItem("theme");
    if (v === "light" || v === "dark" || v === "system") return v;
  } catch {
    // storage blocked
  }
  return "system";
}

function resolve(pref: ThemePref): Theme {
  return pref === "system" ? (media().matches ? "dark" : "light") : pref;
}

function apply(theme: Theme) {
  document.documentElement.setAttribute("data-theme", theme);
  document.querySelector('meta[name="theme-color"]')?.setAttribute("content", theme === "dark" ? "#05070D" : "#F6F4EE");
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [pref, setPrefState] = useState<ThemePref>(readPref);
  const [systemDark, setSystemDark] = useState(() => media().matches);
  const theme: Theme = pref === "system" ? (systemDark ? "dark" : "light") : pref;

  useEffect(() => {
    const m = media();
    const onChange = () => setSystemDark(m.matches);
    m.addEventListener("change", onChange);
    return () => m.removeEventListener("change", onChange);
  }, []);

  useEffect(() => apply(theme), [theme]);

  const setPref = useCallback((next: ThemePref, origin?: { x: number; y: number }) => {
    try {
      localStorage.setItem("theme", next);
    } catch {
      // ignore
    }
    const from = resolve(pref);
    const to = resolve(next);
    const commit = () => {
      flushSync(() => setPrefState(next));
      apply(to);
    };
    if (from === to) {
      setPrefState(next);
      return;
    }
    const doc = document as Document & { startViewTransition?: (cb: () => void) => { ready: Promise<void> } };
    if (doc.startViewTransition && !reducedMotion()) {
      const x = origin?.x ?? window.innerWidth / 2;
      const y = origin?.y ?? 0;
      const r = Math.hypot(Math.max(x, window.innerWidth - x), Math.max(y, window.innerHeight - y));
      const vt = doc.startViewTransition(commit);
      vt.ready
        .then(() => {
          document.documentElement.animate(
            { clipPath: [`circle(0px at ${x}px ${y}px)`, `circle(${r}px at ${x}px ${y}px)`] },
            { duration: 560, easing: "cubic-bezier(0.16, 1, 0.3, 1)", pseudoElement: "::view-transition-new(root)" },
          );
        })
        .catch(() => {});
    } else {
      document.documentElement.classList.add("theme-switching");
      commit();
      window.setTimeout(() => document.documentElement.classList.remove("theme-switching"), 260);
    }
  }, [pref]);

  const value = useMemo(() => ({ pref, theme, setPref }), [pref, theme, setPref]);
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useTheme(): ThemeCtx {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useTheme outside ThemeProvider");
  return ctx;
}
