using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;

namespace Switchify.UpdateLauncher;

internal static class NativeMethods
{
    internal const uint SeeMaskNoCloseProcess = 0x00000040;
    internal const int SwShownormal = 1;

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    internal sealed class ShellExecuteInfo
    {
        public int cbSize = Marshal.SizeOf<ShellExecuteInfo>();
        public uint fMask;
        public IntPtr hwnd;
        public string? lpVerb;
        public string? lpFile;
        public string? lpParameters;
        public string? lpDirectory;
        public int nShow;
        public IntPtr hInstApp;
        public IntPtr lpIDList;
        public string? lpClass;
        public IntPtr hkeyClass;
        public uint dwHotKey;
        public IntPtr hIcon;
        public IntPtr hProcess;
    }

    [DllImport("shell32.dll", EntryPoint = "ShellExecuteExW", SetLastError = true, CharSet = CharSet.Unicode)]
    [return: MarshalAs(UnmanagedType.Bool)]
    internal static extern bool ShellExecuteEx(ShellExecuteInfo info);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    internal static extern bool CloseHandle(IntPtr handle);

    [DllImport("kernel32.dll", SetLastError = true)]
    internal static extern uint GetProcessId(IntPtr process);

    internal static string JoinCommandLineArguments(IEnumerable<string> args)
    {
        return string.Join(" ", args.Select(QuoteCommandLineArgument));
    }

    private static string QuoteCommandLineArgument(string value)
    {
        if (value.Length == 0)
        {
            return "\"\"";
        }

        if (!value.Any(char.IsWhiteSpace) && !value.Contains('"') && !value.Contains('\\'))
        {
            return value;
        }

        var builder = new StringBuilder();
        builder.Append('"');
        var backslashCount = 0;

        foreach (var character in value)
        {
            if (character == '\\')
            {
                backslashCount++;
                continue;
            }

            if (character == '"')
            {
                builder.Append('\\', backslashCount * 2 + 1);
                builder.Append('"');
                backslashCount = 0;
                continue;
            }

            builder.Append('\\', backslashCount);
            builder.Append(character);
            backslashCount = 0;
        }

        builder.Append('\\', backslashCount * 2);
        builder.Append('"');
        return builder.ToString();
    }

    internal static int GetLastWin32Error()
    {
        return Marshal.GetLastWin32Error();
    }

    internal static string Win32Message(int error)
    {
        return new Win32Exception(error).Message;
    }
}
