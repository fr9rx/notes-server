import { useReducedMotion } from "motion/react";
import { useEffect, useRef, type RefObject } from "react";

import { useTheme } from "../app/theme";
import { liveNow, subscribeLive } from "../stats/store";
import { inkSprites, sprites } from "./sprites";

interface Ripple {
  x: number;
  y: number;
  r: number;
  strength: number;
  born: number;
}
interface Column {
  col: number;
  born: number;
}

const smoothstep = (a: number, b: number, v: number) => {
  const t = Math.min(1, Math.max(0, (v - a) / (b - a)));
  return t * t * (3 - 2 * t);
};

/**
 * The hero background (DESIGN.md §4.9): a huge, dim LED panel. Clouds of
 * light drift across it, a spotlight follows the pointer, and every real
 * request to the board sends a ripple out of the LED plate.
 */
export function LEDField({
  hostRef,
  originRef,
  avoidRef,
}: {
  /** The hero section (pointer events are read from it). */
  hostRef: RefObject<HTMLElement | null>;
  /** Ripples start at this element's centre (the LED plate). */
  originRef: RefObject<HTMLElement | null>;
  /** Dots dim inside this box so the headline stays readable. */
  avoidRef: RefObject<HTMLElement | null>;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const { theme } = useTheme();
  const reduced = useReducedMotion() ?? false;

  useEffect(() => {
    const canvas = canvasRef.current;
    const host = hostRef.current;
    if (!canvas || !host) return;
    const g = canvas.getContext("2d");
    if (!g) return;

    const coarse = matchMedia("(pointer: coarse)").matches;
    const weak = coarse && (navigator.hardwareConcurrency ?? 8) <= 4;
    const dpr = Math.min(window.devicePixelRatio || 1, weak ? 1.5 : 2);
    let pitch = 24;
    let W = 0;
    let H = 0;
    let cols = 0;
    let rows = 0;
    let set = theme === "dark" ? sprites(4.4, dpr, true) : inkSprites(4.4, dpr);
    let lowPower = false;
    const ripples: Ripple[] = [];
    const columns: Column[] = [];
    const pointer = { x: -9999, y: -9999, tx: -9999, ty: -9999, presence: 0, target: 0 };
    let origin = { x: 0, y: 0 };
    let avoid = { x0: 0, y0: 0, x1: 0, y1: 0 };
    let raf = 0;
    let visible = true;
    let lastTick = liveNow().tick;
    let frameTimes: number[] = [];
    let skip = false;

    const measure = () => {
      const rect = host.getBoundingClientRect();
      W = rect.width;
      H = rect.height;
      pitch = W < 640 ? 20 : 24;
      cols = Math.ceil(W / pitch) + 1;
      rows = Math.ceil(H / pitch) + 1;
      canvas.width = Math.round(W * dpr);
      canvas.height = Math.round(H * dpr);
      canvas.style.width = `${W}px`;
      canvas.style.height = `${H}px`;
      const o = originRef.current?.getBoundingClientRect();
      if (o) origin = { x: o.left - rect.left + o.width / 2, y: o.top - rect.top + o.height / 2 };
      const a = avoidRef.current?.getBoundingClientRect();
      if (a) avoid = { x0: a.left - rect.left - 24, y0: a.top - rect.top - 24, x1: a.right - rect.left + 24, y1: a.bottom - rect.top + 24 };
    };

    const frame = (nowMs: number) => {
      const t = nowMs / 1000;
      g.clearRect(0, 0, canvas.width, canvas.height);
      pointer.x += (pointer.tx - pointer.x) * 0.12;
      pointer.y += (pointer.ty - pointer.y) * 0.12;
      pointer.presence += (pointer.target - pointer.presence) * 0.08;

      for (const r of ripples) {
        r.r = ((nowMs - r.born) / 1000) * 520;
        r.strength *= 0.965;
      }
      for (let i = ripples.length - 1; i >= 0; i--) {
        const r = ripples[i];
        if (r && (r.r > Math.hypot(W, H) || r.strength < 0.02)) ripples.splice(i, 1);
      }
      for (let i = columns.length - 1; i >= 0; i--) {
        const c = columns[i];
        if (c && nowMs - c.born > 1400) columns.splice(i, 1);
      }

      const spriteSize = set.size;
      const half = spriteSize / 2;
      for (let j = 0; j < rows; j++) {
        const y = j * pitch;
        const yFade = smoothstep(0, 0.3, 1 - y / H);
        for (let i = 0; i < cols; i++) {
          const x = i * pitch;
          const n =
            0.5 * Math.sin(x * 0.009 + t * 0.33) * Math.cos(y * 0.011 - t * 0.21) +
            0.35 * Math.sin((x + y) * 0.0052 + t * 0.47) +
            0.15 * Math.cos(x * 0.017 - y * 0.008 - t * 0.62);
          let v = smoothstep(0.18, 1, n) * 0.72;
          if (pointer.presence > 0.01) {
            const d2 = (x - pointer.x) ** 2 + (y - pointer.y) ** 2;
            v += 0.85 * Math.exp(-d2 / (2 * 110 * 110)) * pointer.presence;
          }
          for (const r of ripples) {
            const band = 1 - Math.abs(Math.hypot(x - r.x, y - r.y) - r.r) / 48;
            if (band > 0) v += band * r.strength;
          }
          for (const c of columns) {
            if (Math.abs(i - c.col) <= 1) {
              const front = H - ((nowMs - c.born) / 900) * H;
              const d = y - front;
              if (d > 0 && d < 160) v += (1 - d / 160) * 0.9;
            }
          }
          const edge = smoothstep(0, 60, x) * smoothstep(0, 60, W - x);
          v *= yFade * edge;
          if (x > avoid.x0 && x < avoid.x1 && y > avoid.y0 && y < avoid.y1) v *= 0.45;
          const level = Math.min(7, Math.floor(Math.min(1, Math.max(0, v)) * 8));
          const sprite = set.levels[level];
          if (sprite) g.drawImage(sprite, x * dpr - half, y * dpr - half);
        }
      }
    };

    const loop = (now: number) => {
      raf = 0;
      if (!visible || document.hidden) return;
      skip = !skip;
      if (!(coarse || lowPower) || !skip) {
        const start = performance.now();
        frame(now);
        // Adaptive quality: if frames are slow, drop to 30 fps and plain dots.
        frameTimes.push(performance.now() - start);
        if (frameTimes.length === 60) {
          const avg = frameTimes.reduce((a, b) => a + b, 0) / 60;
          frameTimes = [];
          if (avg > 12 && !lowPower) {
            lowPower = true;
            if (theme === "dark") set = sprites(4.4, dpr, false);
          }
        }
      }
      raf = requestAnimationFrame(loop);
    };

    measure();
    if (reduced) {
      frame(0); // one static frame, no loop
    } else {
      raf = requestAnimationFrame(loop);
    }

    const ro = new ResizeObserver(() => {
      measure();
      if (reduced) frame(0);
    });
    ro.observe(host);
    const io = new IntersectionObserver(([e]) => {
      visible = e?.isIntersecting ?? true;
      if (visible && !raf && !reduced) raf = requestAnimationFrame(loop);
    });
    io.observe(host);

    const fine = matchMedia("(hover: hover) and (pointer: fine)").matches;
    const onMove = (e: PointerEvent) => {
      const rect = host.getBoundingClientRect();
      pointer.tx = e.clientX - rect.left;
      pointer.ty = e.clientY - rect.top;
      if (pointer.x < -1000) {
        pointer.x = pointer.tx;
        pointer.y = pointer.ty;
      }
      pointer.target = 1;
    };
    const onLeave = () => {
      pointer.target = 0;
    };
    if (fine && !reduced) {
      host.addEventListener("pointermove", onMove);
      host.addEventListener("pointerleave", onLeave);
    }

    const onVisible = () => {
      if (!document.hidden && !raf && !reduced) raf = requestAnimationFrame(loop);
    };
    document.addEventListener("visibilitychange", onVisible);

    const unsub = subscribeLive(() => {
      const snap = liveNow();
      if (snap.tick === lastTick || reduced) return;
      lastTick = snap.tick;
      measure();
      const n = snap.stats?.requests[12] ?? 0;
      const count = Math.min(3, Math.ceil(Math.log2(n + 1)));
      for (let k = 0; k < count && ripples.length < 6; k++) {
        ripples.push({ x: origin.x, y: origin.y, r: 0, strength: 0.6, born: performance.now() + k * 120 });
      }
      if ((snap.stats?.uploads[12] ?? 0) > 0) {
        columns.push({ col: Math.floor(Math.random() * cols), born: performance.now() });
      }
    });

    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
      io.disconnect();
      host.removeEventListener("pointermove", onMove);
      host.removeEventListener("pointerleave", onLeave);
      document.removeEventListener("visibilitychange", onVisible);
      unsub();
    };
  }, [hostRef, originRef, avoidRef, theme, reduced]);

  return <canvas ref={canvasRef} aria-hidden className="pointer-events-none absolute inset-0" />;
}
