import clsx from "clsx";
import { AnimatePresence, m } from "motion/react";
import { CircleAlert } from "lucide-react";
import { forwardRef, useId, type InputHTMLAttributes, type ReactNode, type TextareaHTMLAttributes } from "react";

import { dur } from "../motion/springs";

const fieldClass = (error?: string) =>
  clsx(
    "peer w-full rounded-[10px] border bg-surface-1 px-3.5 t-body text-fg-1 placeholder:text-fg-3 outline-none transition-colors duration-150",
    "hover:border-line-strong focus:border-accent focus-visible:shadow-[0_0_0_4px_var(--accent-soft)] focus-visible:outline-none",
    "disabled:opacity-60 disabled:bg-surface-3",
    error ? "border-err bg-err-soft pr-10" : "border-line",
  );

function Frame({
  id,
  label,
  optional,
  required,
  error,
  hint,
  children,
}: {
  id: string;
  label: string;
  optional?: boolean;
  required?: boolean;
  error?: string;
  hint?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-1.5">
      <label htmlFor={id} className="t-label text-fg-2">
        {label}
        {required && <span className="text-accent"> *</span>}
        {optional && <span className="font-normal text-fg-3"> (optional)</span>}
      </label>
      <div className="relative">
        {children}
        {/* LED underline that lights left -> right on focus */}
        <span
          aria-hidden
          className="pointer-events-none absolute inset-x-3 bottom-[3px] h-[2px] origin-left scale-x-0 transition-transform duration-[240ms] ease-[cubic-bezier(0.22,1,0.36,1)] peer-focus:scale-x-100"
          style={{
            backgroundImage: "radial-gradient(circle, var(--accent) 1px, transparent 1.2px)",
            backgroundSize: "5px 2px",
          }}
        />
        {error && <CircleAlert size={16} className="absolute right-3 top-3.5 text-err" aria-hidden />}
      </div>
      <div className="flex items-start justify-between gap-3">
        <AnimatePresence initial={false}>
          {error && (
            <m.p
              id={`${id}-err`}
              initial={{ opacity: 0, height: 0 }}
              animate={{ opacity: 1, height: "auto" }}
              exit={{ opacity: 0, height: 0 }}
              transition={{ duration: dur.base }}
              className="t-body-s text-err"
            >
              {error}
            </m.p>
          )}
        </AnimatePresence>
        {hint && <div className="ml-auto">{hint}</div>}
      </div>
    </div>
  );
}

type InputProps = InputHTMLAttributes<HTMLInputElement> & {
  label: string;
  optional?: boolean;
  error?: string;
  hint?: ReactNode;
};

export const Input = forwardRef<HTMLInputElement, InputProps>(function Input(
  { label, optional, error, hint, className, id: idProp, required, ...rest },
  ref,
) {
  const autoId = useId();
  const id = idProp ?? autoId;
  return (
    <Frame id={id} label={label} optional={optional} required={required} error={error} hint={hint}>
      <input
        ref={ref}
        id={id}
        aria-invalid={error ? true : undefined}
        aria-describedby={error ? `${id}-err` : undefined}
        aria-required={required || undefined}
        className={clsx(fieldClass(error), "h-12 sm:h-11", className)}
        {...rest}
      />
    </Frame>
  );
});

type TextareaProps = TextareaHTMLAttributes<HTMLTextAreaElement> & {
  label: string;
  optional?: boolean;
  error?: string;
};

export const Textarea = forwardRef<HTMLTextAreaElement, TextareaProps>(function Textarea(
  { label, optional, error, className, id: idProp, ...rest },
  ref,
) {
  const autoId = useId();
  const id = idProp ?? autoId;
  return (
    <Frame id={id} label={label} optional={optional} error={error}>
      <textarea
        ref={ref}
        id={id}
        rows={4}
        aria-invalid={error ? true : undefined}
        className={clsx(fieldClass(error), "min-h-[112px] max-h-[320px] resize-none py-3 [field-sizing:content]", className)}
        {...rest}
      />
    </Frame>
  );
});
