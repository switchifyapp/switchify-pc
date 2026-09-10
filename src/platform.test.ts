import { describe, expect, it } from "vitest";
import { browserPlatform } from "./platform";

describe("browser sample platform", () => {
  it.each([
    ["Mozilla/5.0 (X11; Linux x86_64)", "linux"],
    ["Mozilla/5.0 (Windows NT 10.0; Win64; x64)", "windows"],
    ["Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)", "macos"],
    ["Mozilla/5.0 (Linux; Android 16)", "windows"],
  ])("selects the desktop sample for %s", (agent, expected) => {
    expect(browserPlatform(agent)).toBe(expected);
  });
});
