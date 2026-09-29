import clsx from "clsx";
import { m, useInView, useReducedMotion, useScroll, useSpring } from "motion/react";
import { ChevronRight } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Link, useParams } from "react-router";

import { Page, useDirection, usePageItemVariants } from "../app/page";
import { ButtonLink } from "../components/Button";
import { DotShimmer, Delayed, TextSkeleton } from "../components/Skeleton";
import { StatePanel } from "../components/StatePanel";
import { LED_COLORS } from "../led/sprites";
import { LEDMatrix } from "../led/LEDMatrix";
import { monogramText } from "../led/views";
import { ApiError } from "../lib/api";
import { pad2, plural } from "../lib/format";
import { useCourse } from "../lib/queries";
import type { ChapterSummary } from "../lib/types";
import { ease, spring } from "../motion/springs";

export default function CoursePage() {
  const { slug = "" } = useParams();
  const { data, error, isPending } = useCourse(slug);
  const reduced = useReducedMotion();

  useEffect(() => {
    if (data) document.title = `${data.name} · notes`;
  }, [data]);

  if (error) {
    const gone = error instanceof ApiError && error.status === 404;
    return (
      <Page className="pt-24">
        <StatePanel
          pattern={gone ? "x" : "blinkX"}
          title={gone ? "This course isn't here anymore" : "Can't reach the board"}
          titleAs="h1"
          actions={<ButtonLink to="/" variant="secondary">Back to courses</ButtonLink>}
        >
          {gone ? "It may have been renamed or removed." : "It might be restarting — this page will retry on its own."}
        </StatePanel>
      </Page>
    );
  }

  const totals = data?.chapters.reduce(
    (acc, c) => ({ notes: acc.notes + c.note_count, photos: acc.photos + c.image_count }),
    { notes: 0, photos: 0 },
  );

  return (
    <Page className="mx-auto max-w-[1240px] px-4 pt-20 sm:px-6 md:pt-24 lg:px-8">
      {/* The card surface from Home morphs into this panel. */}
      <m.header
        layoutId={`course:${slug}:surface`}
        transition={spring.morph}
        style={{ borderRadius: 28 }}
        className="relative overflow-hidden border border-line-subtle bg-surface-1 p-5 shadow-[var(--shadow-2),var(--hl-inset)] sm:p-8"
      >
        <div
          aria-hidden
          className="pointer-events-none absolute inset-0 opacity-60"
          style={{
            backgroundImage: "radial-gradient(circle at 1px 1px, var(--grid-dot) 1px, transparent 0)",
            backgroundSize: "14px 14px",
            maskImage: "linear-gradient(110deg, #000, transparent 70%)",
          }}
        />
        {data?.custom_cover && data.cover_url && (
          // The admin's cover photo, bleeding in from the right behind the title.
          <m.img
            src={data.cover_url}
            alt=""
            aria-hidden
            draggable={false}
            initial={reduced ? { opacity: 0 } : { opacity: 0, scale: 1.06 }}
            animate={{ opacity: 1, scale: 1 }}
            transition={{ duration: 1.1, ease: ease.outExpo }}
            className="pointer-events-none absolute inset-y-0 right-0 h-full w-full object-cover opacity-40 sm:w-[64%] sm:opacity-100"
            style={{
              maskImage: "linear-gradient(to left, #000 30%, transparent 96%)",
              WebkitMaskImage: "linear-gradient(to left, #000 30%, transparent 96%)",
            }}
          />
        )}
        <div className="relative flex flex-col gap-6 sm:flex-row sm:items-center sm:gap-10">
          <m.div layoutId={`course:${slug}:monogram`} transition={spring.morph} className="shrink-0 self-start">
            {data ? (
              <LEDMatrix
                size="lg"
                dot={9}
                pitch={14}
                pattern="monogram"
                monogram={monogramText(data.name)}
                seed={slug}
                scan={1}
              />
            ) : (
              <DotShimmer className="h-[152px] w-[222px] rounded-[28px]" />
            )}
          </m.div>
          <div className="min-w-0">
            {data ? (
              <m.div
                initial={reduced ? { opacity: 0 } : { opacity: 0, y: 8 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ ...spring.soft, delay: 0.12 }}
              >
                <h1 className="t-display-l text-fg-1">{data.name}</h1>
                {data.description && <p className="t-body-l mt-3 max-w-[60ch] text-fg-2">{data.description}</p>}
                <p className="t-caption mt-5 text-fg-3">
                  {plural(data.chapters.length, "chapter")} · {plural(totals?.notes ?? 0, "note")} ·{" "}
                  {plural(totals?.photos ?? 0, "photo")}
                </p>
              </m.div>
            ) : (
              <Delayed>
                <TextSkeleton widths={["60%", "90%", "40%"]} />
              </Delayed>
            )}
          </div>
        </div>
      </m.header>

      <section className="mx-auto mt-12 max-w-[880px] md:mt-16" aria-label="Chapters">
        {isPending ? (
          <Delayed>
            <div className="space-y-3">
              {Array.from({ length: 5 }, (_, i) => (
                <div key={i} className="flex items-center gap-5 py-4 pl-12">
                  <div className="h-3 w-8 rounded-full bg-surface-3" />
                  <TextSkeleton className="flex-1" widths={["50%", "25%"]} />
                </div>
              ))}
            </div>
          </Delayed>
        ) : data && data.chapters.length === 0 ? (
          <StatePanel pattern="waiting" title="No chapters yet">
            The admin adds chapters from <code className="t-meta rounded bg-surface-3 px-1.5 py-0.5">notes-admin</code>. Check back
            soon.
          </StatePanel>
        ) : data ? (
          <ChapterRail slug={slug} chapters={data.chapters} />
        ) : null}
      </section>
    </Page>
  );
}

