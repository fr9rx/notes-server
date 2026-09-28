import clsx from "clsx";
import { AnimatePresence, m, useMotionValueEvent, useReducedMotion, useScroll, useTransform } from "motion/react";
import { AlignLeft, ArrowDownWideNarrow, ArrowUpNarrowWide, Images, Upload } from "lucide-react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState, type DragEvent } from "react";
import { useNavigate, useParams } from "react-router";

import { Page, PageItem, useDirection } from "../app/page";
import { Badge } from "../components/Badge";
import { Button, ButtonLink } from "../components/Button";
import { ImageTile } from "../components/ImageTile";
import { LEDLoader } from "../components/LEDLoader";
import { Segmented } from "../components/Segmented";
import { DotShimmer, Delayed, TextSkeleton } from "../components/Skeleton";
import { StatePanel } from "../components/StatePanel";
import { LED_COLORS } from "../led/sprites";
import { ApiError } from "../lib/api";
import { fullDate, isRecent, pad2, plural, relativeTime } from "../lib/format";
import { useChapterNotes, useCourse, type SortOrder } from "../lib/queries";
import type { Note } from "../lib/types";
import { dur, ease, gridDelay, spring } from "../motion/springs";
import { isJustPosted, preloadModals, setPendingFiles } from "../upload/pending";

const TEXT_BLOCK = 76; // title (2 lines) + meta, fixed so layout is final before images load
const GAP_MOBILE = 10;
const GAP = 16;

function useMedia(query: string) {
  const [match, setMatch] = useState(() => window.matchMedia(query).matches);
  useEffect(() => {
    const m = window.matchMedia(query);
    const on = () => setMatch(m.matches);
    m.addEventListener("change", on);
    return () => m.removeEventListener("change", on);
  }, [query]);
  return match;
}

function readOrder(): SortOrder {
  try {
    return sessionStorage.getItem("notes.order") === "asc" ? "asc" : "desc";
  } catch {
    return "desc";
  }
}

