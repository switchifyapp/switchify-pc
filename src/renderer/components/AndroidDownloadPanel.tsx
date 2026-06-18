import type { MouseEvent, ReactElement } from 'react';
import androidQrCodeUrl from '../assets/switchify-android-play-store-qr.svg';

export const SWITCHIFY_ANDROID_DOWNLOAD_URL = 'https://play.google.com/store/apps/details?id=com.enaboapps.switchify';

export function AndroidDownloadPanel({
  onOpenDownload
}: {
  onOpenDownload: (url: string) => Promise<{ ok: boolean }>;
}): ReactElement {
  const openDownload = (event: MouseEvent<HTMLAnchorElement>): void => {
    event.preventDefault();
    void onOpenDownload(SWITCHIFY_ANDROID_DOWNLOAD_URL);
  };

  return (
    <section className="android-download-panel" aria-label="Download Switchify for Android">
      <img src={androidQrCodeUrl} alt="QR code for Switchify on Google Play" />
      <div>
        <h2>Get Switchify for Android</h2>
        <p>Scan the QR code or open the Google Play listing on your Android device.</p>
        <a href={SWITCHIFY_ANDROID_DOWNLOAD_URL} onClick={openDownload}>
          Download Switchify for Android
        </a>
      </div>
    </section>
  );
}
