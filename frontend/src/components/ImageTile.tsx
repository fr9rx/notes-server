import clsx from "clsx";
import { ImageOff } from "lucide-react";
import { useState } from "react";

import { DotShimmer } from "./Skeleton";

/**
 * An image with a dot-shimmer placeholder and a blur-up reveal (DESIGN.md §6.8).
 * Fills its parent; the parent sets the aspect ratio.
 */
export function ImageTile({
  src,
  srcSet,
  sizes,
  alt,
  eager,
  blur = true,
  className,
  imgClassName,
  index = 0,
}: {
  src: string;
  srcSet?: string;
  sizes?: string;
  alt: string;
  eager?: boolean;
  /** Blur-up only the first dozen tiles (perf). */
  blur?: boolean;
  className?: string;
  imgClassName?: string;
  index?: number;
}) {
  const [state, setState] = useState<"loading" | "loaded" | "error">("loading");
  return (
    <div className={clsx("absolute inset-0 overflow-hidden", className)}>
      {state !== "loaded" && <DotShimmer className="absolute inset-0" index={index} />}
      {state === "error" ? (
        <div className="absolute inset-0 flex flex-col items-center justify-center gap-2 bg-surface-3 text-fg-3">
          <ImageOff size={20} aria-hidden />
          <span className="t-meta px-2 text-center">Couldn't load this photo</span>
        </div>
      ) : (
        <img
          src={src}
          srcSet={srcSet}
          sizes={sizes}
          alt={alt}
          loading={eager ? "eager" : "lazy"}
          decoding="async"
          fetchPriority={eager ? "high" : "auto"}
          onLoad={() => setState("loaded")}
          onError={() => setState("error")}
          className={clsx(
            "absolute inset-0 h-full w-full object-cover transition-[opacity,filter,transform] duration-[360ms] ease-[cubic-bezier(0.16,1,0.3,1)]",
            state === "loaded" ? "scale-100 opacity-100 blur-0" : clsx("opacity-0", blur && "scale-[1.04] blur-[12px]"),
            imgClassName,
          )}
          draggable={false}
        />
      )}
    </div>
  );
}
