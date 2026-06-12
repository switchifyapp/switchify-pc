import { describe, expect, it } from 'vitest';
import { compareVersions, normalizeVersion, parseVersion } from './version';

describe('compareVersions', () => {
  it('detects a patch version update', () => {
    expect(compareVersions('0.1.1', '0.1.0')).toBe(1);
  });

  it('treats a v-prefixed matching version as equal', () => {
    expect(compareVersions('v0.1.0', '0.1.0')).toBe(0);
  });

  it('detects a minor version update', () => {
    expect(compareVersions('0.2.0', '0.1.9')).toBe(1);
  });

  it('detects a major version update', () => {
    expect(compareVersions('1.0.0', '0.9.9')).toBe(1);
  });

  it('rejects invalid versions', () => {
    expect(parseVersion('0.1')).toBeNull();
    expect(compareVersions('latest', '0.1.0')).toBeNull();
  });

  it('rejects prerelease-style versions for the first update pass', () => {
    expect(normalizeVersion('0.1.0-beta.1')).toBeNull();
  });
});
