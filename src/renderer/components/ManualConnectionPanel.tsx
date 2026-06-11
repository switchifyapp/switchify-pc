import { useEffect, useMemo, useState, type ReactElement } from 'react';
import QRCode from 'qrcode';
import type { ConnectionDetails } from '../../shared/server-status';
import { createManualConnectionDetails } from '../manual-connection';

export function ManualConnectionPanel({
  appName,
  connectionDetails
}: {
  appName: string;
  connectionDetails: ConnectionDetails | null;
}): ReactElement {
  const details = useMemo(() => {
    if (!connectionDetails) return null;
    return createManualConnectionDetails({ appName, connectionDetails });
  }, [appName, connectionDetails]);
  const [qrDataUrl, setQrDataUrl] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setQrDataUrl(null);

    if (!details?.payloadJson) return;

    void QRCode.toDataURL(details.payloadJson, {
      errorCorrectionLevel: 'M',
      margin: 1,
      scale: 6
    }).then((dataUrl) => {
      if (!cancelled) setQrDataUrl(dataUrl);
    });

    return () => {
      cancelled = true;
    };
  }, [details?.payloadJson]);

  if (!connectionDetails) {
    return (
      <section className="manual-connection">
        <h3>Manual connection</h3>
        <p>Connection details are not available.</p>
      </section>
    );
  }

  if (!details?.payload || !details.payloadJson) {
    return (
      <section className="manual-connection">
        <h3>Manual connection</h3>
        <p>No local network address is available.</p>
      </section>
    );
  }

  const primaryUrl = details.urls[0];
  const payloadJson = details.payloadJson;

  async function copyValue(label: string, value: string): Promise<void> {
    await navigator.clipboard.writeText(value);
    setCopied(label);
    window.setTimeout(() => setCopied(null), 1400);
  }

  return (
    <section className="manual-connection">
      <div className="manual-connection-header">
        <h3>Manual connection</h3>
        <p>Scan this code from Switchify if your PC does not appear automatically.</p>
      </div>

      <div className="manual-connection-main">
        <div className="manual-connection-qr" aria-label="Manual connection QR code">
          {qrDataUrl ? <img src={qrDataUrl} alt="" /> : <span>Generating...</span>}
        </div>

        <div className="manual-connection-details">
          <div className="manual-detail">
            <span>Primary address</span>
            <strong>{primaryUrl}</strong>
            <button type="button" onClick={() => void copyValue('address', primaryUrl)}>
              {copied === 'address' ? 'Copied' : 'Copy address'}
            </button>
          </div>
          <div className="manual-detail">
            <span>Desktop id</span>
            <strong>{connectionDetails.desktopId}</strong>
            <button type="button" onClick={() => void copyValue('desktopId', connectionDetails.desktopId)}>
              {copied === 'desktopId' ? 'Copied' : 'Copy desktop id'}
            </button>
          </div>
        </div>
      </div>

      <details className="manual-connection-more">
        <summary>All addresses</summary>
        <ul className="technical-list">
          {details.urls.map((url) => (
            <li key={url}>
              <strong>{url}</strong>
            </li>
          ))}
        </ul>
        <div className="action-row">
          <button type="button" onClick={() => void copyValue('payload', payloadJson)}>
            {copied === 'payload' ? 'Copied' : 'Copy QR payload'}
          </button>
        </div>
      </details>
    </section>
  );
}
