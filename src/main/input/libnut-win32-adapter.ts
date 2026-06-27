import {
  getMousePos,
  keyTap,
  mouseClick,
  mouseToggle,
  moveMouse,
  scrollMouse as nativeScrollMouse,
  typeString
} from '@nut-tree-fork/libnut-win32';
import { clipboard } from 'electron';
import type { KeyboardKey, MediaAction, MouseButton, ShortcutKey, WindowControlAction } from '../../shared/protocol';
import {
  DEFAULT_POINTER_MOVEMENT_SETTINGS,
  normalizePointerMovementSettings,
  pointerMovementFractionFor,
  type PointerMovementSettings,
  type PointerMovementSizeKey
} from '../../shared/pointer-movement-settings';
import type { DesktopInputAdapter } from './desktop-input-adapter';
import { DesktopInputError } from './desktop-input-adapter';
import { insertText } from './text-inserter';
import { createTextInputBackend, type TextInputBackend } from './text-input-helper-client';
import { runWindowsWindowControlAction } from './windows-window-control';

type Point = { x: number; y: number };
type PointerDisplay = {
  bounds: { x: number; y: number; width: number; height: number };
  scaleFactor: number;
};
type PointerDisplayProvider = (position: Point) => PointerDisplay;

export const NATIVE_SCROLL_DELTA_MULTIPLIER = 8;
export const REFERENCE_POINTER_SHORT_EDGE = 1080;
const BASELINE_POINTER_DELTAS = {
  small: 48,
  medium: 128,
  large: 280
};
const SMALL_MEDIUM_POINTER_BOUNDARY = (BASELINE_POINTER_DELTAS.small + BASELINE_POINTER_DELTAS.medium) / 2;
const MEDIUM_LARGE_POINTER_BOUNDARY = (BASELINE_POINTER_DELTAS.medium + BASELINE_POINTER_DELTAS.large) / 2;

function defaultPointerDisplayProvider(): PointerDisplay {
  return {
    bounds: { x: 0, y: 0, width: REFERENCE_POINTER_SHORT_EDGE, height: REFERENCE_POINTER_SHORT_EDGE },
    scaleFactor: 1
  };
}

export class LibnutWin32InputAdapter implements DesktopInputAdapter {
  private pointerMovementSettings: PointerMovementSettings;

  constructor(
    private readonly getPointerDisplay: PointerDisplayProvider = defaultPointerDisplayProvider,
    private readonly textInputBackend: TextInputBackend = createTextInputBackend(),
    pointerMovementSettings: PointerMovementSettings = DEFAULT_POINTER_MOVEMENT_SETTINGS
  ) {
    this.pointerMovementSettings = normalizePointerMovementSettings(pointerMovementSettings);
  }

  setPointerMovementSettings(settings: PointerMovementSettings): void {
    this.pointerMovementSettings = normalizePointerMovementSettings(settings);
  }

  getMousePosition(): { x: number; y: number } {
    const current = getMousePos();
    return { x: current.x, y: current.y };
  }

  async moveMouseBy(delta: { dx: number; dy: number }): Promise<void> {
    const current = this.getMousePosition();
    const display = this.getPointerDisplay(current);
    const target = calculateDisplayNormalizedMouseTarget(current, delta, display, this.pointerMovementSettings);
    moveMouse(target.x, target.y);
  }

  async setMouseButtonDown(button: MouseButton, down: boolean): Promise<void> {
    const toggle = toLibnutMouseToggle(down);
    mouseToggle(toggle, toLibnutMouseButton(button));
  }

  async clickMouse(button: MouseButton): Promise<void> {
    mouseClick(toLibnutMouseButton(button), false);
  }

  async doubleClickMouse(button: MouseButton): Promise<void> {
    mouseClick(toLibnutMouseButton(button), true);
  }

  async scrollMouse(delta: { dx: number; dy: number }): Promise<void> {
    const nativeDelta = calculateNativeScrollDelta(delta);
    nativeScrollMouse(nativeDelta.dx, nativeDelta.dy);
  }

  async pressKey(key: KeyboardKey): Promise<void> {
    keyTap(toLibnutKeyboardKey(key));
  }

  async pressShortcut(keys: ShortcutKey[]): Promise<void> {
    if (keys.length === 1) {
      keyTap(toLibnutKeyboardKey(keys[0]));
      return;
    }

    const [key, ...modifiers] = [...keys].reverse();
    keyTap(toLibnutKeyboardKey(key), modifiers.map(toLibnutKeyboardKey));
  }

  async typeText(text: string): Promise<void> {
    try {
      await insertText(text, {
        clipboard,
        typeString,
        typeUnicodeText: (value) => this.textInputBackend.typeText(value),
        pasteFromClipboard: () => keyTap('v', ['control']),
        scheduleRestore: (callback, delayMs) => {
          setTimeout(callback, delayMs);
        },
        wait: (delayMs) => new Promise((resolve) => setTimeout(resolve, delayMs))
      });
    } catch (error) {
      throw new DesktopInputError('adapter_failure', 'Text insertion failed.', { cause: error });
    }
  }

  async typeCharacter(text: string): Promise<void> {
    try {
      await this.textInputBackend.typeText(text);
    } catch (error) {
      throw new DesktopInputError('adapter_failure', 'Character insertion failed.', { cause: error });
    }
  }

  async mediaControl(action: MediaAction): Promise<void> {
    keyTap(toLibnutMediaKey(action));
  }

