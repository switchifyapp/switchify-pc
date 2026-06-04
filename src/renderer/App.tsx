import type { ReactElement } from 'react';
import { useEffect, useState } from 'react';
import type { PcServerStatus } from '../shared/server-status';

const features = [
  'Local network pairing will appear here.',
  'WebSocket server status will appear here.',
  'Connected Android devices will appear here.'
];

export function App(): ReactElement {
  const bridge = window.switchifyPc;
  const [serverStatus, setServerStatus] = useState<PcServerStatus | null>(null);

  useEffect(() => {
    let cancelled = false;
    const refresh = async (): Promise<void> => {
      const status = await bridge.getServerStatus();
      if (!cancelled) setServerStatus(status);
    };

    void refresh();
    const interval = window.setInterval(() => {
      void refresh();
    }, 1000);

    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, [bridge]);

  return (
    <main className="app-shell">
      <section className="status-panel" aria-labelledby="app-title">
        <div className="eyebrow">Desktop companion</div>
        <h1 id="app-title">{bridge.appName}</h1>
        <p className="summary">
          {bridge.status} This window is the foundation for pairing Switchify Android with this
          PC.
        </p>

        <div className="status-card" aria-label="Connection status">
          <span className="status-dot" />
          <div>
            <div className="status-label">Server status</div>
            <div className="status-value">{formatServerStatus(serverStatus)}</div>
          </div>
        </div>

        <ul className="feature-list">
          {features.map((feature) => (
            <li key={feature}>{feature}</li>
          ))}
        </ul>
      </section>
    </main>
  );
}

function formatServerStatus(status: PcServerStatus | null): string {
  if (!status) return 'Loading.';
  if (status.state === 'listening') {
    return `Listening on port ${status.port}. ${status.connectedClientCount} connected.`;
  }
  if (status.state === 'error') return status.lastError ?? 'Server error.';
  return `${status.state}.`;
}
