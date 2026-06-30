using System.Text.Json;
using SwitchifyPc.Core.Diagnostics;
using SwitchifyPc.Core.Startup;

namespace SwitchifyPc.Tests;

public sealed class JsonlDiagnosticsTests : IDisposable
{
    private readonly string tempDir = Path.Combine(Path.GetTempPath(), $"switchify-diagnostics-{Guid.NewGuid():N}");

    [Fact]
    public void AppendsStartupDiagnosticsEntry()
    {
        string filePath = Path.Combine(tempDir, "startup-diagnostics.jsonl");

        JsonlDiagnostics.AppendStartupDiagnostics(filePath, StartupEntry(startedAt: "2026-06-29T12:00:00.000Z"));

        string[] lines = ReadLines(filePath);
        Assert.Single(lines);
        using JsonDocument document = JsonDocument.Parse(lines[0]);
        JsonElement root = document.RootElement;
        Assert.Equal("2026-06-29T12:00:00.000Z", root.GetProperty("startedAt").GetString());
        Assert.Equal("0.2.0", root.GetProperty("version").GetString());
        Assert.True(root.GetProperty("startHidden").GetBoolean());
        Assert.True(root.GetProperty("startupRegistration").GetProperty("startWithSystem").GetBoolean());
    }

    [Fact]
    public void KeepsOnlyNewestStartupDiagnosticsLines()
    {
        string filePath = Path.Combine(tempDir, "startup-diagnostics.jsonl");

        for (int index = 0; index < 55; index++)
        {
            JsonlDiagnostics.AppendStartupDiagnostics(filePath, StartupEntry(startedAt: $"2026-06-29T12:{index:00}:00.000Z"));
        }

        string[] lines = ReadLines(filePath);
        Assert.Equal(50, lines.Length);
        Assert.Equal("2026-06-29T12:05:00.000Z", JsonDocument.Parse(lines[0]).RootElement.GetProperty("startedAt").GetString());
        Assert.Equal("2026-06-29T12:54:00.000Z", JsonDocument.Parse(lines[^1]).RootElement.GetProperty("startedAt").GetString());
    }

    [Fact]
    public void StartupDiagnosticsCreatesParentDirectoryAndDropsMalformedExistingLines()
    {
        string filePath = Path.Combine(tempDir, "nested", "startup-diagnostics.jsonl");
        Directory.CreateDirectory(Path.GetDirectoryName(filePath)!);
        File.WriteAllText(filePath, "not json\n{\"startedAt\":\"old\"}\n");

        JsonlDiagnostics.AppendStartupDiagnostics(filePath, StartupEntry());

        string[] lines = ReadLines(filePath);
        Assert.Equal(2, lines.Length);
        Assert.Equal("old", JsonDocument.Parse(lines[0]).RootElement.GetProperty("startedAt").GetString());
    }

    [Fact]
    public void AppendsUpdateInstallDiagnosticEntry()
    {
        string filePath = Path.Combine(tempDir, "updates", "update-install-diagnostics.jsonl");

        JsonlDiagnostics.AppendUpdateInstallDiagnostic(filePath, new UpdateInstallDiagnosticEntry(
            Event: "install_requested",
            At: "2026-06-29T12:00:00.000Z",
            Version: "0.2.0"));

        string[] lines = ReadLines(filePath);
        Assert.Single(lines);
        using JsonDocument document = JsonDocument.Parse(lines[0]);
        Assert.Equal("install_requested", document.RootElement.GetProperty("event").GetString());
        Assert.False(document.RootElement.TryGetProperty("installerPath", out _));
        Assert.False(document.RootElement.TryGetProperty("thumbprint", out _));
    }

