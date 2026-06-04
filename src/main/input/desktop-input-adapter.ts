import type { KeyboardKey, MediaAction, MouseButton, ShortcutKey } from '../../shared/protocol';

export type DesktopInputAdapter = {
  moveMouseBy(delta: { dx: number; dy: number }): Promise<void>;
  clickMouse(button: MouseButton): Promise<void>;
  doubleClickMouse(button: MouseButton): Promise<void>;
  scrollMouse(delta: { dx: number; dy: number }): Promise<void>;
  pressKey(key: KeyboardKey): Promise<void>;
  pressShortcut(keys: ShortcutKey[]): Promise<void>;
  typeText(text: string): Promise<void>;
  mediaControl(action: MediaAction): Promise<void>;
};

export type DesktopInputErrorCode = 'unsupported_command' | 'unsafe_payload' | 'adapter_failure';

export class DesktopInputError extends Error {
  constructor(
    public readonly code: DesktopInputErrorCode,
    message: string,
    options?: ErrorOptions
  ) {
    super(message, options);
    this.name = 'DesktopInputError';
  }
}
