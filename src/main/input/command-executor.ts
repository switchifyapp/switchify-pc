import type { CommandRequest, MouseButton } from '../../shared/protocol';
import {
  MAX_POINTER_DELTA,
  MAX_SCROLL_DELTA,
  MAX_SHORTCUT_KEYS,
  MAX_TEXT_LENGTH,
  PROTOCOL_VERSION
} from '../../shared/protocol';
import type { DesktopInputAdapter } from './desktop-input-adapter';
import { DesktopInputError } from './desktop-input-adapter';

export type CommandExecutionResult =
  | { ok: true }
  | { ok: false; code: 'unsupported_command' | 'unsafe_payload' | 'adapter_failure'; message: string };

export type CursorOverlayNotifier = {
  show(event: 'move' | 'click' | 'drag'): void;
  hide?(): void;
  markControlActive?(): void;
  setDragActive?(active: boolean): void;
};

type TextInputStreamState = {
  deviceId: string;
  streamId: string;
  nextSeq: number;
  failed: boolean;
  errorMessage?: string;
  openedAtMs: number;
  updatedAtMs: number;
};

const TEXT_STREAM_TTL_MS = 60_000;
const TEXT_STREAM_CHUNK_CHARACTER_DELAY_MS = 35;

export class DesktopCommandExecutor {
  private pointerActionQueue: Promise<void> = Promise.resolve();
  private activeDragButton: MouseButton | null = null;
  private pendingRealtimeMove: { dx: number; dy: number } | null = null;
  private realtimeMoveFlush: Promise<CommandExecutionResult> | null = null;
  private readonly textInputStreams = new Map<string, TextInputStreamState>();

  constructor(
    private readonly adapter: DesktopInputAdapter,
    private readonly cursorOverlay?: CursorOverlayNotifier
  ) {}

