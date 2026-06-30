using System.Text.Json;
using SwitchifyPc.Core.Startup;
using SwitchifyPc.Core.Storage;

namespace SwitchifyPc.Core.Diagnostics;

public sealed record StartupDiagnosticsEntry(
    string StartedAt,
    string Version,
    bool IsPackaged,
    string Platform,
    string ExecutablePath,
    IReadOnlyList<string> Argv,
    bool StartHidden,
    StartupDiagnosticsRegistration? StartupRegistration = null,
    StartupTaskDiagnostics? StartupTask = null);

public sealed record StartupDiagnosticsRegistration(
    bool StartWithSystem,
    string? RegisteredCommand,
    string StartupApproved);

public sealed record StartupTaskDiagnostics(
    bool Exists,
    bool Enabled,
    string? RegisteredExecutablePath,
    IReadOnlyList<string> RegisteredArguments,
    string? LastRunResult);

public sealed record UpdateInstallDiagnosticEntry(
    string Event,
    string At,
    string Version,
    string? Reason = null);

public static class JsonlDiagnostics
{
    public const int MaxStartupDiagnosticLines = 50;
    public const int MaxUpdateInstallDiagnosticLines = 100;

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase
    };

    public static void AppendStartupDiagnostics(
        string filePath,
        StartupDiagnosticsEntry entry,
        Action<string>? warn = null)
    {
        try
        {
            IReadOnlyList<string> existing = ReadExistingLines(filePath, requireValidJson: true);
            AppendBounded(filePath, existing, JsonSerializer.Serialize(entry, JsonOptions), MaxStartupDiagnosticLines);
        }
        catch (Exception error)
        {
            (warn ?? Console.WriteLine)(string.IsNullOrWhiteSpace(error.Message) ? "Could not write startup diagnostics." : error.Message);
        }
    }

    public static void AppendUpdateInstallDiagnostic(
        string filePath,
        UpdateInstallDiagnosticEntry entry,
        Action<string>? warn = null)
    {
        try
        {
            IReadOnlyList<string> existing = ReadExistingLines(filePath, requireValidJson: false);
            AppendBounded(filePath, existing, JsonSerializer.Serialize(entry, JsonOptions), MaxUpdateInstallDiagnosticLines);
        }
        catch
        {
            (warn ?? Console.WriteLine)("Switchify update install diagnostics could not be written.");
        }
    }

    public static StartupDiagnosticsRegistration? RegistrationFromSettings(SystemStartupSettings settings)
    {
        return settings.Registration is null
            ? null
            : new StartupDiagnosticsRegistration(
                settings.StartWithSystem,
                settings.Registration.RegisteredCommand,
                settings.Registration.StartupApproved);
    }

    public static StartupTaskDiagnostics? TaskFromSettings(SystemStartupSettings settings)
    {
        return settings.TaskRegistration is null
            ? null
            : new StartupTaskDiagnostics(
                settings.TaskRegistration.Exists,
                settings.TaskRegistration.Enabled,
                settings.TaskRegistration.RegisteredExecutablePath,
                settings.TaskRegistration.RegisteredArguments,
                settings.TaskRegistration.LastRunResult);
    }

    private static void AppendBounded(string filePath, IReadOnlyList<string> existing, string nextLine, int maxLines)
    {
        List<string> lines = [.. existing, nextLine];
        if (lines.Count > maxLines)
        {
            lines = lines.TakeLast(maxLines).ToList();
        }

        JsonFileStore.WriteJsonFileAtomicSync(filePath, string.Join('\n', lines) + "\n");
    }

    private static IReadOnlyList<string> ReadExistingLines(string filePath, bool requireValidJson)
    {
        try
        {
            return File.ReadAllText(filePath)
                .Split(["\r\n", "\n"], StringSplitOptions.None)
                .Select(line => line.Trim())
                .Where(line => line.Length > 0)
                .Where(line => !requireValidJson || IsValidJson(line))
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
}
