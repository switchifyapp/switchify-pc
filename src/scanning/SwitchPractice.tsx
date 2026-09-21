import { createPortal } from "react-dom";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { actions, type SwitchAction } from "./useSwitches";

type View = { active: boolean; source: string; message: string; input: string; action: SwitchAction | null; completed: number; rectangles: number[][]; tiles: { rect: number[]; text: string; selected: boolean }[]; label: string };
export function SwitchPractice({ disabled = false }: { disabled?: boolean }) {
  const [open, setOpen] = useState(false);
  const [view, setView] = useState<View | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const [remote, setRemote] = useState(false);
  const dialog = useRef<HTMLElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const request = useRef(0);
  const restoreFocus = useRef(false);
  useEffect(() => { if (!open && !pending && restoreFocus.current) { restoreFocus.current = false; trigger.current?.focus(); } }, [open, pending]);
  useEffect(() => () => { request.current++; if ("__TAURI_INTERNALS__" in window) void invoke("end_switch_practice").catch(() => {}); }, []);
  useEffect(() => {
    if (!open) return;
    const background = [...document.body.children].filter((element) => !element.contains(dialog.current));
    const previous = background.map(element => element.hasAttribute("inert"));
    background.forEach(element => element.setAttribute("inert", ""));
    dialog.current?.focus();
    let alive = true;
    let polling = false;
    const timer = window.setInterval(() => {
      if (polling) return;
      polling = true;
      void invoke<View>("get_switch_practice").then(v => { if (alive) setView(v); }).catch(e => {
        if (alive) setError(String(e));
        void invoke("end_switch_practice").catch(() => {});
      }).finally(() => { polling = false; });
    }, 100);
    return () => { alive = false; window.clearInterval(timer); background.forEach((element, i) => { if (!previous[i]) element.removeAttribute("inert"); }); };
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
    request.current++; setPending(true);
    try { await invoke("end_switch_practice"); restoreFocus.current = true; setOpen(false); setView(null); }
    catch (e) { setError(String(e)); }
    finally { setPending(false); }
  };
  return <div className="switch-practice-entry">
    <p>Test your saved switches safely. Select starts a scan; Next and Previous move an active scan, not Tab or Shift+Tab focus. Automatic scanning moves for you; auto-selection can choose a point after a delay. Adjust these in Scanning.</p>
    <label>Test input <select aria-label="Test input" value={remote ? "remote" : "local"} disabled={open || pending || disabled} onChange={e => setRemote(e.target.value === "remote")}><option value="local">Local keyboard switches</option><option value="remote">Remote forwarding switches</option></select></label>
    <button ref={trigger} type="button" className="secondary" disabled={pending || disabled || !("__TAURI_INTERNALS__" in window)} onClick={() => void begin()}>Test switches</button>
    {!open && error && <p role="alert">{error}</p>}
    {open && createPortal(<div className="modal-backdrop"><section ref={dialog} className="switch-practice-dialog" role="dialog" aria-modal="true" aria-labelledby="practice-title" tabIndex={-1} onKeyDown={e => {
      e.stopPropagation();
      if (e.key === "Escape") { e.preventDefault(); void close(); }
      if (e.key === "Tab") {
        const buttons = [...(dialog.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? [])];
        if (buttons.length) { e.preventDefault(); const i = buttons.indexOf(document.activeElement as HTMLButtonElement); buttons[(i + (e.shiftKey ? buttons.length - 1 : 1)) % buttons.length].focus(); }
      }
    }}>
      <h2 id="practice-title">Safe switch practice</h2>
      <p>Press Select to start, use Next/Previous to move, then Select to choose a point and an action. Nothing here clicks, types, or changes settings. Practice uses your saved line/grid and timing settings.</p>
      <p>Press Escape, use Stop scanning, or hold any switch through its emergency stop to finish. Practice also stops on loss of focus, disconnect, or after two minutes. Remote forwarding stops when practice ends.</p>
      <p role="status" aria-live="polite">{view?.message ?? "Starting practice…"}</p>
      <p>{view?.source} · {view?.input ? `Last input: ${view.input}` : "No input received yet"}{view?.action && ` · ${actions[view.action]}`} · Completed: {view?.completed ?? 0}</p>
      <svg className="switch-practice-preview" viewBox="0 0 1280 720" role="img" aria-label={view?.label || "Practice scanning area"}>
        <rect width="1280" height="720" fill="#18202c" />
        <circle cx="640" cy="360" r="45" fill="#45546a" />
        <text x="640" y="450" textAnchor="middle" fill="white" fontSize="24">Choose any point to practise</text>
        {view?.rectangles.map(([x,y,width,height],i) => <rect key={i} x={x} y={y} width={width} height={height} fill="#64a6ff" opacity="0.65" />)}
        {view?.tiles.map(({rect:[x,y,width,height],text,selected},i) => <g key={i}><rect x={x} y={y} width={width} height={height} fill={selected ? "#356394" : "#253040"} stroke={selected ? "white" : "#8092ac"}/><text x={x+width/2} y={y+height/2} textAnchor="middle" dominantBaseline="middle" fill="white" fontSize="18">{text}</text></g>)}
      </svg>
      <p>{view?.label}</p>
      {error && <p role="alert">{error}</p>}
      <div className="switch-practice-buttons"><button type="button" className="secondary" disabled={pending || view?.active} onClick={() => void begin()}>Start another test</button><button type="button" className="primary" disabled={pending} onClick={() => void close()}>Exit practice</button></div>
    </section></div>, document.body)}
  </div>;
}
