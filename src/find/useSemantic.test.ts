import { renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { packModel } from "./semantic";
import { hasModelAsset } from "./useSemantic";
beforeEach(() => vi.resetModules());
afterEach(() => vi.unstubAllGlobals());
// A checkout that has not run scripts/fetch-find-model.mjs has no asset to
// fetch; the hook then stays keyword-only, which the last test covers.
it.skipIf(!hasModelAsset)("waits for a nonempty query, then shares one load and embeds the index", async () => {
  const bytes = packModel(["[UNK]", "rate"], 2, new Float32Array([0, 0, 1, 0]));
  const fetcher = vi.fn().mockResolvedValue({ ok: true, arrayBuffer: async () => bytes.buffer });
  vi.stubGlobal("fetch", fetcher);
  const { useSemantic } = await import("./useSemantic");
  const hook = renderHook(({ query }) => useSemantic(query), { initialProps: { query: " " } });
  expect(fetcher).not.toHaveBeenCalled();
  hook.rerender({ query: "snail" });
  await waitFor(() => expect(hook.result.current.semantic?.vectors.get("scan-rate")).toBeTruthy());
  expect(hook.result.current.loading).toBe(false);
  hook.rerender({ query: "rate" });
  expect(fetcher).toHaveBeenCalledTimes(1);
});
it("quietly falls back when fetch is unavailable", async () => {
  vi.stubGlobal("fetch", undefined);
  const { useSemantic } = await import("./useSemantic");
  const hook = renderHook(() => useSemantic("snail"));
  await waitFor(() => expect(hook.result.current.loading).toBe(false));
  expect(hook.result.current.semantic).toBeUndefined();
});
it.skipIf(!hasModelAsset)("keeps keyword search when the model cannot be decoded", async () => {
  const fetcher = vi.fn().mockResolvedValue({ ok: true, arrayBuffer: async () => new ArrayBuffer(0) });
  vi.stubGlobal("fetch", fetcher);
  const { useSemantic } = await import("./useSemantic");
  const hook = renderHook(() => useSemantic("rate"));
  await waitFor(() => expect(fetcher).toHaveBeenCalledTimes(1));
  await waitFor(() => expect(hook.result.current.loading).toBe(false));
  expect(hook.result.current.semantic).toBeUndefined();
});

it.skipIf(!hasModelAsset)("reports loading until a delayed model request settles, including failure", async () => {
  let finish!: (response: { ok: boolean }) => void;
  vi.stubGlobal("fetch", vi.fn(() => new Promise((resolve) => { finish = resolve; })));
  const { useSemantic } = await import("./useSemantic");
  const hook = renderHook(({ query }) => useSemantic(query), { initialProps: { query: "" } });
  expect(hook.result.current.loading).toBe(false);
  hook.rerender({ query: "slow down" });
  expect(hook.result.current.loading).toBe(true);
  finish({ ok: false });
  await waitFor(() => expect(hook.result.current.loading).toBe(false));
  hook.rerender({ query: "another query" });
  expect(hook.result.current.loading).toBe(false);
});
