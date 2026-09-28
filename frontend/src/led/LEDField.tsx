import { useReducedMotion } from "motion/react";
import { useEffect, useRef, type RefObject } from "react";

import { useTheme } from "../app/theme";
import { inkSprites, sprites } from "./sprites";

interface Ripple {
  x: number;
  y: number;
  r: number;
  strength: number;
  born: number;
}

const smoothstep = (a: number, b: number, v: number) => {
  const t = Math.min(1, Math.max(0, (v - a) / (b - a)));
  return t * t * (3 - 2 * t);
};

/**
 * The hero background: a huge, dim LED panel. Clouds of light drift across
 * it, a spotlight follows the pointer, and a tap or click sends a ripple out.
 */
export function LEDField({
  hostRef,
  avoidRef,
}: {
  /** The hero section (pointer events are read from it). */
  hostRef: RefObject<HTMLElement | null>;
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
    const pointer = { x: -9999, y: -9999, tx: -9999, ty: -9999, presence: 0, target: 0 };
    let avoid = { x0: 0, y0: 0, x1: 0, y1: 0 };
    let raf = 0;
    let visible = true;
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

    // A tap or click sends a ripple through the field.
    const onDown = (e: PointerEvent) => {
      if (reduced) return;
      const rect = host.getBoundingClientRect();
      if (ripples.length >= 6) ripples.shift();
      ripples.push({ x: e.clientX - rect.left, y: e.clientY - rect.top, r: 0, strength: 0.7, born: performance.now() });
    };
    host.addEventListener("pointerdown", onDown);

    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
      io.disconnect();
      host.removeEventListener("pointermove", onMove);
      host.removeEventListener("pointerleave", onLeave);
      document.removeEventListener("visibilitychange", onVisible);
      host.removeEventListener("pointerdown", onDown);
    };
  }, [hostRef, avoidRef, theme, reduced]);

  return <canvas ref={canvasRef} aria-hidden className="pointer-events-none absolute inset-0" />;
}
