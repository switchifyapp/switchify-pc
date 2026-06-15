import { app, BrowserWindow, screen } from 'electron';
import { join } from 'node:path';
import {
  NativeWindowsCursorOverlayBackend,
  nativeHelperCursorPosition,
  type CursorOverlayBackend,
  type CursorOverlayEvent,
  type CursorOverlayRenderOptions
} from './cursor-overlay-helper-client';
import { cursorOverlayBounds } from './cursor-overlay-state';
import {
  DEFAULT_CURSOR_OVERLAY_SETTINGS,
  normalizeCursorOverlaySettings,
  resolveCursorOverlayColorRgb,
  resolveCursorOverlaySizePixels,
  type CursorOverlaySettings
} from '../shared/cursor-overlay-settings';

export type { CursorOverlayEvent };

export type CursorOverlayOptions = {
  idleTimeoutMs?: number;
  settings?: CursorOverlaySettings;
  followIntervalMs?: number;
};

const DEFAULT_IDLE_TIMEOUT_MS = 900;
const DEFAULT_FOLLOW_INTERVAL_MS = 75;

export class CursorOverlay {
  private readonly backend: CursorOverlayBackend;
  private readonly idleTimeoutMs: number;
  private readonly followIntervalMs: number;
  private settings: CursorOverlaySettings;
  private followTimer: NodeJS.Timeout | null = null;

  constructor(private readonly options: CursorOverlayOptions) {
    this.idleTimeoutMs = options.idleTimeoutMs ?? DEFAULT_IDLE_TIMEOUT_MS;
    this.followIntervalMs = options.followIntervalMs ?? DEFAULT_FOLLOW_INTERVAL_MS;
    this.settings = normalizeCursorOverlaySettings(options.settings ?? DEFAULT_CURSOR_OVERLAY_SETTINGS);
    const electronBackend = new ElectronCursorOverlayBackend({ idleTimeoutMs: this.idleTimeoutMs });
    this.backend =
      process.platform === 'win32' && app.isPackaged
        ? new NativeWindowsCursorOverlayBackend({
            helperPath: resolveNativeOverlayHelperPath(),
            fallback: electronBackend,
            getCursorPosition: () => nativeHelperCursorPosition(screen),
            getSettings: () => this.getSettings(),
            resolveSizePixels: () => this.resolveSizePixels(),
            idleTimeoutMs: this.idleTimeoutMs,
            onFailure: (message) => console.error('Switchify cursor overlay helper failed.', message)
          })
        : electronBackend;
  }

  setEnabled(enabled: boolean): void {
    this.setSettings({ ...this.settings, enabled });
  }

  isEnabled(): boolean {
    return this.settings.enabled;
  }

  getSettings(): CursorOverlaySettings {
    return { ...this.settings };
  }

  setSettings(settings: CursorOverlaySettings): void {
    const previous = this.settings;
    this.settings = normalizeCursorOverlaySettings(settings);
    if (!this.settings.enabled || this.settings.visibility !== 'whileControlling') {
      this.stopFollowing();
    }
    if (!this.settings.enabled) {
      this.hide();
      return;
    }
    if (
      previous.size !== this.settings.size ||
      previous.crosshairs !== this.settings.crosshairs ||
      previous.color !== this.settings.color
    ) {
      this.refreshPersistentOverlay();
    }
  }

  markControlActive(): void {
    if (!this.settings.enabled || this.settings.visibility !== 'whileControlling') return;
    this.startFollowing();
  }

  show(event: CursorOverlayEvent): void {
    if (!this.settings.enabled) return;
    if (this.settings.visibility === 'whileControlling') {
      this.startFollowing();
    }
    this.backend.show(event, this.renderOptions());
  }

  hide(): void {
    this.stopFollowing();
    this.backend.hide();
  }

  endControlSession(): void {
    this.hide();
  }

  destroy(): void {
    this.stopFollowing();
    this.backend.destroy();
  }

