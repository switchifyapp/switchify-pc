import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { basename, join, resolve } from "node:path";

const [root, version, tag, output] = process.argv.slice(2);
if (!root || !version || !tag || !output) {
  throw new Error("Usage: node scripts/create-update-feed.mjs <artifacts> <version> <tag> <output>");
}
if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(version)) {
  throw new Error("Invalid semantic version.");
}
if (tag !== `v${version}`) throw new Error(`Release tag ${tag} does not match v${version}.`);

const files = [];
const walk = (directory) => {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) walk(path);
    else files.push(path);
  }
};
walk(resolve(root));

const pick = (predicate, label) => {
  const matches = files.filter((path) => predicate(basename(path)) && existsSync(`${path}.sig`));
  if (matches.length !== 1) throw new Error(`Expected one signed ${label} artifact, found ${matches.length}.`);
  return matches[0];
};
const mac = pick((name) => name.endsWith(".app.tar.gz"), "macOS");
const windows = pick((name) => name.endsWith("-setup.exe"), "Windows NSIS");
const asset = (path) => `https://github.com/switchifyapp/switchify-pc/releases/download/${encodeURIComponent(tag)}/${encodeURIComponent(basename(path))}`;
const platform = (path) => {
  const signature = readFileSync(`${path}.sig`, "utf8").trim();
  if (!signature) throw new Error(`Updater signature is empty for ${basename(path)}.`);
  return { signature, url: asset(path) };
};

writeFileSync(resolve(output), `${JSON.stringify({
  version,
  notes: `Switchify PC ${version}`,
  pub_date: new Date().toISOString(),
  platforms: {
    "darwin-aarch64": platform(mac),
    "windows-x86_64": platform(windows),
  },
}, null, 2)}\n`);
