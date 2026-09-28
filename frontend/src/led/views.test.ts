import { describe, expect, it } from "vitest";

import {
  H,
  MAX,
  W,
  barHeight,
  drawArrow,
  drawDashboard,
  drawText,
  drawX,
  loaderLevels,
  monogramText,
  newFrame,
  textScrollDone,
} from "./views";

const at = (f: Uint8Array, x: number, y: number) => f[y * W + x];

describe("firmware parity (mcu/notes-matrix/src/view.c)", () => {
  it("bar heights use the same log2 scale", () => {
    expect([0, 1, 2, 3, 4, 7, 8, 64, 1000].map(barHeight)).toEqual([0, 1, 2, 2, 3, 3, 4, 7, 7]);
  });

  it("dashboard: bars at level 2, upload tops at 7, heartbeat fades", () => {
    const f = newFrame();
    const req = new Array(13).fill(0);
    const up = new Array(13).fill(0);
    req[12] = 100; // -> 7 dots
    up[12] = 1;
    req[0] = 1;
    drawDashboard(f, { requests: req, uploads: up, warning: false, now: 0, sinceStatus: 0 });
    expect(at(f, 0, H - 1)).toBe(2);
    expect(at(f, 0, H - 2)).toBe(0);
    for (let y = 2; y < H; y++) expect(at(f, 12, y)).toBe(2);
    expect(at(f, 12, 1)).toBe(MAX); // bright top where images were uploaded
    expect(at(f, 12, 0)).toBe(MAX); // heartbeat just fired

    const later = newFrame();
    drawDashboard(later, { requests: req, uploads: up, warning: false, now: 0, sinceStatus: 200 });
    expect(at(later, 12, 0)).toBe(MAX - 3);
  });

  it("X matches the firmware coordinates", () => {
    const f = newFrame();
    drawX(f);
    for (let i = 0; i < H; i++) {
      expect(at(f, 3 + i, i)).toBe(MAX);
      expect(at(f, 10 - i, i)).toBe(MAX);
    }
    expect([...f].filter((v) => v === MAX)).toHaveLength(16);
  });

  it("arrow starts below the matrix and leaves above it", () => {
    const start = newFrame();
    drawArrow(start, 0);
    expect([...start].every((v) => v === 0)).toBe(true);
    const mid = newFrame();
    drawArrow(mid, 450);
    expect([...mid].some((v) => v === MAX)).toBe(true);
    const end = newFrame();
    drawArrow(end, 899);
    expect([...end].filter((v) => v > 0).length).toBeLessThan(3);
  });

  it("text uses the 3x5 font and scrolls off", () => {
    const f = newFrame();
    drawText(f, "1", 0);
    // "1" = rows 2,6,2,2,7
    expect([at(f, 0, 1), at(f, 1, 1), at(f, 2, 1)]).toEqual([0, MAX, 0]);
    expect([at(f, 0, 5), at(f, 1, 5), at(f, 2, 5)]).toEqual([MAX, MAX, MAX]);
    expect(textScrollDone("OK", 0)).toBe(false);
    expect(textScrollDone("OK", 70 * 30)).toBe(true);
  });
});

describe("web helpers", () => {
  it("monogram initials", () => {
    expect(monogramText("Linear Algebra")).toBe("LA");
    expect(monogramText("Math 101")).toBe("M1");
    expect(monogramText("physics")).toBe("PHY");
    expect(monogramText("Intro to Data Science")).toBe("ITD");
    expect(monogramText("  ")).toBe("?");
  });

  it("loader comet stays in range", () => {
    for (let t = 0; t < 3000; t += 70) {
      const l = loaderLevels(13, t);
      expect(l).toHaveLength(13);
      expect(Math.max(...l)).toBe(MAX);
    }
  });
});
