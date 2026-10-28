import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AppState, LicenceView } from "../types";
import { Button } from "../ui/controls";
import { SettingGroup } from "./controls";

export function LicenceSection({ state }: { state: AppState }) {
  const [updated, setUpdated] = useState<LicenceView | null>(null);
  const [key, setKey] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirm, setConfirm] = useState(false);
  const [recoveryPaused, setRecoveryPaused] = useState(false);
  const scope = useRef<HTMLDivElement>(null);
  const highlighted = useRef<HTMLButtonElement | null>(null);
  const view = updated ?? state.licence;
  const locked = view?.configured && ["expired", "unavailable"].includes(view.status);
  useEffect(() => { setUpdated(null); }, [state.licence]);
  useEffect(() => {
    if (!locked || recoveryPaused || !("__TAURI_INTERNALS__" in window)) return;
    void invoke("set_licence_recovery", { active: true });
    // Automatic scanning stays inside the licence page, without native mouse
    // or keyboard injection. Any configured local switch selects the focus.
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Tab") setRecoveryPaused(true);
    };
    window.addEventListener("keydown", onKeyDown);
    let index = -1;
    const timer = window.setInterval(() => {
      const buttons = Array.from(scope.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? []);
      if (!buttons.length) return;
      index = (index + 1) % buttons.length;
      highlighted.current = buttons[index];
      buttons[index].focus();
    }, 1500);
    let alive = true;
    const stop = listen("licence-recovery-select", () => { if (alive) highlighted.current?.click(); });
    return () => {
      alive = false;
      window.clearInterval(timer);
      window.removeEventListener("keydown", onKeyDown);
      highlighted.current = null;
      void stop.then((unlisten) => unlisten());
      void invoke("set_licence_recovery", { active: false });
    };
  }, [locked, recoveryPaused]);
  const run = async (command: string, args?: Record<string, unknown>) => {
    setBusy(true); setError(null);
    try {
      const next = await invoke<LicenceView>(command, args);
      setUpdated(next); setKey(""); setConfirm(false);
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  };
  return <SettingGroup id="settings-licence" title="Licence" description="Your purchase and trial on this computer">
    <div ref={scope} onPointerDownCapture={() => setRecoveryPaused(true)} onFocusCapture={(event) => {
      if (event.target.matches("textarea,input,select")) setRecoveryPaused(true);
    }}>
      <p role="status">{view?.message ?? "Licensing is not enabled in this build."}</p>
      {view?.storageWarning && <p>Your licence storage backup is unavailable. Unlock your keychain and restart Switchify PC.</p>}
      {locked && <p>Switch control here stays available. Buttons receive focus automatically; press any configured local switch to choose one. Copy your key first, then choose Paste and activate. Using the mouse, Tab key or key entry field pauses automatic button scanning.</p>}
      {locked && recoveryPaused && <Button onClick={() => setRecoveryPaused(false)}>Resume automatic button scanning</Button>}
      {view?.configured && <>
        {view.licenceId && <p>Licence ID: {view.licenceId}</p>}
        <p>€100 one-time purchase for one computer. A key works offline, without a Switchify account.</p>
        <label htmlFor="licence-key">Licence key</label>
        <textarea id="licence-key" value={key} maxLength={2048} spellCheck={false} autoComplete="off" onChange={(e) => setKey(e.target.value)} />
        <div className="actions">
          <Button disabled={busy || !key.trim()} onClick={() => void run("redeem_licence", { key })}>Activate key</Button>
          <Button disabled={busy} onClick={() => void run("paste_licence")}>Paste and activate</Button>
        </div>
        <label htmlFor="licence-file">Or import a licence file</label>
        <input id="licence-file" type="file" accept=".txt,.licence" disabled={busy} onChange={(e) => {
          const file = e.target.files?.[0];
          e.target.value = "";
          if (!file) return;
          if (file.size > 4096) { setError("This licence file is too large."); return; }
          void file.text().then((text) => run("redeem_licence", { key: text })).catch(() => setError("Could not read this licence file."));
        }} />
        {view.status === "licensed" && (confirm ? <div>
          <p>Remove the key from this computer? This does not refund your purchase or restart your trial.</p>
          <Button disabled={busy} onClick={() => void run("remove_licence")}>Remove key</Button>
          <Button disabled={busy} onClick={() => setConfirm(false)}>Keep key</Button>
        </div> : <Button disabled={busy} onClick={() => setConfirm(true)}>Remove licence from this computer</Button>)}
      </>}
      {error && <p role="alert">{error}</p>}
      <p>For a lost key or help entering one, contact <a href="mailto:info@theswitchify.app">info@theswitchify.app</a>.</p>
    </div>
  </SettingGroup>;
}