export default function ChapterPage() {
  const { slug = "", chapterId = "" } = useParams();
  const navigate = useNavigate();
  const reduced = useReducedMotion();
  const course = useCourse(slug);
  const [order, setOrder] = useState<SortOrder>(readOrder);
  const notes = useChapterNotes(chapterId, order);
  const desktop = useMedia("(min-width: 768px)");
  const [dragging, setDragging] = useState(false);
  const dragDepth = useRef(0);

  const summary = course.data?.chapters.find((c) => c.id === chapterId);
  const first = notes.data?.pages[0];
  const title = first?.title ?? summary?.title ?? "";
  const position = first?.position ?? summary?.position ?? 0;
  const total = first?.total_notes ?? summary?.note_count ?? 0;
  const photos = summary?.image_count ?? 0;
  const all = useMemo(() => notes.data?.pages.flatMap((p) => p.notes) ?? [], [notes.data]);

  useEffect(() => {
    if (title) document.title = `${title} · ${course.data?.name ?? "notes"}`;
  }, [title, course.data]);

  const openUpload = () => navigate(`/c/${slug}/${chapterId}/upload`, { state: { modal: true } });

  // "U" opens upload (when not typing).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement;
      if (e.key.toLowerCase() === "u" && !e.metaKey && !e.ctrlKey && !/INPUT|TEXTAREA|SELECT/.test(t.tagName) && !t.isContentEditable) {
        if (!location.pathname.endsWith("/upload") && !location.pathname.includes("/n/")) openUpload();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  // Big title fades into the header's compact title on scroll.
  const { scrollY } = useScroll();
  const titleOpacity = useTransform(scrollY, [0, 140], [1, 0]);
  const titleY = useTransform(scrollY, [0, 140], [0, -12]);

  // Full-window drop target (desktop).
  const hasFiles = (e: DragEvent) => Array.from(e.dataTransfer?.types ?? []).includes("Files");
  const onDragEnter = (e: DragEvent) => {
    if (!hasFiles(e)) return;
    dragDepth.current++;
    setDragging(true);
  };
  const onDragLeave = () => {
    dragDepth.current = Math.max(0, dragDepth.current - 1);
    if (dragDepth.current === 0) setDragging(false);
  };
  const onDrop = (e: DragEvent) => {
    if (!hasFiles(e)) return;
    e.preventDefault();
    dragDepth.current = 0;
    setDragging(false);
    setPendingFiles(Array.from(e.dataTransfer.files));
    openUpload();
  };

  const notFound = notes.error instanceof ApiError && notes.error.status === 404;
  if (notFound) {
    return (
      <Page className="pt-24">
        <StatePanel
          pattern="x"
          title="This chapter isn't here anymore"
          titleAs="h1"
          actions={<ButtonLink to={`/c/${slug}`} variant="secondary">Back to course</ButtonLink>}
        />
      </Page>
    );
  }

  return (
    <Page className="mx-auto max-w-[1240px] px-4 pt-20 sm:px-6 md:pt-28 lg:px-8">
      <div
        onDragEnter={onDragEnter}
        onDragOver={(e) => hasFiles(e) && e.preventDefault()}
        onDragLeave={onDragLeave}
        onDrop={onDrop}
        className="min-h-[60vh]"
      >
        <header className="flex flex-col gap-5 md:flex-row md:items-end md:justify-between">
          <m.div style={reduced ? undefined : { opacity: titleOpacity, y: titleY }}>
            <PageItem i={0}>
              <p className="t-caption text-accent">
                Chapter{" "}
                <m.span layoutId={`chapter:${chapterId}:index`} transition={spring.morph} className="inline-block">
                  {pad2(position + 1)}
                </m.span>
              </p>
            </PageItem>
            <m.h1 layoutId={`chapter:${chapterId}:title`} transition={spring.morph} className="t-display-m mt-2 text-fg-1">
              {title || <span className="inline-block h-10 w-64 rounded-full bg-surface-3 align-middle" />}
            </m.h1>
            <PageItem i={1}>
              <p className="t-meta mt-3 text-fg-3">
                {total === 0 && !notes.isPending ? "No notes yet — be first" : `${plural(total, "note")} · ${plural(photos, "photo")}`}
              </p>
            </PageItem>
          </m.div>
          <PageItem i={2} className="flex flex-wrap items-center gap-3">
            <Segmented
              label="Sort notes"
              value={order}
              onChange={(v) => {
                setOrder(v);
                try {
                  sessionStorage.setItem("notes.order", v);
                } catch {
                  // ignore
                }
              }}
              options={[
                { value: "desc", label: "Newest", icon: <ArrowDownWideNarrow size={14} aria-hidden /> },
                { value: "asc", label: "Oldest", icon: <ArrowUpNarrowWide size={14} aria-hidden /> },
              ]}
            />
            {desktop && (
              <m.div
                layoutId="upload:surface"
                style={{ borderRadius: 14 }}
                initial={{ scale: 0.8, opacity: 0 }}
                animate={{ scale: 1, opacity: 1 }}
                // The CTA pops in last; morphs into the upload sheet with the morph spring.
                transition={{ ...spring.pop, delay: 0.3, layout: spring.morph }}
              >
                <Button
                  variant="primary"
                  size="lg"
                  icon={<Upload size={20} aria-hidden />}
                  onClick={openUpload}
                  onPointerEnter={preloadModals}
                >
                  Upload notes
                </Button>
              </m.div>
            )}
          </PageItem>
        </header>

        <div className="mt-10">
          {notes.isPending ? (
            <Delayed>
              <MasonrySkeleton />
            </Delayed>
          ) : notes.error ? (
            <StatePanel
              pattern="blinkX"
              title="Can't reach the board"
              actions={
                <Button variant="secondary" onClick={() => notes.refetch()}>
                  Retry now
                </Button>
              }
            >
              It might be restarting — this page will retry on its own.
            </StatePanel>
          ) : all.length === 0 ? (
            <StatePanel
              pattern="arrowLoop"
              title="No notes here yet"
              actions={
                <Button variant="primary" size="lg" icon={<Upload size={20} aria-hidden />} onClick={openUpload}>
                  Upload the first note
                </Button>
              }
            >
              Be the first — snap a photo of your notes and everyone in {title || "this chapter"} can see them.
            </StatePanel>
          ) : (
            <>
              <Masonry notes={all} slug={slug} chapterId={chapterId} />
              <Paging
                hasMore={Boolean(notes.hasNextPage)}
                loading={notes.isFetchingNextPage}
                onMore={() => notes.fetchNextPage()}
                total={total}
              />
            </>
          )}
        </div>
      </div>

      {!desktop && <Fab onClick={openUpload} />}

      <AnimatePresence>{dragging && <DropOverlay chapter={title} />}</AnimatePresence>
    </Page>
  );
}

// ---- Masonry ------------------------------------------------------------------------------

function columnsFor(width: number) {
  if (width >= 1400) return 5;
  if (width >= 1060) return 4;
  if (width >= 720) return 3;
  return 2;
}

const clampRatio = (w: number, h: number) => Math.min(1.333, Math.max(0.75, w / Math.max(1, h)));

function Masonry({ notes, slug, chapterId }: { notes: Note[]; slug: string; chapterId: string }) {
  const ref = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    setWidth(el.clientWidth);
    let raf = 0;
    const ro = new ResizeObserver(() => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(() => setWidth(el.clientWidth));
    });
    ro.observe(el);
    return () => {
      ro.disconnect();
      cancelAnimationFrame(raf);
    };
  }, []);

  const cols = columnsFor(width);
  const gap = width < 720 ? GAP_MOBILE : GAP;
  const colWidth = (width - gap * (cols - 1)) / cols;

  // Shortest-column placement keeps reading order stable when pages are appended.
  const columns = useMemo(() => {
    const out: { note: Note; index: number }[][] = Array.from({ length: cols }, () => []);
    const heights = new Array<number>(cols).fill(0);
    notes.forEach((note, index) => {
      const img = note.images[0];
      const ratio = img ? clampRatio(img.thumb_width, img.thumb_height) : 1;
      const h = colWidth / ratio + TEXT_BLOCK + gap;
      let target = 0;
      for (let c = 1; c < cols; c++) if ((heights[c] ?? 0) < (heights[target] ?? 0)) target = c;
      out[target]?.push({ note, index });
      heights[target] = (heights[target] ?? 0) + h;
    });
    return out;
  }, [notes, cols, colWidth, gap]);

  return (
    <div ref={ref} className="flex items-start" style={{ gap }}>
      {width > 0 &&
        columns.map((col, c) => (
          <div key={c} className="flex min-w-0 flex-1 flex-col" style={{ gap }}>
            {col.map(({ note, index }) => (
              <NoteCard
                key={note.id}
                note={note}
                slug={slug}
                chapterId={chapterId}
                delay={gridDelay(index % 24, cols)}
                eager={index < cols * 2}
                blur={index < 12}
              />
            ))}
          </div>
        ))}
    </div>
  );
}

