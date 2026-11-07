import { createContext, useContext, useId, useLayoutEffect, useRef, type ComponentProps, type RefObject } from "react";
import { createPortal } from "react-dom";
import { Button } from "./controls";
import { FeedbackMessage } from "./patterns";

type Entry = { backdrop: HTMLDivElement; dialog: HTMLElement; previous: HTMLElement | null; id: symbol; parent: symbol | null };
const ParentDialog = createContext<{ id: symbol; suspended: boolean } | null>(null);
const dialogs: Entry[] = [];
const originalInert = new Map<Element, boolean>();

function updateBackground() {
  for (const [element, inert] of originalInert) element.toggleAttribute("inert", inert || element.hasAttribute("hidden"));
  const top = dialogs.at(-1);
  if (!top) { originalInert.clear(); return; }
  for (const element of document.body.children) {
    if (!originalInert.has(element)) originalInert.set(element, element.hasAttribute("inert"));
    if (element !== top.backdrop) element.setAttribute("inert", "");
  }
}

function controls(dialog: HTMLElement) {
  return [...dialog.querySelectorAll<HTMLElement>('button, a[href], input, select, textarea, summary, [tabindex]')]
    .filter((element) => {
      if (element.tabIndex < 0 || element.matches(":disabled") || element.closest('[hidden], [inert]')) return false;
      for (let ancestor: HTMLElement | null = element; ancestor && ancestor !== dialog; ancestor = ancestor.parentElement) {
        const style = getComputedStyle(ancestor);
        if (style.display === "none" || style.visibility === "hidden") return false;
      }
      return true;
    });
}

type DialogProps = Omit<ComponentProps<"section">, "ref" | "aria-label" | "aria-labelledby"> & ({ "aria-label": string; "aria-labelledby"?: string } | { "aria-labelledby": string; "aria-label"?: string }) & {
  dialogRef?: RefObject<HTMLElement | null>;
  initialFocus?: RefObject<HTMLElement | null>;
  onEscape?: () => void;
  active?: boolean;
  suspended?: boolean;
  backdropClassName?: string;
  /** Native learning owns Escape and all keyboard edges. */
  captureKeyboard?: boolean;
};

