const numberFmt = new Intl.NumberFormat();
const rtf = new Intl.RelativeTimeFormat(undefined, { numeric: "auto", style: "narrow" });
const dateFmt = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });
const fullFmt = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

export const formatNumber = (n: number) => numberFmt.format(n);

/** "just now", "2h", "3d", then "Sep 12". */
export function relativeTime(iso: string, now = Date.now()): string {
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return "";
  const s = Math.round((now - t) / 1000);
  if (s < 45) return "just now";
  const m = Math.round(s / 60);
  if (m < 60) return rtf.format(-m, "minute");
  const h = Math.round(m / 60);
  if (h < 24) return rtf.format(-h, "hour");
  const d = Math.round(h / 24);
  if (d < 7) return rtf.format(-d, "day");
  return dateFmt.format(t);
}

export const fullDate = (iso: string) => {
  const t = Date.parse(iso);
  return Number.isNaN(t) ? "" : fullFmt.format(t);
};

export const isRecent = (iso: string, hours = 24) => Date.now() - Date.parse(iso) < hours * 3600_000;

export function formatBytes(n: number): string {
  if (n >= 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  if (n >= 1024) return `${Math.round(n / 1024)} KB`;
  return `${n} B`;
}

/** "3d 4h", "5h 12m", "42m", "30s" */
export function formatUptime(secs: number): string {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (d > 0) return `${d}d ${h}h`;
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m`;
  return `${secs}s`;
}

export const plural = (n: number, one: string, many = `${one}s`) => `${formatNumber(n)} ${n === 1 ? one : many}`;

export const pad2 = (n: number) => String(n).padStart(2, "0");
