import type { ReactNode } from "react";
import type { Page } from "../find/index";

// Shared page family: a 24-unit grid, rounded ends and one stroke weight.
// Keep the shapes tied to the page rather than to the input method it accepts.
const drawings: Record<Page, ReactNode> = {
  home: <path d="m3 10 9-7 9 7v9a2 2 0 0 1-2 2h-4v-7H9v7H5a2 2 0 0 1-2-2z" />,
  switches: <><ellipse cx="10" cy="8" rx="7" ry="3.5" /><path d="M3 8v4c0 2 3.1 3.5 7 3.5s7-1.5 7-3.5V8m0 4h1a3 3 0 0 1 3 3v3h-4" /></>,
  scanning: <path d="M7 3H4a1 1 0 0 0-1 1v3m14-4h3a1 1 0 0 1 1 1v3M3 17v3a1 1 0 0 0 1 1h3m14-4v3a1 1 0 0 1-1 1h-3M9 6v12m4-6h5m-2-2 2 2-2 2" />,
  panels: <><rect x="3" y="3" width="7" height="7" rx="1.5" /><rect x="14" y="3" width="7" height="7" rx="1.5" /><rect x="3" y="14" width="7" height="7" rx="1.5" /><rect x="14" y="14" width="7" height="7" rx="1.5" /></>,
  mobile: <><rect x="6" y="2.5" width="12" height="19" rx="2.5" /><path d="M10 5h4m-3 13.5h2" /></>,
  settings: <><path d="M3 6h8m4 0h6M3 12h2m4 0h12M3 18h12m4 0h2" /><rect x="11" y="4" width="4" height="4" rx="1" /><rect x="5" y="10" width="4" height="4" rx="1" /><rect x="15" y="16" width="4" height="4" rx="1" /></>,
  support: <><circle cx="12" cy="12" r="9" /><path d="M9.5 9a2.5 2.5 0 0 1 5 .4c0 1.8-2.5 2-2.5 4M12 17h.01" /></>,
};

/** Decorative companion to visible text naming a page, device or task. */
export function PageIcon({ page, size = 24 }: { page: Page; size?: number }) {
  return <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" focusable="false">{drawings[page]}</svg>;
}
