// Which updater feed a release belongs to. A pre-release version (one with a
// hyphen, such as 1.0.1-rc.1) is offered only to pre-release installs. A
// stable version is offered to everyone, so a stable release writes both feeds.
//
// Builds before the split read the stable feed whatever their version. Until
// a stable release has been written there, a pre-release is written to the
// stable feed as well, so those installs keep receiving release candidates
// and then the first stable release.
export const FEED_BASE_URL = "https://raw.githubusercontent.com/switchifyapp/switchify-pc/update-feed/";

export const isPrerelease = (version) => version.includes("-");

export const feedFileName = (version) => (isPrerelease(version) ? "prerelease.json" : "latest.json");

export const feedEndpoint = (version) => `${FEED_BASE_URL}${feedFileName(version)}`;

export const feedFilesToPublish = (version, stableFeedVersion = "") => {
  if (!isPrerelease(version)) return ["latest.json", "prerelease.json"];
  const stableFeedHasStableRelease = stableFeedVersion !== "" && !isPrerelease(stableFeedVersion);
  return stableFeedHasStableRelease ? ["prerelease.json"] : ["prerelease.json", "latest.json"];
};

if (process.argv[1] && (await import("node:url")).pathToFileURL(process.argv[1]).href === import.meta.url) {
  const [version, stableFeedVersion = ""] = process.argv.slice(2);
  if (!version) throw new Error("Usage: node scripts/update-channel.mjs <version> [stable-feed-version]");
  process.stdout.write(`${feedFilesToPublish(version, stableFeedVersion).join(" ")}\n`);
}
