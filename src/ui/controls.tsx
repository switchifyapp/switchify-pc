import { useEffect, useId, useRef, useState, type ComponentProps, type ReactNode } from "react";

// Shape and size live in the stylesheet (.btn, .field-control); className
// picks the variant, so a variant can change anything the base sets.
const classes = (...names: (string | false)[]) => names.filter(Boolean).join(" ") || undefined;
export function Button({ className = "", type = "button", ...props }: ComponentProps<"button">) {
  return <button type={type} className={classes("btn", className)} {...props} />;
}
// A status tile: green with a tick when done, amber with a mark when
// something needs doing; `ok` omitted marks a row with only a subject.
export function StatusIcon({ ok, children }: { ok?: boolean; children: ReactNode }) {
  return <span className="status-icon" data-ok={ok} aria-hidden="true">{children}</span>;
}
export function Input({ className = "", type, ...props }: ComponentProps<"input">) {
  const visible = type !== "checkbox" && type !== "radio" && type !== "hidden";
  return <input type={type} className={classes(visible && "field-control", className)} {...props} />;
}
export function Select({ className = "", ...props }: ComponentProps<"select">) {
  return <select className={classes("field-control", className)} {...props} />;
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
    <Button aria-label={accessibleLabel} className="secondary more-options-toggle" aria-expanded={open} aria-controls={id} onClick={() => setOpen(!open)}>{label}<span aria-hidden="true">{open ? "−" : "+"}</span></Button>
    <div id={id} ref={content} hidden={!open} onInvalidCapture={() => setOpen(true)} className="more-options-content">{children}</div>
  </section>;
}