function MasonrySkeleton() {
  const ratios = [0.8, 1.2, 1, 0.75, 1.33, 0.9, 1.1, 0.85];
  return (
    <div className="grid grid-cols-2 gap-2.5 md:grid-cols-3 md:gap-4 lg:grid-cols-4">
      {ratios.map((r, i) => (
        <div key={i} className="rounded-[20px] border border-line-subtle bg-surface-1 p-1.5">
          <DotShimmer className="rounded-[14px]" style={{ aspectRatio: String(r) }} index={i} />
          <TextSkeleton className="px-2 pb-2 pt-3" widths={["80%", "40%"]} />
        </div>
      ))}
    </div>
  );
}

function NoteCard({
  note,
  slug,
  chapterId,
  delay,
  eager,
  blur,
}: {
  note: Note;
  slug: string;
  chapterId: string;
  delay: number;
  eager: boolean;
  blur: boolean;
}) {
  const navigate = useNavigate();
  const reduced = useReducedMotion();
  const dir = useDirection();
  const ref = useRef<HTMLAnchorElement>(null);
  const [hover, setHover] = useState(false);
  const img = note.images[0];
  const clip = img ? clampRatio(img.thumb_width, img.thumb_height) : 1;
  const trueRatio = img ? img.width / Math.max(1, img.height) : 1;
  const fresh = isJustPosted(note.id);
  const href = `/c/${slug}/${chapterId}/n/${note.id}`;

  // Inner box sized to the photo's true aspect, centred in the clipped cover, so the
  // lightbox morph "uncrops" instead of stretching (DESIGN.md §7.4).
  const inner =
    trueRatio > clip
      ? { width: `${(trueRatio / clip) * 100}%`, height: "100%", left: `${(-(trueRatio / clip - 1) / 2) * 100}%`, top: 0 }
      : { width: "100%", height: `${(clip / trueRatio) * 100}%`, top: `${(-(clip / trueRatio - 1) / 2) * 100}%`, left: 0 };

  return (
    <m.div
      custom={{ dir, i: 0 }}
      variants={{
        hidden: fresh ? { opacity: 0, scale: 0.6 } : { opacity: 0, y: 14, filter: "blur(4px)" },
        show: {
          opacity: 1,
          y: 0,
          scale: 1,
          filter: "blur(0px)",
          transition: fresh ? { ...spring.pop, delay: 0.3 } : { ...spring.soft, delay: 0.1 + delay },
        },
        exit: { opacity: 0, y: -8, transition: { duration: 0.16 } },
      }}
      initial="hidden"
      animate="show"
      layout="position"
      transition={spring.snappy}
    >
      <m.a
        ref={ref}
        href={href}
        onClick={(e) => {
          if (e.metaKey || e.ctrlKey || e.shiftKey || e.button !== 0) return;
          e.preventDefault();
          navigate(href, { state: { modal: true } });
        }}
        onPointerEnter={() => {
          setHover(true);
          preloadModals();
        }}
        onPointerLeave={() => setHover(false)}
        onFocus={preloadModals}
        onPointerMove={(e) => {
          const el = ref.current;
          if (!el) return;
          const r = el.getBoundingClientRect();
          el.style.setProperty("--mx", `${((e.clientX - r.left) / r.width) * 100}%`);
          el.style.setProperty("--my", `${((e.clientY - r.top) / r.height) * 100}%`);
        }}
        whileHover={reduced ? undefined : { y: -3 }}
        whileTap={{ scale: 0.985 }}
        transition={spring.ui}
        className="spotlight group relative block rounded-[20px] border border-line-subtle bg-surface-1 p-1.5 shadow-[var(--hl-inset)] transition-colors hover:border-line"
      >
        {fresh && (
          <m.span
            aria-hidden
            className="pointer-events-none absolute -inset-[3px] z-[4] rounded-[23px] border-2 border-hi"
            initial={{ opacity: 0 }}
            animate={{ opacity: [0, 1, 0, 1, 0] }}
            transition={{ duration: 2.4, delay: 0.4 }}
          />
        )}
        <div className="relative z-[1] overflow-hidden rounded-[14px] bg-surface-3" style={{ aspectRatio: String(clip) }}>
          {img ? (
            <m.div
              layoutId={`note:${note.id}:cover`}
              transition={spring.morph}
              className="absolute"
              style={inner}
            >
              <m.div
                className="absolute inset-0"
                animate={{ scale: hover && !reduced ? 1.04 : 1 }}
                transition={{ duration: dur.slow, ease: ease.out }}
              >
                <ImageTile
                  src={img.thumb_url}
                  srcSet={`${img.thumb_url} 300w, ${img.url} 1600w`}
                  sizes="(min-width:1400px) 240px, (min-width:1060px) 290px, (min-width:720px) 33vw, 50vw"
                  alt={`${note.title} — photo 1 of ${note.images.length}`}
                  eager={eager}
                  blur={blur}
                />
              </m.div>
            </m.div>
          ) : null}
          {note.images.length > 1 && (
            <span className="absolute right-2 top-2 z-[2]">
              <Badge variant="glass" icon={<Images size={12} aria-hidden />}>
                {note.images.length}
              </Badge>
            </span>
          )}
          {isRecent(note.created_at) && (
            <span className="absolute left-2 top-2 z-[2]">
              <Badge variant="new" pop>
                New
              </Badge>
            </span>
          )}
        </div>
        <div className="relative z-[1] px-2 pb-2 pt-2.5">
          <h3 className="t-title-s line-clamp-2 text-fg-1 sm:t-title-m sm:text-[17px]">{note.title}</h3>
          <p className="t-meta mt-1 flex items-center gap-1.5 truncate text-fg-3">
            <span className="truncate">{note.author_name ?? "Anonymous"}</span>
            <span aria-hidden>·</span>
            <time dateTime={note.created_at} title={fullDate(note.created_at)}>
              {relativeTime(note.created_at)}
            </time>
            {note.body && <AlignLeft size={12} className="ml-auto shrink-0" aria-label="Has text" />}
          </p>
        </div>
      </m.a>
    </m.div>
  );
}