  async execute(command: CommandRequest): Promise<CommandExecutionResult> {
    if (command.type === 'mouse.move' && command.responseMode === 'none') {
      return this.enqueueCoalescedMouseMove(command);
    }

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
      this.cursorOverlay?.setDragActive?.(false);
      this.cursorOverlay?.hide?.();
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

  private enqueueCoalescedMouseMove(command: CommandRequest & { type: 'mouse.move' }): Promise<CommandExecutionResult> {
    this.pendingRealtimeMove = addMouseDeltas(this.pendingRealtimeMove, command.payload);

    if (this.realtimeMoveFlush) {
      return Promise.resolve({ ok: true });
    }

    const result = this.pointerActionQueue.then(() => this.flushRealtimeMouseMoves());
    this.realtimeMoveFlush = result;
    this.pointerActionQueue = result.then(
      () => undefined,
      () => undefined
    );

    return result;
  }

  private async flushRealtimeMouseMoves(): Promise<CommandExecutionResult> {
    try {
      while (this.pendingRealtimeMove) {
        const payload = this.pendingRealtimeMove;
        this.pendingRealtimeMove = null;
        const result = await this.executeNow({
          version: PROTOCOL_VERSION,
          id: 'realtime-move',
          deviceId: 'realtime',
          timestamp: Date.now(),
          type: 'mouse.move',
          payload,
          auth: '',
          responseMode: 'none'
        });
        if (!result.ok) {
          return result;
        }
      }
      return { ok: true };
    } finally {
      this.realtimeMoveFlush = null;
      if (this.pendingRealtimeMove) {
        const result = this.pointerActionQueue.then(() => this.flushRealtimeMouseMoves());
        this.realtimeMoveFlush = result;
        this.pointerActionQueue = result.then(
          () => undefined,
          () => undefined
        );
      }
    }
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
          this.cursorOverlay?.show(this.activeDragButton ? 'drag' : 'move');
          return { ok: true };
        case 'mouse.dragStart':
          await this.startDrag(command.payload.button);
          this.cursorOverlay?.setDragActive?.(true);
          this.cursorOverlay?.show('drag');
          return { ok: true };
        case 'mouse.dragEnd':
          await this.endDrag(command.payload.button);
          this.cursorOverlay?.setDragActive?.(false);
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
        case 'keyboard.textStream.open':
          this.openTextInputStream(command.deviceId, command.payload.streamId);
          return { ok: true };
        case 'keyboard.textStream.char':
          return await this.executeTextStreamItem(
            command.deviceId,
            command.payload.streamId,
            command.payload.seq,
            () => this.adapter.typeCharacter(command.payload.text),
            'Text stream character insertion failed.'
          );
        case 'keyboard.textStream.chunk':
          return await this.executeTextStreamItem(
            command.deviceId,
            command.payload.streamId,
            command.payload.seq,
            () => this.typeTextStreamChunk(command.payload.text),
            'Text stream chunk insertion failed.'
          );
        case 'keyboard.textStream.key':
          return await this.executeTextStreamItem(
            command.deviceId,
            command.payload.streamId,
            command.payload.seq,
            () => this.adapter.pressKey(command.payload.key),
            'Text stream key insertion failed.'
          );
        case 'keyboard.textStream.close':
          return this.closeTextInputStream(command.deviceId, command.payload.streamId, command.payload.expectedCount);
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

  private openTextInputStream(deviceId: string, streamId: string): void {
    this.expireTextInputStreams();
    const now = Date.now();
    this.textInputStreams.set(textInputStreamKey(deviceId, streamId), {
      deviceId,
      streamId,
      nextSeq: 0,
      failed: false,
      openedAtMs: now,
      updatedAtMs: now
    });
  }

  private async executeTextStreamItem(
    deviceId: string,
    streamId: string,
    seq: number,
    execute: () => Promise<void>,
    failureMessage: string
  ): Promise<CommandExecutionResult> {
    const stream = this.textInputStreams.get(textInputStreamKey(deviceId, streamId));
    if (!stream) {
      return { ok: false, code: 'adapter_failure', message: 'Text stream is not open.' };
    }

    stream.updatedAtMs = Date.now();
    if (seq < stream.nextSeq) {
      return stream.failed
        ? { ok: false, code: 'adapter_failure', message: stream.errorMessage ?? 'Text stream failed.' }
        : { ok: true };
    }

    if (seq > stream.nextSeq) {
      stream.failed = true;
      stream.errorMessage = 'Text stream sequence mismatch.';
      stream.nextSeq = Math.max(stream.nextSeq, seq + 1);
      return { ok: false, code: 'adapter_failure', message: stream.errorMessage };
    }

    stream.nextSeq = seq + 1;
    if (stream.failed) {
      return { ok: false, code: 'adapter_failure', message: stream.errorMessage ?? 'Text stream failed.' };
    }

    try {
      await execute();
      return { ok: true };
    } catch {
      stream.failed = true;
      stream.errorMessage = failureMessage;
      return { ok: false, code: 'adapter_failure', message: failureMessage };
    }
  }

  private closeTextInputStream(
    deviceId: string,
    streamId: string,
    expectedCount: number
  ): CommandExecutionResult {
    this.expireTextInputStreams();
    const key = textInputStreamKey(deviceId, streamId);
    const stream = this.textInputStreams.get(key);
    if (!stream) {
      return { ok: false, code: 'adapter_failure', message: 'Text stream is not open.' };
    }

    this.textInputStreams.delete(key);
    if (expectedCount !== stream.nextSeq) {
      return { ok: false, code: 'adapter_failure', message: 'Text stream did not receive every item.' };
    }

    if (stream.failed) {
      return { ok: false, code: 'adapter_failure', message: stream.errorMessage ?? 'Text stream failed.' };
    }

    return { ok: true };
  }

  private async typeTextStreamChunk(text: string): Promise<void> {
    const characters = Array.from(text);
    for (const [index, character] of characters.entries()) {
      await this.adapter.typeCharacter(character);
      if (index < characters.length - 1) {
        await delay(TEXT_STREAM_CHUNK_CHARACTER_DELAY_MS);
      }
    }
  }

  private expireTextInputStreams(): void {
    const expiresBefore = Date.now() - TEXT_STREAM_TTL_MS;
    for (const [key, stream] of this.textInputStreams) {
      if (stream.updatedAtMs < expiresBefore) {
        this.textInputStreams.delete(key);
      }
    }
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

function textInputStreamKey(deviceId: string, streamId: string): string {
  return `${deviceId}:${streamId}`;
}

function delay(delayMs: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, delayMs));
}

function addMouseDeltas(
  current: { dx: number; dy: number } | null,
  next: { dx: number; dy: number }
): { dx: number; dy: number } {
  return {
    dx: clampDelta((current?.dx ?? 0) + next.dx),
    dy: clampDelta((current?.dy ?? 0) + next.dy)
  };
}

function clampDelta(value: number): number {
  return Math.max(-MAX_POINTER_DELTA, Math.min(MAX_POINTER_DELTA, value));
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
