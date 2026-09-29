import clsx from "clsx";
import { useReducedMotion } from "motion/react";
import { useEffect, useRef } from "react";

import { useTheme } from "../app/theme";
import { sprites } from "./sprites";
import {
  H,
  MAX,
  UPLOAD_ANIM_MS,
  W,
  drawArrow,
  drawCheck,
  drawStarting,
  drawStopped,
  drawText,
  drawWaiting,
  drawX,
  newFrame,
  textScrollDone,
  textScrollX,
  type Frame,
} from "./views";

export type LEDPattern =
  | "waiting"
  | "starting"
  | "x"
  | "blinkX"
  | "arrow"
  | "arrowLoop"
  | "check"
  | "404"
  | "stopped"
  | "warning";

export type LEDSize = "micro" | "xs" | "sm" | "md" | "lg" | "hero";

const SIZES: Record<LEDSize, { dot: number; pitch: number; pad: number; radius: number }> = {
  micro: { dot: 2, pitch: 3, pad: 0, radius: 0 },
  xs: { dot: 3, pitch: 5, pad: 6, radius: 10 },
  sm: { dot: 5, pitch: 8, pad: 10, radius: 14 },
  md: { dot: 8, pitch: 12, pad: 16, radius: 28 },
  lg: { dot: 11, pitch: 17, pad: 20, radius: 28 },
  hero: { dot: 14, pitch: 22, pad: 24, radius: 28 },
};

export interface LEDMatrixProps {
  pattern?: LEDPattern;
  /** Scrolls once (then `onTextDone`), or forever with `loopText`. Overrides the view while scrolling. */
  text?: string;
  loopText?: boolean;
  onTextDone?: () => void;
  size?: LEDSize;
  /** Override dot/pitch (CSS px). */
  dot?: number;
  pitch?: number;
  plate?: boolean;
  glow?: boolean;
  label?: string;
  screws?: boolean;
  silk?: string;
  className?: string;
}

