import { AnimatePresence, m } from "motion/react";
import { Monitor, Moon, Sun } from "lucide-react";

import { useTheme, type ThemePref } from "../app/theme";
import { spring } from "../motion/springs";
import { Segmented } from "./Segmented";

const NEXT: Record<ThemePref, ThemePref> = { system: "light", light: "dark", dark: "system" };
const ICON = { system: Monitor, light: Sun, dark: Moon };
const NAME = { system: "System", light: "Light", dark: "Dark" };

/** Header icon button cycling System -> Light -> Dark with a circular reveal. */
export function ThemeToggle() {
  const { pref, theme, setPref } = useTheme();
  const Icon = ICON[pref];
  const label = `Theme: ${NAME[pref]}${pref === "system" ? ` (${theme})` : ""}`;
  return (
    <m.button
      type="button"
      whileTap={{ scale: 0.9 }}
      transition={spring.press}
      onClick={(e) => {
        const r = e.currentTarget.getBoundingClientRect();
        setPref(NEXT[pref], { x: r.left + r.width / 2, y: r.top + r.height / 2 });
      }}
      aria-label={label}
      title={label}
      className="relative grid h-11 w-11 place-items-center overflow-hidden rounded-[12px] text-fg-2 transition-colors hover:bg-surface-3/70 hover:text-fg-1"
    >
      <AnimatePresence mode="popLayout" initial={false}>
        <m.span
          key={pref}
          initial={{ rotate: 90, scale: 0.5, opacity: 0 }}
          animate={{ rotate: 0, scale: 1, opacity: 1 }}
          exit={{ rotate: -90, scale: 0.5, opacity: 0 }}
          transition={spring.snappy}
          className="grid place-items-center"
        >
          <Icon size={20} strokeWidth={1.75} />
        </m.span>
      </AnimatePresence>
    </m.button>
  );
}

/** Footer 3-way selector. */
export function ThemeSelect() {
  const { pref, setPref } = useTheme();
  return (
    <Segmented
      label="Theme"
      value={pref}
      onChange={(v) => setPref(v)}
      options={[
        { value: "system", label: "System", icon: <Monitor size={14} /> },
        { value: "light", label: "Light", icon: <Sun size={14} /> },
        { value: "dark", label: "Dark", icon: <Moon size={14} /> },
      ]}
    />
  );
}
