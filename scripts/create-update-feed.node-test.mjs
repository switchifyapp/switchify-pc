import { afterEach, describe, it } from "node:test";
import assert from "node:assert/strict";
import { gzipSync } from "node:zlib";
import { existsSync, mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { createUpdateFeed } from "./create-update-feed.mjs";

const temporaryDirectories = [];
afterEach(() => {
  for (const directory of temporaryDirectories.splice(0)) rmSync(directory, { recursive: true, force: true });
});

const fixtures = () => {
  const root = mkdtempSync(join(tmpdir(), "switchify-feed-"));
  temporaryDirectories.push(root);
  const macDirectory = join(root, "macos-release");
  const windowsDirectory = join(root, "windows-release");
  mkdirSync(macDirectory);
  mkdirSync(windowsDirectory);
  const mac = join(macDirectory, "Switchify.PC.app.tar.gz");
  const windows = join(windowsDirectory, "Switchify.PC_1.0.0_x64-setup.exe");
  writeFileSync(mac, gzipSync("archive fixture"));
  const pe = Buffer.alloc(128);
  pe.write("MZ", 0, "ascii");
  pe.writeUInt32LE(64, 0x3c);
  pe.write("PE\0\0", 64, "ascii");
  writeFileSync(windows, pe);
  writeFileSync(`${mac}.sig`, "mac-signature");
  writeFileSync(`${windows}.sig`, "windows-signature");
  return { root, mac, windows, output: join(root, "latest.json") };
};

const options = (fixture, verifySignature = () => {}) => ({
  ...fixture,
  version: "1.0.0-beta.1",
  tag: "v1.0.0-beta.1",
  verifySignature,
});

describe("createUpdateFeed", () => {
  it("publishes only structurally valid artifacts whose signatures verify", () => {
    const fixture = fixtures();
    const verified = [];
    const feed = createUpdateFeed(options(fixture, (artifact) => verified.push(artifact)));
    assert.deepEqual(verified, [fixture.mac, fixture.windows]);
    assert.equal(feed.platforms["darwin-aarch64"].signature, "mac-signature");
    assert.equal(feed.platforms["windows-x86_64"].signature, "windows-signature");
  });

  it("refuses a non-empty signature that fails cryptographic verification", () => {
    const fixture = fixtures();
    assert.throws(
      () => createUpdateFeed(options(fixture, () => { throw new Error("invalid signature"); })),
      /invalid signature/,
    );
    assert.equal(existsSync(fixture.output), false);
  });

  it("refuses a wrong-format payload even if its signature verifier succeeds", () => {
    const fixture = fixtures();
    writeFileSync(fixture.mac, "not an updater archive");
    assert.throws(() => createUpdateFeed(options(fixture)), /not a gzip archive/);
    assert.equal(existsSync(fixture.output), false);
  });

  it("does not search outside the expected platform artifact directories", () => {
    const fixture = fixtures();
    rmSync(fixture.mac);
    rmSync(`${fixture.mac}.sig`);
    const unexpectedDirectory = join(fixture.root, "renamed-download");
    mkdirSync(unexpectedDirectory);
    writeFileSync(join(unexpectedDirectory, "Switchify.PC.app.tar.gz"), gzipSync("archive"));
    writeFileSync(join(unexpectedDirectory, "Switchify.PC.app.tar.gz.sig"), "signature");
    assert.throws(() => createUpdateFeed(options(fixture)), /Expected one signed macOS artifact, found 0/);
  });
});
