import { describe, expect, it } from 'vitest';
import {
  CLIPBOARD_RESTORE_DELAY_MS,
  CLIPBOARD_WRITE_VERIFY_DELAY_MS,
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
  setUnicodeFailure: () => void;
  setReadAfterWriteOverrides: (values: string[]) => void;
} {
  const calls: string[] = [];
  const scheduled: ScheduledRestore[] = [];
  let clipboardText = initialClipboard;
  let failRead = false;
  let failWrite = false;
  let failUnicode = false;
  let justWrote = false;
  let readAfterWriteOverrides: string[] = [];

  return {
    deps: {
      clipboard: {
        readText: () => {
          calls.push('readText');
          if (failRead) {
            throw new Error('Clipboard read failed.');
          }
          if (justWrote && readAfterWriteOverrides.length > 0) {
            justWrote = false;
            return readAfterWriteOverrides.shift() ?? clipboardText;
          }
          justWrote = false;
          return clipboardText;
        },
        writeText: (text) => {
          calls.push(`writeText:${text}`);
          if (failWrite) {
            throw new Error('Clipboard write failed.');
          }
          clipboardText = text;
          justWrote = true;
        }
      },
      typeString: (text) => {
        calls.push(`typeString:${text}`);
      },
      typeUnicodeText: async (text) => {
        calls.push(`typeUnicodeText:${text}`);
        if (failUnicode) {
          throw new Error('Unicode input failed.');
        }
      },
      pasteFromClipboard: () => {
        calls.push('pasteFromClipboard');
      },
      scheduleRestore: (callback, delayMs) => {
        calls.push(`scheduleRestore:${delayMs}`);
        scheduled.push({ callback, delayMs });
      },
      wait: async (delayMs) => {
        calls.push(`wait:${delayMs}`);
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
    },
    setUnicodeFailure: () => {
      failUnicode = true;
    },
    setReadAfterWriteOverrides: (values) => {
      readAfterWriteOverrides = [...values];
    }
  };
}

describe('insertText', () => {
  it('uses native typing for simple lowercase text', async () => {
    const { deps, calls, scheduled, getClipboardText } = createDeps();

    await expect(insertText('hello', deps)).resolves.toBe('native');
    expect(calls).toEqual(['typeString:hello']);
    expect(scheduled).toEqual([]);
    expect(getClipboardText()).toBe('previous');
  });

  it('uses native typing for simple lowercase letters, digits, and spaces', async () => {
    const { deps, calls } = createDeps();

    await expect(insertText('abc 123', deps)).resolves.toBe('native');
    expect(calls).toEqual(['typeString:abc 123']);
  });

  it('returns none for empty text without side effects', async () => {
    const { deps, calls, scheduled, getClipboardText } = createDeps();

    await expect(insertText('', deps)).resolves.toBe('none');
    expect(calls).toEqual([]);
    expect(scheduled).toEqual([]);
    expect(getClipboardText()).toBe('previous');
  });

  it.each(['Hello', 'Hello, world!', 'cafe!', 'wave :)', 'line one\nline two'])(
    'uses Unicode text input for %j',
    async (text) => {
      const { deps, calls, scheduled, getClipboardText } = createDeps();

      await expect(insertText(text, deps)).resolves.toBe('unicode');
      expect(calls).toEqual([`typeUnicodeText:${text}`]);
      expect(scheduled).toHaveLength(0);
      expect(getClipboardText()).toBe('previous');
    }
  );

  it('uses verified clipboard fallback when Unicode text input fails', async () => {
    const { deps, calls, scheduled, getClipboardText, setUnicodeFailure } = createDeps();
    setUnicodeFailure();

    await expect(insertText('Hello', deps)).resolves.toBe('clipboard');

    expect(calls).toEqual([
      'typeUnicodeText:Hello',
      'readText',
      'writeText:Hello',
      'readText',
      'pasteFromClipboard',
      `scheduleRestore:${CLIPBOARD_RESTORE_DELAY_MS}`
    ]);
    expect(scheduled).toHaveLength(1);
    expect(scheduled[0].delayMs).toBe(CLIPBOARD_RESTORE_DELAY_MS);
    expect(getClipboardText()).toBe('Hello');
  });

  it('retries clipboard verification before pasting fallback text', async () => {
    const { deps, calls, setUnicodeFailure, setReadAfterWriteOverrides } = createDeps('saved text');
    setUnicodeFailure();
    setReadAfterWriteOverrides(['saved text']);

    await expect(insertText('Hello', deps)).resolves.toBe('clipboard');

    expect(calls).toEqual([
      'typeUnicodeText:Hello',
      'readText',
      'writeText:Hello',
      'readText',
      `wait:${CLIPBOARD_WRITE_VERIFY_DELAY_MS}`,
      'writeText:Hello',
      'readText',
      'pasteFromClipboard',
      `scheduleRestore:${CLIPBOARD_RESTORE_DELAY_MS}`
    ]);
  });

  it('throws without pasting if clipboard fallback cannot verify the write', async () => {
    const { deps, calls, getClipboardText, setUnicodeFailure, setReadAfterWriteOverrides } = createDeps('saved text');
    setUnicodeFailure();
    setReadAfterWriteOverrides(['saved text', 'saved text', 'saved text']);

    await expect(insertText('Hello', deps)).rejects.toThrow('Clipboard did not contain requested text after write.');

    expect(calls).not.toContain('pasteFromClipboard');
    expect(getClipboardText()).toBe('saved text');
  });

  it('restores previous text if clipboard still contains inserted fallback text', async () => {
    const { deps, scheduled, getClipboardText, setUnicodeFailure } = createDeps('saved text');
    setUnicodeFailure();

    await insertText('Hello', deps);
    scheduled[0].callback();

    expect(getClipboardText()).toBe('saved text');
  });

  it('does not restore if clipboard changed after fallback paste', async () => {
    const { deps, scheduled, getClipboardText, setUnicodeFailure } = createDeps('saved text');
    setUnicodeFailure();

    await insertText('Hello', deps);
    deps.clipboard.writeText('user changed clipboard');
    scheduled[0].callback();

    expect(getClipboardText()).toBe('user changed clipboard');
  });

  it('restores an empty previous clipboard value after fallback paste', async () => {
    const { deps, scheduled, getClipboardText, setUnicodeFailure } = createDeps('');
    setUnicodeFailure();

    await insertText('Hello', deps);
    scheduled[0].callback();

    expect(getClipboardText()).toBe('');
  });

  it('swallows restore errors', async () => {
    const { deps, scheduled, setClipboardReadFailure, setUnicodeFailure } = createDeps('saved text');
    setUnicodeFailure();

    await insertText('Hello', deps);
    setClipboardReadFailure();

    expect(() => scheduled[0].callback()).not.toThrow();
  });

  it('throws fallback clipboard write failures', async () => {
    const { deps, setClipboardWriteFailure, setUnicodeFailure } = createDeps();
    setUnicodeFailure();
    setClipboardWriteFailure();

    await expect(insertText('Hello', deps)).rejects.toThrow('Clipboard write failed.');
  });
});

describe('shouldUseClipboardPaste', () => {
  it('keeps lowercase ASCII letters, digits, and spaces on native typing', () => {
    expect(shouldUseClipboardPaste('abc 123')).toBe(false);
  });

  it.each([
    ['uppercase', 'Hello'],
    ['punctuation', 'hello!'],
    ['unicode', 'cafe!'],
    ['newline', 'hello\nworld'],
    ['long text', 'a'.repeat(81)]
  ])('uses non-simple path for %s', (_label, text) => {
    expect(shouldUseClipboardPaste(text)).toBe(true);
  });
});