// ---- Paging -----------------------------------------------------------------------------------

function Paging({ hasMore, loading, onMore, total }: { hasMore: boolean; loading: boolean; onMore: () => void; total: number }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el || !hasMore) return;
    const io = new IntersectionObserver(([e]) => e?.isIntersecting && !loading && onMore(), { rootMargin: "1200px 0px" });
    io.observe(el);
    return () => io.disconnect();
  }, [hasMore, loading, onMore]);

  return (
    <div ref={ref} className="mt-10 flex flex-col items-center gap-4">
      {hasMore ? (
        <>
          {loading && <LEDLoader label="Loading more notes" />}
          <button type="button" onClick={onMore} className="t-meta rounded-[10px] px-3 py-2 text-fg-3 hover:bg-surface-3 hover:text-fg-1">
            Load more notes
          </button>
        </>
      ) : (
        <EndOfList total={total} />
      )}
    </div>
  );
}

function EndOfList({ total }: { total: number }) {
  return (
    <m.p
      className="t-meta flex items-center gap-3 text-fg-3"
      initial="off"
      whileInView="on"
      viewport={{ once: true }}
    >
      <Dots />
      That's all {plural(total, "note")}
      <Dots reverse />
    </m.p>
  );
}

function Dots({ reverse }: { reverse?: boolean }) {
  return (
    <span className="flex items-center gap-1.5" aria-hidden>
      {Array.from({ length: 7 }, (_, i) => (
        <m.span
          key={i}
          className="h-1 w-1 rounded-full"
          variants={{
            off: { backgroundColor: LED_COLORS[1] },
            on: {
              backgroundColor: i === (reverse ? 0 : 6) ? LED_COLORS[7] : LED_COLORS[2],
              transition: { delay: (reverse ? 6 - i : i) * 0.03, duration: 0 },
            },
          }}
        />
      ))}
    </span>
  );
}

