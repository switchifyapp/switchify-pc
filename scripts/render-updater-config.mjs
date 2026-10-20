import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { feedEndpoint } from "./update-channel.mjs";

const output = process.argv[2];
const windows = process.argv.includes("--windows");
// The feed a build checks follows its own version: a pre-release build checks
// the pre-release feed, a stable build the stable feed.
const { version } = JSON.parse(
  readFileSync(resolve(dirname(fileURLToPath(import.meta.url)), "..", "package.json"), "utf8"),
);
const endpoint = process.env.SWITCHIFY_UPDATER_ENDPOINT ?? feedEndpoint(version);
const pubkey = process.env.SWITCHIFY_UPDATER_PUBLIC_KEY?.trim();

if (!output) throw new Error("Usage: node scripts/render-updater-config.mjs <output> [--windows]");
if (!pubkey) throw new Error("SWITCHIFY_UPDATER_PUBLIC_KEY is required.");
let endpointUrl;
try {
  endpointUrl = new URL(endpoint);
} catch {
  throw new Error("SWITCHIFY_UPDATER_ENDPOINT must be a valid URL.");
}
if (endpointUrl.protocol !== "https:") throw new Error("SWITCHIFY_UPDATER_ENDPOINT must use HTTPS.");

const config = {
  plugins: { updater: { endpoints: [endpoint], pubkey } },
  bundle: { createUpdaterArtifacts: true },
};

if (windows) {
  config.bundle.externalBin = ["binaries/switchify-pc-startup", "binaries/switchify-smol-worker", "binaries/switchify-smol-worker-avx2"];
  config.bundle.windows = {
    signCommand: {
      cmd: "powershell.exe",
      args: ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "../scripts/Sign-Windows.ps1", "%1"],
    },
    nsis: { installMode: "perMachine", installerHooks: "windows/installer-hooks.nsh" },
  };
}

writeFileSync(resolve(output), `${JSON.stringify(config, null, 2)}\n`, { mode: 0o600 });
