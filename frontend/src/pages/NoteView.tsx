import clsx from "clsx";
import { animate, m, useMotionValue, useReducedMotion, useTransform, type MotionValue } from "motion/react";
import { ChevronLeft, ChevronRight, Download, ExternalLink, Share2, X, ZoomIn, ZoomOut, Maximize2 } from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type PointerEvent as RPointerEvent } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useLocation, useNavigate, useSearchParams } from "react-router";

import { Button, ButtonLink } from "../components/Button";
import { LEDLoader } from "../components/LEDLoader";
import { StatePanel } from "../components/StatePanel";
import { LED_COLORS } from "../led/sprites";
import { ApiError } from "../lib/api";
import { formatBytes, fullDate, relativeTime } from "../lib/format";
import { findCachedNote, useNote } from "../lib/queries";
import type { Note, NoteImage } from "../lib/types";
import { dur, ease, spring } from "../motion/springs";

const MAX_ZOOM = 4;

export default function NoteView({ slug, chapterId, noteId }: { slug: string; chapterId: string; noteId: string }) {
  const client = useQueryClient();
  const cached = useMemo(() => findCachedNote(client, noteId), [client, noteId]);
  const { data: note, error } = useNote(noteId, cached);
  const navigate = useNavigate();
  const location = useLocation();
  const [params, setParams] = useSearchParams();
  const reduced = useReducedMotion() ?? false;
  const chapterPath = `/c/${slug}/${chapterId}`;
  const count = note?.images.length ?? 0;
  const index = Math.min(Math.max(0, Number(params.get("i") ?? 0) || 0), Math.max(0, count - 1));
  // Only the first photo morphs from the card; deep links / other photos fade in.
  const openedAt = useRef(index);
  const morph = openedAt.current === 0 && Boolean(cached) && !reduced;

  const close = useCallback(() => {
    if ((location.state as { modal?: boolean } | null)?.modal) navigate(-1);
    else navigate(chapterPath, { replace: true });
  }, [location.state, navigate, chapterPath]);

  const go = useCallback(
    (i: number) => {
      const next = Math.min(Math.max(0, i), count - 1);
      if (next === index) return;
      setParams(next === 0 ? {} : { i: String(next) }, { replace: true, state: location.state });
    },
    [count, index, setParams, location.state],
  );

  useEffect(() => {
    const before = document.title;
    if (note) document.title = `${note.title} · notes`;
    return () => {
      document.title = before;
    };
  }, [note]);

  // Scroll lock + focus management.
  const dialogRef = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const prevOverflow = document.documentElement.style.overflow;
    document.documentElement.style.overflow = "hidden";
    const opener = document.activeElement as HTMLElement | null;
    return () => {
      document.documentElement.style.overflow = prevOverflow;
      opener?.focus?.();
    };
  }, []);
  useEffect(() => {
    dialogRef.current?.querySelector<HTMLElement>("[data-autofocus]")?.focus();
  }, []);

  const notFound = error instanceof ApiError && error.status === 404;
  const titleId = `note-title-${noteId}`;

  return (
    <m.div
      ref={dialogRef}
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
      className="fixed inset-0 z-[70]"
      onKeyDown={(e) => trapFocus(e, dialogRef.current)}
    >
      <Scrim onClose={close} />
      {notFound ? (
        <div className="absolute inset-0 grid place-items-center">
          <StatePanel
            pattern="x"
            title="This note was removed"
            actions={<ButtonLink to={chapterPath} replace variant="secondary">Back to chapter</ButtonLink>}
          />
        </div>
      ) : note ? (
        <Gallery
          note={note}
          index={index}
          go={go}
          close={close}
          morph={morph}
          titleId={titleId}
          reduced={reduced}
        />
      ) : (
        <div className="absolute inset-0 grid place-items-center">
          <LEDLoader label="Loading note" />
        </div>
      )}
    </m.div>
  );
}

