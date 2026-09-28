import clsx from "clsx";
import { AnimatePresence, m } from "motion/react";
import { Check, CircleAlert, Info, TriangleAlert, X } from "lucide-react";
import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";

import { LED_COLORS } from "../led/sprites";
import { dur, spring } from "../motion/springs";

export type ToastKind = "success" | "info" | "warning" | "error";
export interface ToastInput {
  kind?: ToastKind;
  title: string;
  body?: string;
  action?: { label: string; onClick: () => void };
}
interface ToastItem extends ToastInput {
  id: number;
  kind: ToastKind;
}

const Ctx = createContext<(t: ToastInput) => void>(() => {});
export const useToast = () => useContext(Ctx);

const DURATION: Record<ToastKind, number> = { success: 4000, info: 4000, warning: 6000, error: 8000 };
const ICON = { success: Check, info: Info, warning: TriangleAlert, error: CircleAlert };
const TONE = { success: "text-ok", info: "text-accent", warning: "text-warn", error: "text-err" };

let nextId = 1;

export function ToastProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<ToastItem[]>([]);
  const push = useCallback((t: ToastInput) => {
    setItems((list) => [...list.slice(-2), { ...t, kind: t.kind ?? "info", id: nextId++ }]);
  }, []);
  const dismiss = useCallback((id: number) => setItems((list) => list.filter((t) => t.id !== id)), []);
  const value = useMemo(() => push, [push]);
  return (
    <Ctx.Provider value={value}>
      {children}
      <div
        className="pointer-events-none fixed inset-x-0 bottom-[calc(16px+env(safe-area-inset-bottom))] z-[80] flex flex-col items-center gap-2 px-4 sm:inset-x-auto sm:bottom-6 sm:right-6 sm:items-end"
        aria-live="polite"
      >
        <AnimatePresence initial={false}>
          {items.map((t, i) => (
            <ToastView key={t.id} toast={t} depth={items.length - 1 - i} onDismiss={() => dismiss(t.id)} />
          ))}
        </AnimatePresence>
      </div>
    </Ctx.Provider>
  );
}

function ToastView({ toast, depth, onDismiss }: { toast: ToastItem; depth: number; onDismiss: () => void }) {
  const Icon = ICON[toast.kind];
  const total = DURATION[toast.kind];
  const [left, setLeft] = useState(total);
  const paused = useRef(false);
  useEffect(() => {
    let last = performance.now();
    const id = window.setInterval(() => {
      const now = performance.now();
      if (!paused.current && !document.hidden) setLeft((l) => l - (now - last));
      last = now;
    }, 100);
    return () => window.clearInterval(id);
  }, []);
  useEffect(() => {
    if (left <= 0) onDismiss();
  }, [left, onDismiss]);
  const lit = Math.ceil((left / total) * 13);

  return (
    <m.div
      layout
      role={toast.kind === "error" ? "alert" : "status"}
      initial={{ y: 24, scale: 0.96, opacity: 0 }}
      animate={{ y: 0, scale: 1 - depth * 0.03, opacity: 1 }}
      exit={{ x: 40, opacity: 0, transition: { duration: dur.fast } }}
      transition={spring.ui}
      drag="x"
      dragConstraints={{ left: 0, right: 0 }}
      dragElastic={0.6}
      onDragEnd={(_, info) => {
        if (Math.abs(info.offset.x) > 80 || Math.abs(info.velocity.x) > 500) onDismiss();
      }}
      onPointerEnter={() => (paused.current = true)}
      onPointerLeave={() => (paused.current = false)}
      onFocus={() => (paused.current = true)}
      onBlur={() => (paused.current = false)}
      className="pointer-events-auto relative w-full max-w-[420px] overflow-hidden rounded-[14px] border border-line bg-surface-2 px-3.5 py-3 shadow-[var(--shadow-3)] sm:w-[380px]"
    >
      <div className="flex items-start gap-3">
        <Icon size={18} className={clsx("mt-0.5 shrink-0", TONE[toast.kind])} aria-hidden />
        <div className="min-w-0 flex-1">
          <p className="t-label text-fg-1">{toast.title}</p>
          {toast.body && <p className="t-body-s mt-0.5 text-fg-2">{toast.body}</p>}
        </div>
        {toast.action && (
          <button
            type="button"
            onClick={() => {
              toast.action?.onClick();
              onDismiss();
            }}
            className="t-label shrink-0 rounded-[8px] px-2 py-1 text-accent hover:bg-accent-soft"
          >
            {toast.action.label}
          </button>
        )}
        <button
          type="button"
          onClick={onDismiss}
          aria-label="Dismiss"
          className="-mr-1 -mt-0.5 shrink-0 rounded-[8px] p-1 text-fg-3 hover:bg-surface-3 hover:text-fg-1"
        >
          <X size={16} />
        </button>
      </div>
      {/* 13 LED dots that go out right to left as time passes */}
      <div className="absolute inset-x-3.5 bottom-1 flex justify-between" aria-hidden>
        {Array.from({ length: 13 }, (_, i) => (
          <span
            key={i}
            className="h-[3px] w-[3px] rounded-full"
            style={{ background: i < lit ? LED_COLORS[5] : LED_COLORS[1] }}
          />
        ))}
      </div>
    </m.div>
  );
}
