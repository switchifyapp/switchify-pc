import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "../api";
import type { AccountView, SyncView } from "../types";
import { AccountSection } from "./AccountSection";

const signedOut: AccountView = { available: true, signedIn: false, email: null, pendingEmail: null, keychainUnavailable: false };
const pending: AccountView = { ...signedOut, pendingEmail: "me@example.com" };
const signedIn: AccountView = { ...signedOut, signedIn: true, email: "me@example.com" };

function start(view: AccountView, sync: SyncView = { status: "upToDate", lastSyncedAt: null, message: null }) {
  vi.spyOn(api, "account").mockResolvedValue(view);
  vi.spyOn(api, "onAccount").mockResolvedValue(() => undefined);
  vi.spyOn(api, "settingsSync").mockResolvedValue(sync);
  vi.spyOn(api, "onSettingsSync").mockResolvedValue(() => undefined);
  render(<AccountSection />);
}

describe("AccountSection", () => {
  afterEach(() => vi.restoreAllMocks());

  describe("settings sync", () => {
    it("shows when settings last synced and can sync now", async () => {
      start(signedIn, { status: "upToDate", lastSyncedAt: Date.UTC(2026, 9, 5, 12, 0), message: null });
      expect(await screen.findByText(/Settings are up to date. Last synced/)).toBeInTheDocument();
      const now = vi.spyOn(api, "syncSettingsNow").mockResolvedValue({ status: "upToDate", lastSyncedAt: Date.now(), message: null });
      fireEvent.click(screen.getByRole("button", { name: "Sync now" }));
      expect(now).toHaveBeenCalled();
    });

    it("asks which settings to keep and resolves the choice", async () => {
      start(signedIn, { status: "needsChoice", lastSyncedAt: null, message: null });
      expect(await screen.findByText(/different settings. Choose which to keep/)).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "Sync now" })).not.toBeInTheDocument();
      const resolve = vi.spyOn(api, "resolveSettingsSync").mockResolvedValue({ status: "upToDate", lastSyncedAt: Date.now(), message: null });
      fireEvent.click(screen.getByRole("button", { name: "Keep this computer's settings" }));
      expect(resolve).toHaveBeenCalledWith("local");
      await waitFor(() => expect(screen.getByRole("button", { name: "Sync now" })).toHaveFocus());
    });

    it("moves focus to the choice when Sync now finds different settings", async () => {
      start(signedIn);
      vi.spyOn(api, "syncSettingsNow").mockResolvedValue({ status: "needsChoice", lastSyncedAt: null, message: null });
      const now = await screen.findByRole("button", { name: "Sync now" });
      now.focus();
      fireEvent.click(now);
      await waitFor(() => expect(screen.getByRole("button", { name: "Use my account's settings" })).toHaveFocus());
    });

    it("can take the account settings instead", async () => {
      start(signedIn, { status: "needsChoice", lastSyncedAt: null, message: null });
      const resolve = vi.spyOn(api, "resolveSettingsSync").mockResolvedValue({ status: "upToDate", lastSyncedAt: Date.now(), message: null });
      fireEvent.click(await screen.findByRole("button", { name: "Use my account's settings" }));
      expect(resolve).toHaveBeenCalledWith("cloud");
    });

    it("shows the backend message for errors and newer app versions", async () => {
      start(signedIn, { status: "updateRequired", lastSyncedAt: null, message: "Your synced settings were saved by a newer Switchify PC." });
      expect(await screen.findByText(/saved by a newer Switchify PC/)).toBeInTheDocument();
    });

    it("follows sync changes from the backend", async () => {
      let receive: (view: SyncView) => void = () => undefined;
      vi.spyOn(api, "account").mockResolvedValue(signedIn);
      vi.spyOn(api, "onAccount").mockResolvedValue(() => undefined);
      vi.spyOn(api, "settingsSync").mockResolvedValue({ status: "upToDate", lastSyncedAt: null, message: null });
      vi.spyOn(api, "onSettingsSync").mockImplementation(async (handler) => { receive = handler; return () => undefined; });
      render(<AccountSection />);
      await screen.findByText("Settings are up to date.");
      receive({ status: "error", lastSyncedAt: null, message: "Could not reach Switchify to sync settings." });
      expect(await screen.findByText("Could not reach Switchify to sync settings.")).toBeInTheDocument();
    });

    it("is hidden while confirming account deletion", async () => {
      start(signedIn);
      await screen.findByRole("button", { name: "Sync now" });
      fireEvent.click(screen.getByRole("button", { name: "Delete account…" }));
      expect(screen.queryByRole("button", { name: "Sync now" })).not.toBeInTheDocument();
    });
  });

  it("says when accounts are unavailable in this build", async () => {
    start({ ...signedOut, available: false });
    expect(await screen.findByText("Accounts are unavailable in this build.")).toBeInTheDocument();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });

  it("requests a code, then signs in with it", async () => {
    start(signedOut);
    const request = vi.spyOn(api, "requestSignInCode").mockResolvedValue(pending);
    const verify = vi.spyOn(api, "verifySignInCode").mockResolvedValue(signedIn);
    const send = await screen.findByRole("button", { name: "Email me a code" });
    expect(send).toHaveAttribute("aria-disabled", "true");
    fireEvent.click(send);
    expect(request).not.toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText("Email"), { target: { value: "me@example.com" } });
    fireEvent.click(send);
    expect(request).toHaveBeenCalledWith("me@example.com");
    const codeField = await screen.findByLabelText("Code from the email");
    await waitFor(() => expect(codeField).toHaveFocus());
    expect(screen.getByRole("status")).toHaveTextContent("We sent a code to me@example.com");
    fireEvent.change(codeField, { target: { value: "123456" } });
    fireEvent.click(screen.getByRole("button", { name: "Sign in" }));
    expect(verify).toHaveBeenCalledWith("123456");
    await waitFor(() => expect(screen.getByRole("button", { name: "Sign out" })).toHaveFocus());
    expect(screen.getByText(/Signed in as/)).toHaveTextContent("Signed in as me@example.com.");
  });

  it("shows a wrong-code error and keeps the code form", async () => {
    start(pending);
    vi.spyOn(api, "verifySignInCode").mockRejectedValue("That code is wrong or has expired.");
    fireEvent.change(await screen.findByLabelText("Code from the email"), { target: { value: "000000" } });
    fireEvent.click(screen.getByRole("button", { name: "Sign in" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("That code is wrong or has expired.");
    expect(screen.getByLabelText("Code from the email")).toHaveAttribute("aria-invalid", "true");
  });

  it("can go back to choose a different email", async () => {
    start(pending);
    const cancel = vi.spyOn(api, "cancelSignIn").mockResolvedValue(signedOut);
    fireEvent.click(await screen.findByRole("button", { name: "Use a different email" }));
    expect(cancel).toHaveBeenCalled();
    await waitFor(() => expect(screen.getByLabelText("Email")).toHaveFocus());
  });

  it("signs out", async () => {
    start(signedIn);
    const signOut = vi.spyOn(api, "signOut").mockResolvedValue(signedOut);
    fireEvent.click(await screen.findByRole("button", { name: "Sign out" }));
    expect(signOut).toHaveBeenCalled();
    await waitFor(() => expect(screen.getByLabelText("Email")).toHaveFocus());
  });

  it("asks for confirmation before deleting the account", async () => {
    start(signedIn);
    const remove = vi.spyOn(api, "deleteAccount").mockResolvedValue(signedOut);
    fireEvent.click(await screen.findByRole("button", { name: "Delete account…" }));
    expect(remove).not.toHaveBeenCalled();
    expect(screen.getByText(/removes it and its saved settings from every device/)).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole("button", { name: "Keep account" })).toHaveFocus());
    fireEvent.click(screen.getByRole("button", { name: "Keep account" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Delete account…" })).toHaveFocus());
    fireEvent.click(screen.getByRole("button", { name: "Delete account…" }));
    fireEvent.click(screen.getByRole("button", { name: "Delete permanently" }));
    expect(remove).toHaveBeenCalled();
    await waitFor(() => expect(screen.getByLabelText("Email")).toHaveFocus());
  });

  it("keeps focus on the field when a request fails, and repeats the same error", async () => {
    start(signedOut);
    vi.spyOn(api, "requestSignInCode").mockRejectedValue("Could not send a sign-in code. Try again.");
    const field = await screen.findByLabelText("Email");
    fireEvent.change(field, { target: { value: "me@example.com" } });
    field.focus();
    fireEvent.submit(field.closest("form")!);
    const first = await screen.findByRole("alert");
    expect(field).toHaveFocus();
    expect(field).not.toBeDisabled();
    fireEvent.submit(field.closest("form")!);
    await waitFor(() => expect(screen.getByRole("alert")).not.toBe(first));
    expect(screen.getByRole("alert")).toHaveTextContent("Could not send a sign-in code.");
  });

  it("ignores repeat presses and keeps focus while a request is running", async () => {
    start(signedOut);
    let finish: (view: AccountView) => void = () => undefined;
    const request = vi.spyOn(api, "requestSignInCode").mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
    const field = await screen.findByLabelText("Email");
    fireEvent.change(field, { target: { value: "me@example.com" } });
    field.focus();
    const send = screen.getByRole("button", { name: "Email me a code" });
    fireEvent.click(send);
    await waitFor(() => expect(send).toHaveAttribute("aria-disabled", "true"));
    expect(field).toHaveFocus();
    expect(field).toHaveAttribute("readonly");
    fireEvent.click(send);
    fireEvent.submit(field.closest("form")!);
    expect(request).toHaveBeenCalledTimes(1);
    finish(pending);
    await waitFor(() => expect(screen.getByLabelText("Code from the email")).toHaveFocus());
  });

  it("never carries a delete confirmation into the next sign-in after a rejected session", async () => {
    let receive: (view: AccountView) => void = () => undefined;
    vi.spyOn(api, "account").mockResolvedValue(signedIn);
    vi.spyOn(api, "onAccount").mockImplementation(async (handler) => { receive = handler; return () => undefined; });
    // The backend signs out (and emits) before the command fails.
    vi.spyOn(api, "deleteAccount").mockImplementation(async () => {
      receive(signedOut);
      throw "You were signed out. Sign in again.";
    });
    render(<AccountSection />);
    fireEvent.click(await screen.findByRole("button", { name: "Delete account…" }));
    fireEvent.click(screen.getByRole("button", { name: "Delete permanently" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("You were signed out.");
    await waitFor(() => expect(screen.getByLabelText("Email")).toHaveFocus());
    receive(signedIn);
    expect(await screen.findByRole("button", { name: "Sign out" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Delete permanently" })).not.toBeInTheDocument();
  });

  it("focuses the keychain notice when a request finds the keychain locked", async () => {
    let receive: (view: AccountView) => void = () => undefined;
    vi.spyOn(api, "account").mockResolvedValue(signedOut);
    vi.spyOn(api, "onAccount").mockImplementation(async (handler) => { receive = handler; return () => undefined; });
    vi.spyOn(api, "requestSignInCode").mockImplementation(async () => {
      receive({ ...signedOut, keychainUnavailable: true });
      throw "This computer's keychain is unavailable. Unlock it, then try again.";
    });
    render(<AccountSection />);
    const field = await screen.findByLabelText("Email");
    fireEvent.change(field, { target: { value: "me@example.com" } });
    fireEvent.click(screen.getByRole("button", { name: "Email me a code" }));
    await waitFor(() => expect(screen.getByText(/keychain is locked or unavailable/)).toHaveFocus());
  });

  it("leaves focus alone when the backend changes screens while focus is elsewhere", async () => {
    let receive: (view: AccountView) => void = () => undefined;
    vi.spyOn(api, "account").mockResolvedValue(signedOut);
    vi.spyOn(api, "onAccount").mockImplementation(async (handler) => { receive = handler; return () => undefined; });
    vi.spyOn(api, "settingsSync").mockResolvedValue({ status: "upToDate", lastSyncedAt: null, message: null });
    vi.spyOn(api, "onSettingsSync").mockResolvedValue(() => undefined);
    render(<><button>Elsewhere</button><AccountSection /></>);
    await screen.findByLabelText("Email");
    const elsewhere = screen.getByRole("button", { name: "Elsewhere" });
    elsewhere.focus();
    receive(signedIn);
    await screen.findByRole("button", { name: "Sign out" });
    expect(elsewhere).toHaveFocus();
  });

  it("does not move focus when the tab opens with a pending code", async () => {
    start(pending);
    const field = await screen.findByLabelText("Code from the email");
    expect(field).not.toHaveFocus();
  });

  it("explains a locked keychain instead of offering sign-in", async () => {
    start({ ...signedOut, keychainUnavailable: true });
    expect(await screen.findByText(/keychain is locked or unavailable/)).toBeInTheDocument();
    expect(screen.queryByLabelText("Email")).not.toBeInTheDocument();
  });

  it("follows account changes from the backend", async () => {
    let receive: (view: AccountView) => void = () => undefined;
    vi.spyOn(api, "account").mockResolvedValue(signedOut);
    vi.spyOn(api, "onAccount").mockImplementation(async (handler) => { receive = handler; return () => undefined; });
    vi.spyOn(api, "settingsSync").mockResolvedValue({ status: "upToDate", lastSyncedAt: null, message: null });
    vi.spyOn(api, "onSettingsSync").mockResolvedValue(() => undefined);
    render(<AccountSection />);
    await screen.findByLabelText("Email");
    receive(signedIn);
    expect(await screen.findByRole("button", { name: "Sign out" })).toBeInTheDocument();
  });

  it("is not a Settings tab", async () => {
    const { App } = await import("../App");
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    expect(screen.queryByRole("tab", { name: "Account" })).not.toBeInTheDocument();
  });
});
