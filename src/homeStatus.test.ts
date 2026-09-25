import { describe, expect, it } from "vitest";
import { homeStatus, type HomeStatusInput } from "./homeStatus";
import { defaultPointScanConfig, type PointScanState } from "./scanning/useScanning";

const scanning: PointScanState = { config: defaultPointScanConfig, enabled: true, paused: false, phase: "idle", message: "Scanning is off.", supported: true };
const ready: HomeStatusInput = { accessibility: "granted", bluetooth: "advertising", switchesLoaded: true, hasSelect: true, error: null, scanning };

describe("homeStatus", () => {
  it.each<[string, Partial<HomeStatusInput>, string, string]>([
    ["ready", {}, "Ready", "ready"],
    ["an error", { error: "Could not save switches." }, "Switch control needs attention", "attention"],
    ["input access required", { accessibility: "required" }, "Finish setting up", "attention"],
    ["input access unavailable", { accessibility: "unavailable", hasSelect: false }, "Input access unavailable", "neutral"],
    ["scanning still loading", { scanning: null }, "Switch control", "neutral"],
    ["switches still loading", { switchesLoaded: false, hasSelect: false }, "Switch control", "neutral"],
    ["scanning unsupported", { scanning: { ...scanning, supported: false, message: "Unsupported." }, hasSelect: false }, "Switch control unavailable", "neutral"],
    ["Remote in control when otherwise ready", { scanning: { ...scanning, remote: true } }, "Remote in control", "neutral"],
    ["Remote in control without a local Select", { scanning: { ...scanning, remote: true }, bluetooth: "connected", hasSelect: false }, "Remote in control", "neutral"],
    ["a mobile connection without a local Select", { bluetooth: "connected", hasSelect: false }, "Paused for mobile", "neutral"],
    ["no Select switch", { hasSelect: false }, "Finish setting up", "attention"],
    ["paused", { scanning: { ...scanning, paused: true } }, "Paused", "neutral"],
    ["scanning off", { scanning: { ...scanning, enabled: false } }, "Switch control", "neutral"],
  ])("names %s", (_, change, title, tone) => {
    expect(homeStatus({ ...ready, ...change })).toMatchObject({ title, tone });
  });

  it("keeps the message on the same case as the title", () => {
    expect(homeStatus({ ...ready, bluetooth: "connected", hasSelect: false }).message).toBe("Local scanning is paused while a mobile device is connected.");
    expect(homeStatus({ ...ready, switchesLoaded: false, hasSelect: false }).message).toBe("Loading switch control...");
    expect(homeStatus({ ...ready, scanning: { ...scanning, supported: false, message: "Unsupported." }, hasSelect: false }).message).toBe("Unsupported.");
  });
});