  private startFollowing(): void {
    this.refreshPersistentOverlay();
    if (this.followTimer) return;
    this.followTimer = setInterval(() => {
      if (!this.settings.enabled || this.settings.visibility !== 'whileControlling') {
        this.hide();
        return;
      }
      this.backend.show('move', this.renderOptions());
    }, this.followIntervalMs);
    this.followTimer.unref?.();
  }

  private stopFollowing(): void {
    if (this.followTimer) {
      clearInterval(this.followTimer);
      this.followTimer = null;
    }
  }

  private refreshPersistentOverlay(): void {
    if (!this.settings.enabled || this.settings.visibility !== 'whileControlling') return;
    this.backend.show('move', this.renderOptions());
  }

  private renderOptions(): CursorOverlayRenderOptions {
    return {
      size: this.resolveSizePixels(),
      idleTimeoutMs: this.idleTimeoutMs,
      crosshairs: this.settings.crosshairs,
      persistent: this.settings.visibility === 'whileControlling',
      colorRgb: resolveCursorOverlayColorRgb(this.settings.color)
    };
  }

  private resolveSizePixels(): number {
    return resolveCursorOverlaySizePixels(this.settings.size);
  }
}

type ElectronCursorOverlayBackendOptions = {
  idleTimeoutMs: number;
};

class ElectronCursorOverlayBackend implements CursorOverlayBackend {
  private window: BrowserWindow | null = null;
  private windowReady: Promise<void> | null = null;
  private hideTimer: NodeJS.Timeout | null = null;
  private failedToCreate = false;
  private readonly idleTimeoutMs: number;

  constructor(options: ElectronCursorOverlayBackendOptions) {
    this.idleTimeoutMs = options.idleTimeoutMs;
  }

  show(event: CursorOverlayEvent, options: CursorOverlayRenderOptions): void {
    if (this.failedToCreate) return;

    void this.showWhenReady(event, options);
  }

