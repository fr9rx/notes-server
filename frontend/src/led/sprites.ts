// Pre-rendered LED dot sprites (one per brightness level), cached by geometry.
// Drawing a frame is then just 104 drawImage calls: no gradients at runtime.

export const LED_COLORS = [
  "#0D1628", "#16305F", "#2457B4", "#2E6CD8", "#3A81F1", "#4C9AFF", "#8CBEFF", "#DCEBFF",
] as const;

// Glow per level (DESIGN.md §2.3): [alpha, radius multiple of the dot radius]
const GLOW: Record<number, [number, number]> = { 4: [0.15, 1.8], 5: [0.25, 2.2], 6: [0.35, 2.6], 7: [0.55, 3.2] };

export interface SpriteSet {
  /** Canvas per level; each is `size` x `size` device pixels, dot centred. */
  levels: HTMLCanvasElement[];
  size: number;
}

const cache = new Map<string, SpriteSet>();

/** `dot` is the dot diameter in CSS px. */
export function sprites(dot: number, dpr: number, glow: boolean): SpriteSet {
  const key = `${dot}:${dpr}:${glow}`;
  const hit = cache.get(key);
  if (hit) return hit;

  const r = (dot / 2) * dpr;
  const glowR = glow ? r * 3.2 : r;
  const size = Math.ceil(glowR * 2 + 2);
  const c = size / 2;
  const levels = LED_COLORS.map((color, level) => {
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = size;
    const g = canvas.getContext("2d");
    if (!g) return canvas;
    const spec = glow ? GLOW[level] : undefined;
    if (spec) {
      const [alpha, mult] = spec;
      const grad = g.createRadialGradient(c, c, r * 0.6, c, c, r * mult);
      grad.addColorStop(0, `rgba(76,154,255,${alpha})`);
      grad.addColorStop(1, "rgba(76,154,255,0)");
      g.fillStyle = grad;
      g.fillRect(0, 0, size, size);
    }
    g.fillStyle = color;
    g.beginPath();
    g.arc(c, c, r, 0, Math.PI * 2);
    g.fill();
    if (level >= 6) {
      // a hot core, like a real LED die
      g.fillStyle = "rgba(255,255,255,0.35)";
      g.beginPath();
      g.arc(c, c, r * 0.45, 0, Math.PI * 2);
      g.fill();
    }
    return canvas;
  });
  const set = { levels, size };
  cache.set(key, set);
  return set;
}

/** Ink-on-paper dots for the light-theme hero field (no glow). */
export function inkSprites(dot: number, dpr: number): SpriteSet {
  const key = `ink:${dot}:${dpr}`;
  const hit = cache.get(key);
  if (hit) return hit;
  const alphas = [0.06, 0.09, 0.13, 0.18, 0.25, 0.34, 0.46, 0.6];
  const r = (dot / 2) * dpr;
  const size = Math.ceil(r * 2 + 2);
  const levels = alphas.map((a, level) => {
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = size;
    const g = canvas.getContext("2d");
    if (g) {
      g.fillStyle = `rgba(31,94,255,${a})`;
      g.beginPath();
      g.arc(size / 2, size / 2, r * (0.8 + level * 0.05), 0, Math.PI * 2);
      g.fill();
    }
    return canvas;
  });
  const set = { levels, size };
  cache.set(key, set);
  return set;
}
