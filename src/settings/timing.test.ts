import { describe, expect, it } from "vitest";
import { formatSeconds, stepTime } from "./timing";

describe("timing steppers", () => {
  const range = { min: 100, max: 1000, step: 50 };

  it("moves one step at a time and stops at the ends", () => {
    expect(stepTime(250, 1, range)).toBe(300);
    expect(stepTime(250, -1, range)).toBe(200);
    expect(stepTime(1000, 1, range)).toBe(1000);
    expect(stepTime(100, -1, range)).toBe(100);
  });

  it("brings a time between steps onto the next step in that direction", () => {
    expect(stepTime(260, 1, range)).toBe(300);
    expect(stepTime(260, -1, range)).toBe(250);
  });

  it("writes seconds without floating-point noise", () => {
    expect(formatSeconds(300)).toBe("0.3 s");
    expect(formatSeconds(350)).toBe("0.35 s");
    expect(formatSeconds(1100)).toBe("1.1 s");
    expect(formatSeconds(10000)).toBe("10 s");
  });
});