export function LEDMatrix({
  pattern = "waiting",
  text,
  loopText,
  onTextDone,
  size = "sm",
  dot: dotOverride,
  pitch: pitchOverride,
  plate,
  glow: glowProp,
  label,
  screws,
  silk,
  className,
}: LEDMatrixProps) {
  const { theme } = useTheme();
  const reduced = useReducedMotion() ?? false;
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const geo = SIZES[size];
  const dot = dotOverride ?? geo.dot;
  const pitch = pitchOverride ?? geo.pitch;
  const showPlate = plate ?? (size === "md" || size === "lg" || size === "hero" || size === "sm");
  const glow = glowProp ?? (theme === "dark" && (size === "md" || size === "lg" || size === "hero"));
  const width = W * pitch;
  const height = H * pitch;

  // Everything the render loop reads lives in a ref, so props changes don't restart it.
  const props = useRef({ pattern, text, loopText, onTextDone, reduced });
  props.current = { pattern, text, loopText, onTextDone, reduced };

  const textStart = useRef(0);
  useEffect(() => {
    textStart.current = performance.now();
  }, [text]);

  const aria = label;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const g = canvas.getContext("2d");
    if (!g) return;

    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const set = sprites(dot, dpr, glow);
    const margin = Math.ceil(set.size / dpr / 2);
    canvas.width = Math.round((width + margin * 2) * dpr);
    canvas.height = Math.round((height + margin * 2) * dpr);
    canvas.style.width = `${width + margin * 2}px`;
    canvas.style.height = `${height + margin * 2}px`;
    canvas.style.margin = `-${margin}px`;

    const mountedAt = performance.now();
    const shown = new Float32Array(W * H); // phosphor persistence
    let lastFrameAt = mountedAt;
    let textDoneFired = false;
    let raf = 0;
    let visible = true;
    const fpsCap = size === "micro" || size === "xs" ? 30 : 60;
    let lastDraw = 0;

    const compute = (now: number): { frame: Frame; still: boolean } => {
      const p = props.current;
      const f = newFrame();
      const t = now - mountedAt;
      let still = false;

      // Text scroll overrides the view while it runs (like the firmware).
      if (p.text) {
        const elapsed = now - textStart.current;
        const done = textScrollDone(p.text, elapsed);
        if (!done || p.loopText) {
          const e = p.loopText ? elapsed % ((13 + p.text.length * 4 + 4) * 70) : elapsed;
          drawText(f, p.text, textScrollX(e));
          return { frame: f, still: false };
        }
        if (!textDoneFired) {
          textDoneFired = true;
          queueMicrotask(() => p.onTextDone?.());
        }
      } else {
        textDoneFired = false;
      }

      switch (p.pattern) {
        case "waiting":
          drawWaiting(f, t);
          break;
        case "starting":
          drawStarting(f, t);
          break;
        case "x":
          drawX(f);
          still = true;
          break;
        case "blinkX":
          if (Math.floor(t / 500) % 2 === 0) drawX(f);
          break;
        case "arrow":
          if (t < UPLOAD_ANIM_MS) drawArrow(f, t);
          else still = true;
          break;
        case "arrowLoop":
          if (t % 2400 < UPLOAD_ANIM_MS) drawArrow(f, t % 2400);
          break;
        case "check":
          drawCheck(f);
          still = true;
          break;
        case "404":
          // One glitch to the firmware X every 6 s, for 2 frames.
          if (!p.reduced && t % 6000 > 5880) drawX(f);
          else drawText(f, "404", 1);
          break;
        case "stopped":
          drawStopped(f);
          still = true;
          break;
        case "warning":
          if (Math.floor(t / 500) % 2 === 0) for (let x = 0; x < W; x++) f[x] = MAX;
          break;
      }

      return { frame: f, still };
    };

    const draw = (now: number) => {
      const { frame, still } = compute(now);
      const reducedNow = props.current.reduced;
      const dt = now - lastFrameAt;
      lastFrameAt = now;
      // Phosphor: rises are instant, falls decay one level per 45 ms.
      let settling = false;
      for (let i = 0; i < frame.length; i++) {
        const target = frame[i] ?? 0;
        const cur = shown[i] ?? 0;
        if (reducedNow || target >= cur) shown[i] = target;
        else {
          shown[i] = Math.max(target, cur - dt / 45);
          settling = true;
        }
      }

      g.clearRect(0, 0, canvas.width, canvas.height);
      let sum = 0;
      const off = margin * dpr - set.size / 2 + (pitch / 2) * dpr;
      for (let y = 0; y < H; y++) {
        for (let x = 0; x < W; x++) {
          const level = Math.round(shown[y * W + x] ?? 0);
          sum += level;
          const sprite = set.levels[level];
          if (sprite) g.drawImage(sprite, off + x * pitch * dpr, off + y * pitch * dpr);
        }
      }
      return still && !settling;
    };

    const loop = (now: number) => {
      raf = 0;
      if (!visible || document.hidden) return;
      if (now - lastDraw >= 1000 / fpsCap - 2) {
        lastDraw = now;
        if (draw(now)) return; // static: stop until something changes
      }
      raf = requestAnimationFrame(loop);
    };
    const kick = () => {
      if (!raf && visible && !document.hidden) raf = requestAnimationFrame(loop);
    };

    const io = new IntersectionObserver(([entry]) => {
      visible = entry?.isIntersecting ?? true;
      kick();
    });
    io.observe(canvas);
    document.addEventListener("visibilitychange", kick);
    kick();
    // Props like `pattern`/`text` change through the ref; poll for a restart cheaply.
    const wake = window.setInterval(kick, 250);

    return () => {
      cancelAnimationFrame(raf);
      io.disconnect();
      document.removeEventListener("visibilitychange", kick);
      window.clearInterval(wake);
    };
  }, [dot, pitch, glow, width, height, size]);

  const canvas = (
    <canvas
      ref={canvasRef}
      className="block"
      role="img"
      aria-label={aria}
      aria-hidden={aria ? undefined : true}
    />
  );

  if (!showPlate) {
    return (
      <span className={clsx("inline-block overflow-visible", className)} style={{ width, height }}>
        {canvas}
      </span>
    );
  }

  return (
    <div
      className={clsx("relative inline-flex flex-col items-center", className)}
      style={{
        padding: geo.pad,
        borderRadius: geo.radius,
        background: "var(--plate)",
        border: "1px solid var(--plate-bezel)",
        boxShadow:
          "inset 0 1px 0 rgba(255,255,255,0.04), inset 0 -12px 24px rgba(0,0,0,0.35), var(--shadow-2)",
      }}
    >
      {(screws ?? (size === "md" || size === "lg" || size === "hero")) && <Screws />}
      <span className="relative block overflow-visible" style={{ width, height }}>
        {canvas}
      </span>
      {silk && (
        <span className="t-caption mt-3 select-none" style={{ color: "var(--plate-silk)", fontSize: 10 }}>
          {silk}
        </span>
      )}
    </div>
  );
}

function Screws() {
  const s = { background: "var(--plate-screw)" } as const;
  return (
    <>
      {["left-2 top-2", "right-2 top-2", "bottom-2 left-2", "bottom-2 right-2"].map((pos) => (
        <span key={pos} aria-hidden className={clsx("absolute h-1 w-1 rounded-full", pos)} style={s} />
      ))}
    </>
  );
}

