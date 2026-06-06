import { BrowserWindow, screen } from 'electron';
import { cursorOverlayBounds, nativeCursorToElectronPoint, type CursorPoint } from './cursor-overlay-state';

export type CursorOverlayEvent = 'move' | 'click';

export type CursorOverlayOptions = {
  getCursorPosition: () => CursorPoint;
  idleTimeoutMs?: number;
  windowSize?: number;
};

const DEFAULT_IDLE_TIMEOUT_MS = 900;
const DEFAULT_WINDOW_SIZE = 96;

export class CursorOverlay {
  private window: BrowserWindow | null = null;
  private hideTimer: NodeJS.Timeout | null = null;
  private enabled = true;
  private failedToCreate = false;
  private readonly idleTimeoutMs: number;
  private readonly windowSize: number;

  constructor(private readonly options: CursorOverlayOptions) {
    this.idleTimeoutMs = options.idleTimeoutMs ?? DEFAULT_IDLE_TIMEOUT_MS;
    this.windowSize = options.windowSize ?? DEFAULT_WINDOW_SIZE;
  }

  setEnabled(enabled: boolean): void {
    this.enabled = enabled;
    if (!enabled) {
      this.hide();
    }
  }

  isEnabled(): boolean {
    return this.enabled;
  }

  show(event: CursorOverlayEvent): void {
    if (!this.enabled || this.failedToCreate) return;

    try {
      const cursor = nativeCursorToElectronPoint(this.options.getCursorPosition(), screen.getAllDisplays());
      const window = this.ensureWindow();
      if (!window) return;

      window.setBounds(cursorOverlayBounds(cursor, this.windowSize), false);
      void window.webContents.executeJavaScript(createOverlayEventScript(event)).catch(() => {});
      window.showInactive();
      this.resetHideTimer();
    } catch {
      this.failedToCreate = true;
      this.hide();
    }
  }

  hide(): void {
    this.clearHideTimer();
    if (this.window && !this.window.isDestroyed()) {
      this.window.hide();
    }
  }

  destroy(): void {
    this.clearHideTimer();
    if (this.window && !this.window.isDestroyed()) {
      this.window.destroy();
    }
    this.window = null;
  }

  private ensureWindow(): BrowserWindow | null {
    if (this.window && !this.window.isDestroyed()) {
      return this.window;
    }

    const window = new BrowserWindow({
      width: this.windowSize,
      height: this.windowSize,
      frame: false,
      transparent: true,
      resizable: false,
      movable: false,
      show: false,
      skipTaskbar: true,
      focusable: false,
      alwaysOnTop: true,
      hasShadow: false,
      backgroundColor: '#00000000',
      webPreferences: {
        contextIsolation: true,
        nodeIntegration: false,
        sandbox: false
      }
    });

    window.setIgnoreMouseEvents(true);
    window.setAlwaysOnTop(true, 'screen-saver');
    try {
      window.setVisibleOnAllWorkspaces(true, { visibleOnFullScreen: true });
    } catch {
      // Best-effort only; Windows utility behavior still works with always-on-top.
    }
    void window.loadURL(createOverlayDataUrl());
    this.window = window;
    return window;
  }

  private resetHideTimer(): void {
    this.clearHideTimer();
    this.hideTimer = setTimeout(() => {
      this.hide();
    }, this.idleTimeoutMs);
  }

  private clearHideTimer(): void {
    if (this.hideTimer) {
      clearTimeout(this.hideTimer);
      this.hideTimer = null;
    }
  }
}

function createOverlayDataUrl(): string {
  return `data:text/html;charset=utf-8,${encodeURIComponent(createOverlayHtml())}`;
}

function createOverlayEventScript(event: CursorOverlayEvent): string {
  return `
    document.body.className = ${JSON.stringify(event === 'click' ? 'click' : 'move')};
    if (${JSON.stringify(event)} === 'click') {
      window.setTimeout(() => {
        if (document.body.className === 'click') {
          document.body.className = 'move';
        }
      }, 190);
    }
  `;
}

function createOverlayHtml(): string {
  return `<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <style>
      html,
      body {
        width: 100%;
        height: 100%;
        margin: 0;
        overflow: hidden;
        background: transparent;
      }

      body {
        display: grid;
        place-items: center;
      }

      .ring {
        width: 42px;
        height: 42px;
        border: 3px solid rgba(62, 138, 68, 0.95);
        border-radius: 999px;
        box-shadow:
          0 0 0 6px rgba(62, 138, 68, 0.18),
          0 0 24px rgba(62, 138, 68, 0.35);
        opacity: 0.95;
        transform: scale(1);
        transition:
          opacity 130ms ease,
          transform 130ms ease;
      }

      body.click .ring {
        animation: click-pulse 180ms ease-out;
      }

      @keyframes click-pulse {
        0% {
          transform: scale(0.82);
          box-shadow:
            0 0 0 2px rgba(62, 138, 68, 0.3),
            0 0 14px rgba(62, 138, 68, 0.5);
        }
        100% {
          transform: scale(1.18);
          box-shadow:
            0 0 0 10px rgba(62, 138, 68, 0.08),
            0 0 28px rgba(62, 138, 68, 0.24);
        }
      }
    </style>
  </head>
  <body>
    <div class="ring"></div>
  </body>
</html>`;
}
