import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { feedEndpoint, feedFileName, feedFilesToPublish, isPrerelease } from "./update-channel.mjs";

describe("update channel", () => {
  it("treats a hyphenated version as a pre-release", () => {
    assert.equal(isPrerelease("1.0.0-rc.32"), true);
    assert.equal(isPrerelease("1.0.1-beta.1"), true);
    assert.equal(isPrerelease("1.0.0"), false);
    assert.equal(isPrerelease("2.3.10"), false);
  });

  it("sends a pre-release build to the pre-release feed and a stable build to the stable feed", () => {
    assert.equal(feedFileName("1.0.1-rc.1"), "prerelease.json");
    assert.equal(feedFileName("1.0.0"), "latest.json");
    assert.equal(
      feedEndpoint("1.0.1-rc.1"),
      "https://raw.githubusercontent.com/switchifyapp/switchify-pc/update-feed/prerelease.json",
    );
    assert.equal(
      feedEndpoint("1.0.0"),
      "https://raw.githubusercontent.com/switchifyapp/switchify-pc/update-feed/latest.json",
    );
  });

  it("publishes a stable release to both feeds", () => {
    assert.deepEqual(feedFilesToPublish("1.0.0", "1.0.0-rc.32"), ["latest.json", "prerelease.json"]);
    assert.deepEqual(feedFilesToPublish("1.1.0", "1.0.0"), ["latest.json", "prerelease.json"]);
  });

  it("publishes a pre-release to its own feed only once a stable release is on the stable feed", () => {
    assert.deepEqual(feedFilesToPublish("1.0.1-rc.1", "1.0.0"), ["prerelease.json"]);
  });

  it("keeps earlier release-candidate installs updated through the stable feed until 1.0 ships", () => {
    // Builds before the split read latest.json whatever their version.
    assert.deepEqual(feedFilesToPublish("1.0.0-rc.33", "1.0.0-rc.32"), ["prerelease.json", "latest.json"]);
    assert.deepEqual(feedFilesToPublish("1.0.0-rc.33"), ["prerelease.json", "latest.json"]);
    assert.deepEqual(feedFilesToPublish("1.0.0-rc.33", ""), ["prerelease.json", "latest.json"]);
  });

  it("prints the feeds to publish when run as a script", () => {
    const out = (...args) =>
      execFileSync(process.execPath, ["scripts/update-channel.mjs", ...args], { encoding: "utf8" }).trim();
    assert.equal(out("1.0.1-rc.1", "1.0.0"), "prerelease.json");
    assert.equal(out("1.0.0-rc.33", "1.0.0-rc.32"), "prerelease.json latest.json");
    assert.equal(out("1.0.0", "1.0.0-rc.32"), "latest.json prerelease.json");
  });

  it("keeps the current app version on the feed its channel expects", () => {
    const { version } = JSON.parse(readFileSync("package.json", "utf8"));
    assert.equal(feedFileName(version), version.includes("-") ? "prerelease.json" : "latest.json");
  });
});
