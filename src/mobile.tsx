import { Demonstration } from "./help/Demonstration";
import type { AppState } from "./types";

export const mobileQrUrl = new URL("./assets/mobile-download-qr.png", import.meta.url).href;
export const mobileDownloadUrl = "https://play.google.com/store/apps/details?id=com.enaboapps.switchify";

export const bluetoothLabels: Record<AppState["bluetooth"], string> = {
  initializing: "Starting Bluetooth...", advertising: "Ready to connect", connected: "Device connected",
  poweredOff: "Bluetooth is off", unauthorized: "Bluetooth permission required",
  conflict: "Current Switchify PC is running", unsupported: "Bluetooth unavailable", error: "Bluetooth unavailable",
};

export const bluetoothDescriptions: Record<AppState["bluetooth"], string> = {
  initializing: "Preparing this computer for nearby devices.",
  advertising: "Waiting for a nearby mobile device.",
  connected: "Mobile device connected.",
  poweredOff: "Turn on Bluetooth to connect a mobile device.",
  unauthorized: "Allow Bluetooth access in System Settings to connect.",
  conflict: "Quit the other Switchify PC instance, then reopen this app.",
  unsupported: "This computer does not support the required Bluetooth features.",
  error: "Bluetooth could not start. Try restarting Switchify PC.",
};

/** Whether this computer can never pair, as opposed to needing Bluetooth turned on or allowed. */
export function bluetoothUnavailable(bluetooth: AppState["bluetooth"]) {
  return bluetooth === "unsupported" || bluetooth === "error";
}

/** Getting Switchify for mobile and pairing it, shared by the Mobile page and the setup guide. */
export function MobileInstallCards({ attributes = true }: { attributes?: boolean }) {
  return <div className="mobile-cards" {...(attributes ? { "data-section": "pairing", "data-setting": "pairing" } : {})}>
    <div className="mobile-download">
      <h2>Install Switchify for mobile</h2><p>Scan to get the app.</p>
      <figure><img src={mobileQrUrl} alt="QR code for Switchify on Google Play" /><figcaption>Get Switchify</figcaption></figure>
      <a className="secondary" href={mobileDownloadUrl} target="_blank" rel="noreferrer">Open Google Play</a>
    </div>
    <section className="mobile-pairing"><h2>Connect your mobile</h2><ol><li>Open Switchify on your mobile device and select this computer.</li><li>Compare the pairing codes shown in both apps.</li><li>Approve the pairing request on this computer only if the codes match.</li></ol><Demonstration kind="pair" /></section>
  </div>;
}
