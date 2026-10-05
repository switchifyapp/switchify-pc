import { useEffect, useRef, useState, type FormEvent } from "react";
import { api } from "../api";
import { Button, Input } from "../ui/controls";
import type { AccountView } from "../types";
import { SettingGroup } from "./controls";

const description = "Sign in with the same account as Switchify on Android. We email you a code; there is no password.";

export function AccountSection() {
  const [account, setAccount] = useState<AccountView | null>(null);
  const [email, setEmail] = useState("");
  const [code, setCode] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const codeRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    let active = true;
    void api.account().then((view) => { if (active) setAccount(view); });
    const unlisten = api.onAccount((view) => setAccount(view));
    return () => { active = false; void unlisten.then((stop) => stop()); };
  }, []);

  useEffect(() => {
    if (account?.pendingEmail) codeRef.current?.focus();
  }, [account?.pendingEmail]);

  const run = async (action: () => Promise<AccountView>) => {
    setBusy(true);
    setError(null);
    try {
      setAccount(await action());
      return true;
    } catch (failure) {
      setError(failure instanceof Error ? failure.message : String(failure));
      return false;
    } finally {
      setBusy(false);
    }
  };

  const requestCode = (event: FormEvent) => {
    event.preventDefault();
    void run(() => api.requestSignInCode(email)).then((ok) => { if (ok) setCode(""); });
  };
  const verifyCode = (event: FormEvent) => {
    event.preventDefault();
    void run(() => api.verifySignInCode(code)).then((ok) => { if (ok) setCode(""); });
  };

  if (!account) return <SettingGroup title="Account" description={description}><p className="setting-note">Loading…</p></SettingGroup>;

  const errorText = error && <span className="field-error" id="account-error" role="alert">{error}</span>;

  if (!account.available) {
    return <SettingGroup title="Account" description={description}><p className="setting-note">Accounts are unavailable in this build.</p></SettingGroup>;
  }

  if (account.signedIn) {
    return <SettingGroup title="Account" description={description}>
      <p className="setting-note" role="status">Signed in as <strong>{account.email}</strong>.</p>
      {confirmDelete
        ? <div role="group" aria-label="Confirm account deletion">
          <p className="setting-note">Deleting your account removes it and its saved settings from every device, including Switchify on Android. This cannot be undone.</p>
          <div className="privacy-choice">
            <Button className="secondary" disabled={busy} onClick={() => void run(api.deleteAccount).then(() => setConfirmDelete(false))}>Delete permanently</Button>
            <Button className="secondary" disabled={busy} onClick={() => setConfirmDelete(false)}>Keep account</Button>
          </div>
        </div>
        : <div className="privacy-choice">
          <Button className="secondary" disabled={busy} onClick={() => void run(api.signOut)}>Sign out</Button>
          <Button className="secondary" disabled={busy} onClick={() => setConfirmDelete(true)}>Delete account…</Button>
        </div>}
      {errorText}
    </SettingGroup>;
  }

  if (account.pendingEmail) {
    return <SettingGroup title="Account" description={description}>
      <form onSubmit={verifyCode}>
        <p className="setting-note" role="status">We sent a code to <strong>{account.pendingEmail}</strong>. It can take a minute to arrive.</p>
        <label className="field"><span>Code from the email</span>
          <Input ref={codeRef} value={code} inputMode="numeric" autoComplete="one-time-code" maxLength={12} disabled={busy}
            aria-invalid={Boolean(error)} aria-describedby={error ? "account-error" : undefined}
            onChange={(event) => setCode(event.target.value)} />
        </label>
        {errorText}
        <div className="privacy-choice">
          <Button className="secondary" type="submit" disabled={busy || !code.trim()}>Sign in</Button>
          <Button className="secondary" disabled={busy} onClick={() => { setError(null); void run(api.cancelSignIn); }}>Use a different email</Button>
        </div>
      </form>
    </SettingGroup>;
  }

  return <SettingGroup title="Account" description={description}>
    <form onSubmit={requestCode}>
      <label className="field"><span>Email</span>
        <Input type="email" value={email} autoComplete="email" maxLength={254} disabled={busy}
          aria-invalid={Boolean(error)} aria-describedby={error ? "account-error" : undefined}
          onChange={(event) => setEmail(event.target.value)} />
      </label>
      {errorText}
      <div className="privacy-choice">
        <Button className="secondary" type="submit" disabled={busy || !email.trim()}>Email me a code</Button>
      </div>
    </form>
  </SettingGroup>;
}
