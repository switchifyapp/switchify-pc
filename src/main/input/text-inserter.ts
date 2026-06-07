export const CLIPBOARD_RESTORE_DELAY_MS = 250;

export type TextClipboard = {
  readText(): string;
  writeText(text: string): void;
};

export type TextInsertionDeps = {
  clipboard: TextClipboard;
  typeString(text: string): void;
  pasteFromClipboard(): void;
  scheduleRestore(callback: () => void, delayMs: number): void;
};

export type TextInsertionMethod = 'none' | 'native' | 'clipboard';

const SIMPLE_TEXT_PATTERN = /^[a-z0-9 ]{1,80}$/;

export function shouldUseClipboardPaste(text: string): boolean {
  return !SIMPLE_TEXT_PATTERN.test(text);
}

export function insertText(text: string, deps: TextInsertionDeps): TextInsertionMethod {
  if (text.length === 0) {
    return 'none';
  }

  if (!shouldUseClipboardPaste(text)) {
    deps.typeString(text);
    return 'native';
  }

  const previousText = deps.clipboard.readText();
  deps.clipboard.writeText(text);
  deps.pasteFromClipboard();
  deps.scheduleRestore(() => {
    try {
      if (previousText.length === 0) {
        return;
      }
      if (deps.clipboard.readText() === text) {
        deps.clipboard.writeText(previousText);
      }
    } catch {
      // Best-effort restoration should not affect an already completed paste.
    }
  }, CLIPBOARD_RESTORE_DELAY_MS);

  return 'clipboard';
}
