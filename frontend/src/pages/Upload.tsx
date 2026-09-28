import clsx from "clsx";
import { AnimatePresence, Reorder, m, useDragControls, useReducedMotion } from "motion/react";
import { Camera, Clock, HardDrive, ImagePlus, Plus, RotateCcw, TriangleAlert, WifiOff, X } from "lucide-react";
import { useCallback, useEffect, useId, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useLocation, useNavigate } from "react-router";

import { useTheme } from "../app/theme";
import { useToast } from "../app/toast";
import { Badge } from "../components/Badge";
import { Button } from "../components/Button";
import { Input, Textarea } from "../components/Field";
import { LEDLoader } from "../components/LEDLoader";
import { StatePanel } from "../components/StatePanel";
import { LED_COLORS } from "../led/sprites";
import { LEDMatrix } from "../led/LEDMatrix";
import { ApiError, uploadNote } from "../lib/api";
import { formatBytes } from "../lib/format";
import { useCourse } from "../lib/queries";
import type { Note } from "../lib/types";
import { dur, ease, spring } from "../motion/springs";
import { notifyLocalUpload } from "../stats/store";
import { burst } from "../upload/celebrate";
import { markJustPosted, takePendingFiles } from "../upload/pending";

const MAX_FILES = 10;
const MAX_FILE = 10 * 1024 * 1024;
const MAX_TOTAL = 60 * 1024 * 1024;
const TYPES = ["image/jpeg", "image/png", "image/webp", "image/gif"];
const EXT = /\.(jpe?g|png|webp|gif)$/i;
const IMAGE_ERR = /^image (\d+) \((.+?)\): (.+)$/i;

interface Tile {
  id: string;
  file: File;
  url: string;
}

type Phase = "edit" | "uploading" | "processing" | "success" | "gone";
type Banner =
  | { kind: "error"; icon: "alert" | "wifi"; text: string; retry?: boolean }
  | { kind: "rate"; until: number; seconds: number }
  | { kind: "full" };

let tileSeq = 0;
function problemOf(file: File): string | undefined {
  if (!TYPES.includes(file.type) && !EXT.test(file.name)) return "Not a JPEG, PNG, WebP or GIF.";
  if (file.size > MAX_FILE) return "Over 10 MB — too big for the board.";
  return undefined;
}

function readDraft(chapterId: string) {
  try {
    const d = JSON.parse(sessionStorage.getItem(`notes.draft.${chapterId}`) ?? "{}") as { title?: string; body?: string };
    return { title: d.title ?? "", body: d.body ?? "" };
  } catch {
    return { title: "", body: "" };
  }
}

