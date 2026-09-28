import clsx from "clsx";
import { AnimatePresence, m, useMotionValueEvent, useReducedMotion, useScroll, useTransform } from "motion/react";
import { ChevronLeft, ChevronRight, House, Upload } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Link, matchPath, useLocation, useNavigate } from "react-router";

import { LEDMatrix } from "../led/LEDMatrix";
import { formatUptime } from "../lib/format";
import { useCourse } from "../lib/queries";
import { dur, spring } from "../motion/springs";
import { useLive } from "../stats/store";
import { StatusDot } from "./Badge";
import { ThemeToggle } from "./ThemeToggle";

interface Crumb {
  key: string;
  label: string;
  to: string;
}

function useCrumbs(): { crumbs: Crumb[]; chapterPath?: string } {
  const { pathname } = useLocation();
  const course = matchPath("/c/:slug/*", pathname);
  const chapter = matchPath("/c/:slug/:chapterId/*", pathname);
  const slug = course?.params.slug ?? "";
  const { data } = useCourse(slug, slug !== "");
  const crumbs: Crumb[] = [];
  if (slug) crumbs.push({ key: "course", label: data?.name ?? "Course", to: `/c/${slug}` });
  const chapterId = chapter?.params.chapterId;
  let chapterPath: string | undefined;
  if (chapterId) {
    chapterPath = `/c/${slug}/${chapterId}`;
    const title = data?.chapters.find((c) => c.id === chapterId)?.title ?? "Chapter";
    crumbs.push({ key: "chapter", label: title, to: chapterPath });
  }
  return { crumbs, chapterPath };
}

export function Header() {
  const { crumbs, chapterPath } = useCrumbs();
  const { pathname, state } = useLocation();
  const navigate = useNavigate();
  const reduced = useReducedMotion();
  const { scrollY } = useScroll();
  const bgOpacity = useTransform(scrollY, [0, 24], [0, 1]);
  const [hidden, setHidden] = useState(false);
  const [scrolled, setScrolled] = useState(false);
  const last = useRef(0);
  const headerRef = useRef<HTMLElement>(null);

  useMotionValueEvent(scrollY, "change", (y) => {
    const delta = y - last.current;
    last.current = y;
    setScrolled(y > 200);
    if (reduced) return;
    const focusInside = headerRef.current?.contains(document.activeElement);
    if (y > 240 && delta > 12 && !focusInside) setHidden(true);
    else if (delta < 0) setHidden(false);
  });
  useEffect(() => setHidden(false), [pathname]);

  const onChapter = Boolean(chapterPath) && !pathname.endsWith("/upload");
  const parent = crumbs.length >= 2 ? crumbs[crumbs.length - 2] : undefined;
  const back = crumbs.length ? (parent ?? { key: "home", label: "Home", to: "/" }) : undefined;
  void state;

  return (
    <m.header
      ref={headerRef}
      animate={{ y: hidden ? "-100%" : "0%" }}
      transition={spring.snappy}
      className="fixed inset-x-0 top-0 z-40"
    >
      <m.div
        aria-hidden
        style={{ opacity: bgOpacity }}
        className="absolute inset-0 border-b border-line-subtle bg-[color-mix(in_oklab,var(--bg-0)_72%,transparent)] backdrop-blur-[16px] backdrop-saturate-[1.4]"
      />
      <a
        href="#main"
        className="t-label absolute left-4 top-2 z-10 -translate-y-20 rounded-[10px] bg-accent px-3 py-2 text-accent-fg focus:translate-y-0"
      >
        Skip to content
      </a>
      <div className="relative mx-auto flex h-14 max-w-[1240px] items-center gap-3 px-4 sm:px-6 md:h-16 lg:px-8">
        {/* Mobile: back crumb on inner pages, wordmark on Home */}
        {back ? (
          <Link
            to={back.to}
            className="-ml-2 flex h-11 min-w-0 items-center gap-1 rounded-[12px] pr-3 pl-1 text-fg-2 hover:text-fg-1 md:hidden"
          >
            <ChevronLeft size={20} aria-hidden />
            <span className="t-label max-w-[60vw] truncate">{back.label}</span>
          </Link>
        ) : null}
        <Wordmark className={clsx(back && "hidden md:flex")} />

        <nav aria-label="Breadcrumb" className="hidden min-w-0 md:block">
          <ol className="flex min-w-0 items-center gap-1">
            <AnimatePresence initial={false} mode="popLayout">
              {crumbs.map((c, i) => {
                const current = i === crumbs.length - 1;
                return (
                  <m.li
                    key={c.key}
                    layout
                    initial={{ x: 8, opacity: 0 }}
                    animate={{ x: 0, opacity: 1 }}
                    exit={{ x: -4, opacity: 0, transition: { duration: dur.fast } }}
                    transition={{ ...spring.snappy, delay: 0.12 }}
                    className="flex min-w-0 items-center gap-1"
                  >
                    <ChevronRight size={14} className="shrink-0 text-fg-4" aria-hidden />
                    {current ? (
                      <span
                        aria-current="page"
                        className={clsx("t-label max-w-[220px] truncate text-fg-1", scrolled && "font-[580]")}
                      >
                        {c.label}
                      </span>
                    ) : (
                      <Link to={c.to} className="t-label max-w-[220px] truncate text-fg-3 transition-colors hover:text-fg-1">
                        {c.label}
                      </Link>
                    )}
                  </m.li>
                );
              })}
            </AnimatePresence>
          </ol>
        </nav>

        <div className="ml-auto flex items-center gap-1.5">
          <LivePill />
          <AnimatePresence>
            {onChapter && scrolled && chapterPath && (
              <m.button
                type="button"
                initial={{ opacity: 0, scale: 0.9 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0, scale: 0.9 }}
                transition={spring.pop}
                whileTap={{ scale: 0.97 }}
                onClick={() => navigate(`${chapterPath}/upload`, { state: { modal: true } })}
                className="t-label relative hidden h-9 items-center gap-1.5 overflow-hidden rounded-[10px] bg-accent px-3 text-accent-fg hover:bg-accent-hover md:inline-flex"
              >
                <span className="sheen" aria-hidden />
                <Upload size={16} aria-hidden />
                Upload
              </m.button>
            )}
          </AnimatePresence>
          <ThemeToggle />
        </div>
      </div>
    </m.header>
  );
}

