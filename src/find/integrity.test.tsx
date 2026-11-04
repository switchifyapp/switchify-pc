import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "../App";
import { api, browserState } from "../api";
import { available, index, pageLabels, sections, type FindContext, type Page } from "./index";

// Every entry in the index must land somewhere real: a page the nav reaches,
// a section that renders there, and an element marked with its id. Jumps,
// the strip and Also see all rely on it, so the index can never drift from
// the UI without this test saying so.
describe("the index matches the rendered app", () => {
  const original = structuredClone(browserState);
  beforeEach(() => {
    Object.assign(browserState, structuredClone(original));
    browserState.setup = { shown: true, completed: true, autoOpenEligible: false };
    browserState.capabilities = { ...browserState.capabilities, cursorOverlay: true };
    vi.spyOn(api, "accountStatus").mockResolvedValue({ available: true, signedIn: false, email: null, pendingEmail: null, keychainUnavailable: false });
    vi.spyOn(api, "account").mockResolvedValue({ available: true, signedIn: false, email: null, pendingEmail: null, keychainUnavailable: false });
    vi.spyOn(api, "settingsSync").mockResolvedValue({ status: "upToDate", lastSyncedAt: null, message: null });
  });
  afterEach(() => { vi.restoreAllMocks(); });

  const open = async (page: Page) => {
    fireEvent.click(screen.getByRole("button", { name: pageLabels[page] }));
    await waitFor(() => expect(screen.getByRole("button", { name: pageLabels[page] })).toHaveAttribute("aria-current", "page"));
  };

  it("renders a section and a marked element for every entry on every page", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    const builtins = await api.listBuiltinPanels();
    const ctx: FindContext = {
      state: structuredClone(browserState), settings: browserState.settings, config: null, scan: null, bindings: [], holdIntervalMs: 1000,
      panels: [], builtins, profiles: [], account: { available: true, signedIn: false, email: null, pendingEmail: null, keychainUnavailable: false },
    };
    const pages = Object.keys(pageLabels) as Page[];
    for (const page of pages) {
      await open(page);
      if (page === "panels") await screen.findByRole("heading", { name: "Built-in menus and panels" });
      if (page === "mobile") await screen.findByRole("button", { name: "New profile" });
      for (const section of sections[page]) {
        if (section.when && !section.when(ctx)) continue;
        await waitFor(() => expect(document.querySelector(`[data-section="${section.id}"]`), `${page} section ${section.id}`).not.toBeNull());
      }
      for (const e of index) {
        if (e.page !== page || !available(e, ctx)) continue;
        if (e.area) {
          const names = { point: "point scanning", mouse: "mouse scanning", menu: "menus", keyboard: "keyboard" };
          fireEvent.click(screen.getByRole("button", { name: `Customise ${names[e.area]}`, hidden: true }));
          await screen.findByRole("button", { name: "Back to scanning settings" });
        }
        await waitFor(() => expect(document.querySelector(`[data-setting="${e.id}"]`), `${page} setting ${e.id}`).not.toBeNull());
        if (e.area) {
          fireEvent.click(screen.getByRole("button", { name: "Back to scanning settings" }));
          await screen.findByRole("button", { name: "Customise point scanning" });
        }
      }
    }
  });

  it("gives every page in the nav an entry in the index", () => {
    for (const page of Object.keys(pageLabels) as Page[]) {
      if (page === "home") continue;
      expect(index.some((e) => e.page === page), `${page} has entries`).toBe(true);
    }
  });
});
