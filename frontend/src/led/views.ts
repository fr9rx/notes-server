// Pure LED-matrix views: a TypeScript port of the firmware's
// mcu/notes-matrix/src/view.c, so the web matrix looks exactly like the board.
// A frame is 104 brightness levels (0..7), row-major: index = y * 13 + x.

export const W = 13;
export const H = 8;
export const MAX = 7;
export type Frame = Uint8Array;

export const newFrame = (): Frame => new Uint8Array(W * H);

/** Sets a pixel, keeping the brighter of old and new (like the firmware's px()). */
export function px(f: Frame, x: number, y: number, level: number): void {
  if (x >= 0 && x < W && y >= 0 && y < H) {
    const i = y * W + x;
    if (level > (f[i] ?? 0)) f[i] = level;
  }
}

// ---- 3x5 font, verbatim from view.c: 5 rows of 3 bits, bit 2 = left column ----
const FONT: Record<string, readonly number[]> = {
  "0": [7, 5, 5, 5, 7], "1": [2, 6, 2, 2, 7], "2": [7, 1, 7, 4, 7], "3": [7, 1, 7, 1, 7],
  "4": [5, 5, 7, 1, 1], "5": [7, 4, 7, 1, 7], "6": [7, 4, 7, 5, 7], "7": [7, 1, 2, 2, 2],
  "8": [7, 5, 7, 5, 7], "9": [7, 5, 7, 1, 7], A: [2, 5, 7, 5, 5], B: [6, 5, 6, 5, 6],
  C: [3, 4, 4, 4, 3], D: [6, 5, 5, 5, 6], E: [7, 4, 6, 4, 7], F: [7, 4, 6, 4, 4],
  G: [3, 4, 5, 5, 3], H: [5, 5, 7, 5, 5], I: [7, 2, 2, 2, 7], J: [1, 1, 1, 5, 2],
  K: [5, 5, 6, 5, 5], L: [4, 4, 4, 4, 7], M: [5, 7, 5, 5, 5], N: [6, 5, 5, 5, 5],
  O: [2, 5, 5, 5, 2], P: [6, 5, 6, 4, 4], Q: [2, 5, 5, 6, 3], R: [6, 5, 6, 5, 5],
  S: [3, 4, 2, 1, 6], T: [7, 2, 2, 2, 2], U: [5, 5, 5, 5, 7], V: [5, 5, 5, 5, 2],
  W: [5, 5, 5, 7, 5], X: [5, 5, 2, 5, 5], Y: [5, 5, 2, 2, 2], Z: [7, 1, 2, 4, 7],
  ".": [0, 0, 0, 0, 2], ":": [0, 2, 0, 2, 0], "-": [0, 0, 7, 0, 0], "/": [1, 1, 2, 4, 4],
  "!": [2, 2, 2, 0, 2], "%": [5, 1, 2, 4, 5],
};
const BLANK = [0, 0, 0, 0, 0] as const;

export function glyph(c: string): readonly number[] {
  return FONT[c.toUpperCase()] ?? BLANK;
}

/** Draws text with its left edge at column x (4 columns per glyph), rows 1..5. */
export function drawText(f: Frame, text: string, x: number, level = MAX, y0 = 1): void {
  [...text].forEach((c, i) => {
    const g = glyph(c);
    const gx = x + i * 4;
    if (gx >= W || gx + 3 < 0) return;
    for (let r = 0; r < 5; r++) {
      for (let col = 0; col < 3; col++) {
        if ((g[r] ?? 0) & (4 >> col)) px(f, gx + col, y0 + r, level);
      }
    }
  });
}

export const textWidth = (text: string) => [...text].length * 4;

/** Scroll position for text started `elapsedMs` ago (70 ms per column, from x = 13). */
export function textScrollX(elapsedMs: number): number {
  return W - Math.floor(elapsedMs / 70);
}

export const textScrollDone = (text: string, elapsedMs: number) =>
  textScrollX(elapsedMs) < -textWidth(text);

// ---- Views ---------------------------------------------------------------------

/** Firmware bar_height(): log2 scale 1, 2-3, 4-7, ... capped at 7. */
export function barHeight(n: number): number {
  if (n <= 0) return 0;
  let h = 1;
  let v = Math.floor(n);
  while (v > 1 && h < H - 1) {
    v >>= 1;
    h++;
  }
  return h;
}

export interface DashboardInput {
  /** 13 values, oldest first. */
  requests: readonly number[];
  uploads: readonly number[];
  warning: boolean;
  now: number;
  /** ms since the last status update (drives the heartbeat pixel). */
  sinceStatus: number;
  /** For the web-only smooth scroll: how many columns (0..1) the bars are shifted left. */
  shift?: number;
  /** Web-only: the newest bar grows dot by dot; this caps its height (Infinity = full). */
  newestMaxHeight?: number;
}