function Scrim({ onClose, opacity }: { onClose: () => void; opacity?: MotionValue<number> }) {
  return (
    <m.div
      className="absolute inset-0 bg-[var(--scrim)] backdrop-blur-[6px]"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0, transition: { duration: dur.base, ease: ease.in } }}
      transition={{ duration: dur.slow, ease: ease.outExpo }}
      style={opacity ? { opacity } : undefined}
      onClick={onClose}
      aria-hidden
    />
  );
}

// ---- Gallery -------------------------------------------------------------------------------

function useSize(ref: React.RefObject<HTMLElement | null>) {
  const [size, setSize] = useState({ w: 0, h: 0 });
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const update = () => setSize({ w: el.clientWidth, h: el.clientHeight });
    update();
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, [ref]);
  return size;
}

function fit(img: NoteImage, w: number, h: number) {
  const r = img.width / Math.max(1, img.height);
  let fw = w;
  let fh = w / r;
  if (fh > h) {
    fh = h;
    fw = h * r;
  }
  return { w: fw, h: fh };
}

interface Gesture {
  pointers: Map<number, { x: number; y: number }>;
  axis: "x" | "y" | "pan" | "pinch" | null;
  startX: number;
  startY: number;
  lastX: number;
  lastY: number;
  lastT: number;
  vx: number;
  vy: number;
  pinchDist: number;
  pinchScale: number;
  pinchMid: { x: number; y: number };
  panStart: { x: number; y: number };
  lastTap: number;
}

