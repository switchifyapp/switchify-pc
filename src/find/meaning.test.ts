import { describe, expect, it } from "vitest";
import { browserState } from "../api";
import { prepareMeaning, search, searchWithMeaning, type FindContext } from "./index";
import type { StaticModel } from "./semantic";
const state = structuredClone(browserState);
const ctx: FindContext = { state, settings: state.settings, config: null, scan: null, bindings: [], holdIntervalMs: null, panels: [], builtins: [], profiles: [], account: null };
const model: StaticModel = { vocab: new Map([["[UNK]", 0], ["snail", 1], ["rate", 2], ["overlay", 3]]), dims: 2, unkId: 0, vectors: new Float32Array([0, 0, 1, 0, 1, 0, 1, 0]) };
describe("meaning additions", () => {
  it("reaches a setting without overlapping keywords", () => {
    expect(search("snail", ctx)).toEqual([]);
    expect(searchWithMeaning("snail", ctx, prepareMeaning(model)).map((e) => e.id)).toContain("scan-rate");
  });
  it("preserves the complete keyword prefix and never duplicates it", () => {
    const keyword = search("rate", ctx);
    const hits = searchWithMeaning("rate", ctx, prepareMeaning(model));
    expect(hits.slice(0, keyword.length)).toEqual(keyword);
    expect(new Set(hits.map((e) => e.id)).size).toBe(hits.length);
    expect(searchWithMeaning("rate", ctx)).toEqual(keyword);
  });
  it("omits unavailable settings, unknown queries and punctuation", () => {
    const noOverlay = { ...ctx, state: { ...state, capabilities: { ...state.capabilities, cursorOverlay: false } } };
    expect(searchWithMeaning("snail", noOverlay, prepareMeaning(model)).map((e) => e.id)).not.toContain("overlay");
    for (const query of ["", "   ", "!!!", "zzzzzzzzzz"]) expect(searchWithMeaning(query, ctx, prepareMeaning(model))).toEqual([]);
  });
});
