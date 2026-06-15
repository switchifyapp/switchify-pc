import { createRequire } from 'node:module';
import { afterEach, describe, expect, it } from 'vitest';

const require = createRequire(import.meta.url);
const { createSigningArgs } = require('../../scripts/package-win-after-pack.cjs') as {
  createSigningArgs: (filePath: string, options: { requireSigning: boolean }) => string[] | null;
};

const originalEnv = { ...process.env };

describe('createSigningArgs', () => {
  afterEach(() => {
    process.env = { ...originalEnv };
  });

  it('uses an installed certificate thumbprint when no PFX password is available', () => {
    delete process.env.SWITCHIFY_DEV_CERT_PASSWORD;
    delete process.env.SWITCHIFY_AZURE_SIGNING_DLIB;
    delete process.env.SWITCHIFY_AZURE_SIGNING_METADATA;
    process.env.SWITCHIFY_DEV_CERT_THUMBPRINT = '24 05 4d 36';

    const args = createSigningArgs('C:\\build\\Switchify PC.exe', { requireSigning: true });

    if (process.platform !== 'win32') {
      expect(args).toBeNull();
      return;
    }

    expect(args).toEqual([
      'sign',
      '/fd',
      'SHA256',
      '/sha1',
      '24054D36',
      '/tr',
      'http://timestamp.digicert.com',
      '/td',
      'SHA256',
      'C:\\build\\Switchify PC.exe'
    ]);
  });
});
