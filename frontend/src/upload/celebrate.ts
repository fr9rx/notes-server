// "LED burst" celebration (DESIGN.md §4.6): particles spray up from a point,
// fall under gravity and flicker out in 7 quantized steps like the board's LEDs.

interface Particle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  size: number;
  rot: number;
  spin: number;
  color: string;
  born: number;
  life: number;
}

const DARK_COLORS = ["#4C9AFF", "#4C9AFF", "#DCEBFF", "#4C9AFF", "#DCEBFF", "#4C9AFF", "#4C9AFF", "#FFD84A", "#FFD84A", "#FFFFFF"];
const LIGHT_COLORS = ["#1F5EFF", "#1F5EFF", "#4C9AFF", "#1F5EFF", "#4C9AFF", "#1F5EFF", "#1F5EFF", "#FFD84A", "#E0B400", "#121826"];

export function burst(originX: number, originY: number, dark: boolean, count = 72): void {
  const canvas = document.createElement("canvas");
  const dpr = Math.min(window.devicePixelRatio || 1, 2);
  canvas.width = window.innerWidth * dpr;
  canvas.height = window.innerHeight * dpr;
  canvas.setAttribute("aria-hidden", "true");
  Object.assign(canvas.style, {
    position: "fixed",
    inset: "0",
    width: "100vw",
    height: "100vh",
    pointerEvents: "none",
    zIndex: "90",
  });
  document.body.appendChild(canvas);
  const g = canvas.getContext("2d");
  if (!g) {
    canvas.remove();
    return;
  }

  const colors = dark ? DARK_COLORS : LIGHT_COLORS;
  const now = performance.now();
  const particles: Particle[] = Array.from({ length: Math.min(120, count) }, () => {
    const angle = -Math.PI / 2 + (Math.random() - 0.5) * 1.8 * Math.PI * 0.62;
    const speed = 380 + Math.random() * 520;
    return {
      x: originX,
      y: originY,
      vx: Math.cos(angle) * speed,
      vy: Math.sin(angle) * speed,
      size: 5 + Math.random() * 3,
      rot: Math.random() * Math.PI,
      spin: (Math.random() - 0.5) * 12,
      color: colors[Math.floor(Math.random() * colors.length)] ?? "#4C9AFF",
      born: now,
      life: 900 + Math.random() * 500,
    };
  });

  let last = now;
  const frame = (t: number) => {
    const dt = Math.min(0.033, (t - last) / 1000);
    last = t;
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.clearRect(0, 0, window.innerWidth, window.innerHeight);
    let alive = 0;
    for (const p of particles) {
      const age = t - p.born;
      if (age > p.life) continue;
      alive++;
      p.vy += 1400 * dt;
      p.vx *= 0.985;
      p.vy *= 0.985;
      p.x += p.vx * dt;
      p.y += p.vy * dt;
      p.rot += p.spin * dt;
      const fade = age > p.life * 0.6 ? 1 - (age - p.life * 0.6) / (p.life * 0.4) : 1;
      const alpha = Math.round(fade * 7) / 7; // quantized, like the board
      if (alpha <= 0) continue;
      g.save();
      g.globalAlpha = alpha;
      g.translate(p.x, p.y);
      g.rotate(p.rot);
      if (dark) {
        g.shadowColor = p.color;
        g.shadowBlur = 10;
      }
      g.fillStyle = p.color;
      const s = p.size;
      g.beginPath();
      g.roundRect(-s / 2, -s / 2, s, s, 1.5);
      g.fill();
      g.restore();
    }
    if (alive > 0) requestAnimationFrame(frame);
    else canvas.remove();
  };
  requestAnimationFrame(frame);
}
