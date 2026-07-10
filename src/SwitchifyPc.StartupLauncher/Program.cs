using System.Diagnostics;
using System.Reflection;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace SwitchifyPc.StartupLauncher;

internal sealed record StartupLauncherDiagnostic(
    string Event,
    string At,
    string Version,
    string? Reason = null,
    int? NativeErrorCode = null);

[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.CamelCase)]
[JsonSerializable(typeof(StartupLauncherDiagnostic))]
internal partial class StartupLauncherJsonContext : JsonSerializerContext
{
}

internal static class StartupLauncherProgram
{
    internal const string MainExecutableName = "Switchify PC.exe";
    internal const string StartHiddenArgument = "--start-hidden";
    internal const int MaxDiagnosticLines = 50;
    internal const int MissingExecutableExitCode = 2;
    internal const int ShellLaunchFailedExitCode = 3;

    internal static int Run(
        string launcherPath,
        string diagnosticsPath,
        Func<ProcessStartInfo, Process?> startProcess,
        DateTimeOffset? now = null,
        string? version = null)
    {
        string? installDirectory = Path.GetDirectoryName(launcherPath);
        string mainExecutablePath = Path.Combine(installDirectory ?? string.Empty, MainExecutableName);
        DateTimeOffset launchedAt = now ?? DateTimeOffset.UtcNow;
        string launcherVersion = version ?? CurrentVersion();

        if (string.IsNullOrWhiteSpace(installDirectory) || !File.Exists(mainExecutablePath))
        {
            AppendDiagnostic(diagnosticsPath, new StartupLauncherDiagnostic(
                Event: "startup_launcher.failed",
                At: launchedAt.ToString("O"),
                Version: launcherVersion,
                Reason: "main_executable_missing"));
            return MissingExecutableExitCode;
        }

        ProcessStartInfo startInfo = CreateStartInfo(mainExecutablePath, installDirectory);
        try
        {
            using Process? process = startProcess(startInfo);
            if (process is null)
            {
                AppendDiagnostic(diagnosticsPath, new StartupLauncherDiagnostic(
                    Event: "startup_launcher.failed",
                    At: launchedAt.ToString("O"),
                    Version: launcherVersion,
                    Reason: "shell_launch_rejected"));
                return ShellLaunchFailedExitCode;
            }

            AppendDiagnostic(diagnosticsPath, new StartupLauncherDiagnostic(
                Event: "startup_launcher.requested",
                At: launchedAt.ToString("O"),
                Version: launcherVersion));
            return 0;
        }
        catch (Exception error)
        {
            AppendDiagnostic(diagnosticsPath, new StartupLauncherDiagnostic(
                Event: "startup_launcher.failed",
                At: launchedAt.ToString("O"),
                Version: launcherVersion,
                Reason: "shell_launch_failed",
                NativeErrorCode: error is System.ComponentModel.Win32Exception nativeError
                    ? nativeError.NativeErrorCode
                    : null));
            return ShellLaunchFailedExitCode;
        }
    }

    internal static ProcessStartInfo CreateStartInfo(string mainExecutablePath, string installDirectory)
    {
        return new ProcessStartInfo(mainExecutablePath)
        {
            UseShellExecute = true,
            Verb = "open",
            Arguments = StartHiddenArgument,
            WorkingDirectory = installDirectory,
            WindowStyle = ProcessWindowStyle.Hidden
        };
    }

    internal static void AppendDiagnostic(string diagnosticsPath, StartupLauncherDiagnostic entry)
    {
        try
        {
            string? directory = Path.GetDirectoryName(diagnosticsPath);
            if (!string.IsNullOrWhiteSpace(directory)) Directory.CreateDirectory(directory);

            IReadOnlyList<string> existing = ReadValidLines(diagnosticsPath);
            List<string> lines = [.. existing, JsonSerializer.Serialize(entry, StartupLauncherJsonContext.Default.StartupLauncherDiagnostic)];
            if (lines.Count > MaxDiagnosticLines) lines = lines.TakeLast(MaxDiagnosticLines).ToList();

            string temporaryPath = diagnosticsPath + ".tmp";
            File.WriteAllText(temporaryPath, string.Join('\n', lines) + "\n");
            File.Move(temporaryPath, diagnosticsPath, overwrite: true);
        }
        catch
        {
            // Startup diagnostics must never prevent the main app launch.
        }
    }

    private static IReadOnlyList<string> ReadValidLines(string diagnosticsPath)
    {
        try
        {
            return File.ReadAllLines(diagnosticsPath)
                .Select(line => line.Trim())
                .Where(line => line.Length > 0 && IsValidJson(line))
                .ToArray();
        }
        catch
        {
            return [];
        }
    }

    private static bool IsValidJson(string line)
    {
        try
        {
            using JsonDocument _ = JsonDocument.Parse(line);
            return true;
        }
        catch
        {
            return false;
        }
    }

    private static string CurrentVersion()
    {
        return typeof(StartupLauncherProgram).Assembly
            .GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion
            .Split('+', 2, StringSplitOptions.RemoveEmptyEntries)[0] ?? "unknown";
    }
}

internal static class Program
{
    [STAThread]
    private static int Main()
    {
        string launcherPath = Environment.ProcessPath ?? string.Empty;
        string appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
        string diagnosticsPath = Path.Combine(appData, "switchify-pc", "startup-launcher-diagnostics.jsonl");
        return StartupLauncherProgram.Run(launcherPath, diagnosticsPath, Process.Start);
    }
}
