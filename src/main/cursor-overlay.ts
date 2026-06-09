import { app, BrowserWindow, screen } from 'electron';
import { cursorOverlayBounds } from './cursor-overlay-state';

export type CursorOverlayEvent = 'move' | 'click';

export type CursorOverlayOptions = {
  idleTimeoutMs?: number;
  windowSize?: number;
};

const DEFAULT_IDLE_TIMEOUT_MS = 900;
const DEFAULT_WINDOW_SIZE = 72;

export class CursorOverlay {
  private window: BrowserWindow | null = null;
  private windowReady: Promise<void> | null = null;
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

    void this.showWhenReady(event);
  }

  private async showWhenReady(event: CursorOverlayEvent): Promise<void> {
    try {
      const cursor = screen.getCursorScreenPoint();
      const window = this.ensureWindow();
      if (!window) return;

      await this.windowReady;
      if (!this.enabled || window.isDestroyed()) return;

      window.setBounds(cursorOverlayBounds(cursor, this.windowSize), false);
      await window.webContents.executeJavaScript(createOverlayEventScript(event));
      this.showOverlayWindow(window);
      this.resetHideTimer();
    } catch (error) {
      if (!this.window || this.window.isDestroyed()) {
        this.failedToCreate = true;
      }
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
    this.windowReady = null;
  }

  private ensureWindow(): BrowserWindow | null {
    if (this.window && !this.window.isDestroyed()) {
      return this.window;
    }

    const overlayMode = resolveOverlayMode();
    const usesTransparentWindow = overlayMode === 'transparent';
    const window = new BrowserWindow({
      width: this.windowSize,
      height: this.windowSize,
      frame: false,
      transparent: usesTransparentWindow,
      resizable: false,
      movable: false,
      show: false,
      paintWhenInitiallyHidden: true,
      skipTaskbar: true,
      focusable: false,
      alwaysOnTop: true,
      hasShadow: false,
      backgroundColor: usesTransparentWindow ? '#00000000' : '#123d1f',
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
    if (overlayMode === 'opaque') {
      window.setOpacity(0.78);
    }
    this.windowReady = window.loadURL(createOverlayDataUrl(overlayMode)).then(() => undefined);
    window.on('closed', () => {
      if (this.window === window) {
        this.window = null;
        this.windowReady = null;
      }
    });
    this.window = window;
    return window;
  }

  private showOverlayWindow(window: BrowserWindow): void {
    window.setAlwaysOnTop(true, 'screen-saver');
    if (process.platform === 'win32') {
      window.show();
    } else {
      window.showInactive();
    }
    window.moveTop();
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

type OverlayMode = 'transparent' | 'opaque';

function resolveOverlayMode(): OverlayMode {
  if (process.platform === 'win32' && app.isPackaged) {
    return 'opaque';
  }

  return 'transparent';
}

function createOverlayDataUrl(overlayMode: OverlayMode): string {
  return `data:text/html;charset=utf-8,${encodeURIComponent(createOverlayHtml(overlayMode))}`;
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

function createOverlayHtml(overlayMode: OverlayMode): string {
  const bodyBackground = overlayMode === 'transparent' ? 'transparent' : '#123d1f';
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
        background: ${bodyBackground};
      }

      body {
        display: grid;
        place-items: center;
        border-radius: 999px;
      }

      .ring {
        width: 34px;
        height: 34px;
        border: 3px solid rgba(132, 255, 145, 0.98);
        border-radius: 999px;
        box-shadow:
          0 0 0 5px rgba(132, 255, 145, 0.22),
          0 0 24px rgba(132, 255, 145, 0.48);
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
            0 0 0 2px rgba(132, 255, 145, 0.3),
            0 0 14px rgba(132, 255, 145, 0.5);
        }
        100% {
          transform: scale(1.18);
          box-shadow:
            0 0 0 8px rgba(132, 255, 145, 0.08),
            0 0 24px rgba(132, 255, 145, 0.24);
        }
      }
    </style>
  </head>
  <body>
    <div class="ring"></div>
  </body>
</html>`;
}