/** One modal owner, including dialogs opened over setup or another editor. */
export function Dialog({ dialogRef, initialFocus, onEscape, active = true, suspended = false, backdropClassName = "", captureKeyboard = false, className = "", role = "dialog", onKeyDown, children, ...props }: DialogProps) {
  const ownRef = useRef<HTMLElement>(null);
  const context = useContext(ParentDialog);
  const parent = context?.id ?? null;
  const paused = suspended || !!context?.suspended;
  const id = useRef(Symbol("dialog")).current;
  const ref = dialogRef ?? ownRef;
  const backdrop = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    if (!active || paused || !ref.current || !backdrop.current) return;
    const entry: Entry = { backdrop: backdrop.current, dialog: ref.current, previous: document.activeElement instanceof HTMLElement ? document.activeElement : null, id, parent };
    const child = dialogs.findIndex((candidate) => {
      let ancestor = candidate.parent;
      while (ancestor) {
        if (ancestor === id) return true;
        ancestor = dialogs.find((dialog) => dialog.id === ancestor)?.parent ?? null;
      }
      return false;
    });
    dialogs.splice(child < 0 ? dialogs.length : child, 0, entry);
    updateBackground();
    if (dialogs.at(-1) === entry) {
      (initialFocus?.current ?? entry.dialog).focus();
      if (!entry.dialog.contains(document.activeElement)) entry.dialog.focus();
    }
    const retainFocus = (event: FocusEvent) => {
      if (dialogs.at(-1) === entry && !entry.dialog.contains(event.target as Node)) entry.dialog.focus();
    };
    const swallow = (event: KeyboardEvent) => {
      if (captureKeyboard && dialogs.at(-1) === entry) { event.preventDefault(); event.stopImmediatePropagation(); }
    };
    document.addEventListener("focusin", retainFocus);
    for (const type of ["keydown", "keyup", "keypress"] as const) document.addEventListener(type, swallow, true);
    return () => {
      document.removeEventListener("focusin", retainFocus);
      for (const type of ["keydown", "keyup", "keypress"] as const) document.removeEventListener(type, swallow, true);
      const wasTop = dialogs.at(-1) === entry;
      dialogs.splice(dialogs.indexOf(entry), 1);
      updateBackground();
      if (wasTop) {
        const top = dialogs.at(-1);
        if (entry.previous?.isConnected && !entry.previous.closest("[inert], [hidden]") && !entry.previous.matches(":disabled") && (!top || top.dialog.contains(entry.previous))) entry.previous.focus();
        else top?.dialog.focus();
      }
    };
  }, [active, paused, captureKeyboard, ref, initialFocus, id, parent]);

  useLayoutEffect(() => {
    const shell = document.querySelector<HTMLElement>(".app-shell");
    if (backdrop.current && shell) {
      backdrop.current.dataset.scan = shell.dataset.scan;
      backdrop.current.dataset.thickness = shell.dataset.thickness;
    }
    // Disabling the focused action during a save must not leave focus outside.
    if (dialogs.at(-1)?.dialog === ref.current && (!ref.current?.contains(document.activeElement) || document.activeElement?.matches(":disabled"))) ref.current?.focus();
  });

  return createPortal(<div ref={backdrop} className={`modal-backdrop ${backdropClassName}`.trim()} hidden={paused} inert={paused}>
    <section {...props} ref={ref} className={`dialog ${className}`.trim()} role={active ? role : undefined} aria-modal={active ? true : undefined} tabIndex={-1} onKeyDown={(event) => {
      onKeyDown?.(event);
      if (!active || paused || event.defaultPrevented || dialogs.at(-1)?.dialog !== ref.current) return;
      event.stopPropagation();
      if (event.key === "Escape") { event.preventDefault(); onEscape?.(); }
      if (event.key !== "Tab") return;
      const items = controls(event.currentTarget);
      const first = items[0], last = items.at(-1);
      if (!first) { event.preventDefault(); event.currentTarget.focus(); }
      else if (!items.includes(document.activeElement as HTMLElement)) { event.preventDefault(); (event.shiftKey ? last! : first).focus(); }
      else if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last!.focus(); }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
    }}><ParentDialog.Provider value={{ id, suspended: paused }}>{children}</ParentDialog.Provider></section>
  </div>, document.body);
}

export function ConfirmDialog({ title, description, action, cancelLabel = "Keep editing", busy = false, destructive = true, error, onCancel, onConfirm }: {
  title: string; description?: string; action: string; cancelLabel?: string; busy?: boolean; destructive?: boolean; error?: string | null; onCancel: () => void; onConfirm: () => void;
}) {
  const cancel = useRef<HTMLButtonElement>(null);
  // IDs are local to the dialog rather than tied to a particular editor.
  const titleId = useId();
  const descriptionId = `${titleId}-description`;
  return <Dialog className="profile-dialog confirm-dialog" role="alertdialog" aria-labelledby={titleId} aria-describedby={description ? descriptionId : undefined} initialFocus={cancel} onEscape={() => { if (!busy) onCancel(); }}>
    <header><div><h2 id={titleId}>{title}</h2>{description && <p id={descriptionId}>{description}</p>}</div></header>
    {error && <FeedbackMessage>{error}</FeedbackMessage>}
    <footer><span /><Button ref={cancel} className="secondary" disabled={busy} onClick={onCancel}>{cancelLabel}</Button><Button className={destructive ? "primary danger" : "primary"} disabled={busy} onClick={onConfirm}>{busy ? "Saving…" : action}</Button></footer>
  </Dialog>;
}
