// Motion tokens (DESIGN.md §3). Everything user-facing uses these.

export const spring = {
  press: { type: "spring", stiffness: 700, damping: 40, mass: 0.6 },
  snappy: { type: "spring", stiffness: 500, damping: 36, mass: 0.8 },
  ui: { type: "spring", stiffness: 380, damping: 32, mass: 1 },
  soft: { type: "spring", stiffness: 210, damping: 26, mass: 1 },
  morph: { type: "spring", stiffness: 320, damping: 34, mass: 1 },
  sheet: { type: "spring", stiffness: 420, damping: 40, mass: 1 },
  pop: { type: "spring", stiffness: 600, damping: 22, mass: 0.7 },
  tilt: { type: "spring", stiffness: 260, damping: 20, mass: 0.5 },
  swipe: { type: "spring", stiffness: 300, damping: 30, mass: 1 },
} as const;

export const counterSpring = { stiffness: 90, damping: 22, mass: 1 };

/** Durations in seconds (motion uses seconds). */
export const dur = {
  instant: 0.08,
  fast: 0.15,
  base: 0.22,
  slow: 0.36,
  slower: 0.56,
  epic: 0.9,
} as const;

export const ease = {
  out: [0.22, 1, 0.36, 1],
  outExpo: [0.16, 1, 0.3, 1],
  inOut: [0.65, 0, 0.35, 1],
  in: [0.55, 0, 1, 0.45],
} as const;

/** Diagonal-wave stagger for grids (§3.4). */
export function gridDelay(index: number, columns: number): number {
  const row = Math.floor(index / columns);
  const col = index % columns;
  return Math.min((row + col) * 0.045, 0.45);
}

/** List stagger: 40 ms per item, capped. */
export function listDelay(index: number, base = 0.08): number {
  return base + Math.min(index, 10) * 0.04;
}
