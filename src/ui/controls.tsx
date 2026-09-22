import { useEffect, useId, useRef, useState, type ComponentProps, type ReactNode } from "react";

const target = "min-h-16 min-w-16 rounded-xl px-5 py-3 text-lg font-semibold leading-snug inline-flex items-center justify-center gap-3";
export function Button({ className = "", type = "button", ...props }: ComponentProps<"button">) {
  return <button type={type} className={`${target} ${className}`} {...props} />;
}
export function Input({ className = "", type, ...props }: ComponentProps<"input">) {
  const visible = type !== "checkbox" && type !== "radio" && type !== "hidden";
  return <input type={type} className={`${visible ? "min-h-16 rounded-xl px-4 py-3 text-lg w-full min-w-0" : ""} ${className}`} {...props} />;
}
export function Select({ className = "", ...props }: ComponentProps<"select">) {
  return <select className={`min-h-16 rounded-xl px-4 py-3 text-lg w-full min-w-0 ${className}`} {...props} />;
}

// Keep children mounted: collapsing never loses an editor, a draft or a value.
// Errors reveal the section even when they arrive after it was closed.
export function MoreOptions({ label = "More options", accessibleLabel, children, attention = false, initiallyOpen = false }: { label?: string; accessibleLabel?: string; children: ReactNode; attention?: boolean; initiallyOpen?: boolean }) {
  const [open, setOpen] = useState(initiallyOpen);
  const id = useId();
  const content = useRef<HTMLDivElement>(null);
  useEffect(() => { if (attention) setOpen(true); }, [attention]);
  useEffect(() => {
    const node = content.current;
    if (!node) return;
    const revealErrors = () => {
      if ([...node.querySelectorAll('[role="alert"], [aria-invalid="true"]')].some(el => el.textContent?.trim() || el.getAttribute('aria-invalid') === 'true')) setOpen(true);
    };
    revealErrors();
    const observer = new MutationObserver(revealErrors);
    observer.observe(node, { childList: true, subtree: true, characterData: true, attributes: true, attributeFilter: ["aria-invalid"] });
    return () => observer.disconnect();
  }, []);
  return <section className="more-options">
    <Button aria-label={accessibleLabel} className="secondary w-full justify-between" aria-expanded={open} aria-controls={id} onClick={() => setOpen(!open)}>{label}<span aria-hidden="true">{open ? "−" : "+"}</span></Button>
    <div id={id} ref={content} hidden={!open} onInvalidCapture={() => setOpen(true)} className="more-options-content">{children}</div>
  </section>;
}