export default function UploadSheet({ slug, chapterId }: { slug: string; chapterId: string }) {
  const navigate = useNavigate();
  const location = useLocation();
  const queryClient = useQueryClient();
  const toast = useToast();
  const { theme } = useTheme();
  const reduced = useReducedMotion() ?? false;
  const course = useCourse(slug);
  const chapter = course.data?.chapters.find((c) => c.id === chapterId);
  const chapterName = chapter?.title ?? "this chapter";
  const titleId = useId();

  const [tiles, setTiles] = useState<Tile[]>([]);
  const [title, setTitle] = useState(() => readDraft(chapterId).title);
  const [body, setBody] = useState(() => readDraft(chapterId).body);
  const [author, setAuthor] = useState(() => {
    try {
      return localStorage.getItem("notes.author") ?? "";
    } catch {
      return "";
    }
  });
  const [phase, setPhase] = useState<Phase>("edit");
  const [progress, setProgress] = useState(0);
  const [banner, setBanner] = useState<Banner | null>(null);
  const [tileErrors, setTileErrors] = useState<Record<string, string>>({});
  const [titleError, setTitleError] = useState<string>();
  const [filesError, setFilesError] = useState<string>();
  const [shake, setShake] = useState(0);
  const [confirm, setConfirm] = useState(false);
  const [celebrating, setCelebrating] = useState(false);
  const [coverLayoutId, setCoverLayoutId] = useState<string>();
  const [announce, setAnnounce] = useState("");
  const abortRef = useRef<AbortController | null>(null);
  const plateRef = useRef<HTMLDivElement>(null);
  const titleRef = useRef<HTMLInputElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const tilesRef = useRef(tiles);
  tilesRef.current = tiles;
  const panelDrag = useDragControlsFor(panelRef);

  const busy = phase === "uploading" || phase === "processing" || phase === "success";
  const dirty = tiles.length > 0 || title.trim() !== "" || body.trim() !== "";
  const total = tiles.reduce((a, t) => a + t.file.size, 0);
  const invalid = tiles.some((t) => problemOf(t.file));

  // Draft text survives a refresh (files can't).
  useEffect(() => {
    try {
      sessionStorage.setItem(`notes.draft.${chapterId}`, JSON.stringify({ title, body }));
    } catch {
      // ignore
    }
  }, [title, body, chapterId]);

  // Revoke preview URLs on unmount.
  useEffect(() => () => tilesRef.current.forEach((t) => URL.revokeObjectURL(t.url)), []);

  const addFiles = useCallback(
    (list: File[]) => {
      if (busy || list.length === 0) return;
      setFilesError(undefined);
      setTiles((prev) => {
        const room = MAX_FILES - prev.length;
        if (list.length > room) {
          toast({ kind: "warning", title: `That's more than ${MAX_FILES} — kept the first ${Math.max(0, room)}.` });
        }
        const added = list.slice(0, Math.max(0, room)).map((file) => ({
          id: `t${++tileSeq}`,
          file,
          url: URL.createObjectURL(file),
        }));
        return [...prev, ...added];
      });
    },
    [busy, toast],
  );

  // Files dropped on the chapter page before the sheet opened.
  useEffect(() => {
    const pending = takePendingFiles();
    if (pending.length) addFiles(pending);
  }, [addFiles]);

  // Paste images (desktop nicety).
  useEffect(() => {
    const onPaste = (e: ClipboardEvent) => {
      const files = Array.from(e.clipboardData?.files ?? []).filter((f) => f.type.startsWith("image/"));
      if (files.length) {
        e.preventDefault();
        addFiles(files);
      }
    };
    window.addEventListener("paste", onPaste);
    return () => window.removeEventListener("paste", onPaste);
  }, [addFiles]);

  const remove = (id: string) => {
    setTiles((prev) => {
      const t = prev.find((x) => x.id === id);
      if (t) URL.revokeObjectURL(t.url);
      return prev.filter((x) => x.id !== id);
    });
    setTileErrors((e) => {
      const { [id]: _drop, ...rest } = e;
      void _drop;
      return rest;
    });
  };

  const move = (id: string, delta: number) => {
    setTiles((prev) => {
      const i = prev.findIndex((t) => t.id === id);
      const j = i + delta;
      if (i < 0 || j < 0 || j >= prev.length) return prev;
      const next = [...prev];
      const [t] = next.splice(i, 1);
      if (t) next.splice(j, 0, t);
      setAnnounce(`Photo ${i + 1} moved to position ${j + 1}`);
      return next;
    });
  };

  // ---- Closing ----------------------------------------------------------------------
  const leave = useCallback(() => {
    if ((location.state as { modal?: boolean } | null)?.modal) navigate(-1);
    else navigate(`/c/${slug}/${chapterId}`, { replace: true });
  }, [location.state, navigate, slug, chapterId]);

  const requestClose = useCallback(() => {
    if (busy) return;
    if (dirty) setConfirm(true);
    else leave();
  }, [busy, dirty, leave]);

  useEffect(() => {
    const onKey = (e: globalThis.KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        if (confirm) setConfirm(false);
        else requestClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [confirm, requestClose]);

  useLayoutEffect(() => {
    const prev = document.documentElement.style.overflow;
    document.documentElement.style.overflow = "hidden";
    return () => {
      document.documentElement.style.overflow = prev;
    };
  }, []);

  // ---- Submitting ----------------------------------------------------------------------
  const validate = () => {
    let ok = true;
    if (tiles.length === 0) {
      setFilesError("Add at least one photo.");
      setShake((s) => s + 1);
      ok = false;
    }
    if (!title.trim()) {
      setTitleError("Give it a title so classmates can find it.");
      if (ok) titleRef.current?.focus();
      ok = false;
    }
    if (invalid || total > MAX_TOTAL) ok = false;
    return ok;
  };

  const succeed = (note: Note) => {
    setPhase("success");
    queryClient.setQueryData(["note", note.id], note);
    void queryClient.invalidateQueries({ queryKey: ["chapter", chapterId] });
    void queryClient.invalidateQueries({ queryKey: ["course", slug] });
    void queryClient.invalidateQueries({ queryKey: ["courses"] });
    markJustPosted(note.id);
    try {
      if (author.trim()) localStorage.setItem("notes.author", author.trim());
      sessionStorage.removeItem(`notes.draft.${chapterId}`);
    } catch {
      // ignore
    }
    const open = () => {
      setCoverLayoutId(`note:${note.id}:cover`);
      navigate(`/c/${slug}/${chapterId}/n/${note.id}`, { replace: true, state: location.state });
      window.setTimeout(() => {
        toast({
          kind: "success",
          title: `Posted to ${chapterName}`,
          action: {
            label: "Copy link",
            onClick: () => {
              void navigator.clipboard
                ?.writeText(`${window.location.origin}/c/${slug}/${chapterId}/n/${note.id}`)
                .then(() => toast({ kind: "info", title: "Link copied" }));
            },
          },
        });
      }, 300);
    };
    setCelebrating(true);
    if (reduced) {
      window.setTimeout(open, 1200);
    } else {
      window.setTimeout(() => {
        const r = plateRef.current?.getBoundingClientRect();
        if (r) burst(r.left + r.width / 2, r.top + r.height / 2, theme === "dark");
      }, 900);
      window.setTimeout(open, 1000);
    }
  };

  const fail = (err: unknown) => {
    setPhase("edit");
    setProgress(0);
    if (!(err instanceof ApiError)) {
      setBanner({ kind: "error", icon: "wifi", text: "Couldn't reach the board. Check you're on the same network and try again.", retry: true });
      return;
    }
    const m = IMAGE_ERR.exec(err.message);
    if (m && (err.status === 400 || err.status === 415 || err.status === 413)) {
      const tile = tiles[Number(m[1]) - 1];
      if (tile) {
        setTileErrors((e) => ({ ...e, [tile.id]: `${m[2]} couldn't be read — it may be corrupt or an unsupported format.` }));
        return;
      }
    }
    switch (err.status) {
      case 404:
        setPhase("gone");
        return;
      case 413:
        setBanner({
          kind: "error",
          icon: "alert",
          text: "That's more than the board accepts in one go. Remove a photo or two, or use smaller ones (max 10 MB each, 60 MB total).",
        });
        return;
      case 415:
        setBanner({ kind: "error", icon: "alert", text: "The board only takes JPEG, PNG, WebP or GIF photos." });
        return;
      case 429: {
        const seconds = Math.ceil(err.retryAfter ?? 30);
        setBanner({ kind: "rate", until: Date.now() + seconds * 1000, seconds });
        return;
      }
      case 507:
        setBanner({ kind: "full" });
        return;
      case 0:
        setBanner({ kind: "error", icon: "wifi", text: err.message, retry: err.message !== "Upload cancelled." });
        return;
      default:
        if (err.status >= 500) {
          setBanner({ kind: "error", icon: "alert", text: "The board hit a snag. Try again in a moment.", retry: true });
        } else {
          setBanner({ kind: "error", icon: "alert", text: `Something's not right: ${err.message}` });
        }
    }
  };

  const submit = async () => {
    if (busy) return;
    setBanner(null);
    setTileErrors({});
    if (!validate()) return;
    if (!navigator.onLine) {
      setBanner({ kind: "error", icon: "wifi", text: "You're offline." });
      return;
    }
    setPhase("uploading");
    setProgress(0);
    notifyLocalUpload();
    const ctrl = new AbortController();
    abortRef.current = ctrl;
    try {
      const note = await uploadNote(
        { chapterId, title: title.trim(), body: body.trim(), authorName: author.trim(), files: tiles.map((t) => t.file) },
        (p) => {
          setProgress(p);
          if (p >= 1) setPhase("processing");
        },
        ctrl.signal,
      );
      succeed(note);
    } catch (e) {
      fail(e);
    } finally {
      abortRef.current = null;
    }
  };

  // Per-file progress from the overall byte count (DESIGN.md §4.6).
  const perFile = useMemo(() => {
    const sent = progress * total;
    let start = 0;
    return tiles.map((t) => {
      const p = Math.min(1, Math.max(0, (sent - start) / Math.max(1, t.file.size)));
      start += t.file.size;
      return phase === "processing" || phase === "success" ? 1 : p;
    });
  }, [progress, total, tiles, phase]);
  const uploadingIndex = perFile.findIndex((p) => p < 1);

  // ---- Render ----------------------------------------------------------------------------
  return (
    <div className="fixed inset-0 z-[70]" role="dialog" aria-modal="true" aria-labelledby={titleId}>
      <m.div
        className="absolute inset-0 bg-[var(--scrim)] backdrop-blur-[6px]"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0, transition: { duration: dur.base } }}
        transition={{ duration: dur.slow, ease: ease.outExpo }}
        onClick={requestClose}
        aria-hidden
      />
      <m.div
        ref={panelRef}
        layoutId="upload:surface"
        transition={spring.morph}
        drag={busy ? false : "y"}
        dragListener={false}
        dragControls={panelDrag}
        dragConstraints={{ top: 0, bottom: 0 }}
        dragElastic={{ top: 0, bottom: 0.7 }}
        onDragEnd={(_, info) => {
          if (info.offset.y > 120 || info.velocity.y > 800) requestClose();
        }}
        style={{ borderRadius: 28 }}
        className={clsx(
          "absolute inset-x-0 bottom-0 top-[calc(12px+env(safe-area-inset-top))] flex flex-col overflow-hidden border border-line bg-surface-2 shadow-[var(--shadow-3)]",
          "sm:inset-auto sm:left-1/2 sm:top-1/2 sm:max-h-[min(88dvh,860px)] sm:w-[min(720px,calc(100vw-48px))] sm:-translate-x-1/2 sm:-translate-y-1/2",
          "max-sm:rounded-b-none",
        )}
      >
        <m.div
          className="flex min-h-0 flex-1 flex-col"
          initial={{ opacity: 0 }}
          animate={{ opacity: celebrating ? 0.1 : 1 }}
          transition={{ duration: dur.base, delay: celebrating ? 0 : 0.1 }}
        >
          {/* Header */}
          <div className="flex items-start justify-between gap-3 px-5 pb-3 pt-3 sm:px-6 sm:pt-5">
            <div className="min-w-0">
              <div
                className="mx-auto mb-3 h-1 w-9 cursor-grab rounded-full bg-line sm:hidden"
                data-grabber
                aria-hidden
              />
              <h2 id={titleId} className="t-title-l text-fg-1">
                Upload to <span className="text-accent">{chapterName}</span>
              </h2>
            </div>
            <Button
              variant="ghost"
              size="icon"
              aria-label={busy ? "Cancel upload" : "Close"}
              onClick={() => {
                if (phase === "uploading" || phase === "processing") {
                  if (window.confirm("Cancel this upload?")) abortRef.current?.abort();
                } else requestClose();
              }}
              className="-mr-2 mt-1 shrink-0"
            >
              <X size={20} />
            </Button>
          </div>

          {phase === "gone" ? (
            <StatePanel
              pattern="x"
              title="This chapter is gone"
              actions={
                <Button variant="secondary" onClick={() => navigate(`/c/${slug}`, { replace: true })}>
                  Back to course
                </Button>
              }
            >
              It was removed while you were uploading. Your photos are still here — pick another chapter to post them.
            </StatePanel>
          ) : (
            <div className="min-h-0 flex-1 space-y-6 overflow-y-auto px-5 pb-6 sm:px-6">
              <AnimatePresence initial={false}>{banner && <BannerView banner={banner} onRetry={submit} onClose={leave} />}</AnimatePresence>

              <Dropzone
                compact={tiles.length > 0}
                disabled={busy || tiles.length >= MAX_FILES}
                full={tiles.length >= MAX_FILES}
                error={filesError}
                shake={shake}
                onFiles={addFiles}
              />

              {tiles.length > 0 && (
                <section aria-label="Photos">
                  <div className="mb-2 flex items-center justify-between">
                    <span className="t-caption text-fg-3">
                      Photos {tiles.length}/{MAX_FILES}
                    </span>
                    {tiles.length > 1 && <span className="t-caption text-fg-4">drag to sort</span>}
                  </div>
                  <Reorder.Group
                    axis="x"
                    values={tiles}
                    onReorder={busy ? () => {} : setTiles}
                    className="no-scrollbar -mx-1 flex gap-3 overflow-x-auto px-1 py-2"
                  >
                    <AnimatePresence initial={false}>
                      {tiles.map((t, i) => (
                        <TileView
                          key={t.id}
                          tile={t}
                          index={i}
                          busy={busy}
                          progress={perFile[i] ?? 0}
                          active={phase === "uploading" && i === uploadingIndex}
                          error={tileErrors[t.id] ?? problemOf(t.file)}
                          layoutId={i === 0 ? coverLayoutId : undefined}
                          onRemove={() => remove(t.id)}
                          onMove={(d) => move(t.id, d)}
                        />
                      ))}
                    </AnimatePresence>
                  </Reorder.Group>
                  <span className="sr-only" aria-live="polite">
                    {announce}
                  </span>
                  {Object.values(tileErrors)[0] && (
                    <p className="t-body-s mt-1 text-err" role="alert">
                      {Object.values(tileErrors)[0]} Remove it or pick another.
                    </p>
                  )}
                </section>
              )}

              <Input
                ref={titleRef}
                label="Title"
                required
                value={title}
                maxLength={200}
                disabled={busy}
                placeholder="e.g. Gaussian elimination"
                error={titleError}
                onChange={(e) => {
                  setTitle(e.target.value);
                  if (titleError && e.target.value.trim()) setTitleError(undefined);
                }}
                onBlur={() => title && !title.trim() && setTitleError("Give it a title so classmates can find it.")}
                hint={
                  title.length > 150 ? (
                    <span className={clsx("t-meta", title.length >= 200 ? "text-err" : title.length >= 190 ? "text-warn" : "text-fg-3")}>
                      {title.length} / 200
                    </span>
                  ) : undefined
                }
              />
              <Textarea label="Notes" optional value={body} disabled={busy} onChange={(e) => setBody(e.target.value)} />
              <Input
                label="Your name"
                optional
                value={author}
                maxLength={100}
                disabled={busy}
                placeholder="Anonymous"
                onChange={(e) => setAuthor(e.target.value)}
              />
            </div>
          )}

          {/* Footer */}
          {phase !== "gone" && (
            <div className="relative border-t border-line-subtle bg-surface-2 px-5 pb-[calc(16px+env(safe-area-inset-bottom))] pt-4 sm:px-6 sm:pb-5">
              {total > MAX_TOTAL && (
                <p className="t-body-s mb-3 text-err" role="alert">
                  Together these are {formatBytes(total)} — the limit is 60 MB per note. Remove a few.
                </p>
              )}
              <SubmitBar
                phase={phase}
                progress={progress}
                count={tiles.length}
                current={Math.max(1, uploadingIndex + 1)}
                disabled={invalid || total > MAX_TOTAL || banner?.kind === "full"}
                onSubmit={submit}
              />
              <AnimatePresence>
                {confirm && (
                  <m.div
                    initial={{ y: "100%" }}
                    animate={{ y: 0 }}
                    exit={{ y: "100%" }}
                    transition={spring.sheet}
                    className="absolute inset-0 flex items-center justify-between gap-3 bg-surface-3 px-5 sm:px-6"
                    role="alertdialog"
                    aria-label="Discard this upload?"
                  >
                    <span className="t-label text-fg-1">
                      Discard {tiles.length ? `${tiles.length} ${tiles.length === 1 ? "photo" : "photos"}` : "this note"}
                      {title.trim() ? " and your title" : ""}?
                    </span>
                    <div className="flex gap-2">
                      <Button variant="ghost" size="sm" onClick={() => setConfirm(false)} autoFocus>
                        Keep editing
                      </Button>
                      <Button
                        variant="danger"
                        size="sm"
                        onClick={() => {
                          try {
                            sessionStorage.removeItem(`notes.draft.${chapterId}`);
                          } catch {
                            // ignore
                          }
                          leave();
                        }}
                      >
                        Discard
                      </Button>
                    </div>
                  </m.div>
                )}
              </AnimatePresence>
            </div>
          )}
        </m.div>

        {/* Celebration: the board's upload arrow, then the LED burst */}
        <AnimatePresence>
          {celebrating && (
            <m.div
              className="pointer-events-none absolute inset-0 grid place-items-center"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: dur.fast }}
            >
              <m.div ref={plateRef} initial={{ scale: 0.9 }} animate={{ scale: 1 }} transition={spring.pop}>
                <LEDMatrix size="lg" pattern={reduced ? "check" : "arrow"} label="Posted" />
              </m.div>
            </m.div>
          )}
        </AnimatePresence>
      </m.div>
    </div>
  );
}