export function drawDashboard(f: Frame, d: DashboardInput): void {
  for (let x = 0; x < W; x++) {
    let h = barHeight(d.requests[x] ?? 0);
    if (x === W - 1 && d.newestMaxHeight !== undefined) h = Math.min(h, d.newestMaxHeight);
    for (let i = 0; i < h; i++) px(f, x, H - 1 - i, 2);
    if ((d.uploads[x] ?? 0) > 0) px(f, x, H - 1 - (h > 0 ? h - 1 : 0), MAX);
  }
  if (d.warning && Math.floor(d.now / 500) % 2 === 0) {
    for (let x = 0; x < W; x++) px(f, x, 0, MAX);
  }
  if (d.sinceStatus >= 0 && d.sinceStatus < 400) {
    px(f, W - 1, 0, MAX - Math.floor((d.sinceStatus * MAX) / 400));
  }
}

export function drawX(f: Frame, level = MAX): void {
  for (let i = 0; i < H; i++) {
    px(f, 3 + i, i, level);
    px(f, 10 - i, i, level);
  }
}

export function drawWaiting(f: Frame, now: number): void {
  const pos = Math.floor(now / 120) % (2 * (W - 1));
  px(f, pos < W ? pos : 2 * (W - 1) - pos, H - 1, 2);
}

/** The firmware's 12 ring points (view_starting). */
export const RING: readonly (readonly [number, number])[] = [
  [6, 0], [8, 1], [9, 2], [9, 4], [9, 5], [8, 6],
  [6, 7], [4, 6], [3, 5], [3, 4], [3, 2], [4, 1],
];

export function drawStarting(f: Frame, now: number): void {
  const head = Math.floor(now / 70) % 12;
  for (let k = 0; k < 5; k++) {
    const p = RING[(head - k + 12) % 12];
    if (p) px(f, p[0], p[1], MAX - k);
  }
}

export const UPLOAD_ANIM_MS = 900;
const ARROW = [0x04, 0x0e, 0x15, 0x04, 0x04] as const;

/** The upload arrow flying up, `elapsed` ms into its 900 ms flight. */
export function drawArrow(f: Frame, elapsed: number): void {
  const top = H - Math.floor((elapsed * (H + 5)) / UPLOAD_ANIM_MS);
  ARROW.forEach((bits, r) => {
    for (let c = 0; c < 5; c++) if (bits & (0x10 >> c)) px(f, 4 + c, top + r, MAX);
  });
}

export function drawStopped(f: Frame): void {
  for (let x = 4; x <= 8; x++) {
    px(f, x, 3, 2);
    px(f, x, 4, 2);
  }
}

/** A check mark in dots (reduced-motion success). */
export function drawCheck(f: Frame): void {
  const pts: [number, number][] = [[3, 4], [4, 5], [5, 6], [6, 5], [7, 4], [8, 3], [9, 2], [10, 1]];
  for (const [x, y] of pts) px(f, x, y, MAX);
}

// ---- Monograms (§6.5) ---------------------------------------------------------------

/** Initials of the first 3 words, or the first 3 letters of a single word. */
export function monogramText(name: string): string {
  const words = name
    .toUpperCase()
    .split(/[^A-Z0-9]+/)
    .filter(Boolean);
  if (words.length === 0) return "?";
  if (words.length === 1) return (words[0] ?? "").slice(0, 3);
  return words
    .slice(0, 3)
    .map((w) => w[0])
    .join("");
}

function hash(s: string): number {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

export function drawMonogram(f: Frame, text: string, seed: string, level = 6): void {
  const t = text.slice(0, 3);
  const x = t.length === 3 ? 1 : t.length === 2 ? 3 : 5;
  drawText(f, t, x, level);
  // A deterministic sparkle in the unused top/bottom rows: every course has a fingerprint.
  const h = hash(seed);
  px(f, h % W, (h >> 8) % 2 === 0 ? 0 : H - 1, MAX);
}

// ---- 1D loader (§6.13) ---------------------------------------------------------------

/** Levels for an n-dot row running the comet ping-pong (70 ms per step). */
export function loaderLevels(n: number, now: number): number[] {
  const period = 2 * (n - 1);
  const step = Math.floor(now / 70) % period;
  const head = step < n ? step : period - step;
  const dir = step < n ? 1 : -1;
  const levels = new Array<number>(n).fill(0);
  [MAX, 5, 3, 1].forEach((lv, k) => {
    const i = head - dir * k;
    if (i >= 0 && i < n) levels[i] = Math.max(levels[i] ?? 0, lv);
  });
  return levels;
}

/** Average brightness 0..1 (drives the hero plate's bloom). */
export function brightness(f: Frame): number {
  let sum = 0;
  for (const v of f) sum += v;
  return sum / (f.length * MAX);
}
