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

  it('uses Certum SimplySign args when explicitly configured', () => {
    process.env.SWITCHIFY_SIGNING_MODE = 'certum-simplysign';
    process.env.SWITCHIFY_CERTUM_CERT_THUMBPRINT = '9A 27 06 A2 E8 6F 13 97 95 DA 2E 45 2B 31 2A 8C 23 AA 0C 48';

    const args = createSigningArgs('C:\\build\\Switchify PC.exe', { requireSigning: true });

    if (process.platform !== 'win32') {
      expect(args).toBeNull();
      return;
    }

    expect(args).toEqual([
      'sign',
      '/v',
      '/fd',
      'SHA256',
      '/sha1',
      '9A2706A2E86F139795DA2E452B312A8C23AA0C48',
      '/tr',
      'http://time.certum.pl',
      '/td',
      'SHA256',
      'C:\\build\\Switchify PC.exe'
    ]);
  });

  it('requires a Certum thumbprint when Certum mode is explicit', () => {
    process.env.SWITCHIFY_SIGNING_MODE = 'certum-simplysign';
    delete process.env.SWITCHIFY_CERTUM_CERT_THUMBPRINT;

    if (process.platform !== 'win32') {
      expect(createSigningArgs('C:\\build\\Switchify PC.exe', { requireSigning: true })).toBeNull();
      return;
    }

    expect(() => createSigningArgs('C:\\build\\Switchify PC.exe', { requireSigning: true })).toThrow(
      'Certum SimplySign signing requires SWITCHIFY_CERTUM_CERT_THUMBPRINT.'
    );
  });
});
