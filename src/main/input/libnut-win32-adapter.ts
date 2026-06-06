import {
  getMousePos,
  keyTap,
  mouseClick,
  moveMouse,
  scrollMouse as nativeScrollMouse,
  typeString
} from '@nut-tree-fork/libnut-win32';
import type { KeyboardKey, MediaAction, MouseButton, ShortcutKey } from '../../shared/protocol';
import type { DesktopInputAdapter } from './desktop-input-adapter';
import { DesktopInputError } from './desktop-input-adapter';

export class LibnutWin32InputAdapter implements DesktopInputAdapter {
  getMousePosition(): { x: number; y: number } {
    const current = getMousePos();
    return { x: current.x, y: current.y };
  }

  async moveMouseBy(delta: { dx: number; dy: number }): Promise<void> {
    const current = this.getMousePosition();
    moveMouse(current.x + delta.dx, current.y + delta.dy);
  }

  async clickMouse(button: MouseButton): Promise<void> {
    mouseClick(toLibnutMouseButton(button), false);
  }

  async doubleClickMouse(button: MouseButton): Promise<void> {
    mouseClick(toLibnutMouseButton(button), true);
  }

  async scrollMouse(delta: { dx: number; dy: number }): Promise<void> {
    nativeScrollMouse(delta.dx, delta.dy);
  }

  async pressKey(key: KeyboardKey): Promise<void> {
    keyTap(toLibnutKey(key));
  }

  async pressShortcut(keys: ShortcutKey[]): Promise<void> {
    if (keys.length === 1) {
      keyTap(toLibnutKey(keys[0]));
      return;
    }

    const [key, ...modifiers] = [...keys].reverse();
    keyTap(toLibnutKey(key), modifiers.map(toLibnutKey));
  }

  async typeText(text: string): Promise<void> {
    typeString(text);
  }

  async mediaControl(action: MediaAction): Promise<void> {
    keyTap(toLibnutMediaKey(action));
  }
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

function toLibnutKey(key: KeyboardKey | ShortcutKey): string {
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
    case 'Ctrl':
      return 'control';
    case 'Alt':
      return 'alt';
    case 'Shift':
      return 'shift';
    case 'Meta':
      return 'command';
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