/** Drag-to-dismiss starts only from the grabber (mobile). */
function useDragControlsFor(panelRef: React.RefObject<HTMLDivElement | null>) {
  const controls = useDragControls();
  useEffect(() => {
    const panel = panelRef.current;
    if (!panel) return;
    const onDown = (e: PointerEvent) => {
      if ((e.target as HTMLElement).closest("[data-grabber]")) controls.start(e);
    };
    panel.addEventListener("pointerdown", onDown);
    return () => panel.removeEventListener("pointerdown", onDown);
  }, [controls, panelRef]);
  return controls;
}

// ---- Dropzone ------------------------------------------------------------------------------

function Dropzone({
  compact,
  disabled,
  full,
  error,
  shake,
  onFiles,
}: {
  compact: boolean;
  disabled: boolean;
  full: boolean;
  error?: string;
  shake: number;
  onFiles: (f: File[]) => void;
}) {
  const [over, setOver] = useState<"none" | "ok" | "bad">("none");
  const pick = useRef<HTMLInputElement>(null);
  const cam = useRef<HTMLInputElement>(null);
  const coarse = typeof window !== "undefined" && window.matchMedia("(pointer: coarse)").matches;
  const reduced = useReducedMotion();

  if (full) {
    return <p className="t-meta rounded-[14px] bg-surface-3 px-4 py-3 text-fg-3">10 of 10 photos — remove one to add more</p>;
  }
  const stroke = error || over === "bad" ? "var(--danger)" : over === "ok" ? "var(--accent)" : "var(--border-strong)";

  return (
    <m.div
      key={shake}
      animate={shake && !reduced ? { x: [0, -6, 6, -4, 4, -2, 0] } : undefined}
      transition={{ duration: 0.36, ease: ease.inOut }}
    >
      <m.div
        layout
        transition={spring.snappy}
        role="button"
        tabIndex={disabled ? -1 : 0}
        aria-disabled={disabled}
        aria-label="Add photos of your notes"
        onClick={() => !disabled && pick.current?.click()}
        onKeyDown={(e: KeyboardEvent) => {
          if ((e.key === "Enter" || e.key === " ") && !disabled) {
            e.preventDefault();
            pick.current?.click();
          }
        }}
        onDragOver={(e) => {
          e.preventDefault();
          const items = Array.from(e.dataTransfer.items);
          setOver(items.every((i) => i.kind === "file" && i.type.startsWith("image/")) ? "ok" : "bad");
        }}
        onDragLeave={() => setOver("none")}
        onDrop={(e) => {
          e.preventDefault();
          e.stopPropagation();
          setOver("none");
          onFiles(Array.from(e.dataTransfer.files));
        }}
        className={clsx(
          "relative flex cursor-pointer rounded-[20px] transition-colors duration-150",
          compact ? "items-center gap-4 px-5 py-4" : "flex-col items-center justify-center gap-3 px-6 py-8 text-center",
          over === "ok" ? "bg-accent-soft" : "bg-surface-1",
          disabled && "pointer-events-none opacity-60",
        )}
      >
        <svg className="pointer-events-none absolute inset-0 h-full w-full overflow-visible" aria-hidden>
          <rect
            x="1.5"
            y="1.5"
            rx="19"
            fill="none"
            stroke={stroke}
            strokeWidth="3"
            strokeLinecap="round"
            strokeDasharray="0 10"
            className={clsx(over === "ok" && "marching")}
            style={{ width: "calc(100% - 3px)", height: "calc(100% - 3px)" }}
          />
        </svg>
        <m.span animate={over === "ok" ? { y: -4 } : { y: 0 }} transition={spring.pop} className="text-accent">
          <ImagePlus size={compact ? 22 : 28} aria-hidden />
        </m.span>
        <div className={clsx(compact && "flex-1")}>
          <p className={clsx("t-title-s text-fg-1", compact && "whitespace-nowrap")}>
            {over === "ok" ? "Drop to add" : over === "bad" ? "Only photos, please" : compact ? "Add more" : "Add photos of your notes"}
          </p>
          {!compact && <p className="t-meta mt-1 text-fg-3">JPEG, PNG, WebP or GIF · up to 10 photos, 10 MB each</p>}
        </div>
        <div className={clsx("flex gap-2", !compact && "mt-2 w-full max-w-[360px]")} onClick={(e) => e.stopPropagation()}>
          {coarse && (
            <Button variant="secondary" size={compact ? "sm" : "md"} icon={<Camera size={18} aria-hidden />} className="flex-1" onClick={() => cam.current?.click()}>
              {compact ? "" : "Take photo"}
            </Button>
          )}
          <Button
            variant={compact ? "ghost" : "secondary"}
            size={compact ? "sm" : "md"}
            icon={<Plus size={18} aria-hidden />}
            className="flex-1"
            onClick={() => pick.current?.click()}
          >
            {compact ? (
              <>
                Choose<span className="hidden sm:inline"> photos</span>
              </>
            ) : (
              "Choose photos"
            )}
          </Button>
        </div>
        <input
          ref={pick}
          type="file"
          accept="image/jpeg,image/png,image/webp,image/gif"
          multiple
          hidden
          onChange={(e) => {
            onFiles(Array.from(e.target.files ?? []));
            e.target.value = "";
          }}
        />
        <input
          ref={cam}
          type="file"
          accept="image/*"
          capture="environment"
          hidden
          onChange={(e) => {
            onFiles(Array.from(e.target.files ?? []));
            e.target.value = "";
          }}
        />
      </m.div>
      <AnimatePresence>
        {error && (
          <m.p
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            exit={{ opacity: 0, height: 0 }}
            className="t-body-s mt-2 text-err"
            role="alert"
          >
            {error}
          </m.p>
        )}
      </AnimatePresence>
    </m.div>
  );
}