function Gallery({
  note,
  index,
  go,
  close,
  morph,
  titleId,
  reduced,
}: {
  note: Note;
  index: number;
  go: (i: number) => void;
  close: () => void;
  morph: boolean;
  titleId: string;
  reduced: boolean;
}) {
  const stageRef = useRef<HTMLDivElement>(null);
  const { w, h } = useSize(stageRef);
  const images = note.images;
  const current = images[index];
  const desktop = w > 0 && typeof window !== "undefined" && window.innerWidth >= 1024;

  const trackX = useMotionValue(0);
  const dismissY = useMotionValue(0);
  const zoom = useMotionValue(1);
  const panX = useMotionValue(0);
  const panY = useMotionValue(0);
  const [zoomed, setZoomed] = useState(false);
  const [hover, setHover] = useState(false);
  const dismissScale = useTransform(dismissY, [0, 300], [1, 0.82], { clamp: true });
  const chromeOpacity = useTransform(dismissY, [0, 200], [1, 0], { clamp: true });

  // Keep the track on the current slide.
  useEffect(() => {
    if (!w) return;
    const target = -index * w;
    if (reduced) animate(trackX, target, { duration: 0.15 });
    else animate(trackX, target, spring.swipe);
  }, [index, w, trackX, reduced]);

  const resetZoom = useCallback(() => {
    animate(zoom, 1, spring.snappy);
    animate(panX, 0, spring.snappy);
    animate(panY, 0, spring.snappy);
    setZoomed(false);
  }, [zoom, panX, panY]);
  useEffect(() => resetZoom(), [index, resetZoom]);

  const clampPan = useCallback(
    (s: number) => {
      if (!current) return;
      const box = fit(current, w, h);
      const mx = Math.max(0, (box.w * s - w) / 2);
      const my = Math.max(0, (box.h * s - h) / 2);
      animate(panX, Math.min(mx, Math.max(-mx, panX.get())), spring.snappy);
      animate(panY, Math.min(my, Math.max(-my, panY.get())), spring.snappy);
    },
    [current, w, h, panX, panY],
  );

  const zoomTo = useCallback(
    (s: number, anchor?: { x: number; y: number }) => {
      const next = Math.min(MAX_ZOOM, Math.max(1, s));
      if (next <= 1.05) return resetZoom();
      if (anchor) {
        // Keep the anchor point under the finger/cursor.
        const k = next / zoom.get();
        panX.set((panX.get() - anchor.x) * k + anchor.x);
        panY.set((panY.get() - anchor.y) * k + anchor.y);
      }
      animate(zoom, next, spring.snappy);
      setZoomed(true);
      requestAnimationFrame(() => clampPan(next));
    },
    [zoom, panX, panY, resetZoom, clampPan],
  );

  // Keyboard shortcuts.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement).closest("input,textarea")) return;
      switch (e.key) {
        case "ArrowLeft":
          go(index - 1);
          break;
        case "ArrowRight":
          go(index + 1);
          break;
        case "Home":
          go(0);
          break;
        case "End":
          go(images.length - 1);
          break;
        case "Escape":
          if (zoomed) resetZoom();
          else close();
          break;
        case "+":
        case "=":
          zoomTo(zoom.get() * 1.5);
          break;
        case "-":
          zoomTo(zoom.get() / 1.5);
          break;
        case "0":
          resetZoom();
          break;
        case "d":
        case "D":
          if (current) download(current, note, index);
          break;
        default:
          return;
      }
      e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [go, index, images.length, zoomed, resetZoom, close, zoomTo, zoom, current, note]);

  // Preload neighbours' full images.
  useEffect(() => {
    for (const i of [index - 1, index + 1]) {
      const img = images[i];
      if (img) new Image().src = img.url;
    }
  }, [index, images]);

  // ---- Pointer gestures: swipe, swipe-down dismiss, pan, pinch, double-tap ----
  const g = useRef<Gesture>({
    pointers: new Map(),
    axis: null,
    startX: 0,
    startY: 0,
    lastX: 0,
    lastY: 0,
    lastT: 0,
    vx: 0,
    vy: 0,
    pinchDist: 0,
    pinchScale: 1,
    pinchMid: { x: 0, y: 0 },
    panStart: { x: 0, y: 0 },
    lastTap: 0,
  });

  const local = (e: { clientX: number; clientY: number }) => {
    const r = stageRef.current?.getBoundingClientRect();
    return { x: e.clientX - (r?.left ?? 0) - w / 2, y: e.clientY - (r?.top ?? 0) - h / 2 };
  };

  const onPointerDown = (e: RPointerEvent) => {
    if ((e.target as HTMLElement).closest("button,a")) return;
    const s = g.current;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    s.pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (s.pointers.size === 2) {
      const [a, b] = [...s.pointers.values()] as [{ x: number; y: number }, { x: number; y: number }];
      s.axis = "pinch";
      s.pinchDist = Math.hypot(a.x - b.x, a.y - b.y);
      s.pinchScale = zoom.get();
      s.pinchMid = local({ clientX: (a.x + b.x) / 2, clientY: (a.y + b.y) / 2 });
      return;
    }
    s.axis = zoom.get() > 1.01 ? "pan" : null;
    s.startX = s.lastX = e.clientX;
    s.startY = s.lastY = e.clientY;
    s.lastT = performance.now();
    s.vx = s.vy = 0;
    s.panStart = { x: panX.get(), y: panY.get() };
  };

  const onPointerMove = (e: RPointerEvent) => {
    const s = g.current;
    if (!s.pointers.has(e.pointerId)) return;
    s.pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (s.axis === "pinch" && s.pointers.size === 2) {
      const [a, b] = [...s.pointers.values()] as [{ x: number; y: number }, { x: number; y: number }];
      const next = Math.min(MAX_ZOOM, Math.max(0.8, (s.pinchScale * Math.hypot(a.x - b.x, a.y - b.y)) / s.pinchDist));
      const k = next / zoom.get();
      panX.set((panX.get() - s.pinchMid.x) * k + s.pinchMid.x);
      panY.set((panY.get() - s.pinchMid.y) * k + s.pinchMid.y);
      zoom.set(next);
      setZoomed(next > 1.01);
      return;
    }
    const now = performance.now();
    const dt = Math.max(1, now - s.lastT);
    s.vx = ((e.clientX - s.lastX) / dt) * 1000;
    s.vy = ((e.clientY - s.lastY) / dt) * 1000;
    s.lastX = e.clientX;
    s.lastY = e.clientY;
    s.lastT = now;
    const dx = e.clientX - s.startX;
    const dy = e.clientY - s.startY;
    if (s.axis === "pan") {
      panX.set(s.panStart.x + dx);
      panY.set(s.panStart.y + dy);
      return;
    }
    if (!s.axis && Math.hypot(dx, dy) > 8) s.axis = Math.abs(dx) > Math.abs(dy) ? "x" : dy > 0 && e.pointerType !== "mouse" ? "y" : "x";
    if (s.axis === "x") {
      const atEdge = (index === 0 && dx > 0) || (index === images.length - 1 && dx < 0);
      trackX.set(-index * w + (atEdge ? dx * 0.18 : dx));
    } else if (s.axis === "y") {
      dismissY.set(Math.max(0, dy));
    }
  };

  const onPointerUp = (e: RPointerEvent) => {
    const s = g.current;
    const wasPinch = s.axis === "pinch";
    s.pointers.delete(e.pointerId);
    if (s.pointers.size > 0) return;
    const dx = e.clientX - s.startX;
    const dy = e.clientY - s.startY;
    if (wasPinch) {
      zoomTo(zoom.get());
    } else if (s.axis === "x") {
      if ((dx < -w * 0.22 || s.vx < -500) && index < images.length - 1) go(index + 1);
      else if ((dx > w * 0.22 || s.vx > 500) && index > 0) go(index - 1);
      else animate(trackX, -index * w, spring.swipe);
    } else if (s.axis === "y") {
      if (dy > 120 || s.vy > 800) close();
      else animate(dismissY, 0, spring.sheet);
    } else if (s.axis === "pan") {
      clampPan(zoom.get());
    } else if (Math.hypot(dx, dy) < 6) {
      // A tap: double tap / double click toggles zoom at that point.
      const now = performance.now();
      if (now - s.lastTap < 300) {
        s.lastTap = 0;
        if (zoom.get() > 1.01) resetZoom();
        else zoomTo(2.5, local(e));
      } else s.lastTap = now;
    }
    s.axis = null;
  };

  const onWheel = (e: React.WheelEvent) => {
    if (e.ctrlKey || e.metaKey) {
      zoomTo(zoom.get() * (e.deltaY < 0 ? 1.15 : 1 / 1.15), local(e));
    } else if (zoom.get() > 1.01) {
      panX.set(panX.get() - e.deltaX);
      panY.set(panY.get() - e.deltaY);
      clampPan(zoom.get());
    }
  };

  const totalBytes = images.reduce((a, b) => a + b.size_bytes, 0);

  return (
    <div className="absolute inset-0 flex flex-col lg:flex-row lg:gap-6 lg:p-6">
      <m.div className="relative flex min-h-0 flex-1 flex-col" style={{ scale: dismissScale }}>
        {/* Stage */}
        <div
          ref={stageRef}
          className="relative min-h-0 flex-1 touch-none select-none overflow-hidden lg:rounded-[28px]"
          style={{ background: "#05070D" }}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onPointerCancel={onPointerUp}
          onWheel={onWheel}
          onPointerEnter={() => setHover(true)}
          onPointerLeave={() => setHover(false)}
        >
          <m.div className="absolute inset-y-0 left-0 flex" style={{ x: trackX, width: w * images.length }}>
            {images.map((img, i) =>
              Math.abs(i - index) <= 1 ? (
                <Slide
                  key={img.id}
                  note={note}
                  img={img}
                  i={i}
                  w={w}
                  h={h}
                  current={i === index}
                  morph={morph && i === 0}
                  zoom={zoom}
                  panX={panX}
                  panY={panY}
                />
              ) : null,
            )}
          </m.div>

          {/* Top bar */}
          <m.div
            style={{ opacity: chromeOpacity }}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ delay: 0.2, duration: dur.base }}
            className="pointer-events-none absolute inset-x-0 top-0 flex items-center justify-between bg-gradient-to-b from-black/50 to-transparent p-3 pt-[calc(12px+env(safe-area-inset-top))]"
          >
            <Button
              variant="glass"
              size="icon"
              aria-label="Close"
              data-autofocus
              onClick={close}
              className="pointer-events-auto"
            >
              <X size={20} />
            </Button>
            <span className="t-meta rounded-full bg-black/40 px-3 py-1.5 text-white/90 backdrop-blur-md" aria-live="polite">
              <span className="sr-only">Photo </span>
              {index + 1} / {images.length}
            </span>
          </m.div>

          {/* Prev / next (desktop) */}
          {images.length > 1 && (
            <>
              <NavArrow dir={-1} disabled={index === 0} show={hover} onClick={() => go(index - 1)} />
              <NavArrow dir={1} disabled={index === images.length - 1} show={hover} onClick={() => go(index + 1)} />
            </>
          )}

          {/* Bottom controls (desktop) */}
          <div className="absolute inset-x-0 bottom-0 hidden items-center justify-between p-3 lg:flex">
            <div className="flex gap-1.5">
              <Button variant="glass" size="icon-sm" aria-label="Zoom out" onClick={() => zoomTo(zoom.get() / 1.5)}>
                <ZoomOut size={18} />
              </Button>
              <Button variant="glass" size="icon-sm" aria-label="Zoom in" onClick={() => zoomTo(zoom.get() * 1.5)}>
                <ZoomIn size={18} />
              </Button>
              <Button variant="glass" size="icon-sm" aria-label="Reset zoom" onClick={resetZoom} disabled={!zoomed}>
                <Maximize2 size={16} />
              </Button>
            </div>
            {current && <ImageActions img={current} note={note} index={index} />}
          </div>

          {/* Page dots (mobile) */}
          {images.length > 1 && (
            <div className="absolute inset-x-0 bottom-3 flex justify-center gap-1.5 lg:hidden" aria-hidden>
              {images.map((img, i) => (
                <span
                  key={img.id}
                  className="h-1.5 w-1.5 rounded-full transition-colors duration-150"
                  style={{ background: i === index ? LED_COLORS[7] : LED_COLORS[2] }}
                />
              ))}
            </div>
          )}
        </div>

        {/* Thumbnail strip (desktop, under the stage) */}
        {desktop && images.length > 1 && <ThumbStrip images={images} index={index} go={go} className="mt-3" />}
      </m.div>

      {/* Details: side panel on desktop, bottom sheet on mobile */}
      <m.aside
        style={{ opacity: chromeOpacity }}
        initial={reduced ? { opacity: 0 } : { opacity: 0, x: desktop ? 24 : 0, y: desktop ? 0 : 40 }}
        animate={{ opacity: 1, x: 0, y: 0 }}
        exit={{ opacity: 0, x: desktop ? 24 : 0, y: desktop ? 0 : 40, transition: { duration: dur.fast } }}
        transition={{ ...spring.soft, delay: 0.14 }}
        className="relative z-10 flex max-h-[45dvh] shrink-0 flex-col overflow-hidden rounded-t-[28px] border-t border-line bg-surface-2 lg:max-h-none lg:w-[380px] lg:rounded-[28px] lg:border"
      >
        <div className="mx-auto mt-2 h-1 w-9 rounded-full bg-line lg:hidden" aria-hidden />
        <div className="min-h-0 flex-1 overflow-y-auto p-5 lg:p-6">
          <h2 id={titleId} className="t-title-l text-fg-1">
            {note.title}
          </h2>
          <p className="t-meta mt-2 text-fg-3">
            {note.author_name ?? "Anonymous"} ·{" "}
            <time dateTime={note.created_at} title={fullDate(note.created_at)}>
              {relativeTime(note.created_at)}
            </time>
            {!desktop && ` · ${images.length} ${images.length === 1 ? "photo" : "photos"}`}
          </p>
          {!desktop && images.length > 1 && <ThumbStrip images={images} index={index} go={go} className="mt-4" />}
          {note.body && (
            <div className="t-body mt-5 max-w-[68ch] whitespace-pre-wrap border-t border-line-subtle pt-5 text-fg-2">
              {note.body}
            </div>
          )}
          {!desktop && current && (
            <div className="mt-5">
              <ImageActions img={current} note={note} index={index} solid />
            </div>
          )}
        </div>
        {current && (
          <div className="t-meta border-t border-line-subtle px-5 py-3 text-fg-3 lg:px-6">
            <p>
              {images.length} {images.length === 1 ? "photo" : "photos"} · {formatBytes(totalBytes)}
            </p>
            <p className="mt-0.5 truncate">
              {current.original_filename ?? `photo ${index + 1}`} · {current.width}×{current.height} · {formatBytes(current.size_bytes)}
            </p>
          </div>
        )}
      </m.aside>
    </div>
  );
}

