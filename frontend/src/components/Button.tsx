import clsx from "clsx";
import { m, type HTMLMotionProps } from "motion/react";
import { forwardRef, type ReactNode } from "react";
import { Link, type LinkProps } from "react-router";

import { spring } from "../motion/springs";
import { LEDLoader } from "./LEDLoader";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "glass" | "danger";
export type ButtonSize = "sm" | "md" | "lg" | "icon" | "icon-sm";

const base =
  "relative inline-flex select-none items-center justify-center gap-2 whitespace-nowrap t-label transition-[background-color,border-color,color] duration-150 disabled:opacity-45 disabled:pointer-events-none";

const variants: Record<ButtonVariant, string> = {
  primary: "bg-accent text-accent-fg hover:bg-accent-hover shadow-[var(--hl-inset)]",
  secondary: "bg-surface-2 text-fg-1 border border-line hover:bg-surface-3 hover:border-line-strong shadow-[var(--hl-inset)]",
  ghost: "text-fg-2 hover:text-fg-1 hover:bg-surface-3/70",
  glass: "text-white bg-[rgba(5,7,13,0.55)] backdrop-blur-md hover:bg-[rgba(5,7,13,0.7)]",
  danger: "text-err hover:bg-err-soft",
};

const sizes: Record<ButtonSize, string> = {
  sm: "h-9 px-3 rounded-[10px]",
  md: "h-11 px-4 rounded-[14px]",
  lg: "h-13 px-[22px] rounded-[14px] text-[15px]",
  icon: "h-11 w-11 rounded-[12px]",
  "icon-sm": "h-9 w-9 rounded-[10px]",
};

export function buttonClass(variant: ButtonVariant = "secondary", size: ButtonSize = "md", className?: string) {
  return clsx(base, variants[variant], sizes[size], className);
}

type Common = {
  variant?: ButtonVariant;
  size?: ButtonSize;
  icon?: ReactNode;
  trailing?: ReactNode;
  loading?: boolean;
  children?: ReactNode;
};

export const Button = forwardRef<HTMLButtonElement, Common & Omit<HTMLMotionProps<"button">, "children">>(
  function Button({ variant = "secondary", size = "md", icon, trailing, loading, children, className, type, ...rest }, ref) {
    return (
      <m.button
        ref={ref}
        type={type ?? "button"}
        whileTap={{ scale: size.startsWith("icon") ? 0.9 : 0.97 }}
        transition={spring.press}
        className={buttonClass(variant, size, className)}
        aria-busy={loading || undefined}
        {...rest}
      >
        {variant === "primary" && <span className="sheen" aria-hidden />}
        {loading ? (
          <LEDLoader dots={5} className="mx-2" />
        ) : (
          <>
            {icon}
            {children}
            {trailing}
          </>
        )}
      </m.button>
    );
  },
);

export function ButtonLink({
  variant = "secondary",
  size = "md",
  icon,
  trailing,
  children,
  className,
  ...rest
}: Common & LinkProps) {
  return (
    <Link className={buttonClass(variant, size, clsx("transition-transform active:scale-[0.97]", className))} {...rest}>
      {variant === "primary" && <span className="sheen" aria-hidden />}
      {icon}
      {children}
      {trailing}
    </Link>
  );
}