// ---- Tiles --------------------------------------------------------------------------------------

function TileView({
  tile,
  index,
  busy,
  progress,
  active,
  error,
  layoutId,
  onRemove,
  onMove,
}: {
  tile: Tile;
  index: number;
  busy: boolean;
  progress: number;
  active: boolean;
  error?: string;
  layoutId?: string;
  onRemove: () => void;
  onMove: (delta: number) => void;
}) {
  const controls = useDragControls();
  const timer = useRef<number | undefined>(undefined);
  const start = useRef({ x: 0, y: 0 });
  const [lifted, setLifted] = useState(false);
  const done = progress >= 1 && busy;
  const lit = Math.round(progress * 13);

  return (
    <Reorder.Item
      value={tile}
      dragListener={false}
      dragControls={controls}
      onDragStart={() => setLifted(true)}
      onDragEnd={() => setLifted(false)}
      initial={{ scale: 0.6, opacity: 0, rotate: index % 2 ? 3 : -3 }}
      animate={{ scale: lifted ? 1.06 : 1, opacity: 1, rotate: lifted ? (index % 2 ? 2 : -2) : 0 }}
      exit={{ scale: 0.6, opacity: 0, transition: { duration: dur.fast } }}
      transition={spring.pop}
      className={clsx("relative shrink-0 list-none", lifted && "z-[100]")}
      style={{ touchAction: "pan-x" }}
    >
      <m.div
        layoutId={layoutId}
        tabIndex={0}
        role="group"
        aria-label={`Photo ${index + 1}: ${tile.file.name}, ${formatBytes(tile.file.size)}${error ? `. ${error}` : ""}`}
        onKeyDown={(e) => {
          if (busy) return;
          if ((e.altKey || e.ctrlKey) && e.key === "ArrowLeft") {
            e.preventDefault();
            onMove(-1);
          } else if ((e.altKey || e.ctrlKey) && e.key === "ArrowRight") {
            e.preventDefault();
            onMove(1);
          } else if (e.key === "Delete" || e.key === "Backspace") {
            e.preventDefault();
            onRemove();
          }
        }}
        onPointerDown={(e) => {
          if (busy || (e.target as HTMLElement).closest("button")) return;
          if (e.pointerType === "mouse") {
            controls.start(e);
            return;
          }
          // Touch: long-press 250 ms to lift, so horizontal scrolling still works.
          start.current = { x: e.clientX, y: e.clientY };
          const ev = e.nativeEvent;
          timer.current = window.setTimeout(() => {
            navigator.vibrate?.(8);
            controls.start(ev);
          }, 250);
        }}
        onPointerMove={(e) => {
          if (timer.current && Math.hypot(e.clientX - start.current.x, e.clientY - start.current.y) > 8) {
            window.clearTimeout(timer.current);
            timer.current = undefined;
          }
        }}
        onPointerUp={() => window.clearTimeout(timer.current)}
        className={clsx(
          "group relative h-24 w-24 overflow-hidden rounded-[14px] bg-surface-3 outline-none sm:h-[120px] sm:w-[120px]",
          lifted ? "shadow-[var(--shadow-3)]" : "shadow-[var(--shadow-1)]",
          error ? "ring-2 ring-err" : "focus-visible:ring-2 focus-visible:ring-accent",
          done && "ring-2 ring-accent",
        )}
      >
        <img src={tile.url} alt="" className="h-full w-full object-cover" draggable={false} />
        {index === 0 && (
          <m.span layoutId="upload:coverBadge" transition={spring.snappy} className="absolute left-1.5 top-1.5">
            <Badge variant="cover">Cover</Badge>
          </m.span>
        )}
        {!busy && (
          <button
            type="button"
            onClick={onRemove}
            aria-label={`Remove photo ${index + 1}`}
            className="absolute right-0 top-0 grid h-11 w-11 place-items-center opacity-100 sm:opacity-0 sm:group-hover:opacity-100 sm:group-focus-visible:opacity-100"
          >
            <span className="grid h-6 w-6 place-items-center rounded-full bg-[rgba(5,7,13,0.65)] text-white backdrop-blur-md">
              <X size={14} />
            </span>
          </button>
        )}
        {error && (
          <span className="absolute bottom-1.5 right-1.5 grid h-5 w-5 place-items-center rounded-full bg-err text-[12px] font-bold text-white" aria-hidden>
            !
          </span>
        )}
        {!busy && !error && (
          <span className="t-meta absolute bottom-1 left-1.5 text-[11px] text-white opacity-0 drop-shadow transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100">
            {formatBytes(tile.file.size)}
          </span>
        )}
        {busy && (
          <>
            {/* dark veil lifting from the bottom as this file is sent */}
            <m.span
              aria-hidden
              className="absolute inset-0 origin-top bg-black/45"
              animate={{ scaleY: 1 - progress }}
              transition={{ duration: 0.1, ease: "linear" }}
            />
            <span className="absolute inset-x-1.5 bottom-1.5 flex justify-between" aria-hidden>
              {Array.from({ length: 13 }, (_, i) => (
                <span
                  key={i}
                  className="h-[3px] w-[3px] rounded-full"
                  style={{ background: i < lit ? LED_COLORS[done ? 7 : 5] : LED_COLORS[1] }}
                />
              ))}
            </span>
            {active && <span className="absolute inset-0 animate-pulse ring-2 ring-inset ring-accent/60" aria-hidden />}
          </>
        )}
      </m.div>
    </Reorder.Item>
  );
}

