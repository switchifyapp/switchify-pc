import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createRequire } from 'node:module';
import { afterEach, describe, expect, it } from 'vitest';

const require = createRequire(import.meta.url);
const {
  createSha512Base64,
  updateLatestYmlForReferencedInstaller,
  updateLatestYmlForSignedInstaller
} = require('../../scripts/sign-win-artifacts.cjs') as {
  createSha512Base64(filePath: string): string;
  updateLatestYmlForReferencedInstaller(latestPath: string): void;
  updateLatestYmlForSignedInstaller(installerPath: string): void;
};

const tempDirs: string[] = [];

afterEach(() => {
  for (const tempDir of tempDirs.splice(0)) {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
});

describe('sign-win-artifacts updater metadata', () => {
  it('updates latest.yml checksums after installer bytes change', () => {
    const distDir = createTempDir();
    const installerPath = path.join(distDir, 'Switchify-PC-Setup-0.1.1-x64.exe');
    const latestPath = path.join(distDir, 'latest.yml');
    fs.writeFileSync(installerPath, 'unsigned installer bytes');
    fs.writeFileSync(
      latestPath,
      [
        'version: 0.1.1',
        'files:',
        '  - url: Switchify-PC-Setup-0.1.1-x64.exe',
        '    sha512: old-file-checksum',
        'path: Switchify-PC-Setup-0.1.1-x64.exe',
        'sha512: old-path-checksum'
      ].join('\n')
    );

    fs.writeFileSync(installerPath, 'signed installer bytes');
    updateLatestYmlForSignedInstaller(installerPath);

    const sha512 = createSha512Base64(installerPath);
    const latest = fs.readFileSync(latestPath, 'utf8');
    expect(latest.match(/^sha512: .+$/gm)).toEqual([`sha512: ${sha512}`]);
    expect(latest.match(/^\s+sha512: .+$/gm)).toEqual([`    sha512: ${sha512}`]);
  });

  it('ignores latest.yml when it does not reference the signed installer', () => {
    const distDir = createTempDir();
    const installerPath = path.join(distDir, 'Switchify-PC-Setup-0.1.1-x64.exe');
    fs.writeFileSync(installerPath, 'signed installer bytes');
    fs.writeFileSync(
      path.join(distDir, 'latest.yml'),
      [
        'version: 0.1.1',
        'files:',
        '  - url: Other-Setup.exe',
        '    sha512: old-file-checksum',
        'path: Other-Setup.exe',
        'sha512: old-path-checksum'
      ].join('\n')
    );

    expect(() => updateLatestYmlForSignedInstaller(installerPath)).not.toThrow();
    expect(fs.readFileSync(path.join(distDir, 'latest.yml'), 'utf8')).toContain('old-file-checksum');
  });

  it('updates latest.yml when the installer name is URL-encoded', () => {
    const distDir = createTempDir();
    const installerPath = path.join(distDir, 'Switchify PC Setup 0.1.1 x64.exe');
    const latestPath = path.join(distDir, 'latest.yml');
    fs.writeFileSync(installerPath, 'signed installer bytes');
    fs.writeFileSync(
      latestPath,
      [
        'version: 0.1.1',
        'files:',
        '  - url: Switchify%20PC%20Setup%200.1.1%20x64.exe',
        '    sha512: old-file-checksum',
        'path: Switchify%20PC%20Setup%200.1.1%20x64.exe',
        'sha512: old-path-checksum'
      ].join('\n')
    );

    updateLatestYmlForSignedInstaller(installerPath);

    const sha512 = createSha512Base64(installerPath);
    const latest = fs.readFileSync(latestPath, 'utf8');
    expect(latest.match(/^sha512: .+$/gm)).toEqual([`sha512: ${sha512}`]);
    expect(latest.match(/^\s+sha512: .+$/gm)).toEqual([`    sha512: ${sha512}`]);
  });

  it('does nothing when latest.yml is missing', () => {
    const distDir = createTempDir();
    const installerPath = path.join(distDir, 'Switchify-PC-Setup-0.1.1-x64.exe');
    fs.writeFileSync(installerPath, 'signed installer bytes');

    expect(() => updateLatestYmlForSignedInstaller(installerPath)).not.toThrow();
  });

  it('updates the installer referenced by latest.yml', () => {
    const distDir = createTempDir();
    const installerPath = path.join(distDir, 'Switchify-PC-Setup-0.1.1-x64.exe');
    const latestPath = path.join(distDir, 'latest.yml');
    fs.writeFileSync(installerPath, 'signed installer bytes');
    fs.writeFileSync(
      latestPath,
      [
        'version: 0.1.1',
        'files:',
        '  - url: Switchify-PC-Setup-0.1.1-x64.exe',
        '    sha512: old-file-checksum',
        'path: Switchify-PC-Setup-0.1.1-x64.exe',
        'sha512: old-path-checksum'
      ].join('\n')
    );

    updateLatestYmlForReferencedInstaller(latestPath);

    const sha512 = createSha512Base64(installerPath);
    const latest = fs.readFileSync(latestPath, 'utf8');
    expect(latest.match(/^sha512: .+$/gm)).toEqual([`sha512: ${sha512}`]);
    expect(latest.match(/^\s+sha512: .+$/gm)).toEqual([`    sha512: ${sha512}`]);
  });
});

function createTempDir(): string {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'switchify-sign-test-'));
  tempDirs.push(tempDir);
  return tempDir;
}
