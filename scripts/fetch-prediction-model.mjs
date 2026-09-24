// Downloads the enhanced word prediction model into src-tauri/resources/prediction-model.
// The model is too large to commit, so every file is pinned to one upstream
// revision and verified by SHA-256. Verified files are not downloaded again.
import { createHash } from "node:crypto";
import { createReadStream, createWriteStream } from "node:fs";
import { mkdir, rename, rm, stat } from "node:fs/promises";
import { dirname, join } from "node:path";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { fileURLToPath } from "node:url";

const revision = "d0ae6834f1df45e0e95b5fdae95e536f9ca7cd3f";
const base = `https://huggingface.co/onnx-community/SmolLM2-135M-ONNX/resolve/${revision}`;
const files = [
  { name: "model.onnx", source: "onnx/model_int8.onnx", size: 135658354, sha256: "50ba80511ce74634d232a043b6c37775cca756b826b49d0a4a8eff958c4bbcc9" },
  { name: "tokenizer.json", source: "tokenizer.json", size: 2053526, sha256: "139d2f4b4919b90953bdd3c0c40c94c9b23074799a766508dc3bf5eb8ab73351" },
  { name: "config.json", source: "config.json", size: 1035, sha256: "2c5f23fddabecdf9c47d0048f555899822ec87ec4a28169393840ac7e74192c4" },
];
const target = join(dirname(fileURLToPath(import.meta.url)), "..", "src-tauri", "resources", "prediction-model");

async function digest(path) {
  const hash = createHash("sha256");
  await pipeline(createReadStream(path), hash);
  return hash.digest("hex");
}

async function verified(path, file) {
  try {
    return (await stat(path)).size === file.size && (await digest(path)) === file.sha256;
  } catch {
    return false;
  }
}

async function download(file) {
  const path = join(target, file.name);
  if (await verified(path, file)) return false;
  const pending = `${path}.pending`;
  for (let attempt = 1; ; attempt++) {
    try {
      const response = await fetch(`${base}/${file.source}`);
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      await pipeline(Readable.fromWeb(response.body), createWriteStream(pending));
      if (!(await verified(pending, file))) throw new Error("size or SHA-256 mismatch");
      await rename(pending, path);
      return true;
    } catch (error) {
      await rm(pending, { force: true });
      if (attempt === 3) throw new Error(`Could not fetch ${file.name}: ${error.message}`);
      await new Promise((resolve) => setTimeout(resolve, attempt * 2000));
    }
  }
}

await mkdir(target, { recursive: true });
for (const file of files) {
  if (await download(file)) console.log(`Fetched ${file.name}`);
}