// ---- Submit + banners --------------------------------------------------------------------------------

function SubmitBar({
  phase,
  progress,
  count,
  current,
  disabled,
  onSubmit,
}: {
  phase: Phase;
  progress: number;
  count: number;
  current: number;
  disabled: boolean;
  onSubmit: () => void;
}) {
  const [slow, setSlow] = useState(false);
  useEffect(() => {
    if (phase !== "processing") return setSlow(false);
    const t = window.setTimeout(() => setSlow(true), 8000);
    return () => window.clearTimeout(t);
  }, [phase]);

  if (phase === "uploading" || phase === "processing" || phase === "success") {
    const pct = Math.round(progress * 100);
    return (
      <m.div
        layout
        className={clsx(
          "relative h-[52px] overflow-hidden rounded-[14px]",
          phase === "success" ? "bg-ok" : "bg-accent-soft",
        )}
        animate={phase === "success" ? { scale: [1, 1.04, 1] } : undefined}
        transition={spring.pop}
        role="progressbar"
        aria-valuenow={phase === "uploading" ? pct : undefined}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-label="Upload progress"
      >
        {phase === "uploading" && (
          <m.span
            aria-hidden
            className="absolute inset-y-0 left-0 w-full origin-left bg-accent"
            animate={{ scaleX: progress }}
            transition={{ duration: 0.15, ease: "linear" }}
          />
        )}
        <span
          className={clsx(
            "t-label relative flex h-full items-center justify-center gap-3 tabular-nums",
            phase === "processing" ? "text-fg-1" : "text-accent-fg",
            phase === "success" && "text-[#04130c]",
          )}
        >
          {phase === "uploading" && `Uploading ${current} of ${count} · ${pct}%`}
          {phase === "processing" && (
            <>
              <LEDLoader />
              <span>
                Processing on the board…
                {slow && <span className="t-body-s block text-fg-3">Big photos take a moment on a tiny computer.</span>}
              </span>
            </>
          )}
          {phase === "success" && "✓ Posted"}
        </span>
      </m.div>
    );
  }
  return (
    <Button variant="primary" size="lg" className="w-full" disabled={disabled} onClick={onSubmit}>
      {count > 0 ? `Post note (${count} ${count === 1 ? "photo" : "photos"})` : "Post note"}
    </Button>
  );
}

