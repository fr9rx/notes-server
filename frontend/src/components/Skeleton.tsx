import clsx from "clsx";
import { useEffect, useState, type CSSProperties } from "react";

/** Dot-shimmer surface ("LEDs scanning"). */
export function DotShimmer({ className, style, index = 0 }: { className?: string; style?: CSSProperties; index?: number }) {
  return (
    <div
      className={clsx("dot-shimmer", className)}
      style={{ ...style, ["--shimmer-delay" as string]: `${index * 200}ms` }}
      aria-hidden
    />
  );
}

export function TextSkeleton({ widths = ["92%", "64%"], className }: { widths?: string[]; className?: string }) {
  return (
    <div className={clsx("space-y-2", className)} aria-hidden>
      {widths.map((w, i) => (
        <div key={i} className="h-2.5 rounded-full bg-surface-3" style={{ width: w }} />
      ))}
    </div>
  );
}

/** Shows children only after `delay` ms (avoids skeleton flashes on a fast LAN). */
export function Delayed({ children, delay = 150 }: { children: React.ReactNode; delay?: number }) {
  const [show, setShow] = useState(delay === 0);
  useEffect(() => {
    const t = window.setTimeout(() => setShow(true), delay);
    return () => window.clearTimeout(t);
  }, [delay]);
  return show ? <>{children}</> : null;
}