function Wordmark({ className }: { className?: string }) {
  const [hi, setHi] = useState<string | undefined>();
  return (
    <Link
      to="/"
      aria-label="notes — home"
      onPointerEnter={() => setHi("HI")}
      className={clsx("flex shrink-0 items-center gap-2.5 rounded-[10px] py-1 pr-1", className)}
    >
      <span className="rounded-[6px] bg-plate p-[5px] shadow-[inset_0_0_0_1px_var(--plate-bezel)]">
        <LEDMatrix source="stats" size="micro" plate={false} text={hi} onTextDone={() => setHi(undefined)} label="" />
      </span>
      <span className="font-display text-[20px] font-[680] tracking-[-0.02em] text-fg-1">notes</span>
    </Link>
  );
}

function LivePill() {
  const live = useLive();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const now = live.stats?.requests[12] ?? 0;
  useEffect(() => {
    if (!open) return;
    const close = (e: PointerEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("pointerdown", close);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("pointerdown", close);
      document.removeEventListener("keydown", esc);
    };
  }, [open]);

  return (
    <div ref={ref} className="relative hidden md:block">
      <button
        type="button"
        aria-expanded={open}
        aria-haspopup="dialog"
        onClick={() => setOpen((o) => !o)}
        className="flex h-9 items-center gap-2 rounded-full border border-line-subtle px-3 text-fg-2 transition-colors hover:border-line hover:bg-surface-3/60 hover:text-fg-1"
      >
        <span className="rounded-[5px] bg-plate px-[4px] py-[3px] leading-none">
          <LEDMatrix source="stats" size="micro" plate={false} label="" />
        </span>
        <span className="t-meta tabular-nums">{live.state === "down" ? "offline" : `${now}/s`}</span>
      </button>
      <AnimatePresence>{open && <StatsPopover />}</AnimatePresence>
    </div>
  );
}

const STATUS_TEXT = {
  waiting: "Connecting to the board…",
  ok: "All systems normal",
  warning: "Storage almost full — uploads paused",
  error: "Database not responding",
  down: "Board unreachable",
} as const;

export function statusTone(state: keyof typeof STATUS_TEXT) {
  return state === "ok" ? "ok" : state === "warning" ? "warn" : state === "waiting" ? "off" : "err";
}

function StatsPopover() {
  const live = useLive();
  const s = live.stats;
  const req = s?.requests ?? [];
  const peak = req.length ? Math.max(...req) : 0;
  const uploads = s?.uploads.reduce((a, b) => a + b, 0) ?? 0;
  return (
    <m.div
      role="dialog"
      aria-label="Live from the board"
      initial={{ opacity: 0, scale: 0.96, y: -4 }}
      animate={{ opacity: 1, scale: 1, y: 0 }}
      exit={{ opacity: 0, scale: 0.96, y: -4, transition: { duration: dur.fast } }}
      transition={spring.snappy}
      style={{ transformOrigin: "top right" }}
      className="absolute right-0 top-[calc(100%+8px)] z-50 w-[300px] rounded-[20px] border border-line bg-surface-2 p-4 shadow-[var(--shadow-3)]"
    >
      <p className="t-caption text-fg-3">Live from the board</p>
      <div className="mt-3 flex justify-center">
        <LEDMatrix source="stats" size="sm" />
      </div>
      <dl className="t-meta mt-4 grid grid-cols-2 gap-x-4 gap-y-1.5 text-fg-2">
        <dt className="text-fg-3">requests now</dt>
        <dd className="text-right text-fg-1">{req[12] ?? 0}/s</dd>
        <dt className="text-fg-3">peak (13 s)</dt>
        <dd className="text-right text-fg-1">{peak}/s</dd>
        <dt className="text-fg-3">uploads (13 s)</dt>
        <dd className="text-right text-fg-1">{uploads} photos</dd>
        <dt className="text-fg-3">uptime</dt>
        <dd className="text-right text-fg-1">{s ? formatUptime(s.uptime_secs) : "—"}</dd>
      </dl>
      <p className="t-body-s mt-3 flex items-center gap-2 text-fg-2">
        <StatusDot tone={statusTone(live.state)} pulse={live.state === "ok" && live.tick % 2 === 0} />
        {STATUS_TEXT[live.state]}
      </p>
      <p className="t-body-s mt-3 border-t border-line-subtle pt-3 text-fg-3">
        Each column is one second. Height is requests (log scale). Bright tops are uploads.
      </p>
    </m.div>
  );
}

export { STATUS_TEXT };
export const HomeIcon = House;