// ---- Upload affordances --------------------------------------------------------------------------

function Fab({ onClick }: { onClick: () => void }) {
  const { scrollY } = useScroll();
  const reduced = useReducedMotion();
  const [collapsed, setCollapsed] = useState(false);
  const last = useRef(0);
  useMotionValueEvent(scrollY, "change", (y) => {
    const d = y - last.current;
    last.current = y;
    if (reduced) return;
    if (d > 6 && y > 120) setCollapsed(true);
    else if (d < -6) setCollapsed(false);
  });
  return (
    <m.button
      type="button"
      layoutId="upload:surface"
      onClick={onClick}
      onTouchStart={preloadModals}
      initial={{ scale: 0.6, opacity: 0 }}
      animate={{ scale: 1, opacity: 1 }}
      transition={{ ...spring.pop, delay: 0.3 }}
      whileTap={{ scale: 0.95 }}
      style={{ borderRadius: 999 }}
      aria-label="Upload notes"
      className={clsx(
        "fixed bottom-[calc(16px+env(safe-area-inset-bottom))] right-4 z-30 flex h-14 items-center justify-center gap-2 overflow-hidden bg-accent font-[560] text-accent-fg shadow-[var(--glow-md),var(--shadow-3)]",
        collapsed ? "w-14" : "px-5",
      )}
    >
      <Upload size={22} aria-hidden />
      <AnimatePresence initial={false}>
        {!collapsed && (
          <m.span
            initial={{ opacity: 0, width: 0 }}
            animate={{ opacity: 1, width: "auto" }}
            exit={{ opacity: 0, width: 0 }}
            transition={spring.snappy}
            className="overflow-hidden whitespace-nowrap"
          >
            Upload notes
          </m.span>
        )}
      </AnimatePresence>
    </m.button>
  );
}

function DropOverlay({ chapter }: { chapter: string }) {
  return (
    <m.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={{ duration: dur.base }}
      className="pointer-events-none fixed inset-0 z-[75] bg-[var(--scrim)] p-6"
    >
      <div className="relative grid h-full place-items-center rounded-[28px]">
        <svg className="absolute inset-0 h-full w-full" aria-hidden>
          <rect
            x="2"
            y="2"
            style={{ width: "calc(100% - 4px)", height: "calc(100% - 4px)" }}
            rx="28"
            fill="none"
            stroke="var(--accent)"
            strokeWidth="4"
            strokeLinecap="round"
            strokeDasharray="0 12"
            className="marching"
          />
        </svg>
        <m.p initial={{ scale: 0.9 }} animate={{ scale: 1 }} transition={spring.pop} className="t-title-l text-white">
          Drop to add photos to <strong className="text-accent">{chapter || "this chapter"}</strong>
        </m.p>
      </div>
    </m.div>
  );
}
