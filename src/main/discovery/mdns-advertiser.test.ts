import { describe, expect, it } from 'vitest';
import { SWITCHIFY_DISCOVERY_KIND } from '../../shared/discovery';
import { MdnsAdvertiser } from './mdns-advertiser';

describe('MdnsAdvertiser', () => {
  it('advertises a Switchify DNS-SD service without secrets', async () => {
    const fakeResponder = new FakeResponder();
    const advertiser = new MdnsAdvertiser({
      getDesktopId: async () => 'desktop-1',
      getPort: () => 7347,
      getResponder: (options) => fakeResponder.getResponder(options)
    });

    await advertiser.start();

    expect(advertiser.isRunning()).toBe(true);
    expect(fakeResponder.serviceOptions).toMatchObject({
      name: 'Switchify PC',
      type: 'switchify',
      port: 7347,
      txt: {
        kind: SWITCHIFY_DISCOVERY_KIND,
        version: '1',
        desktopId: 'desktop-1',
        protocolVersion: '1',
        pairing: 'approval'
      }
    });
    expect(fakeResponder.responderOptions).toMatchObject({
      advertiseIpv4: true,
      advertiseIpv6: true
    });
    const serializedOptions = JSON.stringify(fakeResponder.serviceOptions);
    expect(serializedOptions).not.toContain('token');
    expect(serializedOptions).not.toContain('nonce');
    expect(serializedOptions).not.toContain('auth');
    expect(serializedOptions).not.toContain('secret');
  });

  it('stops and destroys the advertised service', async () => {
    const fakeResponder = new FakeResponder();
    const advertiser = new MdnsAdvertiser({
      getDesktopId: async () => 'desktop-1',
      getPort: () => 7347,
      getResponder: (options) => fakeResponder.getResponder(options)
    });
    await advertiser.start();

    await advertiser.stop();

    expect(advertiser.isRunning()).toBe(false);
    expect(fakeResponder.service.ended).toBe(true);
    expect(fakeResponder.service.destroyed).toBe(true);
    expect(fakeResponder.shutdownCalled).toBe(true);
  });
});

class FakeResponder {
  readonly service = new FakeService();
  serviceOptions: unknown = null;
  responderOptions: unknown = null;
  shutdownCalled = false;

  getResponder(options?: unknown): FakeResponder {
    this.responderOptions = options;
    return this;
  }

  createService(options: unknown): FakeService {
    this.serviceOptions = options;
    return this.service;
  }

  async shutdown(): Promise<void> {
    this.shutdownCalled = true;
  }
}

class FakeService {
  advertised = false;
  ended = false;
  destroyed = false;

  async advertise(): Promise<void> {
    this.advertised = true;
  }

  async end(): Promise<void> {
    this.ended = true;
  }

  destroy(): void {
    this.destroyed = true;
  }
}