function Slide({
  note,
  img,
  i,
  w,
  h,
  current,
  morph,
  zoom,
  panX,
  panY,
}: {
  note: Note;
  img: NoteImage;
  i: number;
  w: number;
  h: number;
  current: boolean;
  morph: boolean;
  zoom: MotionValue<number>;
  panX: MotionValue<number>;
  panY: MotionValue<number>;
}) {
  const box = fit(img, Math.max(1, w), Math.max(1, h));
  const [full, setFull] = useState(false);
  const [morphDone, setMorphDone] = useState(!morph);
  const fullRef = useRef<HTMLImageElement>(null);
  useEffect(() => {
    const el = fullRef.current;
    if (!el) return;
    let alive = true;
    const show = () => alive && setFull(true);
    if (el.complete && el.naturalWidth) show();
    else el.decode().then(show, () => {});
    return () => {
      alive = false;
    };
  }, [img.url]);

  return (
    <div className="absolute top-0 flex items-center justify-center" style={{ left: i * w, width: w, height: h }}>
      <m.div
        layoutId={morph ? `note:${note.id}:cover` : undefined}
        transition={spring.morph}
        onLayoutAnimationComplete={() => setMorphDone(true)}
        initial={morph ? undefined : { opacity: 0, scale: 0.96 }}
        animate={{ opacity: 1, scale: 1 }}
        className="relative"
        style={{ width: box.w, height: box.h }}
      >
        <m.div
          className="absolute inset-0"
          style={current ? { scale: zoom, x: panX, y: panY } : undefined}
        >
          {/* thumb first (cached, instant), then the full photo once decoded */}
          <img
            src={img.thumb_url}
            alt=""
            aria-hidden
            className={clsx("absolute inset-0 h-full w-full object-cover", morphDone && !full && "blur-[6px]")}
            draggable={false}
          />
          <img
            ref={fullRef}
            src={img.url}
            alt={`${note.title} — photo ${i + 1} of ${note.images.length}`}
            className={clsx(
              "absolute inset-0 h-full w-full object-cover transition-opacity duration-[360ms]",
              full ? "opacity-100" : "opacity-0",
            )}
            draggable={false}
          />
        </m.div>
      </m.div>
    </div>
  );
}

