import clsx from "clsx";
import { AnimatePresence, m, useMotionValueEvent, useReducedMotion, useScroll, useTransform } from "motion/react";
import { ChevronLeft, ChevronRight, Upload } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Link, matchPath, useLocation, useNavigate } from "react-router";

import { useCourse } from "../lib/queries";
import { dur, spring } from "../motion/springs";
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
  return (
    <Link to="/" aria-label="notes — home" className={clsx("group flex shrink-0 items-center gap-2.5 rounded-[10px] py-1 pr-1", className)}>
      <LogoMark size={32} />
      <span className="font-display text-[20px] font-[680] tracking-[-0.02em] text-fg-1">notes</span>
    </Link>
  );
}

/** The brand mark: a small dot grid with a rising bar graph (static artwork). */
export function LogoMark({ size = 32 }: { size?: number }) {
  const lit = new Set(["2,1", "1,2", "2,2", "3,2", "0,3", "1,3", "2,3", "3,3", "4,3"]);
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" aria-hidden className="shrink-0">
      <rect width="32" height="32" rx="8" fill="var(--plate)" />
      {Array.from({ length: 4 }, (_, row) =>
        Array.from({ length: 5 }, (_, col) => {
          const on = lit.has(`${col},${row}`);
          return (
            <circle
              key={`${col},${row}`}
              cx={7 + col * 4.5}
              cy={9 + row * 4.7}
              r={on ? 1.7 : 1.4}
              fill={on ? "#4C9AFF" : "#16305F"}
              className={on ? "transition-[fill] duration-300 group-hover:fill-[#8CBEFF]" : undefined}
            />
          );
        }),
      )}
      <circle cx="25" cy="9" r="1.9" fill="#DCEBFF" />
    </svg>
  );
}