  private async showWhenReady(event: CursorOverlayEvent, options: CursorOverlayRenderOptions): Promise<void> {
    try {
      const cursor = screen.getCursorScreenPoint();
      const display = screen.getDisplayNearestPoint(cursor);
      const bounds = options.crosshairs ? display.bounds : cursorOverlayBounds(cursor, options.size);
      const window = this.ensureWindow();
      if (!window) return;

      await this.windowReady;
      if (window.isDestroyed()) return;

      window.setBounds(bounds, false);
      await window.webContents.executeJavaScript(
        createOverlayEventScript(event, {
          centerX: cursor.x - bounds.x,
          centerY: cursor.y - bounds.y,
          crosshairs: options.crosshairs,
          colorRgb: options.colorRgb,
          size: options.size
        })
      );
      this.showOverlayWindow(window);
      if (options.persistent) {
        this.clearHideTimer();
      } else {
        this.resetHideTimer(options.idleTimeoutMs);
      }
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

    const window = new BrowserWindow({
      width: 128,
      height: 128,
      frame: false,
      transparent: true,
      resizable: false,
      movable: false,
      show: false,
      paintWhenInitiallyHidden: true,
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
    this.windowReady = window.loadURL(createOverlayDataUrl()).then(() => undefined);
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

  private resetHideTimer(idleTimeoutMs = this.idleTimeoutMs): void {
    this.clearHideTimer();
    this.hideTimer = setTimeout(() => {
      this.hide();
    }, idleTimeoutMs);
  }

  private clearHideTimer(): void {
    if (this.hideTimer) {
      clearTimeout(this.hideTimer);
      this.hideTimer = null;
    }
  }
}

function resolveNativeOverlayHelperPath(): string {
  return app.isPackaged
    ? join(process.resourcesPath, 'native', 'SwitchifyCursorOverlay.exe')
    : join(process.cwd(), 'build', 'native', 'cursor-overlay-helper', 'win-x64', 'SwitchifyCursorOverlay.exe');
}

function createOverlayDataUrl(): string {
  return `data:text/html;charset=utf-8,${encodeURIComponent(createOverlayHtml())}`;
}

function createOverlayEventScript(
  event: CursorOverlayEvent,
  options: {
    centerX: number;
    centerY: number;
    crosshairs: boolean;
    colorRgb: [number, number, number];
    size: number;
  }
): string {
  return `
    document.body.className = ${JSON.stringify(event === 'click' ? 'click' : 'move')};
    document.body.classList.toggle('crosshairs-enabled', ${JSON.stringify(options.crosshairs)});
    document.documentElement.style.setProperty('--overlay-rgb', '${options.colorRgb.join(', ')}');
    document.documentElement.style.setProperty('--center-x', '${Math.round(options.centerX)}px');
    document.documentElement.style.setProperty('--center-y', '${Math.round(options.centerY)}px');
    document.documentElement.style.setProperty('--ring-size', '${Math.round(options.size * 0.5625)}px');
    document.documentElement.style.setProperty('--ring-stroke', '${Math.max(4, Math.round(options.size * 0.039))}px');
    document.documentElement.style.setProperty('--glow-stroke', '${Math.max(18, Math.round(options.size * 0.1875))}px');
    if (${JSON.stringify(event)} === 'click') {
      window.setTimeout(() => {
        if (document.body.classList.contains('click')) {
          document.body.classList.remove('click');
          document.body.classList.add('move');
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
        border-radius: 999px;
      }

      body.crosshairs-enabled {
        border-radius: 0;
      }

      .ring {
        position: absolute;
        left: var(--center-x, 64px);
        top: var(--center-y, 64px);
        width: var(--ring-size, 72px);
        height: var(--ring-size, 72px);
        border: var(--ring-stroke, 5px) solid rgba(var(--overlay-rgb, 211, 47, 47), 0.98);
        border-radius: 999px;
        box-shadow:
          0 0 0 calc(var(--glow-stroke, 24px) * 0.42) rgba(var(--overlay-rgb, 211, 47, 47), 0.22),
          0 0 38px rgba(var(--overlay-rgb, 211, 47, 47), 0.48);
        opacity: 0.95;
        transform: translate(-50%, -50%) scale(1);
        transition:
          opacity 130ms ease,
          transform 130ms ease;
      }

      .crosshair {
        position: absolute;
        display: none;
        pointer-events: none;
        background: rgba(var(--overlay-rgb, 211, 47, 47), 0.72);
        box-shadow: 0 0 14px rgba(var(--overlay-rgb, 211, 47, 47), 0.35);
      }

      body.crosshairs-enabled .crosshair {
        display: block;
      }

      .crosshair-horizontal {
        left: 0;
        top: var(--center-y, 64px);
        width: 100%;
        height: 2px;
        transform: translateY(-1px);
      }

      .crosshair-vertical {
        left: var(--center-x, 64px);
        top: 0;
        width: 2px;
        height: 100%;
        transform: translateX(-1px);
      }

      body.click .ring {
        animation: click-pulse 180ms ease-out;
      }

      @keyframes click-pulse {
        0% {
          transform: translate(-50%, -50%) scale(0.82);
          box-shadow:
            0 0 0 4px rgba(var(--overlay-rgb, 211, 47, 47), 0.3),
            0 0 24px rgba(var(--overlay-rgb, 211, 47, 47), 0.5);
        }
        100% {
          transform: translate(-50%, -50%) scale(1.18);
          box-shadow:
            0 0 0 15px rgba(var(--overlay-rgb, 211, 47, 47), 0.08),
            0 0 38px rgba(var(--overlay-rgb, 211, 47, 47), 0.24);
        }
      }
    </style>
  </head>
  <body>
    <div class="crosshair crosshair-horizontal"></div>
    <div class="crosshair crosshair-vertical"></div>
    <div class="ring"></div>
  </body>
</html>`;
}