function BannerView({ banner, onRetry, onClose }: { banner: Banner; onRetry: () => void; onClose: () => void }) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (banner.kind !== "rate") return;
    const id = window.setInterval(() => setNow(Date.now()), 250);
    return () => window.clearInterval(id);
  }, [banner]);

  const tone = banner.kind === "error" ? "danger" : "warning";
  const Icon = banner.kind === "rate" ? Clock : banner.kind === "full" ? HardDrive : banner.icon === "wifi" ? WifiOff : TriangleAlert;
  const left = banner.kind === "rate" ? Math.max(0, Math.ceil((banner.until - now) / 1000)) : 0;

  return (
    <m.div
      layout
      initial={{ opacity: 0, height: 0 }}
      animate={{ opacity: 1, height: "auto" }}
      exit={{ opacity: 0, height: 0 }}
      transition={{ duration: dur.base }}
      role="alert"
      className="overflow-hidden"
    >
      <div
        className={clsx(
          "flex items-start gap-3 rounded-[14px] border p-4",
          tone === "danger" ? "border-err/40 bg-err-soft" : "border-warn/40 bg-warn-soft",
        )}
      >
        <m.span initial={{ scale: 0.5 }} animate={{ scale: 1 }} transition={spring.pop} className={tone === "danger" ? "text-err" : "text-warn"}>
          <Icon size={20} aria-hidden />
        </m.span>
        <div className="min-w-0 flex-1">
          <p className="t-body-s text-fg-1">
            {banner.kind === "error" && banner.text}
            {banner.kind === "rate" && `Easy there — too many uploads in a short time. You can try again in ${left}s.`}
            {banner.kind === "full" && "The board's storage is full, so it can't take new photos right now. Let your admin know."}
          </p>
          {banner.kind === "full" && (
            <div className="mt-3 flex items-center gap-3">
              <LEDMatrix size="sm" pattern="warning" />
              <Button variant="secondary" size="sm" onClick={onClose}>
                Close
              </Button>
            </div>
          )}
        </div>
        {banner.kind === "error" && banner.retry && (
          <Button variant="secondary" size="sm" icon={<RotateCcw size={16} aria-hidden />} onClick={onRetry}>
            Try again
          </Button>
        )}
        {banner.kind === "rate" && (
          <Button variant="secondary" size="sm" disabled={left > 0} onClick={onRetry} className="relative">
            {left > 0 && (
              <svg className="absolute -inset-1 h-[calc(100%+8px)] w-[calc(100%+8px)]" aria-hidden>
                <m.rect
                  x="2"
                  y="2"
                  rx="12"
                  fill="none"
                  stroke="var(--warning)"
                  strokeWidth="2"
                  style={{ width: "calc(100% - 4px)", height: "calc(100% - 4px)" }}
                  initial={{ pathLength: 1 }}
                  animate={{ pathLength: 0 }}
                  transition={{ duration: banner.seconds, ease: "linear" }}
                />
              </svg>
            )}
            {left > 0 ? `Try again in ${left}s` : "Try again"}
          </Button>
        )}
      </div>
    </m.div>
  );
}
