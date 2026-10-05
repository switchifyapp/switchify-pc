import { useEffect, useRef, useState, type FormEvent } from "react";
import { api } from "../api";
import { Button, Input } from "../ui/controls";
import type { AccountView, SyncView } from "../types";
import { SettingGroup } from "./controls";

const description = "Sign in to bring your settings to your other computers. It is the same account as Switchify on Android. We email you a code; there is no password.";

type FocusTarget = "email" | "code" | "signOut" | "deleteAccount" | "keepAccount";
type Screen = "loading" | "unavailable" | "keychain" | "signedIn" | "pending" | "signedOut";

function screenOf(account: AccountView | null): Screen {
  if (!account) return "loading";
  if (!account.available) return "unavailable";
  if (account.keychainUnavailable) return "keychain";
  if (account.signedIn) return "signedIn";
  return account.pendingEmail ? "pending" : "signedOut";
}

export function AccountSection() {
  const [account, setAccount] = useState<AccountView | null>(null);
  const [email, setEmail] = useState("");
  const [code, setCode] = useState("");
  const [error, setError] = useState<{ text: string; id: number } | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  // Set only by the user's own actions, so opening the tab never moves focus.
  const [focusNext, setFocusNext] = useState<FocusTarget | null>(null);
  const emailRef = useRef<HTMLInputElement>(null);
  const codeRef = useRef<HTMLInputElement>(null);
  const signOutRef = useRef<HTMLButtonElement>(null);
  const deleteRef = useRef<HTMLButtonElement>(null);
  const keepRef = useRef<HTMLButtonElement>(null);
  const targets: Record<FocusTarget, () => HTMLElement | null> = {
    email: () => emailRef.current,
    code: () => codeRef.current,
    signOut: () => signOutRef.current,
    deleteAccount: () => deleteRef.current,
    keepAccount: () => keepRef.current,
  };

  useEffect(() => {
    let active = true;
    void api.account().then((view) => { if (active) setAccount(view); });
    const unlisten = api.onAccount((view) => setAccount(view));
    return () => { active = false; void unlisten.then((stop) => stop()); };
  }, []);

  // The backend can change screens on its own, e.g. a session rejected in the
  // middle of an action. Never carry a delete confirmation or a code into
  // another screen, and if the focused control vanished, focus the new
  // screen instead of leaving focus on the page. Focus that is still on a
  // control (such as the tabs) is left alone.
  const keychainRef = useRef<HTMLParagraphElement>(null);
  const screen = screenOf(account);
  const previousScreen = useRef<Screen>(screen);
  useEffect(() => {
    const previous = previousScreen.current;
    previousScreen.current = screen;
    if (previous === screen) return;
    if (screen !== "signedIn") setConfirmDelete(false);
    if (screen !== "pending") setCode("");
    const active = document.activeElement;
    if (previous === "loading" || (active && active !== document.body)) return;
    const first: Partial<Record<Screen, () => HTMLElement | null>> = {
      signedOut: () => emailRef.current,
      pending: () => codeRef.current,
      signedIn: () => signOutRef.current,
      keychain: () => keychainRef.current,
    };
    first[screen]?.()?.focus();
  }, [screen]);

  useEffect(() => {
    if (!focusNext) return;
    targets[focusNext]()?.focus();
    setFocusNext(null);
  });

  // Controls stay enabled while busy so focus is never dropped to the page;
  // repeated presses are ignored instead.
  const run = async (action: () => Promise<AccountView>, next: (view: AccountView) => FocusTarget | null) => {
    if (busy) return false;
    setBusy(true);
    setError(null);
    try {
      const view = await action();
      setAccount(view);
      setFocusNext(next(view));
      return true;
    } catch (failure) {
      setError({ text: failure instanceof Error ? failure.message : String(failure), id: Date.now() });
      return false;
    } finally {
      setBusy(false);
    }
  };
  const afterSignOut = (view: AccountView): FocusTarget | null => view.signedIn ? null : "email";

  const requestCode = (event: FormEvent) => {
    event.preventDefault();
    if (!email.trim()) return;
    void run(() => api.requestSignInCode(email), () => "code").then((ok) => { if (ok) setCode(""); });
  };
  const verifyCode = (event: FormEvent) => {
    event.preventDefault();
    if (!code.trim()) return;
    void run(() => api.verifySignInCode(code), () => "signOut").then((ok) => { if (ok) setCode(""); });
  };

  if (!account) return <SettingGroup title="Account" description={description}><p className="setting-note">Loading…</p></SettingGroup>;

  // Keyed by occurrence so the same message is announced again.
  const errorText = error && <span key={error.id} className="field-error" id="account-error" role="alert">{error.text}</span>;
  const errorProps = { "aria-invalid": Boolean(error), "aria-describedby": error ? "account-error" : undefined };

  if (!account.available) {
    return <SettingGroup title="Account" description={description}><p className="setting-note">Accounts are unavailable in this build.</p></SettingGroup>;
  }

  if (account.keychainUnavailable) {
    return <SettingGroup title="Account" description={description}><p ref={keychainRef} tabIndex={-1} className="setting-note" role="status">Your saved sign-in can't be read because this computer's keychain is locked or unavailable. Unlock it, then reopen this tab.</p></SettingGroup>;
  }

  if (account.signedIn) {
    return <SettingGroup title="Account" description={description}>
      <p className="setting-note" role="status">Signed in as <strong>{account.email}</strong>.</p>
      {!confirmDelete && <SyncPanel />}
      {confirmDelete
        ? <div role="group" aria-label="Confirm account deletion">
          <p className="setting-note">Deleting your account removes it and its saved settings from every device, including Switchify on Android. This cannot be undone.</p>
          <div className="privacy-choice">
            <Button className="secondary" aria-disabled={busy} onClick={() => void run(api.deleteAccount, afterSignOut).then((ok) => { if (ok) setConfirmDelete(false); })}>Delete permanently</Button>
            <Button ref={keepRef} className="secondary" aria-disabled={busy} onClick={() => { if (busy) return; setConfirmDelete(false); setFocusNext("deleteAccount"); }}>Keep account</Button>
          </div>
        </div>
        : <div className="privacy-choice">
          <Button ref={signOutRef} className="secondary" aria-disabled={busy} onClick={() => void run(api.signOut, afterSignOut)}>Sign out</Button>
          <Button ref={deleteRef} className="secondary" aria-disabled={busy} onClick={() => { if (busy) return; setError(null); setConfirmDelete(true); setFocusNext("keepAccount"); }}>Delete account…</Button>
        </div>}
      {errorText}
    </SettingGroup>;
  }

  if (account.pendingEmail) {
    return <SettingGroup title="Account" description={description}>
      <form onSubmit={verifyCode} aria-busy={busy}>
        <p className="setting-note" role="status">We sent a code to <strong>{account.pendingEmail}</strong>. It can take a minute to arrive.</p>
        <label className="field"><span>Code from the email</span>
          <Input ref={codeRef} value={code} inputMode="numeric" autoComplete="one-time-code" maxLength={12} readOnly={busy}
            {...errorProps} onChange={(event) => setCode(event.target.value)} />
        </label>
        {errorText}
        <div className="privacy-choice">
          <Button className="secondary" type="submit" aria-disabled={busy || !code.trim()}>Sign in</Button>
          <Button className="secondary" aria-disabled={busy} onClick={() => void run(api.cancelSignIn, () => "email")}>Use a different email</Button>
        </div>
      </form>
    </SettingGroup>;
  }

  return <SettingGroup title="Account" description={description}>
    <form onSubmit={requestCode} aria-busy={busy}>
      <label className="field"><span>Email</span>
        <Input ref={emailRef} type="email" value={email} autoComplete="email" maxLength={254} readOnly={busy}
          {...errorProps} onChange={(event) => setEmail(event.target.value)} />
      </label>
      {errorText}
      <div className="privacy-choice">
        <Button className="secondary" type="submit" aria-disabled={busy || !email.trim()}>Email me a code</Button>
      </div>
    </form>
  </SettingGroup>;
}

