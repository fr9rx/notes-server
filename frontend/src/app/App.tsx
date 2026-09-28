import { AnimatePresence, LayoutGroup, LazyMotion, MotionConfig, domMax } from "motion/react";
import { Suspense, forwardRef, lazy, useLayoutEffect, useRef } from "react";
import { Route, Routes, matchPath, useLocation, useNavigationType, type Location } from "react-router";

import { Footer } from "../components/Footer";
import { Header } from "../components/Header";
import { spring } from "../motion/springs";
import ChapterPage from "../pages/Chapter";
import CoursePage from "../pages/Course";
import Home from "../pages/Home";
import NotFound from "../pages/NotFound";
import { DirectionContext } from "./page";

const NoteView = lazy(() => import("../pages/NoteView"));
const UploadSheet = lazy(() => import("../pages/Upload"));

/** The page a URL belongs to: modal routes (note, upload) stay on their chapter page. */
function pageKey(pathname: string): string {
  const m = pathname.match(/^\/c\/([^/]+)(?:\/([^/]+))?/);
  if (!m) return pathname === "/" ? "/" : `404:${pathname}`;
  return m[2] && m[2] !== "" ? `/c/${m[1]}/${m[2]}` : `/c/${m[1]}`;
}

function depthOf(key: string): number {
  if (key === "/") return 0;
  if (key.startsWith("404:")) return -1;
  return key.split("/").length - 2; // /c/x -> 1, /c/x/y -> 2
}

const RoutePage = forwardRef<HTMLDivElement, { location: Location }>(function RoutePage({ location }, ref) {
  return (
    <div ref={ref} className="w-full">
      <Routes location={location}>
        <Route path="/" element={<Home />} />
        <Route path="/c/:slug" element={<CoursePage />} />
        <Route path="/c/:slug/:chapterId/*" element={<ChapterPage />} />
        <Route path="*" element={<NotFound />} />
      </Routes>
    </div>
  );
});

export function App() {
  const location = useLocation();
  const navType = useNavigationType();
  const key = pageKey(location.pathname);
  const depth = depthOf(key);

  // Direction: deeper = +1, shallower = -1, unrelated (404) = 0.
  const prev = useRef({ key, depth });
  const dirRef = useRef(1);
  if (prev.current.key !== key) {
    const from = prev.current.depth;
    dirRef.current = from < 0 || depth < 0 ? 0 : Math.sign(depth - from) || 1;
  }

  // Scroll: new pages start at the top; Back restores where you were (before paint).
  const scrolls = useRef(new Map<string, number>());
  useLayoutEffect(() => {
    if (prev.current.key === key) return;
    scrolls.current.set(prev.current.key, window.scrollY);
    const saved = navType === "POP" ? scrolls.current.get(key) : undefined;
    window.scrollTo({ top: saved ?? 0, behavior: "instant" as ScrollBehavior });
    prev.current = { key, depth };
  }, [key, depth, navType]);

  const note = matchPath("/c/:slug/:chapterId/n/:noteId", location.pathname);
  const upload = matchPath("/c/:slug/:chapterId/upload", location.pathname);
  const modalOpen = Boolean(note || upload);

  return (
    <LazyMotion features={domMax} strict>
      <MotionConfig reducedMotion="user" transition={spring.ui}>
        <LayoutGroup id="app">
          <div className="page-grid" aria-hidden />
          <Header />
          <DirectionContext.Provider value={dirRef.current}>
            <main id="main" className="relative min-h-[70vh]" inert={modalOpen || undefined}>
              <AnimatePresence mode="popLayout" initial={false} custom={dirRef.current}>
                <RoutePage key={key} location={location} />
              </AnimatePresence>
            </main>
          </DirectionContext.Provider>
          <Footer />
          <Suspense fallback={null}>
            <AnimatePresence>
              {note && (
                <NoteView
                  key={`note:${note.params.noteId}`}
                  slug={note.params.slug ?? ""}
                  chapterId={note.params.chapterId ?? ""}
                  noteId={note.params.noteId ?? ""}
                />
              )}
              {upload && (
                <UploadSheet key="upload" slug={upload.params.slug ?? ""} chapterId={upload.params.chapterId ?? ""} />
              )}
            </AnimatePresence>
          </Suspense>
          <div className="grain" aria-hidden />
        </LayoutGroup>
      </MotionConfig>
    </LazyMotion>
  );
}
