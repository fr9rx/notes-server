// Tiny cross-route hand-offs for the upload flow.

/** Files dropped on the chapter page, picked up by the upload sheet when it opens. */
let pendingFiles: File[] = [];
export function setPendingFiles(files: File[]) {
  pendingFiles = files;
}
export function takePendingFiles(): File[] {
  const f = pendingFiles;
  pendingFiles = [];
  return f;
}

/** The note just posted from this tab: its card pops in with a highlighter ring. */
let justPosted: string | undefined;
export function markJustPosted(id: string) {
  justPosted = id;
  window.setTimeout(() => {
    if (justPosted === id) justPosted = undefined;
  }, 8000);
}
export const isJustPosted = (id: string) => justPosted === id;

/** Preload the lazy chunks on first intent (hover/touch). */
let preloaded = false;
export function preloadModals() {
  if (preloaded) return;
  preloaded = true;
  void import("../pages/NoteView");
  void import("../pages/Upload");
}
