import { getResponder, type ResponderOptions } from '@homebridge/ciao';
import {
  createSwitchifyDiscoveryTxt,
  SWITCHIFY_MDNS_SERVICE_NAME,
  SWITCHIFY_MDNS_SERVICE_TYPE
} from '../../shared/discovery';

export type MdnsAdvertiserOptions = {
  getDesktopId: () => Promise<string>;
  getPort: () => number;
  getServiceName?: () => string;
  getResponder?: (options?: ResponderOptions) => MdnsResponder;
};

type MdnsResponder = {
  createService(options: { name: string; type: string; port: number; txt: Record<string, string> }): MdnsService;
  shutdown(): Promise<void>;
};

type MdnsService = {
  advertise(): Promise<void>;
  end(): Promise<void>;
  destroy(): void;
};

export class MdnsAdvertiser {
  private responder: MdnsResponder | null = null;
  private service: MdnsService | null = null;
  private running = false;

  constructor(private readonly options: MdnsAdvertiserOptions) {}

  async start(): Promise<void> {
    if (this.running) return;

    const desktopId = await this.options.getDesktopId();
    const responderOptions: ResponderOptions = {
      advertiseIpv4: true,
      advertiseIpv6: true
    };
    const responder = this.options.getResponder?.(responderOptions) ?? getResponder(responderOptions);
    const txt = createSwitchifyDiscoveryTxt(desktopId);
    const service = responder.createService({
      name: this.options.getServiceName?.() ?? SWITCHIFY_MDNS_SERVICE_NAME,
      type: SWITCHIFY_MDNS_SERVICE_TYPE,
      port: this.options.getPort(),
      txt
    });

    await service.advertise();
    console.info(
      `Switchify discovery advertising. type=_${SWITCHIFY_MDNS_SERVICE_TYPE}._tcp port=${this.options.getPort()} addressFamilies=IPv4,IPv6 txtKeys=${Object.keys(txt).join(',')}`
    );
    this.responder = responder;
    this.service = service;
    this.running = true;
  }

  async stop(): Promise<void> {
    const service = this.service;
    const responder = this.responder;
    this.service = null;
    this.responder = null;
    this.running = false;

    if (service) {
      await service.end();
      service.destroy();
    }
    if (responder) {
      await responder.shutdown();
    }
  }

  async restart(): Promise<void> {
    await this.stop();
    await this.start();
  }

  isRunning(): boolean {
    return this.running;
  }
}
