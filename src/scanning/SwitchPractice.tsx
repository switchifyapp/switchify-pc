import { Dialog } from "../ui/Dialog";
import { FeedbackMessage } from "../ui/patterns";
import { Button, Select } from "../ui/controls";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { actionLabel, type SwitchAction } from "./useSwitches";

type Block = { id: string; label: string; selected: boolean };
export type PracticeView = { active: boolean; source: string; message: string; input: string; action: SwitchAction | null; completed: number; lastChoice?: string | null; automatic?: boolean; stopped?: boolean; blocks: Block[] };

/** Whether this window can run practice, which needs the Tauri backend. */
export const practiceAvailable = () => "__TAURI_INTERNALS__" in window;

/**
 * A safe practice session in the backend: real switch input moves a highlight
 * among blocks, and nothing reaches other apps. Practice ends when the owner
 * unmounts, so leaving a page or setup step never leaves input captured.
 */
export function usePractice({ onClosed }: { onClosed?: () => void } = {}) {
  const [open, setOpen] = useState(false);
  const [view, setView] = useState<PracticeView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const request = useRef(0);
  const closed = useRef(onClosed);
  closed.current = onClosed;
  useEffect(() => () => { request.current++; if (practiceAvailable()) void invoke("end_switch_practice").catch(() => {}); }, []);
  const close = async () => {
    const id = ++request.current; setPending(true);
    try { await invoke("end_switch_practice"); if (id === request.current) { setOpen(false); setView(null); closed.current?.(); } }
    catch (e) { if (id === request.current) setError(String(e)); }
    finally { if (id === request.current) setPending(false); }
  };
  useEffect(() => {
    if (!open) return;
    let alive = true;
    let polling = false;
    const timer = window.setInterval(() => {
      if (polling) return;
      polling = true;
      void invoke<PracticeView>("get_switch_practice").then(async v => { if (alive) { setView(v); if (!v.active) { await close(); } } }).catch(async e => {
        if (alive) {
          setError(`Practice feedback is unavailable. Ending the test safely. ${String(e)}`);
          await close();
        }
      }).finally(() => { polling = false; });
    }, 100);
    return () => { alive = false; window.clearInterval(timer); };
  }, [open]);
  const begin = async (remote: boolean) => {
    const id = ++request.current;
    setError(null); setPending(true);
    try {
      const next = await invoke<PracticeView>("begin_switch_practice", { remote });
      if (id !== request.current) { await invoke("end_switch_practice"); return; }
      setView(next); setOpen(true);
    } catch (e) { if (id === request.current) setError(String(e)); }
    finally { if (id === request.current) setPending(false); }
  };
  return { open, view, error, pending, begin, close };
}

export function practiceInstructions(automatic: boolean | undefined) {
  return automatic
    ? "Use Select to start. The highlight moves by itself at your menu scan rate; use Select again to choose a block."
    : "Use Select to start, Next and Previous to move among the blocks, then Select to choose one.";
}

export function SwitchPractice({ disabled = false }: { disabled?: boolean }) {
  const [remote, setRemote] = useState(false);
  const trigger = useRef<HTMLButtonElement>(null);
  const restoreFocus = useRef(false);
  const { open, view, error, pending, begin, close } = usePractice({ onClosed: () => { restoreFocus.current = true; } });
  useEffect(() => { if (!open && !pending && restoreFocus.current) { restoreFocus.current = false; trigger.current?.focus(); } }, [open, pending]);
  const highlighted = view?.blocks.filter(block => block.selected).map(block => block.label).join(", ");
  return <div className="switch-practice-entry" data-section="practice" data-setting="practice">
    <p>Try your saved switches safely.</p>
    <label>Test source <Select aria-label="Test source" value={remote ? "remote" : "local"} disabled={open || pending || disabled} onChange={e => setRemote(e.target.value === "remote")}><option value="local">Switches on this computer</option><option value="remote">Switches from Switchify Remote</option></Select></label>
    <Button ref={trigger} type="button" className="secondary" disabled={pending || disabled || !practiceAvailable()} onClick={() => void begin(remote)}>Test switches</Button>
    {!open && error && <FeedbackMessage>{error}</FeedbackMessage>}
    {open && <Dialog className="switch-practice-dialog" aria-labelledby="practice-title" onEscape={() => { if (!pending) void close(); }}>
      <h2 id="practice-title">Safe switch practice</h2>
      <p>{practiceInstructions(view?.automatic)} Nothing here clicks, types, or changes settings.</p>
      <p>Press Escape, use Stop scanning, or hold any switch through its emergency stop to finish. The practice window closes when testing stops, including on loss of focus, disconnect, or after two minutes. Remote forwarding stops when practice ends.</p>
      <p role="status" aria-live="polite">{view?.message ?? "Starting practice…"}</p>
      <p>{view?.source} · {view?.input ? `Last input: ${view.input}` : "No input received yet"}{view?.action && ` · ${actionLabel(view.action)}`} · Completed: {view?.completed ?? 0}</p>
      <ul className="switch-practice-blocks" aria-label="Practice blocks">
        {(view?.blocks ?? []).map(block => <li key={block.id} data-selected={block.selected}>{block.label}</li>)}
      </ul>
      <p aria-live="polite">Highlighted: {highlighted || "None — use Select to start"}</p>
      {error && <FeedbackMessage>{error}</FeedbackMessage>}
      <div className="switch-practice-buttons"><Button type="button" className="primary" disabled={pending} onClick={() => void close()}>Exit practice</Button></div>
    </Dialog>}
  </div>;
}
