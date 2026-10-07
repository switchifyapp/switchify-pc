import { useEffect, useRef, useState, type FormEvent, type ReactNode } from "react";
import { CircleUserRound, ListChecks, LockKeyhole, MailCheck, RefreshCw, Trash2 } from "lucide-react";
import { api } from "../api";
import { Button, Input, StatusIcon } from "../ui/controls";
import type { AccountView, SyncView } from "../types";

const sameAccount = "It's the same account as Switchify on Android.";
const whatSyncs = "Pointer, repeat, dwell and cursor settings; your switches and their keys; scanning and keyboard layout; your switch profiles; and remote switches. Pairings, diagnostics sharing, setup progress and starting with your computer stay on each computer.";

type FocusTarget = "email" | "code" | "signOut" | "deleteAccount" | "keepAccount";
type Screen = "loading" | "unavailable" | "keychain" | "signedIn" | "pending" | "signedOut";

function screenOf(account: AccountView | null): Screen {
  if (!account) return "loading";
  if (!account.available) return "unavailable";
  if (account.keychainUnavailable) return "keychain";
  if (account.signedIn) return "signedIn";
  return account.pendingEmail ? "pending" : "signedOut";
}

// The status band and grouped rows used on Home, so the page reads like the
// rest of the app.
function Band({ tone, ok, icon, title, children, action }: { tone: "ready" | "attention" | "neutral"; ok?: boolean; icon: ReactNode; title: ReactNode; children: ReactNode; action?: ReactNode }) {
  return <section className="connection-band status-hero account-band" data-tone={tone} aria-labelledby="account-title">
    <StatusIcon ok={ok}>{icon}</StatusIcon>
    <div><h2 id="account-title">{title}</h2>{children}</div>
    {action}
  </section>;
}

function WhatSyncs() {
  return <div className="status-list">
    <article><StatusIcon><ListChecks size={19} /></StatusIcon><div><h3>What syncs</h3><p>{whatSyncs}</p></div></article>
  </div>;
}

