using System.ComponentModel;
using System.Runtime.InteropServices;

namespace Switchify.TextInput;

internal static partial class NativeMethods
{
    private const uint INPUT_KEYBOARD = 1;
    private const uint KEYEVENTF_KEYUP = 0x0002;
    private const uint KEYEVENTF_UNICODE = 0x0004;
    private const int MaxInputRecordsPerBatch = 128;

    internal static int SendUnicodeText(string text)
    {
        if (text.Length == 0)
        {
            return 0;
        }

        int sentEvents = 0;
        List<INPUT> batch = new(MaxInputRecordsPerBatch);

        foreach (char codeUnit in text)
        {
            batch.Add(CreateUnicodeInput(codeUnit, keyUp: false));
            batch.Add(CreateUnicodeInput(codeUnit, keyUp: true));

            if (batch.Count >= MaxInputRecordsPerBatch)
            {
                sentEvents += SendBatch(batch);
                batch.Clear();
            }
        }

        if (batch.Count > 0)
        {
            sentEvents += SendBatch(batch);
        }

        return sentEvents;
    }

    private static int SendBatch(IReadOnlyList<INPUT> inputs)
    {
        INPUT[] batch = inputs.ToArray();
        uint sent = SendInput((uint)batch.Length, batch, Marshal.SizeOf<INPUT>());
        if (sent != batch.Length)
        {
            int errorCode = Marshal.GetLastPInvokeError();
            throw new Win32Exception(
                errorCode,
                $"SendInput inserted {sent} of {batch.Length} events. Input may be blocked by Windows integrity level restrictions.");
        }

        return (int)sent;
    }

    private static INPUT CreateUnicodeInput(char codeUnit, bool keyUp)
    {
        return new INPUT
        {
            Type = INPUT_KEYBOARD,
            InputUnion = new INPUTUNION
            {
                KeyboardInput = new KEYBDINPUT
                {
                    VirtualKey = 0,
                    ScanCode = codeUnit,
                    Flags = KEYEVENTF_UNICODE | (keyUp ? KEYEVENTF_KEYUP : 0),
                    Time = 0,
                    ExtraInfo = UIntPtr.Zero
                }
            }
        };
    }

    [LibraryImport("user32.dll", SetLastError = true)]
    private static partial uint SendInput(uint inputCount, [In] INPUT[] inputs, int inputSize);

    [StructLayout(LayoutKind.Sequential)]
    private struct INPUT
    {
        internal uint Type;
        internal INPUTUNION InputUnion;
    }

    [StructLayout(LayoutKind.Explicit, Size = 32)]
    private struct INPUTUNION
    {
        [FieldOffset(0)]
        internal KEYBDINPUT KeyboardInput;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct KEYBDINPUT
    {
        internal ushort VirtualKey;
        internal ushort ScanCode;
        internal uint Flags;
        internal uint Time;
        internal UIntPtr ExtraInfo;
    }
}
