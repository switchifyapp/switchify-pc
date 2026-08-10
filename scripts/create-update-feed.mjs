import {
  existsSync,
  lstatSync,
  openSync,
  closeSync,
  readFileSync,
  readSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import { execFileSync } from "node:child_process";
import { basename, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const SEMVER = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/;

const regularFiles = (directory) => {
  if (!existsSync(directory) || !lstatSync(directory).isDirectory()) {
    throw new Error(`Expected release artifact directory ${directory}.`);
  }
  return readdirSync(directory, { withFileTypes: true })
    .filter((entry) => entry.isFile())
    .map((entry) => join(directory, entry.name));
};

const pickSignedArtifact = (directory, predicate, label) => {
  const files = regularFiles(directory);
  const matches = files.filter((path) => predicate(basename(path)) && existsSync(`${path}.sig`));
  if (matches.length !== 1) {
    throw new Error(`Expected one signed ${label} artifact, found ${matches.length}.`);
  }
  if (!lstatSync(`${matches[0]}.sig`).isFile()) {
    throw new Error(`Updater signature is not a regular file for ${basename(matches[0])}.`);
  }
  return matches[0];
};

const validateMacArtifact = (path) => {
  const header = Buffer.alloc(2);
  const file = openSync(path, "r");
  try {
    if (readSync(file, header, 0, header.length, 0) !== header.length || header[0] !== 0x1f || header[1] !== 0x8b) {
      throw new Error(`macOS updater artifact is not a gzip archive: ${basename(path)}.`);
    }
  } finally {
    closeSync(file);
  }
};

const validateWindowsArtifact = (path) => {
  const dosHeader = Buffer.alloc(64);
  const file = openSync(path, "r");
  try {
    if (readSync(file, dosHeader, 0, dosHeader.length, 0) !== dosHeader.length || dosHeader.toString("ascii", 0, 2) !== "MZ") {
      throw new Error(`Windows updater artifact is not a PE executable: ${basename(path)}.`);
    }
    const peOffset = dosHeader.readUInt32LE(0x3c);
    const peHeader = Buffer.alloc(4);
    if (peOffset < dosHeader.length || readSync(file, peHeader, 0, peHeader.length, peOffset) !== peHeader.length || !peHeader.equals(Buffer.from("PE\0\0"))) {
      throw new Error(`Windows updater artifact has an invalid PE header: ${basename(path)}.`);
    }
  } finally {
    closeSync(file);
  }
};

const verifyWithExecutable = (verifier, artifact, signature) => {
  execFileSync(resolve(verifier), [artifact, signature], { stdio: "pipe" });
};

const githubReleaseAssetName = (path) => basename(path).replaceAll(" ", ".");

export const createUpdateFeed = ({ root, version, tag, output, verifier, verifySignature }) => {
  if (!SEMVER.test(version)) throw new Error("Invalid semantic version.");
  if (tag !== `v${version}`) throw new Error(`Release tag ${tag} does not match v${version}.`);

  const artifactRoot = resolve(root);
  const mac = pickSignedArtifact(join(artifactRoot, "macos-release"), (name) => name.endsWith(".app.tar.gz"), "macOS");
  const windows = pickSignedArtifact(join(artifactRoot, "windows-release"), (name) => name.endsWith("-setup.exe"), "Windows NSIS");
  validateMacArtifact(mac);
  validateWindowsArtifact(windows);

  const verify = verifySignature ?? ((artifact, signature) => verifyWithExecutable(verifier, artifact, signature));
  const platform = (path) => {
    const signaturePath = `${path}.sig`;
    const signature = readFileSync(signaturePath, "utf8").trim();
    if (!signature) throw new Error(`Updater signature is empty for ${basename(path)}.`);
    verify(path, signaturePath);
    return {
      signature,
      url: `https://github.com/switchifyapp/switchify-pc/releases/download/${encodeURIComponent(tag)}/${encodeURIComponent(githubReleaseAssetName(path))}`,
    };
  };

  const feed = {
    version,
    notes: `Switchify PC ${version}`,
    pub_date: new Date().toISOString(),
    platforms: {
      "darwin-aarch64": platform(mac),
      "windows-x86_64": platform(windows),
    },
  };
  writeFileSync(resolve(output), `${JSON.stringify(feed, null, 2)}\n`);
  return feed;
};

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [root, version, tag, output, verifier] = process.argv.slice(2);
  if (!root || !version || !tag || !output || !verifier) {
    throw new Error("Usage: node scripts/create-update-feed.mjs <artifacts> <version> <tag> <output> <signature-verifier>");
  }
  createUpdateFeed({ root, version, tag, output, verifier });
}
