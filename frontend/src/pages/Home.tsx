import clsx from "clsx";
import { AnimatePresence, m, useMotionValue, useReducedMotion, useScroll, useSpring, useTransform } from "motion/react";
import { ArrowDown, ArrowUpRight } from "lucide-react";
import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { Link } from "react-router";

import { Page, PageItem, useDirection } from "../app/page";
import { useToast } from "../app/toast";
import { Button } from "../components/Button";
import { Counter } from "../components/Counter";
import { DotShimmer, Delayed, TextSkeleton } from "../components/Skeleton";
import { StatePanel } from "../components/StatePanel";
import { LEDField } from "../led/LEDField";
import { LEDMatrix } from "../led/LEDMatrix";
import { monogramText } from "../led/views";
import { formatUptime, plural } from "../lib/format";
import { useCourses } from "../lib/queries";
import type { CourseSummary } from "../lib/types";
import { dur, ease, gridDelay, spring } from "../motion/springs";
import { useLive } from "../stats/store";

export default function Home() {
  useEffect(() => {
    document.title = "notes";
  }, []);
  return (
    <Page>
      <Hero />
      <StatsRow />
      <CourseSection />
    </Page>
  );
}

// ---- Hero ------------------------------------------------------------------------

const HEADLINE = [["Every", "note"], ["from", "class,"], ["in", "one", "place."]];

