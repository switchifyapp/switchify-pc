import { render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { browserState } from "../api";
import { FindResults } from "./Find";
import type { FindContext } from "./index";

const modelState = vi.hoisted(() => ({ semantic: undefined, loading: true }));
vi.mock("./useSemantic", () => ({ useSemantic: () => modelState }));
const ctx: FindContext = { state: browserState, settings: browserState.settings, config: null, scan: null, bindings: [], holdIntervalMs: null, panels: [], builtins: [], profiles: [], account: null };

it("waits for meaning search before announcing no matches, while keeping keyword results usable", () => {
  modelState.loading = true;
  const view = render(<FindResults query="unmatchablexyz" ctx={ctx} jump={vi.fn()} />);
  expect(screen.getByRole("heading", { name: "Finding settings…" })).toBeInTheDocument();
  expect(screen.queryByText(/Nothing matches/)).not.toBeInTheDocument();
  expect(screen.queryByRole("button")).not.toBeInTheDocument();
  view.rerender(<FindResults query="speed" ctx={ctx} jump={vi.fn()} />);
  expect(screen.getAllByRole("button").length).toBeGreaterThan(0);
  modelState.loading = false;
  view.rerender(<FindResults query="unmatchablexyz" ctx={ctx} jump={vi.fn()} />);
  expect(screen.getByRole("heading", { name: /Nothing matches/ })).toBeInTheDocument();
});
