import { describe, expect, it } from "vitest";
import { keysNotLearned, unavailableKeyReason } from "./keyAvailability";

describe("unavailableKeyReason", () => {
  it("names the Windows F12 reservation", () => {
    expect(unavailableKeyReason("F12", "windows")).toMatch(/Windows reserves F12/);
  });
  it("names the macOS function key limit", () => {
    expect(unavailableKeyReason("F21", "macos")).toMatch(/macOS has no F21 key/);
    expect(unavailableKeyReason("F24", "macos")).toMatch(/F20 or lower/);
  });
  it("does not blame the platform for keys it supports", () => {
    expect(unavailableKeyReason("F12", "macos")).toMatch(/Learn another key/);
    expect(unavailableKeyReason("F21", "windows")).toMatch(/Learn another key/);
    expect(unavailableKeyReason("Space", "windows")).toMatch(/Learn another key/);
  });
});

describe("keysNotLearned", () => {
  it("warns about each platform's gap before learning", () => {
    expect(keysNotLearned("windows")).toMatch(/F12/);
    expect(keysNotLearned("macos")).toMatch(/F21 to F24/);
    expect(keysNotLearned("linux")).toBeNull();
  });
});