export function AccountSection() {
  const [account, setAccount] = useState<AccountView | null>(null);
  const [email, setEmail] = useState("");
  const [code, setCode] = useState("");
  // Where an error is shown: beside the control that failed, so it is seen
  // by whoever pressed it, not off screen at the bottom of the page.
  type ErrorPlace = "form" | "signOut" | "delete";
  const [error, setError] = useState<{ text: string; id: number; place: ErrorPlace } | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  // Set only by the user's own actions, so opening the page never moves focus.
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
  // control (such as the sidebar) is left alone.
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
  const run = async (action: () => Promise<AccountView>, next: (view: AccountView) => FocusTarget | null, place: ErrorPlace = "form") => {
    if (busy) return false;
    setBusy(true);
    setError(null);
    try {
      const view = await action();
      setAccount(view);
      setFocusNext(next(view));
      return true;
    } catch (failure) {
      setError({ text: failure instanceof Error ? failure.message : String(failure), id: Date.now(), place });
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

  if (!account) return <p className="setting-note">Loading…</p>;

  // Keyed by occurrence so the same message is announced again.
  // Off the signed-in screen (e.g. a session rejected mid-delete) the
  // failed control is gone, so the error joins the form that replaced it.
  const errorAt = (place: ErrorPlace) => error && (account.signedIn ? error.place : "form") === place && <span key={error.id} className="field-error" id="account-error" role="alert">{error.text}</span>;
  const errorText = errorAt("form");
  const errorProps = { "aria-invalid": Boolean(error), "aria-describedby": error ? "account-error" : undefined };

  if (!account.available) {
    return <Band tone="neutral" icon={<CircleUserRound size={26} />} title="Accounts are unavailable in this build"><p>This build was made without account support.</p></Band>;
  }

  if (account.keychainUnavailable) {
    return <Band tone="attention" ok={false} icon={<LockKeyhole size={26} />} title="Can't read your saved sign-in">
      <p ref={keychainRef} tabIndex={-1} role="status">Your saved sign-in can't be read because this computer's keychain is locked or unavailable. Unlock it, then open this page again.</p>
    </Band>;
  }

  if (account.signedIn) {
    return <div className="account-page">
      <Band tone="ready" ok icon={<CircleUserRound size={26} />} title={<span className="account-email">{account.email}</span>}
        action={<Button ref={signOutRef} className="secondary" aria-disabled={busy} onClick={() => void run(api.signOut, afterSignOut, "signOut")}>Sign out</Button>}>
        <p role="status">Signed in. {sameAccount}</p>
        {errorAt("signOut")}
      </Band>
      <div className="status-list"><SyncRow /></div>
      <WhatSyncs />
      <div className="status-list">
        {confirmDelete
          ? <article className="stacked-row" role="group" aria-label="Confirm account deletion">
            <StatusIcon ok={false}><Trash2 size={19} /></StatusIcon>
            <div><h3>Delete your account?</h3><p>This removes your account and its saved settings from every device, including Switchify on Android. It cannot be undone.</p>{errorAt("delete")}</div>
            <div className="privacy-choice row-actions">
              <Button className="primary danger" aria-disabled={busy} onClick={() => void run(api.deleteAccount, afterSignOut, "delete").then((ok) => { if (ok) setConfirmDelete(false); })}>Delete permanently</Button>
              <Button ref={keepRef} className="secondary" aria-disabled={busy} onClick={() => { if (busy) return; setError(null); setConfirmDelete(false); setFocusNext("deleteAccount"); }}>Keep account</Button>
            </div>
          </article>
          : <article>
            <StatusIcon><Trash2 size={19} /></StatusIcon>
            <div><h3>Delete account</h3><p>Remove your account and its saved settings from every device.</p></div>
            <Button ref={deleteRef} className="secondary danger" aria-disabled={busy} onClick={() => { if (busy) return; setError(null); setConfirmDelete(true); setFocusNext("keepAccount"); }}>Delete account…</Button>
          </article>}
      </div>
    </div>;
  }

  if (account.pendingEmail) {
    return <div className="account-page">
      <Band tone="neutral" icon={<MailCheck size={26} />} title="Check your email">
        <p role="status">We sent a code to <strong>{account.pendingEmail}</strong>. It can take a minute to arrive.</p>
      </Band>
      <div className="status-list">
        <form className="account-form" onSubmit={verifyCode} aria-busy={busy}>
          <label className="field"><span>Code from the email</span>
            <Input ref={codeRef} value={code} inputMode="numeric" autoComplete="one-time-code" maxLength={12} readOnly={busy}
              {...errorProps} onChange={(event) => setCode(event.target.value)} />
          </label>
          {errorText}
          <div className="privacy-choice">
            <Button className="primary" type="submit" aria-disabled={busy || !code.trim()}>Sign in</Button>
            <Button className="secondary" aria-disabled={busy} onClick={() => void run(api.cancelSignIn, () => "email")}>Use a different email</Button>
          </div>
        </form>
      </div>
    </div>;
  }

  return <div className="account-page">
    <Band tone="neutral" icon={<CircleUserRound size={26} />} title="Sign in to sync your settings">
      <p>{sameAccount} We email you a code to sign in; there is no password.</p>
    </Band>
    <div className="status-list">
      <form className="account-form" onSubmit={requestCode} aria-busy={busy}>
        <label className="field"><span>Email</span>
          <Input ref={emailRef} type="email" value={email} autoComplete="email" maxLength={254} readOnly={busy}
            {...errorProps} onChange={(event) => setEmail(event.target.value)} />
        </label>
        {errorText}
        <div className="privacy-choice">
          <Button className="primary" type="submit" aria-disabled={busy || !email.trim()}>Email me a code</Button>
        </div>
      </form>
    </div>
    <WhatSyncs />
  </div>;
}

const syncText: Record<SyncView["status"], string> = {
  off: "Settings sync is starting.",
  syncing: "Syncing settings…",
  upToDate: "Settings are up to date.",
  needsChoice: "This computer and your account have different settings. Choose which to keep; the other is replaced.",
  updateRequired: "",
  error: "",
};

function syncOk(status: SyncView["status"]): boolean | undefined {
  if (status === "upToDate") return true;
  if (status === "error" || status === "updateRequired" || status === "needsChoice") return false;
  return undefined;
}

/** The settings sync row: status, last sync, Sync now or the first-sync choice. */
function SyncRow() {
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

  const status = sync?.status ?? "off";
  const last = sync?.lastSyncedAt ? new Date(sync.lastSyncedAt).toLocaleString([], { dateStyle: "medium", timeStyle: "short" }) : null;
  const text = sync ? sync.message ?? syncText[status] : "Checking settings sync…";
  return <article className={status === "needsChoice" ? "sync-row stacked-row" : "sync-row"} aria-busy={busy || status === "syncing"}>
    <StatusIcon ok={sync ? syncOk(status) : undefined}><RefreshCw size={19} /></StatusIcon>
    <div>
      <h3>Settings sync</h3>
      {sync
        ? <p role="status">{text}{status === "upToDate" && last && <> Last synced {last}.</>}</p>
        : <p>{text}</p>}
      {error && <span className="field-error" role="alert">{error}</span>}
    </div>
    {sync && (status === "needsChoice"
      ? <div className="privacy-choice row-actions" role="group" aria-label="Choose which settings to keep">
        <Button ref={firstChoiceRef} className="secondary" aria-disabled={busy} onClick={() => void run(() => api.resolveSettingsSync("cloud"), true)}>Use my account&apos;s settings</Button>
        <Button className="secondary" aria-disabled={busy} onClick={() => void run(() => api.resolveSettingsSync("local"), true)}>Keep this computer&apos;s settings</Button>
      </div>
      : <Button ref={syncNowRef} className="secondary" aria-disabled={busy || status === "syncing"} onClick={() => { if (status !== "syncing") void run(api.syncSettingsNow); }}>Sync now</Button>)}
  </article>;
}
