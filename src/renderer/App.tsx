import type { ReactElement } from 'react';

const features = [
  'Local network pairing will appear here.',
  'WebSocket server status will appear here.',
  'Connected Android devices will appear here.'
];

export function App(): ReactElement {
  const bridge = window.switchifyPc;

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
            <div className="status-value">Not started.</div>
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