    [Fact]
    public void KeepsOnlyNewestUpdateInstallDiagnosticsLines()
    {
        string filePath = Path.Combine(tempDir, "update-install-diagnostics.jsonl");

        for (int index = 0; index < 105; index++)
        {
            JsonlDiagnostics.AppendUpdateInstallDiagnostic(filePath, new UpdateInstallDiagnosticEntry(
                Event: "installer_launch_failed",
                At: $"2026-06-29T12:00:{index:00}.000Z",
                Version: "0.2.0",
                Reason: index.ToString()));
        }

        string[] lines = ReadLines(filePath);
        Assert.Equal(100, lines.Length);
        Assert.Equal("5", JsonDocument.Parse(lines[0]).RootElement.GetProperty("reason").GetString());
        Assert.Equal("104", JsonDocument.Parse(lines[^1]).RootElement.GetProperty("reason").GetString());
    }

    [Fact]
    public void UpdateInstallDiagnosticsPreservesMalformedExistingLines()
    {
        string filePath = Path.Combine(tempDir, "update-install-diagnostics.jsonl");
        Directory.CreateDirectory(tempDir);
        File.WriteAllText(filePath, "not json\n");

        JsonlDiagnostics.AppendUpdateInstallDiagnostic(filePath, new UpdateInstallDiagnosticEntry(
            Event: "installer_started",
            At: "2026-06-29T12:00:00.000Z",
            Version: "0.2.0"));

        string[] lines = ReadLines(filePath);
        Assert.Equal("not json", lines[0]);
        Assert.Equal("installer_started", JsonDocument.Parse(lines[1]).RootElement.GetProperty("event").GetString());
    }

    [Fact]
    public void RegistrationFromSettingsOmitsUnsupportedRegistration()
    {
        SystemStartupSettings unsupported = new(
            Supported: false,
            StartWithSystem: false,
            StartsHidden: true,
            Reason: "unpackaged");

        Assert.Null(JsonlDiagnostics.RegistrationFromSettings(unsupported));
    }

    [Fact]
    public void RegistrationFromSettingsUsesNonSensitiveStartupFields()
    {
        SystemStartupSettings settings = new(
            Supported: true,
            StartWithSystem: true,
            StartsHidden: true,
            Reason: null,
            Registration: new StartupRegistration(
                ExpectedCommand: "\"C:\\Program Files\\Switchify PC\\Switchify PC.exe\" --start-hidden",
                RegisteredCommand: "\"C:\\Program Files\\Switchify PC\\Switchify PC.exe\" --start-hidden",
                StartupApproved: "enabled"));

        StartupDiagnosticsRegistration? registration = JsonlDiagnostics.RegistrationFromSettings(settings);

        Assert.NotNull(registration);
        Assert.True(registration.StartWithSystem);
        Assert.Equal("enabled", registration.StartupApproved);
        Assert.Equal("\"C:\\Program Files\\Switchify PC\\Switchify PC.exe\" --start-hidden", registration.RegisteredCommand);
    }

    public void Dispose()
    {
        if (Directory.Exists(tempDir))
        {
            Directory.Delete(tempDir, recursive: true);
        }
    }

    private static StartupDiagnosticsEntry StartupEntry(string startedAt = "2026-06-29T12:00:00.000Z")
    {
        return new StartupDiagnosticsEntry(
            StartedAt: startedAt,
            Version: "0.2.0",
            IsPackaged: true,
            Platform: "win32",
            ExecutablePath: @"C:\Program Files\Switchify PC\Switchify PC.exe",
            Argv: ["Switchify PC.exe", "--start-hidden"],
            StartHidden: true,
            StartupRegistration: new StartupDiagnosticsRegistration(
                StartWithSystem: true,
                RegisteredCommand: "\"C:\\Program Files\\Switchify PC\\Switchify PC.exe\" --start-hidden",
                StartupApproved: "enabled"));
    }

    private static string[] ReadLines(string filePath)
    {
        return File.ReadAllText(filePath)
            .Split(["\r\n", "\n"], StringSplitOptions.RemoveEmptyEntries);
    }
}
