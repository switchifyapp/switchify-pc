import { Dialog } from "../ui/Dialog";
import { FeedbackMessage } from "../ui/patterns";
import { Button, Select } from "../ui/controls";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { actionLabel, type SwitchAction } from "./useSwitches";

type Block = { id: string; label: string; selected: boolean };
type View = { active: boolean; source: string; message: string; input: string; action: SwitchAction | null; completed: number; blocks: Block[] };

export function SwitchPractice({ disabled = false }: { disabled?: boolean }) {
  const [open, setOpen] = useState(false);
  const [view, setView] = useState<View | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const [remote, setRemote] = useState(false);
  const trigger = useRef<HTMLButtonElement>(null);
  const request = useRef(0);
  const restoreFocus = useRef(false);
  useEffect(() => { if (!open && !pending && restoreFocus.current) { restoreFocus.current = false; trigger.current?.focus(); } }, [open, pending]);
  useEffect(() => () => { request.current++; if ("__TAURI_INTERNALS__" in window) void invoke("end_switch_practice").catch(() => {}); }, []);
  useEffect(() => {
    if (!open) return;
    let alive = true;
    let polling = false;
    const timer = window.setInterval(() => {
      if (polling) return;
      polling = true;
      void invoke<View>("get_switch_practice").then(async v => { if (alive) { setView(v); if (!v.active) { await close(); } } }).catch(async e => {
        if (alive) {
          setError(`Practice feedback is unavailable. Ending the test safely. ${String(e)}`);
          await close();
        }
      }).finally(() => { polling = false; });
    }, 100);
    return () => { alive = false; window.clearInterval(timer); };
  }, [open]);
  const begin = async () => {
    const id = ++request.current;
    setError(null); setPending(true);
    try {
      const next = await invoke<View>("begin_switch_practice", { remote });
      if (id !== request.current) { await invoke("end_switch_practice"); return; }
      setView(next); setOpen(true);
    } catch (e) { if (id === request.current) setError(String(e)); }
    finally { if (id === request.current) setPending(false); }
  };
  const close = async () => {
    const id = ++request.current; setPending(true);
    try { await invoke("end_switch_practice"); if (id === request.current) { restoreFocus.current = true; setOpen(false); setView(null); } }
    catch (e) { if (id === request.current) setError(String(e)); }
    finally { if (id === request.current) setPending(false); }
  };
  const highlighted = view?.blocks.filter(block => block.selected).map(block => block.label).join(", ");
  return <div className="switch-practice-entry" data-section="practice" data-setting="practice">
    <p>Try your saved switches safely.</p>
    <label>Test source <Select aria-label="Test source" value={remote ? "remote" : "local"} disabled={open || pending || disabled} onChange={e => setRemote(e.target.value === "remote")}><option value="local">Switches on this computer</option><option value="remote">Switches from Switchify Remote</option></Select></label>
    <Button ref={trigger} type="button" className="secondary" disabled={pending || disabled || !("__TAURI_INTERNALS__" in window)} onClick={() => void begin()}>Test switches</Button>
    {!open && error && <FeedbackMessage>{error}</FeedbackMessage>}
    {open && <Dialog className="switch-practice-dialog" aria-labelledby="practice-title" onEscape={() => { if (!pending) void close(); }}>
      <h2 id="practice-title">Safe switch practice</h2>
      <p>Use Select to start, Next and Previous to move among the blocks, then Select to choose one. Nothing here clicks, types, or changes settings.</p>
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
