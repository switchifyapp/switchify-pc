export const CLIPBOARD_RESTORE_DELAY_MS = 250;
export const CLIPBOARD_WRITE_VERIFY_ATTEMPTS = 3;
export const CLIPBOARD_WRITE_VERIFY_DELAY_MS = 25;

export type TextClipboard = {
  readText(): string;
  writeText(text: string): void;
};

export type TextInsertionDeps = {
  clipboard: TextClipboard;
  typeString(text: string): void;
  typeUnicodeText(text: string): Promise<void>;
  pasteFromClipboard(): void;
  scheduleRestore(callback: () => void, delayMs: number): void;
  wait(delayMs: number): Promise<void>;
};

export type TextInsertionMethod = 'none' | 'native' | 'unicode' | 'clipboard';

const SIMPLE_TEXT_PATTERN = /^[a-z0-9 ]{1,80}$/;

export function shouldUseClipboardPaste(text: string): boolean {
  return !SIMPLE_TEXT_PATTERN.test(text);
}

export async function insertText(text: string, deps: TextInsertionDeps): Promise<TextInsertionMethod> {
  if (text.length === 0) {
    return 'none';
  }

  if (!shouldUseClipboardPaste(text)) {
    deps.typeString(text);
    return 'native';
  }

  try {
    await deps.typeUnicodeText(text);
    return 'unicode';
  } catch {
    // Fall back to verified clipboard paste for targets where Unicode input is unavailable.
  }

  await pasteTextWithVerifiedClipboard(text, deps);
  return 'clipboard';
}

async function pasteTextWithVerifiedClipboard(text: string, deps: TextInsertionDeps): Promise<void> {
  const previousText = deps.clipboard.readText();
  const verified = await writeAndVerifyClipboardText(text, deps);
  if (!verified) {
    deps.clipboard.writeText(previousText);
    throw new Error('Clipboard did not contain requested text after write.');
  }

  deps.pasteFromClipboard();
  deps.scheduleRestore(() => {
    try {
      if (deps.clipboard.readText() === text) {
        deps.clipboard.writeText(previousText);
      }
    } catch {
      // Best-effort restoration should not affect an already completed paste.
    }
  }, CLIPBOARD_RESTORE_DELAY_MS);

}

async function writeAndVerifyClipboardText(text: string, deps: TextInsertionDeps): Promise<boolean> {
  for (let attempt = 0; attempt < CLIPBOARD_WRITE_VERIFY_ATTEMPTS; attempt += 1) {
    deps.clipboard.writeText(text);
    if (deps.clipboard.readText() === text) {
      return true;
    }
    if (attempt < CLIPBOARD_WRITE_VERIFY_ATTEMPTS - 1) {
      await deps.wait(CLIPBOARD_WRITE_VERIFY_DELAY_MS);
    }
  }

  return false;
}
