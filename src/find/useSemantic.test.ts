import { renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { packModel } from "./semantic";
beforeEach(() => vi.resetModules());
afterEach(() => vi.unstubAllGlobals());
it("waits for a nonempty query, then shares one load and embeds the index", async () => {
  const bytes = packModel(["[UNK]", "rate"], 2, new Float32Array([0, 0, 1, 0]));
  const fetcher = vi.fn().mockResolvedValue({ ok: true, arrayBuffer: async () => bytes.buffer });
  vi.stubGlobal("fetch", fetcher);
  const { useSemantic } = await import("./useSemantic");
  const hook = renderHook(({ query }) => useSemantic(query), { initialProps: { query: " " } });
  expect(fetcher).not.toHaveBeenCalled();
  hook.rerender({ query: "snail" });
  await waitFor(() => expect(hook.result.current?.vectors.get("scan-rate")).toBeTruthy());
  hook.rerender({ query: "rate" });
  expect(fetcher).toHaveBeenCalledTimes(1);
});
it("quietly falls back when fetch is unavailable", async () => {
  vi.stubGlobal("fetch", undefined);
  const { useSemantic } = await import("./useSemantic");
  const hook = renderHook(() => useSemantic("snail"));
  await waitFor(() => expect(hook.result.current).toBeUndefined());
});
it("keeps keyword search when the model cannot be decoded", async () => {
  const fetcher = vi.fn().mockResolvedValue({ ok: true, arrayBuffer: async () => new ArrayBuffer(0) });
  vi.stubGlobal("fetch", fetcher);
  const { useSemantic } = await import("./useSemantic");
  const hook = renderHook(() => useSemantic("rate"));
  await waitFor(() => expect(fetcher).toHaveBeenCalledTimes(1));
  expect(hook.result.current).toBeUndefined();
});
