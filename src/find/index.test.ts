import { describe, expect, it } from "vitest";
import { browserState } from "../api";
import { defaultPointScanConfig } from "../scanning/useScanning";
import { available, commonTasks, entry, index, search, sections, type FindContext } from "./index";

function context(overrides: Partial<FindContext> = {}): FindContext {
  const state = structuredClone(browserState);
  return {
    state, settings: state.settings, config: structuredClone(defaultPointScanConfig), scan: null,
    bindings: [{ id: "space", name: "Space switch", key: "Space", pressAction: "select", holdActions: [] }], holdIntervalMs: 1000,
    panels: [], builtins: [], profiles: null, account: { available: true, signedIn: false, email: null, pendingEmail: null, keychainUnavailable: false },
    ...overrides,
  };
}

describe("the settings index", () => {
  it("gives every entry a page, a section on that page and a value", () => {
    const ctx = context();
    for (const e of index) {
      expect(sections[e.page].map((s) => s.id), `${e.id} section`).toContain(e.section);
      expect(typeof e.value(ctx), `${e.id} value`).toMatch(/string|object/);
      for (const id of e.related ?? []) expect(index.some((other) => other.id === id), `${e.id} relates to ${id}`).toBe(true);
    }
    expect(new Set(index.map((e) => e.id)).size).toBe(index.length);
    for (const task of commonTasks) expect(entry(task.setting)).toBeDefined();
  });

  it("ranks a label match above a word match above a value match", () => {
    const ctx = context();
    const ids = search("speed", ctx).map((e) => e.id);
    expect(ids[0]).toBe("scan-rate");
    expect(ids).toContain("pointer-speed");
    expect(ids.indexOf("pointer-speed")).toBeLessThan(ids.indexOf("point-method"));
    expect(search("phone", ctx).map((e) => e.id)).toContain("connection");
    expect(search("red", ctx).map((e) => e.id)).toContain("colour");
    expect(search("", ctx)).toEqual([]);
    expect(search("   ", ctx)).toEqual([]);
  });

  it("needs every word to match something", () => {
    const ctx = context();
    expect(search("panel size", ctx)[0].id).toBe("panel-size");
    expect(search("panel zzzq", ctx)).toEqual([]);
    expect(search("ZZZQ", ctx)).toEqual([]);
  });

  it("leaves out settings this computer does not have", () => {
    const without = context({ state: { ...structuredClone(browserState), capabilities: { ...browserState.capabilities, cursorOverlay: false } } });
    expect(available(entry("overlay"), without)).toBe(false);
    expect(search("cursor overlay", without).map((e) => e.id)).not.toContain("overlay");
    expect(search("cursor overlay", context()).map((e) => e.id)).toContain("overlay");
    const noAccounts = context({ account: { available: false, signedIn: false, email: null, pendingEmail: null, keychainUnavailable: false } });
    expect(search("sign in", noAccounts).map((e) => e.id)).not.toContain("account");
    expect(search("sign in", context({ account: null })).map((e) => e.id)).toContain("account");
  });

  it("describes the current value in words", () => {
    const ctx = context();
    expect(entry("scan-rate").value(ctx)).toBe("Every 1 s");
    expect(entry("switch-list").value(ctx)).toBe("Space switch: Select");
    expect(entry("switch-list").value(context({ bindings: [] }))).toBe("None yet");
    expect(entry("switch-list").value(context({ bindings: null }))).toBeNull();
    expect(entry("movement").value(ctx)).toBe("Automatic");
    expect(entry("account").value(ctx)).toBe("Not signed in");
    expect(entry("devices").value(ctx)).toBe("0 paired");
  });
});
