import { describe, expect, it } from "vitest";
import { interfaceModeAdvice, keysNotLearned, reservedKeyAdvice, switchKeyLabel, unavailableKeyReason } from "./keyAvailability";

describe("switchKeyLabel", () => {
  it("spells out controller buttons and leaves keys alone", () => {
    expect(switchKeyLabel("Gamepad1")).toBe("Gamepad 1");
    expect(switchKeyLabel("Gamepad8")).toBe("Gamepad 8");
    expect(switchKeyLabel("Space")).toBe("Space");
    expect(switchKeyLabel("F13")).toBe("F13");
  });
  it("tells people gamepad mode is fine now", () => {
    expect(interfaceModeAdvice).toMatch(/keyboard or gamepad mode/);
    expect(interfaceModeAdvice).not.toMatch(/gamepad modes cannot/);
  });
});

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

describe("reservedKeyAdvice", () => {
  it("never recommends keys the platform cannot capture", () => {
    expect(reservedKeyAdvice("macos")).toMatch(/F13 to F20/);
    expect(reservedKeyAdvice("windows")).toMatch(/F13 or higher/);
  });
});

describe("keysNotLearned", () => {
  it("warns about each platform's gap before learning", () => {
    expect(keysNotLearned("windows")).toMatch(/F12/);
    expect(keysNotLearned("macos")).toMatch(/F21 to F24/);
    expect(keysNotLearned("linux")).toBeNull();
  });
});