function NavArrow({ dir, disabled, show, onClick }: { dir: 1 | -1; disabled: boolean; show: boolean; onClick: () => void }) {
  const Icon = dir < 0 ? ChevronLeft : ChevronRight;
  return (
    <m.div
      className={clsx("absolute top-1/2 hidden -translate-y-1/2 lg:block", dir < 0 ? "left-3" : "right-3")}
      animate={{ opacity: show ? (disabled ? 0.3 : 1) : 0 }}
      transition={{ duration: dur.fast }}
    >
      <Button
        variant="glass"
        size="icon"
        aria-label={dir < 0 ? "Previous photo" : "Next photo"}
        disabled={disabled}
        onClick={onClick}
        className="focus-visible:opacity-100"
      >
        <Icon size={22} />
      </Button>
    </m.div>
  );
}

function ThumbStrip({ images, index, go, className }: { images: NoteImage[]; index: number; go: (i: number) => void; className?: string }) {
  const refs = useRef<(HTMLButtonElement | null)[]>([]);
  const reduced = useReducedMotion();
  useEffect(() => {
    refs.current[index]?.scrollIntoView({ inline: "center", block: "nearest", behavior: reduced ? "auto" : "smooth" });
  }, [index, reduced]);
  return (
    <div className={clsx("no-scrollbar flex justify-start gap-2 overflow-x-auto px-1 py-2 lg:justify-center", className)}>
      {images.map((img, i) => (
        <button
          key={img.id}
          ref={(el) => {
            refs.current[i] = el;
          }}
          type="button"
          aria-label={`Photo ${i + 1}`}
          aria-current={i === index || undefined}
          onClick={() => go(i)}
          className={clsx(
            "relative h-14 w-14 shrink-0 rounded-[10px] transition-opacity duration-150 lg:h-16 lg:w-16",
            i === index ? "opacity-100" : "opacity-55 hover:opacity-100",
          )}
        >
          <img src={img.thumb_url} alt="" className="h-full w-full rounded-[10px] object-cover" draggable={false} />
          {i === index && (
            <m.span
              layoutId="strip:indicator"
              transition={spring.snappy}
              className="absolute -inset-[3px] rounded-[12px] border-2 border-accent"
            >
              <span
                className="absolute -bottom-[9px] left-1/2 h-1 w-1 -translate-x-1/2 rounded-full"
                style={{ background: LED_COLORS[7], boxShadow: "0 0 6px #4C9AFF" }}
              />
            </m.span>
          )}
        </button>
      ))}
    </div>
  );
}

