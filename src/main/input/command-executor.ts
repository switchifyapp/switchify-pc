import type { CommandRequest, MouseButton } from '../../shared/protocol';
import {
  MAX_POINTER_DELTA,
  MAX_SCROLL_DELTA,
  MAX_SHORTCUT_KEYS,
  MAX_TEXT_LENGTH
} from '../../shared/protocol';
import type { DesktopInputAdapter } from './desktop-input-adapter';
import { DesktopInputError } from './desktop-input-adapter';

export type CommandExecutionResult =
  | { ok: true }
  | { ok: false; code: 'unsupported_command' | 'unsafe_payload' | 'adapter_failure'; message: string };

export type CursorOverlayNotifier = {
  show(event: 'move' | 'click'): void;
  hide?(): void;
  markControlActive?(): void;
};

export class DesktopCommandExecutor {
  private pointerActionQueue: Promise<void> = Promise.resolve();
  private activeDragButton: MouseButton | null = null;

  constructor(
    private readonly adapter: DesktopInputAdapter,
    private readonly cursorOverlay?: CursorOverlayNotifier
  ) {}

  async execute(command: CommandRequest): Promise<CommandExecutionResult> {
    if (isPointerAction(command)) {
      return this.enqueuePointerAction(command);
    }

    return this.executeNow(command);
  }

  async releaseHeldMouseButtons(): Promise<void> {
    const release = this.pointerActionQueue.then(async () => {
      if (!this.activeDragButton) return;
      const button = this.activeDragButton;
      await this.adapter.setMouseButtonDown(button, false);
      this.activeDragButton = null;
    });
    this.pointerActionQueue = release.then(
      () => undefined,
      () => undefined
    );
    await release;
  }

  private async enqueuePointerAction(
    command: CommandRequest & { type: 'mouse.move' | 'mouse.dragStart' | 'mouse.dragEnd' }
  ): Promise<CommandExecutionResult> {
    const result = this.pointerActionQueue.then(() => this.executeNow(command));
    this.pointerActionQueue = result.then(
      () => undefined,
      () => undefined
    );
    return result;
  }

  private async executeNow(command: CommandRequest): Promise<CommandExecutionResult> {
    try {
      if (isMouseCommand(command)) {
        this.cursorOverlay?.markControlActive?.();
      } else {
        this.cursorOverlay?.hide?.();
      }
      switch (command.type) {
        case 'mouse.move':
          assertBoundedNumber(command.payload.dx, MAX_POINTER_DELTA, 'dx');
          assertBoundedNumber(command.payload.dy, MAX_POINTER_DELTA, 'dy');
          await this.adapter.moveMouseBy(command.payload);
          this.cursorOverlay?.show('move');
          return { ok: true };
        case 'mouse.dragStart':
          await this.startDrag(command.payload.button);
          this.cursorOverlay?.show('move');
          return { ok: true };
        case 'mouse.dragEnd':
          await this.endDrag(command.payload.button);
          this.cursorOverlay?.show('move');
          return { ok: true };
        case 'mouse.click':
          await this.adapter.clickMouse(command.payload.button);
          this.cursorOverlay?.show('click');
          return { ok: true };
        case 'mouse.doubleClick':
          await this.adapter.doubleClickMouse(command.payload.button);
          this.cursorOverlay?.show('click');
          return { ok: true };
        case 'mouse.rightClick':
          await this.adapter.clickMouse('right');
          this.cursorOverlay?.show('click');
          return { ok: true };
        case 'mouse.scroll':
          assertBoundedNumber(command.payload.dx, MAX_SCROLL_DELTA, 'dx');
          assertBoundedNumber(command.payload.dy, MAX_SCROLL_DELTA, 'dy');
          await this.adapter.scrollMouse(command.payload);
          return { ok: true };
        case 'keyboard.key':
          await this.adapter.pressKey(command.payload.key);
          return { ok: true };
        case 'keyboard.shortcut':
          if (command.payload.keys.length === 0 || command.payload.keys.length > MAX_SHORTCUT_KEYS) {
            return unsafe('Shortcut key count is invalid.');
          }
          await this.adapter.pressShortcut(command.payload.keys);
          return { ok: true };
        case 'keyboard.typeText':
          if (command.payload.text.length > MAX_TEXT_LENGTH) {
            return unsafe('Text payload is too long.');
          }
          await this.adapter.typeText(command.payload.text);
          return { ok: true };
        case 'media.control':
          await this.adapter.mediaControl(command.payload.action);
          return { ok: true };
        case 'window.control':
          await this.adapter.controlWindow(command.payload.action);
          return { ok: true };
        case 'connection.ping':
          return { ok: true };
        case 'connection.disconnecting':
          return { ok: false, code: 'unsupported_command', message: 'Disconnect intent must be handled by the server.' };
        case 'pointer.profile':
          return { ok: false, code: 'unsupported_command', message: 'Pointer profile must be handled by the server.' };
      }
    } catch (error) {
      if (error instanceof DesktopInputError) {
        return { ok: false, code: error.code, message: error.message };
      }
      return {
        ok: false,
        code: 'adapter_failure',
        message: error instanceof Error ? error.message : 'Desktop input failed.'
      };
    }
  }

  private async startDrag(button: MouseButton): Promise<void> {
    if (this.activeDragButton === button) return;

    if (this.activeDragButton) {
      const previousButton = this.activeDragButton;
      await this.adapter.setMouseButtonDown(previousButton, false);
      this.activeDragButton = null;
    }

    await this.adapter.setMouseButtonDown(button, true);
    this.activeDragButton = button;
  }

  private async endDrag(_button: MouseButton): Promise<void> {
    if (!this.activeDragButton) return;

    const buttonToRelease = this.activeDragButton;
    await this.adapter.setMouseButtonDown(buttonToRelease, false);
    this.activeDragButton = null;
  }
}

function assertBoundedNumber(value: number, maxAbsValue: number, label: string): void {
  if (!Number.isFinite(value) || Math.abs(value) > maxAbsValue) {
    throw new DesktopInputError('unsafe_payload', `${label} is outside allowed bounds.`);
  }
}

function unsafe(message: string): CommandExecutionResult {
  return { ok: false, code: 'unsafe_payload', message };
}

function isPointerAction(
  command: CommandRequest
): command is CommandRequest & { type: 'mouse.move' | 'mouse.dragStart' | 'mouse.dragEnd' } {
  return command.type === 'mouse.move' || command.type === 'mouse.dragStart' || command.type === 'mouse.dragEnd';
}

function isMouseCommand(command: CommandRequest): boolean {
  return (
    command.type === 'mouse.move' ||
    command.type === 'mouse.click' ||
    command.type === 'mouse.doubleClick' ||
    command.type === 'mouse.rightClick' ||
    command.type === 'mouse.scroll' ||
    command.type === 'mouse.dragStart' ||
    command.type === 'mouse.dragEnd'
  );
}