function ChapterRail({ slug, chapters }: { slug: string; chapters: ChapterSummary[] }) {
  const listRef = useRef<HTMLOListElement>(null);
  const reduced = useReducedMotion();
  const { scrollYProgress } = useScroll({ target: listRef, offset: ["start 70%", "end 60%"] });
  const lit = useSpring(scrollYProgress, { stiffness: 120, damping: 30 });

  return (
    <ol ref={listRef} className="relative">
      {/* The signal travelling down the course: an unlit dotted rail + a lit overlay that follows scroll. */}
      <span
        aria-hidden
        className="absolute bottom-6 left-[19px] top-6 w-[4px]"
        style={{
          backgroundImage: `radial-gradient(circle, ${LED_COLORS[1]} 1.5px, transparent 1.8px)`,
          backgroundSize: "4px 8px",
        }}
      />
      <m.span
        aria-hidden
        className="absolute bottom-6 left-[19px] top-6 w-[4px] origin-top"
        style={{
          scaleY: reduced ? 1 : lit,
          backgroundImage: `radial-gradient(circle, ${LED_COLORS[5]} 1.5px, transparent 1.8px)`,
          backgroundSize: "4px 8px",
          filter: "drop-shadow(0 0 4px rgba(76,154,255,0.6))",
        }}
      />
      {chapters.map((c, i) => (
        <ChapterRow key={c.id} slug={slug} chapter={c} index={i} />
      ))}
    </ol>
  );
}

function ChapterRow({ slug, chapter, index }: { slug: string; chapter: ChapterSummary; index: number }) {
  const ref = useRef<HTMLLIElement>(null);
  const reached = useInView(ref, { margin: "0px 0px -40% 0px", once: true });
  const [settled, setSettled] = useState(false);
  useEffect(() => {
    if (!reached) return;
    const t = window.setTimeout(() => setSettled(true), 280);
    return () => window.clearTimeout(t);
  }, [reached]);
  const variants = usePageItemVariants();
  const dir = useDirection();
  const empty = chapter.note_count === 0;
  const level = !reached ? 1 : empty ? 2 : settled ? 4 : 7;

  return (
    <m.li ref={ref} custom={{ dir, i: index + 1 }} variants={variants} className="relative list-none">
        <Link
          to={`/c/${slug}/${chapter.id}`}
          className="group relative flex items-center gap-4 rounded-[20px] py-4 pl-12 pr-3 transition-colors hover:bg-surface-1 sm:gap-6 sm:py-5"
        >
          <span
            aria-hidden
            className="absolute left-[16px] top-1/2 h-[10px] w-[10px] -translate-y-1/2 rounded-full transition-[background-color,box-shadow] duration-[280ms] [transition-timing-function:steps(7,jump-end)]"
            style={{
              background: LED_COLORS[level],
              boxShadow: level >= 4 ? `0 0 ${level * 2}px rgba(76,154,255,${level / 10})` : "none",
            }}
          />
          <m.span
            layoutId={`chapter:${chapter.id}:index`}
            transition={spring.morph}
            className="t-meta w-8 shrink-0 text-fg-3 sm:text-[28px] sm:leading-none sm:tracking-[-0.02em]"
          >
            {pad2(chapter.position + 1)}
          </m.span>
          <span className="min-w-0 flex-1 transition-transform duration-200 ease-[cubic-bezier(0.22,1,0.36,1)] group-hover:translate-x-1">
            <m.span
              layoutId={`chapter:${chapter.id}:title`}
              transition={spring.morph}
              className="t-title-s block truncate text-fg-1 sm:text-[18px]"
            >
              {chapter.title}
            </m.span>
            <span className={clsx("t-meta mt-0.5 block", "text-fg-3")}>
              {empty ? "No notes yet — be first" : `${plural(chapter.note_count, "note")} · ${plural(chapter.image_count, "photo")}`}
            </span>
          </span>
          {chapter.cover_thumb_url && <ThumbStack src={chapter.cover_thumb_url} />}
          <ChevronRight
            size={18}
            aria-hidden
            className="shrink-0 text-fg-3 transition-transform duration-200 group-hover:translate-x-0.5 group-hover:text-accent"
          />
        </Link>
    </m.li>
  );
}

/** Two overlapping photos, like prints on a desk; they fan out on hover. */
function ThumbStack({ src }: { src: string }) {
  return (
    <span aria-hidden className="relative hidden h-10 w-14 shrink-0 md:block">
      <span className="absolute inset-y-0 right-4 w-10 -rotate-4 overflow-hidden rounded-[8px] border border-line bg-surface-3 shadow-[var(--shadow-1)] transition-transform duration-300 ease-[cubic-bezier(0.22,1,0.36,1)] group-hover:-translate-x-1.5 group-hover:-rotate-8">
        <img src={src} alt="" className="h-full w-full object-cover opacity-70" loading="lazy" />
      </span>
      <span className="absolute inset-y-0 right-0 w-10 rotate-3 overflow-hidden rounded-[8px] border border-line bg-surface-3 shadow-[var(--shadow-2)] transition-transform duration-300 ease-[cubic-bezier(0.22,1,0.36,1)] group-hover:translate-x-1.5 group-hover:rotate-7">
        <img src={src} alt="" className="h-full w-full object-cover" loading="lazy" />
      </span>
    </span>
  );
}