function Hero() {
  const hostRef = useRef<HTMLElement>(null);
  const plateRef = useRef<HTMLDivElement>(null);
  const copyRef = useRef<HTMLDivElement>(null);
  const reduced = useReducedMotion();
  const live = useLive();
  const { scrollY } = useScroll();
  const copyY = useTransform(scrollY, [0, 360], [0, -48]);
  const copyOpacity = useTransform(scrollY, [0, 360], [1, 0]);
  const plateY = useTransform(scrollY, [0, 480], [0, -80]);
  const plateScale = useTransform(scrollY, [0, 480], [1, 0.9]);
  const plateRotateX = useTransform(scrollY, [0, 480], [0, 14]);
  const fieldOpacity = useTransform(scrollY, [0, 600], [1, 0.25]);
  const bloom = useSpring(0.2, { stiffness: 60, damping: 20 });

  // Plate tilts toward the cursor (fine pointers).
  const tiltX = useSpring(0, spring.tilt);
  const tiltY = useSpring(0, spring.tilt);
  const onMove = (e: ReactPointerEvent) => {
    if (reduced || e.pointerType !== "mouse") return;
    const r = plateRef.current?.getBoundingClientRect();
    if (!r) return;
    tiltY.set(((e.clientX - (r.left + r.width / 2)) / window.innerWidth) * 12);
    tiltX.set(-((e.clientY - (r.top + r.height / 2)) / window.innerHeight) * 12);
  };

  const s = live.stats;
  const [how, setHow] = useState(false);

  return (
    <section
      ref={hostRef}
      onPointerMove={onMove}
      onPointerLeave={() => {
        tiltX.set(0);
        tiltY.set(0);
      }}
      className="relative isolate overflow-hidden pb-16 pt-[152px] md:pb-24 md:pt-[208px]"
      style={{ minHeight: "min(88vh, 860px)" }}
    >
      <m.div
        className="absolute inset-0 -z-10"
        style={{ opacity: fieldOpacity }}
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ duration: 0.6, ease: ease.outExpo }}
      >
        <LEDField hostRef={hostRef} originRef={plateRef} avoidRef={copyRef} />
      </m.div>

      <div className="mx-auto grid max-w-[1240px] items-center gap-12 px-4 sm:px-6 lg:grid-cols-12 lg:px-8">
        <m.div ref={copyRef} style={reduced ? undefined : { y: copyY, opacity: copyOpacity }} className="lg:col-span-7">
          <m.p
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ delay: 0.1, duration: dur.base }}
            className="t-caption flex items-center gap-2 text-accent"
          >
            <HeartbeatDot tick={live.tick} down={live.state === "down"} />
            Live on the LAN
          </m.p>
          <h1 className="t-display-xl mt-5 text-fg-1">
            {HEADLINE.map((line, li) => (
              <span key={li} className="block overflow-hidden pb-[0.06em]">
                {line.map((word, wi) => {
                  const idx = HEADLINE.slice(0, li).flat().length + wi;
                  return (
                    <m.span
                      key={wi}
                      className="mr-[0.24em] inline-block"
                      initial={reduced ? { opacity: 0 } : { y: "110%" }}
                      animate={reduced ? { opacity: 1 } : { y: "0%" }}
                      transition={{ ...spring.soft, delay: 0.18 + idx * 0.07 }}
                    >
                      {word === "place." ? (
                        <>
                          place<span className="text-accent">.</span>
                        </>
                      ) : (
                        word
                      )}
                    </m.span>
                  );
                })}
              </span>
            ))}
          </h1>
          <m.p
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ ...spring.soft, delay: 0.52 }}
            className="t-body-l mt-6 max-w-[34rem] text-fg-2"
          >
            Photos of whiteboards, scans and handwritten pages — shared by students, served from a tiny board on your
            network. No accounts, no cloud.
          </m.p>
          <m.div
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ ...spring.soft, delay: 0.64 }}
            className="relative mt-8 flex flex-col gap-3 xs:flex-row"
          >
            <Button
              variant="primary"
              size="lg"
              trailing={<ArrowDown size={18} aria-hidden />}
              onClick={() => document.getElementById("courses")?.scrollIntoView({ behavior: reduced ? "auto" : "smooth" })}
            >
              Browse courses
            </Button>
            <Button variant="ghost" size="lg" aria-expanded={how} onClick={() => setHow((h) => !h)}>
              How it works
            </Button>
            <AnimatePresence>
              {how && (
                <m.ol
                  initial={{ opacity: 0, y: -6, scale: 0.97 }}
                  animate={{ opacity: 1, y: 0, scale: 1 }}
                  exit={{ opacity: 0, y: -6, scale: 0.97, transition: { duration: dur.fast } }}
                  transition={spring.snappy}
                  className="t-body-s absolute left-0 top-[calc(100%+10px)] z-10 w-[min(100%,360px)] space-y-2 rounded-[20px] border border-line bg-surface-2 p-4 text-fg-2 shadow-[var(--shadow-3)] xs:left-auto xs:right-auto xs:translate-x-[180px]"
                >
                  {["Pick a course and chapter.", "Tap Upload and snap your notes.", "Everyone on the network sees them instantly."].map(
                    (t, i) => (
                      <li key={t} className="flex gap-3">
                        <span className="t-meta text-accent">{i + 1}</span>
                        {t}
                      </li>
                    ),
                  )}
                </m.ol>
              )}
            </AnimatePresence>
          </m.div>
        </m.div>

        <m.div
          style={reduced ? undefined : { y: plateY, scale: plateScale, rotateX: plateRotateX, transformPerspective: 1000 }}
          className="flex flex-col items-center lg:col-span-5"
        >
          <m.div
            ref={plateRef}
            initial={reduced ? { opacity: 0 } : { opacity: 0, y: 24, rotateX: 18 }}
            animate={{ opacity: 1, y: 0, rotateX: 0 }}
            transition={{ ...spring.soft, delay: 0.3 }}
            style={{ rotateX: tiltX, rotateY: tiltY, transformPerspective: 900 }}
            className="relative w-full max-w-[340px] lg:max-w-none"
          >
            <div className="flex justify-center">
              <div className="relative origin-top scale-[0.86] xs:scale-100">
                {/* The plate "lights the room" when the board is busy. */}
                <m.div
                  aria-hidden
                  className="pointer-events-none absolute inset-6 -z-10 rounded-[28px]"
                  style={{ opacity: bloom, boxShadow: "var(--glow-lg)" }}
                />
                <LEDMatrix
                  source="stats"
                  size="hero"
                  boot
                  silk="UNO Q · LED 8×13"
                  onBrightness={(b) => bloom.set(0.2 + b * 4)}
                />
              </div>
            </div>
          </m.div>
          <p className="t-caption mt-4 text-fg-3">
            UNO Q · 8×13 · {s?.requests[12] ?? 0} req/s{s ? ` · up ${formatUptime(s.uptime_secs)}` : ""}
          </p>
        </m.div>
      </div>
    </section>
  );
}

function HeartbeatDot({ tick, down }: { tick: number; down: boolean }) {
  return (
    <span className="relative inline-flex h-2 w-2" aria-hidden>
      <m.span
        key={tick}
        className="absolute inset-0 rounded-full bg-accent"
        initial={{ scale: 1, opacity: 0.7 }}
        animate={{ scale: 3.2, opacity: 0 }}
        transition={{ duration: 0.9, ease: ease.outExpo }}
      />
      <span
        className={clsx("relative h-2 w-2 rounded-full", down ? "bg-err" : "bg-accent")}
        style={{ boxShadow: "0 0 10px var(--accent)" }}
      />
    </span>
  );
}

