import { describe, expect, it } from 'vitest';
import {
  CLIPBOARD_RESTORE_DELAY_MS,
  insertText,
  shouldUseClipboardPaste,
  type TextInsertionDeps
} from './text-inserter';

type ScheduledRestore = {
  callback: () => void;
  delayMs: number;
};

function createDeps(initialClipboard = 'previous'): {
  deps: TextInsertionDeps;
  calls: string[];
  scheduled: ScheduledRestore[];
  getClipboardText: () => string;
  setClipboardReadFailure: () => void;
  setClipboardWriteFailure: () => void;
} {
  const calls: string[] = [];
  const scheduled: ScheduledRestore[] = [];
  let clipboardText = initialClipboard;
  let failRead = false;
  let failWrite = false;

  return {
    deps: {
      clipboard: {
        readText: () => {
          calls.push('readText');
          if (failRead) {
            throw new Error('Clipboard read failed.');
          }
          return clipboardText;
        },
        writeText: (text) => {
          calls.push(`writeText:${text}`);
          if (failWrite) {
            throw new Error('Clipboard write failed.');
          }
          clipboardText = text;
        }
      },
      typeString: (text) => {
        calls.push(`typeString:${text}`);
      },
      pasteFromClipboard: () => {
        calls.push('pasteFromClipboard');
      },
      scheduleRestore: (callback, delayMs) => {
        calls.push(`scheduleRestore:${delayMs}`);
        scheduled.push({ callback, delayMs });
      }
    },
    calls,
    scheduled,
    getClipboardText: () => clipboardText,
    setClipboardReadFailure: () => {
      failRead = true;
    },
    setClipboardWriteFailure: () => {
      failWrite = true;
    }
  };
}

describe('insertText', () => {
  it('uses native typing for simple lowercase text', () => {
    const { deps, calls, scheduled, getClipboardText } = createDeps();

    expect(insertText('hello', deps)).toBe('native');
    expect(calls).toEqual(['typeString:hello']);
    expect(scheduled).toEqual([]);
    expect(getClipboardText()).toBe('previous');
  });

  it('uses native typing for simple lowercase letters, digits, and spaces', () => {
    const { deps, calls } = createDeps();

    expect(insertText('abc 123', deps)).toBe('native');
    expect(calls).toEqual(['typeString:abc 123']);
  });

  it('returns none for empty text without side effects', () => {
    const { deps, calls, scheduled, getClipboardText } = createDeps();

    expect(insertText('', deps)).toBe('none');
    expect(calls).toEqual([]);
    expect(scheduled).toEqual([]);
    expect(getClipboardText()).toBe('previous');
  });

  it.each(['Hello', 'Hello, world!', 'café', '👋', 'line one\nline two'])(
    'uses clipboard paste fallback for %j',
    (text) => {
      const { deps, calls, scheduled, getClipboardText } = createDeps();

      expect(insertText(text, deps)).toBe('clipboard');
      expect(calls).toEqual([
        'readText',
        `writeText:${text}`,
        'pasteFromClipboard',
        `scheduleRestore:${CLIPBOARD_RESTORE_DELAY_MS}`
      ]);
      expect(scheduled).toHaveLength(1);
      expect(scheduled[0].delayMs).toBe(CLIPBOARD_RESTORE_DELAY_MS);
      expect(getClipboardText()).toBe(text);
    }
  );

  it('restores previous text if clipboard still contains inserted text', () => {
    const { deps, scheduled, getClipboardText } = createDeps('saved text');

    insertText('Hello', deps);
    scheduled[0].callback();

    expect(getClipboardText()).toBe('saved text');
  });

  it('does not restore if clipboard changed after paste', () => {
    const { deps, scheduled, getClipboardText } = createDeps('saved text');

    insertText('Hello', deps);
    deps.clipboard.writeText('user changed clipboard');
    scheduled[0].callback();

    expect(getClipboardText()).toBe('user changed clipboard');
  });

  it('does not clear clipboard if previous text was empty', () => {
    const { deps, scheduled, getClipboardText } = createDeps('');

    insertText('Hello', deps);
    scheduled[0].callback();

    expect(getClipboardText()).toBe('Hello');
  });

  it('swallows restore errors', () => {
    const { deps, scheduled, setClipboardReadFailure } = createDeps('saved text');

    insertText('Hello', deps);
    setClipboardReadFailure();

    expect(() => scheduled[0].callback()).not.toThrow();
  });

  it('throws main-path clipboard write failures', () => {
    const { deps, setClipboardWriteFailure } = createDeps();
    setClipboardWriteFailure();

    expect(() => insertText('Hello', deps)).toThrow('Clipboard write failed.');
  });
});

describe('shouldUseClipboardPaste', () => {
  it('keeps lowercase ASCII letters, digits, and spaces on native typing', () => {
    expect(shouldUseClipboardPaste('abc 123')).toBe(false);
  });

  it.each([
    ['uppercase', 'Hello'],
    ['punctuation', 'hello!'],
    ['unicode', 'café'],
    ['newline', 'hello\nworld'],
    ['long text', 'a'.repeat(81)]
  ])('uses clipboard paste for %s', (_label, text) => {
    expect(shouldUseClipboardPaste(text)).toBe(true);
  });
});