  async controlWindow(action: WindowControlAction): Promise<void> {
    try {
      if (action === 'switchNext') {
        keyTap('tab', ['alt']);
        return;
      }

      if (action === 'switchPrevious') {
        keyTap('tab', ['alt', 'shift']);
        return;
      }

      await runWindowsWindowControlAction(action);
    } catch (error) {
      throw new DesktopInputError('adapter_failure', 'Window control failed.', { cause: error });
    }
  }
}

export function calculateScaledMouseTarget(
  current: Point,
  delta: { dx: number; dy: number },
  scale: number
): Point {
  const effectiveScale = Number.isFinite(scale) && scale > 0 ? scale : 1;
  return {
    x: Math.round(current.x + delta.dx * effectiveScale),
    y: Math.round(current.y + delta.dy * effectiveScale)
  };
}

export function calculateDisplayNormalizedMouseTarget(
  current: Point,
  delta: { dx: number; dy: number },
  display: PointerDisplay,
  movementSettings: PointerMovementSettings = DEFAULT_POINTER_MOVEMENT_SETTINGS
): Point {
  const scaleFactor = Number.isFinite(display.scaleFactor) && display.scaleFactor > 0 ? display.scaleFactor : 1;
  const shortEdge =
    Number.isFinite(display.bounds.width) && display.bounds.width > 0 &&
    Number.isFinite(display.bounds.height) && display.bounds.height > 0
      ? Math.min(display.bounds.width, display.bounds.height)
      : REFERENCE_POINTER_SHORT_EDGE;
  const settings = normalizePointerMovementSettings(movementSettings);
  const size = inferPointerMovementSize(delta);
  const baselineFraction = pointerMovementFractionFor(DEFAULT_POINTER_MOVEMENT_SETTINGS, size);
  const movementScale = pointerMovementFractionFor(settings, size) / baselineFraction;
  const multiplier = scaleFactor * (shortEdge / REFERENCE_POINTER_SHORT_EDGE) * movementScale;

  return {
    x: Math.round(current.x + delta.dx * multiplier),
    y: Math.round(current.y + delta.dy * multiplier)
  };
}

export function inferPointerMovementSize(delta: { dx: number; dy: number }): PointerMovementSizeKey {
  const magnitude = Math.max(Math.abs(delta.dx), Math.abs(delta.dy));
  if (magnitude <= SMALL_MEDIUM_POINTER_BOUNDARY) return 'small';
  if (magnitude <= MEDIUM_LARGE_POINTER_BOUNDARY) return 'medium';
  return 'large';
}

export function calculateNativeScrollDelta(
  delta: { dx: number; dy: number },
  multiplier = NATIVE_SCROLL_DELTA_MULTIPLIER
): { dx: number; dy: number } {
  const effectiveMultiplier = Number.isFinite(multiplier) && multiplier > 0 ? multiplier : 1;
  return {
    dx: scaleScrollAxis(delta.dx, effectiveMultiplier),
    dy: scaleScrollAxis(delta.dy, effectiveMultiplier)
  };
}

export function toLibnutMouseToggle(down: boolean): 'down' | 'up' {
  return down ? 'down' : 'up';
}

function scaleScrollAxis(value: number, multiplier: number): number {
  if (value === 0) return 0;

  const scaledValue = value * multiplier;
  const roundedValue = Math.round(Math.abs(scaledValue));
  return Math.sign(scaledValue) * Math.max(1, roundedValue);
}

function toLibnutMouseButton(button: MouseButton): string {
  switch (button) {
    case 'left':
      return 'left';
    case 'right':
      return 'right';
    case 'middle':
      return 'middle';
  }
}

export function toLibnutKeyboardKey(key: KeyboardKey | ShortcutKey): string {
  switch (key) {
    case 'Backspace':
      return 'backspace';
    case 'Delete':
      return 'delete';
    case 'Enter':
      return 'enter';
    case 'Escape':
      return 'escape';
    case 'Space':
      return 'space';
    case 'Tab':
      return 'tab';
    case 'ArrowUp':
      return 'up';
    case 'ArrowDown':
      return 'down';
    case 'ArrowLeft':
      return 'left';
    case 'ArrowRight':
      return 'right';
    case 'Home':
      return 'home';
    case 'End':
      return 'end';
    case 'PageUp':
      return 'pageup';
    case 'PageDown':
      return 'pagedown';
    case 'F1':
    case 'F2':
    case 'F3':
    case 'F4':
    case 'F5':
    case 'F6':
    case 'F7':
    case 'F8':
    case 'F9':
    case 'F10':
    case 'F11':
    case 'F12':
      return key.toLowerCase();
    case 'Ctrl':
      return 'control';
    case 'Alt':
      return 'alt';
    case 'Shift':
      return 'shift';
    case 'Meta':
      return 'win';
    case 'A':
    case 'C':
    case 'V':
    case 'X':
    case 'Y':
    case 'Z':
      return key.toLowerCase();
  }
}

function toLibnutMediaKey(action: MediaAction): string {
  switch (action) {
    case 'playPause':
      return 'audio_play';
    case 'nextTrack':
      return 'audio_next';
    case 'previousTrack':
      return 'audio_prev';
    case 'volumeUp':
      return 'audio_vol_up';
    case 'volumeDown':
      return 'audio_vol_down';
    case 'mute':
      return 'audio_mute';
    default:
      throw new DesktopInputError('unsupported_command', `Unsupported media action: ${action}`);
  }
}