const syncText: Record<SyncView["status"], string> = {
  off: "Settings sync is starting.",
  syncing: "Syncing settings…",
  upToDate: "Settings are up to date.",
  needsChoice: "This computer and your account have different settings. Choose which to keep; the other is replaced.",
  updateRequired: "",
  error: "",
};

/** Sync status for the signed-in account, with the first-sync choice. */
function SyncPanel() {
  const [sync, setSync] = useState<SyncView | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const syncNowRef = useRef<HTMLButtonElement>(null);
  const firstChoiceRef = useRef<HTMLButtonElement>(null);
  const restoreFocus = useRef(false);
  const previousStatus = useRef(sync?.status);

  useEffect(() => {
    let active = true;
    void api.settingsSync().then((view) => { if (active) setSync(view); });
    const unlisten = api.onSettingsSync((view) => setSync(view));
    return () => { active = false; void unlisten.then((stop) => stop()); };
  }, []);

  useEffect(() => {
    if (restoreFocus.current && sync?.status !== "needsChoice") {
      restoreFocus.current = false;
      syncNowRef.current?.focus();
    }
  });

  // Entering or leaving the choice swaps the buttons; if the focused one
  // went with them, focus the buttons that replaced it.
  useEffect(() => {
    const previous = previousStatus.current;
    previousStatus.current = sync?.status;
    if (!previous || previous === sync?.status) return;
    const active = document.activeElement;
    if (active && active !== document.body) return;
    (sync?.status === "needsChoice" ? firstChoiceRef : syncNowRef).current?.focus();
  }, [sync?.status]);

  const run = async (action: () => Promise<SyncView>, fromChoice = false) => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      restoreFocus.current = fromChoice;
      setSync(await action());
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : String(failure));
    } finally {
      setBusy(false);
    }
  };

  if (!sync) return null;
  const last = sync.lastSyncedAt ? new Date(sync.lastSyncedAt).toLocaleString([], { dateStyle: "medium", timeStyle: "short" }) : null;
  const text = sync.message ?? syncText[sync.status];
  return <div className="sync-panel" aria-busy={busy || sync.status === "syncing"}>
    <p className="setting-note" role="status">
      {text}{sync.status === "upToDate" && last && <> Last synced {last}.</>}
    </p>
    {sync.status === "needsChoice"
      ? <div className="privacy-choice" role="group" aria-label="Choose which settings to keep">
        <Button ref={firstChoiceRef} className="secondary" aria-disabled={busy} onClick={() => void run(() => api.resolveSettingsSync("cloud"), true)}>Use my account&apos;s settings</Button>
        <Button className="secondary" aria-disabled={busy} onClick={() => void run(() => api.resolveSettingsSync("local"), true)}>Keep this computer&apos;s settings</Button>
      </div>
      : <div className="privacy-choice">
        <Button ref={syncNowRef} className="secondary" aria-disabled={busy || sync.status === "syncing"} onClick={() => { if (sync.status !== "syncing") void run(api.syncSettingsNow); }}>Sync now</Button>
      </div>}
    {error && <span className="field-error" role="alert">{error}</span>}
  </div>;
}
