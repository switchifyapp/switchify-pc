using System.Text.Json;

namespace Switchify.UpdateLauncher;

internal static class Program
{
    private const int ErrorCancelled = 1223;

    private static int Main(string[] args)
    {
        try
        {
            if (args.Length == 1 && args[0] == "--self-test-quote")
            {
                Console.Out.WriteLine(NativeMethods.JoinCommandLineArguments(new[] { "--updated", "--force-run", "value with spaces" }));
                return 0;
            }

            var parsed = ParseArgs(args);
            if (parsed is null)
            {
                WriteResult(false, "invalid_arguments");
                return 2;
            }

            if (!File.Exists(parsed.InstallerPath))
            {
                WriteResult(false, "installer_missing");
                return 3;
            }

            var parameters = NativeMethods.JoinCommandLineArguments(parsed.InstallerArgs);
            var info = new NativeMethods.ShellExecuteInfo
            {
                fMask = NativeMethods.SeeMaskNoCloseProcess,
                lpVerb = "runas",
                lpFile = parsed.InstallerPath,
                lpParameters = parameters,
                nShow = NativeMethods.SwShownormal
            };

            if (!NativeMethods.ShellExecuteEx(info))
            {
                var error = NativeMethods.GetLastWin32Error();
                WriteResult(false, error == ErrorCancelled ? "uac_cancelled" : "launch_failed", null, error);
                return error == ErrorCancelled ? 4 : 5;
            }

            if (info.hProcess == IntPtr.Zero)
            {
                WriteResult(false, "installer_process_unavailable");
                return 6;
            }

            try
            {
                var pid = NativeMethods.GetProcessId(info.hProcess);
                WriteResult(true, "installer_started", pid == 0 ? null : checked((int)pid));
                return 0;
            }
            finally
            {
                NativeMethods.CloseHandle(info.hProcess);
            }
        }
        catch
        {
            WriteResult(false, "unexpected_error");
            return 10;
        }
    }

    private static ParsedArgs? ParseArgs(string[] args)
    {
        string? installerPath = null;
        string? argsJson = null;

        for (var index = 0; index < args.Length; index++)
        {
            var arg = args[index];
            if (arg == "--installer" && index + 1 < args.Length)
            {
                installerPath = args[++index];
                continue;
            }

            if (arg == "--args-json" && index + 1 < args.Length)
            {
                argsJson = args[++index];
                continue;
            }

            return null;
        }

        if (string.IsNullOrWhiteSpace(installerPath) || string.IsNullOrWhiteSpace(argsJson))
        {
            return null;
        }

        string[]? installerArgs;
        try
        {
            installerArgs = JsonSerializer.Deserialize<string[]>(argsJson);
        }
        catch (JsonException)
        {
            return null;
        }

        if (installerArgs is null || installerArgs.Any((value) => value is null))
        {
            return null;
        }

        return new ParsedArgs(installerPath, installerArgs);
    }

    private static void WriteResult(bool ok, string status, int? pid = null, int? win32Error = null)
    {
        Console.Out.WriteLine(JsonSerializer.Serialize(new UpdateLauncherResult
        {
            Ok = ok,
            Status = status,
            Pid = pid,
            Win32Error = win32Error
        }));
    }

    private sealed record ParsedArgs(string InstallerPath, string[] InstallerArgs);
}
