import { describe, expect, it } from 'vitest';
import { secondInstanceAction } from './single-instance';

describe('secondInstanceAction', () => {
  it('shows the main window for a normal Windows second launch', () => {
    expect(secondInstanceAction(['Switchify PC.exe'], 'win32')).toBe('showMainWindow');
  });

  it('ignores Windows startup launches so they stay hidden', () => {
    expect(secondInstanceAction(['Switchify PC.exe', '--start-hidden'], 'win32')).toBe('ignore');
  });

  it('does not suppress non-Windows launches that include the Windows startup flag', () => {
    expect(secondInstanceAction(['Switchify PC', '--start-hidden'], 'darwin')).toBe('showMainWindow');
  });
});
