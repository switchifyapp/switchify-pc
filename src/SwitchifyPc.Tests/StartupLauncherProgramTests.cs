using System.ComponentModel;
using System.Diagnostics;
using System.Text.Json;
using SwitchifyPc.StartupLauncher;

namespace SwitchifyPc.Tests;

public sealed class StartupLauncherProgramTests : IDisposable
{
    private readonly string temporaryDirectory = Path.Combine(Path.GetTempPath(), $"switchify-launcher-tests-{Guid.NewGuid():N}");

    public StartupLauncherProgramTests()
    {
        Directory.CreateDirectory(temporaryDirectory);
    }

    [Fact]
    public void LaunchesOnlySiblingMainExecutableThroughShell()
    {
        string launcher = CreateFile("Switchify PC Startup.exe");
        CreateFile(StartupLauncherProgram.MainExecutableName);
        string diagnostics = Path.Combine(temporaryDirectory, "diagnostics.jsonl");
        ProcessStartInfo? captured = null;

        int exitCode = StartupLauncherProgram.Run(
            launcher,
            diagnostics,
            startInfo =>
            {
                captured = startInfo;
                return Process.GetCurrentProcess();
            },
            now: DateTimeOffset.Parse("2026-07-10T12:00:00Z"),
            version: "0.4.5");

        Assert.Equal(0, exitCode);
        Assert.NotNull(captured);
        Assert.Equal(Path.Combine(temporaryDirectory, StartupLauncherProgram.MainExecutableName), captured.FileName);
        Assert.Equal(StartupLauncherProgram.StartHiddenArgument, captured.Arguments);
        Assert.Equal(temporaryDirectory, captured.WorkingDirectory);
        Assert.True(captured.UseShellExecute);
        Assert.Equal("open", captured.Verb);
        Assert.Equal(ProcessWindowStyle.Hidden, captured.WindowStyle);
        Assert.Contains("startup_launcher.requested", File.ReadAllText(diagnostics));
    }

    [Fact]
    public void MissingSiblingExecutableFailsWithoutCallingShell()
    {
        string launcher = CreateFile("Switchify PC Startup.exe");
        string diagnostics = Path.Combine(temporaryDirectory, "diagnostics.jsonl");
        bool called = false;

        int exitCode = StartupLauncherProgram.Run(
            launcher,
            diagnostics,
            _ =>
            {
                called = true;
                return null;
            },
            version: "0.4.5");

        Assert.Equal(StartupLauncherProgram.MissingExecutableExitCode, exitCode);
        Assert.False(called);
        Assert.Contains("main_executable_missing", File.ReadAllText(diagnostics));
    }

    [Fact]
    public void RecordsNativeShellErrorWithoutAnExecutablePath()
    {
        string launcher = CreateFile("Switchify PC Startup.exe");
        CreateFile(StartupLauncherProgram.MainExecutableName);
        string diagnostics = Path.Combine(temporaryDirectory, "diagnostics.jsonl");

        int exitCode = StartupLauncherProgram.Run(
            launcher,
            diagnostics,
            _ => throw new Win32Exception(740),
            version: "0.4.5");

        string line = File.ReadAllText(diagnostics);
        Assert.Equal(StartupLauncherProgram.ShellLaunchFailedExitCode, exitCode);
        Assert.Contains("shell_launch_failed", line);
        Assert.Contains("740", line);
        Assert.DoesNotContain(temporaryDirectory, line, StringComparison.OrdinalIgnoreCase);
    }

    [Fact]
    public void DiagnosticsAreBoundedAndDiscardMalformedLines()
    {
        string diagnostics = Path.Combine(temporaryDirectory, "diagnostics.jsonl");
        File.WriteAllText(diagnostics, "not-json\n");

        for (int index = 0; index < StartupLauncherProgram.MaxDiagnosticLines + 5; index++)
        {
            StartupLauncherProgram.AppendDiagnostic(diagnostics, new StartupLauncherDiagnostic(
                "startup_launcher.requested",
                DateTimeOffset.UtcNow.ToString("O"),
                index.ToString()));
        }

        string[] lines = File.ReadAllLines(diagnostics);
        Assert.Equal(StartupLauncherProgram.MaxDiagnosticLines, lines.Length);
        Assert.All(lines, line => Assert.NotNull(JsonDocument.Parse(line)));
        Assert.Contains("\"version\":\"5\"", lines[0]);
    }

    public void Dispose()
    {
        Directory.Delete(temporaryDirectory, recursive: true);
    }

    private string CreateFile(string name)
    {
        string path = Path.Combine(temporaryDirectory, name);
        File.WriteAllText(path, string.Empty);
        return path;
    }
}
