import { describe, expect, it } from "vitest";
import pkg from "../package.json";
import { browserState } from "./api";

describe("browser-mode version", () => {
  it("matches package.json so release bumps cannot miss the fallback state", () => {
    expect(browserState.version).toBe(pkg.version);
  });
});
