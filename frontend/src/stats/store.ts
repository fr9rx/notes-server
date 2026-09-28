// One shared poller for GET /api/stats (DESIGN.md §5.1). Every LED widget
// subscribes to this; polling runs only while something is subscribed.

import { useSyncExternalStore } from "react";

import { api } from "../lib/api";
import type { Stats } from "../lib/types";

export type LiveState = "waiting" | "ok" | "warning" | "error" | "down";

export interface LiveSnapshot {
  stats: Stats | null;
  state: LiveState;
  lastOkAt: number;
  /** Increments on every fresh response (drives heartbeats and ripples). */
  tick: number;
  /** performance.now() of the last fresh response. */
  tickAt: number;
  /** performance.now() of the last local upload (plays the arrow right away). */
  localUploadAt: number;
}

const DOWN_AFTER_MS = 5000; // same as the firmware

let snapshot: LiveSnapshot = {
  stats: null,
  state: "waiting",
  lastOkAt: 0,
  tick: 0,
  tickAt: -Infinity,
  localUploadAt: -Infinity,
};
const listeners = new Set<() => void>();
let timer: number | undefined;
let downTimer: number | undefined;
let inflight: AbortController | undefined;

function emit(next: Partial<LiveSnapshot>) {
  snapshot = { ...snapshot, ...next };
  for (const l of listeners) l();
}

function stateFor(stats: Stats): LiveState {
  switch (stats.status) {
    case "warning":
      return "warning";
    case "error":
      return "error";
    default:
      return "ok";
  }
}

async function poll() {
  timer = undefined;
  if (document.hidden) return;
  inflight?.abort();
  const ctrl = new AbortController();
  inflight = ctrl;
  const timeout = window.setTimeout(() => ctrl.abort(), 2500);
  try {
    const stats = await api.stats(ctrl.signal);
    emit({ stats, state: stateFor(stats), lastOkAt: Date.now(), tick: snapshot.tick + 1, tickAt: performance.now() });
  } catch {
    // counted as silence; the down check below decides
  } finally {
    window.clearTimeout(timeout);
  }
  schedule();
}

function schedule() {
  if (listeners.size === 0 || document.hidden || timer !== undefined) return;
  // Align to the server's one-second buckets.
  timer = window.setTimeout(poll, 1000 - (Date.now() % 1000) + 50);
}

function checkDown() {
  if (snapshot.lastOkAt && Date.now() - snapshot.lastOkAt > DOWN_AFTER_MS && snapshot.state !== "down") {
    emit({ state: "down" });
  }
}

function onVisibility() {
  if (!document.hidden) {
    window.clearTimeout(timer);
    timer = undefined;
    void poll();
  }
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  if (listeners.size === 1) {
    document.addEventListener("visibilitychange", onVisibility);
    downTimer = window.setInterval(checkDown, 1000);
    void poll();
  }
  return () => {
    listeners.delete(listener);
    if (listeners.size === 0) {
      document.removeEventListener("visibilitychange", onVisibility);
      window.clearTimeout(timer);
      window.clearInterval(downTimer);
      timer = undefined;
      inflight?.abort();
    }
  };
}

export function useLive(): LiveSnapshot {
  return useSyncExternalStore(subscribe, () => snapshot, () => snapshot);
}

/** Read the latest snapshot without re-rendering (for canvas loops). */
export function liveNow(): LiveSnapshot {
  return snapshot;
}

/** Subscribe from non-React code (canvas loops); returns an unsubscribe. */
export function subscribeLive(listener: () => void): () => void {
  return subscribe(listener);
}

/** Local feedback: plays the upload arrow on every LED widget immediately. */
export function notifyLocalUpload() {
  emit({ localUploadAt: performance.now() });
}