// ---- Stats ---------------------------------------------------------------------------

function StatsRow() {
  const { stats } = useLive();
  const items = [
    ["Courses", stats?.courses],
    ["Chapters", stats?.chapters],
    ["Notes", stats?.notes],
    ["Photos", stats?.images],
  ] as const;
  return (
    <PageItem as="section" i={2} className="mx-auto max-w-[1240px] px-4 sm:px-6 lg:px-8">
      <h2 className="sr-only">At a glance</h2>
      <div className="grid grid-cols-2 gap-2.5 md:grid-cols-4 md:gap-0 md:divide-x md:divide-line-subtle md:border-y md:border-line-subtle">
        {items.map(([label, value]) => (
          <div
            key={label}
            className="rounded-[20px] border border-line-subtle bg-surface-1 p-4 md:rounded-none md:border-0 md:bg-transparent md:px-8 md:py-7"
          >
            <Counter value={value ?? 0} label={label} />
          </div>
        ))}
      </div>
    </PageItem>
  );
}

// ---- Courses -----------------------------------------------------------------------------

function useColumns() {
  const [cols, setCols] = useState(1);
  useEffect(() => {
    const update = () => setCols(window.innerWidth >= 1024 ? 3 : window.innerWidth >= 640 ? 2 : 1);
    update();
    window.addEventListener("resize", update);
    return () => window.removeEventListener("resize", update);
  }, []);
  return cols;
}

function CourseSection() {
  const { data, error, isPending, refetch, isFetching } = useCourses();
  const cols = useColumns();
  const toast = useToast();
  const prevCount = useRef<number | undefined>(undefined);
  useEffect(() => {
    if (!data) return;
    if (prevCount.current === 0 && data.length > 0) {
      toast({ kind: "success", title: `New course: ${data[0]?.name ?? ""}` });
    }
    prevCount.current = data.length;
  }, [data, toast]);

  return (
    <PageItem as="section" i={3} className="mx-auto mt-16 max-w-[1240px] scroll-mt-24 px-4 sm:px-6 md:mt-24 lg:px-8">
      <div id="courses" className="flex items-baseline justify-between gap-4">
        <h2 className="t-display-m text-fg-1">Courses</h2>
        {data && data.length > 0 && <span className="t-meta text-fg-3">{data.length} total</span>}
      </div>

      <div className="mt-8">
        {isPending ? (
          <Delayed>
            <div className="grid gap-4 sm:grid-cols-2 sm:gap-6 lg:grid-cols-3">
              {Array.from({ length: cols === 1 ? 3 : 6 }, (_, i) => (
                <CardSkeleton key={i} index={i} />
              ))}
            </div>
          </Delayed>
        ) : error ? (
          <StatePanel
            pattern="blinkX"
            title="Can't reach the board"
            actions={
              <Button variant="secondary" loading={isFetching} onClick={() => refetch()}>
                Retry now
              </Button>
            }
          >
            The server didn't answer. It might be restarting — this page will retry on its own.
          </StatePanel>
        ) : !data ? null : data.length === 0 ? (
          <StatePanel pattern="waiting" title="No courses yet">
            Courses are created by the admin with the <code className="t-meta rounded bg-surface-3 px-1.5 py-0.5">notes-admin</code>{" "}
            terminal app. As soon as one exists, it'll appear here — no refresh needed.
          </StatePanel>
        ) : (
          <div className="grid gap-4 sm:grid-cols-2 sm:gap-6 lg:grid-cols-3">
            {data.map((c, i) => (
              <CourseCard key={c.id} course={c} delay={gridDelay(i, cols)} />
            ))}
          </div>
        )}
      </div>
    </PageItem>
  );
}

function CardSkeleton({ index }: { index: number }) {
  return (
    <div className="rounded-[20px] border border-line-subtle bg-surface-1 p-1.5">
      <DotShimmer className="aspect-video rounded-[14px]" index={index} />
      <TextSkeleton className="px-2.5 pb-3 pt-4" widths={["70%", "45%"]} />
    </div>
  );
}

