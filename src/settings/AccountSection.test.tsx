import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "../api";
import type { AccountView } from "../types";
import { AccountSection } from "./AccountSection";

const signedOut: AccountView = { available: true, signedIn: false, email: null, pendingEmail: null };
const pending: AccountView = { ...signedOut, pendingEmail: "me@example.com" };
const signedIn: AccountView = { available: true, signedIn: true, email: "me@example.com", pendingEmail: null };

function start(view: AccountView) {
  vi.spyOn(api, "account").mockResolvedValue(view);
  vi.spyOn(api, "onAccount").mockResolvedValue(() => undefined);
  render(<AccountSection />);
}

describe("AccountSection", () => {
  afterEach(() => vi.restoreAllMocks());

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
    expect(send).toBeDisabled();
    fireEvent.change(screen.getByLabelText("Email"), { target: { value: "me@example.com" } });
    fireEvent.click(send);
    expect(request).toHaveBeenCalledWith("me@example.com");
    const codeField = await screen.findByLabelText("Code from the email");
    await waitFor(() => expect(codeField).toHaveFocus());
    expect(screen.getByRole("status")).toHaveTextContent("We sent a code to me@example.com");
    fireEvent.change(codeField, { target: { value: "123456" } });
    fireEvent.click(screen.getByRole("button", { name: "Sign in" }));
    expect(verify).toHaveBeenCalledWith("123456");
    expect(await screen.findByRole("button", { name: "Sign out" })).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("Signed in as me@example.com.");
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
    expect(await screen.findByLabelText("Email")).toBeInTheDocument();
  });

  it("signs out", async () => {
    start(signedIn);
    const signOut = vi.spyOn(api, "signOut").mockResolvedValue(signedOut);
    fireEvent.click(await screen.findByRole("button", { name: "Sign out" }));
    expect(signOut).toHaveBeenCalled();
    expect(await screen.findByLabelText("Email")).toBeInTheDocument();
  });

  it("asks for confirmation before deleting the account", async () => {
    start(signedIn);
    const remove = vi.spyOn(api, "deleteAccount").mockResolvedValue(signedOut);
    fireEvent.click(await screen.findByRole("button", { name: "Delete account…" }));
    expect(remove).not.toHaveBeenCalled();
    expect(screen.getByText(/removes it and its saved settings from every device/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Keep account" }));
    expect(screen.getByRole("button", { name: "Sign out" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Delete account…" }));
    fireEvent.click(screen.getByRole("button", { name: "Delete permanently" }));
    expect(remove).toHaveBeenCalled();
    expect(await screen.findByLabelText("Email")).toBeInTheDocument();
  });

  it("follows account changes from the backend", async () => {
    let receive: (view: AccountView) => void = () => undefined;
    vi.spyOn(api, "account").mockResolvedValue(signedOut);
    vi.spyOn(api, "onAccount").mockImplementation(async (handler) => { receive = handler; return () => undefined; });
    render(<AccountSection />);
    await screen.findByLabelText("Email");
    receive(signedIn);
    expect(await screen.findByRole("button", { name: "Sign out" })).toBeInTheDocument();
  });

  it("is reachable from the Settings tabs", async () => {
    const { App } = await import("../App");
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    fireEvent.click(screen.getByRole("tab", { name: "Account" }));
    expect(await screen.findByRole("heading", { name: "Account" })).toBeInTheDocument();
  });
});