function download(img: NoteImage, note: Note, index: number) {
  const a = document.createElement("a");
  a.href = img.url;
  a.download = img.original_filename ?? `${note.title}-${index + 1}.jpg`;
  a.click();
}

function ImageActions({ img, note, index, solid }: { img: NoteImage; note: Note; index: number; solid?: boolean }) {
  const [shared, setShared] = useState(false);
  const canShare = typeof navigator.share === "function";
  return (
    <div className="flex flex-wrap gap-1.5">
      <a
        href={img.url}
        download={img.original_filename ?? `${note.title}-${index + 1}.jpg`}
        className={clsx(
          "t-label inline-flex h-9 items-center gap-1.5 rounded-[10px] px-3",
          solid ? "border border-line bg-surface-1 text-fg-1" : "bg-black/50 text-white backdrop-blur-md hover:bg-black/70",
        )}
      >
        <Download size={16} aria-hidden /> Download
      </a>
      <a
        href={img.url}
        target="_blank"
        rel="noreferrer"
        className={clsx(
          "t-label inline-flex h-9 items-center gap-1.5 rounded-[10px] px-3",
          solid ? "border border-line bg-surface-1 text-fg-1" : "bg-black/50 text-white backdrop-blur-md hover:bg-black/70",
        )}
      >
        <ExternalLink size={16} aria-hidden /> Open original
      </a>
      {solid && canShare && (
        <button
          type="button"
          onClick={() => {
            navigator.share({ title: note.title, url: window.location.href }).then(() => setShared(true), () => {});
          }}
          className="t-label inline-flex h-9 items-center gap-1.5 rounded-[10px] border border-line bg-surface-1 px-3 text-fg-1"
        >
          <Share2 size={16} aria-hidden /> {shared ? "Shared" : "Share"}
        </button>
      )}
    </div>
  );
}

function trapFocus(e: React.KeyboardEvent, root: HTMLElement | null) {
  if (e.key !== "Tab" || !root) return;
  const items = [...root.querySelectorAll<HTMLElement>("a[href],button:not([disabled]),[tabindex]:not([tabindex='-1'])")].filter(
    (el) => el.offsetParent !== null,
  );
  const first = items[0];
  const last = items[items.length - 1];
  if (!first || !last) return;
  if (e.shiftKey && document.activeElement === first) {
    e.preventDefault();
    last.focus();
  } else if (!e.shiftKey && document.activeElement === last) {
    e.preventDefault();
    first.focus();
  }
}
