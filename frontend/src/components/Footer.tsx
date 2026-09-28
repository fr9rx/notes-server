import { Link } from "react-router";

import { LogoMark } from "./Header";
import { ThemeSelect } from "./ThemeToggle";

export function Footer() {
  return (
    <footer className="mt-24 border-t border-line-subtle bg-bg-1 pb-[calc(32px+env(safe-area-inset-bottom))] pt-12">
      <div className="mx-auto flex max-w-[1240px] flex-col gap-8 px-4 sm:px-6 md:flex-row md:items-center md:justify-between lg:px-8">
        <div className="flex items-center gap-4">
          <Link to="/" aria-label="notes — home" className="rounded-[10px]">
            <LogoMark size={40} />
          </Link>
          <div className="t-meta space-y-1 text-fg-3">
            <p className="font-display text-[18px] font-[680] tracking-[-0.02em] text-fg-1">notes</p>
            <p>Course notes, shared by students on your network.</p>
          </div>
        </div>
        <div className="flex flex-col items-start gap-3 md:items-end">
          <ThemeSelect />
          <p className="t-body-s text-fg-3">No accounts. No cloud. Just notes.</p>
        </div>
      </div>
    </footer>
  );
}
