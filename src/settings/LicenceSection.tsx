import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Check, KeyRound } from "lucide-react";
import type { AppState, LicenceView } from "../types";
import { Button } from "../ui/controls";
import { SettingGroup } from "./controls";
import { BuyLicence } from "../licensing/BuyLicence";

export function LicenceSection({ state }: { state: AppState }) {
  const [updated, setUpdated] = useState<LicenceView | null>(null);
  const [key, setKey] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirm, setConfirm] = useState(false);
  const [recoveryPaused, setRecoveryPaused] = useState(false);
  const scope = useRef<HTMLDivElement>(null);
  const fileInput = useRef<HTMLInputElement>(null);
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
    <div className="licence-panel" ref={scope} onPointerDownCapture={() => setRecoveryPaused(true)} onFocusCapture={(event) => {
      if (event.target.matches("textarea,input,select")) setRecoveryPaused(true);
    }}>
      <div className="licence-status" data-licensed={view?.status === "licensed"}>
        {view?.status === "licensed" ? <Check aria-hidden="true" /> : <KeyRound aria-hidden="true" />}
        <div>
          <p role="status">{view?.message ?? "Licensing is not enabled in this build."}</p>
          {view?.licenceId && <p className="licence-id">Licence ID: {view.licenceId}</p>}
        </div>
      </div>
      {view?.storageWarning && <p className="licence-notice">Your licence storage backup is unavailable. Unlock your keychain and restart Switchify PC.</p>}
      {locked && <p className="licence-notice">Switch control here stays available. Buttons receive focus automatically; press any configured local switch to choose one. Copy your key first, then choose Paste and activate. Using the mouse, Tab key or key entry field pauses automatic button scanning.</p>}
      {locked && recoveryPaused && <Button onClick={() => setRecoveryPaused(false)}>Resume automatic button scanning</Button>}
      {view?.configured && <>
        <div className="licence-activation">
          <div>
            <h3>{view.status === "licensed" ? "Use a different key" : "Activate your licence"}</h3>
            <p className="licence-supporting">A licence covers one computer and works offline, without a Switchify account.</p>
          </div>
          <div className="licence-key-field">
            <label htmlFor="licence-key">Licence key</label>
            <textarea className="field-control" id="licence-key" rows={4} value={key} maxLength={2048} spellCheck={false} autoComplete="off" onChange={(e) => setKey(e.target.value)} />
          </div>
          <div className="licence-actions">
            <Button className={key.trim() ? "primary" : "secondary"} disabled={busy || !key.trim()} onClick={() => void run("redeem_licence", { key })}>Activate key</Button>
            <Button className={key.trim() ? "secondary" : "primary"} disabled={busy} onClick={() => void run("paste_licence")}>Paste and activate</Button>
          </div>
          <div className="licence-import">
            <p className="licence-supporting">Have your key saved in a text file?</p>
            <Button disabled={busy} onClick={() => fileInput.current?.click()}>Import licence file</Button>
          </div>
          {view.status !== "licensed" && <BuyLicence />}
        </div>
        <input ref={fileInput} aria-label="Licence file" type="file" accept=".txt,.licence" hidden disabled={busy} onChange={(e) => {
          const file = e.target.files?.[0];
          e.target.value = "";
          if (!file) return;
          if (file.size > 4096) { setError("This licence file is too large."); return; }
          void file.text().then((text) => run("redeem_licence", { key: text })).catch(() => setError("Could not read this licence file."));
        }} />
        {view.status === "licensed" && <div className="licence-removal">{confirm ? <>
          <p>Remove the key from this computer? This does not refund your purchase or restart your trial.</p>
          <div className="licence-actions">
            <Button className="secondary danger" disabled={busy} onClick={() => void run("remove_licence")}>Remove key</Button>
            <Button disabled={busy} onClick={() => setConfirm(false)}>Keep key</Button>
          </div>
        </> : <Button className="text-button" disabled={busy} onClick={() => setConfirm(true)}>Remove licence from this computer</Button>}</div>}
      </>}
      {error && <p className="field-error" role="alert">{error}</p>}
      <p className="licence-supporting">For a lost key or help entering one, contact <a href="mailto:info@theswitchify.app">info@theswitchify.app</a>.</p>
    </div>
  </SettingGroup>;
}