function CourseCard({ course, delay }: { course: CourseSummary; delay: number }) {
  const ref = useRef<HTMLAnchorElement>(null);
  const reduced = useReducedMotion();
  const dir = useDirection();
  const rx = useMotionValue(0);
  const ry = useMotionValue(0);
  const rotateX = useSpring(rx, spring.tilt);
  const rotateY = useSpring(ry, spring.tilt);
  const [scan, setScan] = useState(0);
  const [hover, setHover] = useState(false);

  const onMove = (e: ReactPointerEvent) => {
    const el = ref.current;
    if (!el || e.pointerType !== "mouse") return;
    const r = el.getBoundingClientRect();
    const px = (e.clientX - r.left) / r.width;
    const py = (e.clientY - r.top) / r.height;
    el.style.setProperty("--mx", `${px * 100}%`);
    el.style.setProperty("--my", `${py * 100}%`);
    if (!reduced) {
      rx.set(-(py - 0.5) * 7);
      ry.set((px - 0.5) * 9);
    }
  };
  const reset = () => {
    rx.set(0);
    ry.set(0);
    setHover(false);
  };

  return (
    <m.div
      custom={{ dir, i: 0 }}
      variants={{
        hidden: { opacity: 0, y: 14, filter: "blur(4px)" },
        show: { opacity: 1, y: 0, filter: "blur(0px)", transition: { ...spring.soft, delay: 0.1 + delay } },
        exit: { opacity: 0, y: -8, transition: { duration: 0.16 } },
      }}
      style={{ perspective: 900 }}
    >
      <m.div
        layoutId={`course:${course.slug}:surface`}
        transition={spring.morph}
        style={{ rotateX, rotateY, borderRadius: 20 }}
        whileHover={reduced ? undefined : { y: -4 }}
        whileTap={{ scale: 0.985 }}
        className="group relative border border-line-subtle bg-surface-1 shadow-[var(--shadow-1),var(--hl-inset)] transition-colors hover:border-line"
      >
        <Link
          ref={ref}
          to={`/c/${course.slug}`}
          onPointerMove={onMove}
          onPointerEnter={() => {
            setScan((s) => s + 1);
            setHover(true);
          }}
          onPointerLeave={reset}
          onFocus={() => setHover(true)}
          onBlur={() => setHover(false)}
          className="spotlight block rounded-[20px] p-1.5 focus-visible:outline-offset-4"
        >
          <div className="relative z-[1] aspect-video overflow-hidden rounded-[14px] bg-plate">
            {course.cover_thumb_url ? (
              <>
                <div className="halftone">
                  <img src={course.cover_thumb_url} alt="" className="h-full w-full object-cover" loading="lazy" />
                </div>
                <m.img
                  src={course.cover_thumb_url}
                  alt=""
                  loading="lazy"
                  className="absolute inset-0 h-full w-full object-cover"
                  animate={{ opacity: hover ? 1 : 0, scale: hover ? 1.04 : 1 }}
                  transition={{ duration: dur.slow, ease: ease.outExpo }}
                />
              </>
            ) : (
              <div className="grid h-full place-items-center">
                <LEDMatrix size="md" plate={false} pattern="monogram" monogram={monogramText(course.name)} seed={course.slug} scan={scan} />
              </div>
            )}
          </div>
          <div className="relative z-[3] -mt-5 ml-2.5 inline-block">
            <m.div layoutId={`course:${course.slug}:monogram`} transition={spring.morph}>
              <LEDMatrix
                size="xs"
                plate
                pattern="monogram"
                monogram={monogramText(course.name)}
                seed={course.slug}
                scan={scan}
              />
            </m.div>
          </div>
          <div className="relative z-[1] px-2.5 pb-2.5 pt-3">
            <h3 className="t-title-m text-fg-1">{course.name}</h3>
            {course.description && <p className="t-body-s mt-1 line-clamp-2 text-fg-2">{course.description}</p>}
            <div className="mt-4 flex items-center justify-between">
              <span className="t-caption text-fg-3">
                {plural(course.chapter_count, "ch", "ch")} · {plural(course.note_count, "note")} ·{" "}
                {plural(course.image_count, "photo")}
              </span>
              <ArrowUpRight
                size={16}
                className="text-fg-3 transition-transform duration-150 group-hover:-translate-y-0.5 group-hover:translate-x-0.5 group-hover:text-accent"
                aria-hidden
              />
            </div>
          </div>
        </Link>
      </m.div>
    </m.div>
  );
}
