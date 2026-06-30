using System.Diagnostics;
using SwitchifyPc.Core.Updates;
using SwitchifyPc.Windows.Updates;

namespace SwitchifyPc.Tests;

public sealed class WindowsUpdateInstallerLauncherTests
{
    [Fact]
    public async Task ReturnsInstallerUnavailableWhenInstallerPathIsMissing()
    {
        FakeProcessShell shell = new();
        WindowsUpdateInstallerLauncher launcher = new(shell, _ => false);

        UpdateInstallerLaunchResult result = await launcher.LaunchAsync(@"C:\cache\missing.exe");

        Assert.False(result.Ok);
        Assert.Equal(UpdateInstallFailureReason.InstallerUnavailable, result.Reason);
        Assert.Null(shell.LastStartInfo);
    }

    [Fact]
    public async Task ReturnsInstallerUnavailableWhenInstallerPathIsNull()
    {
        FakeProcessShell shell = new();
        WindowsUpdateInstallerLauncher launcher = new(shell, _ => true);

        UpdateInstallerLaunchResult result = await launcher.LaunchAsync(null);

        Assert.False(result.Ok);
        Assert.Equal(UpdateInstallFailureReason.InstallerUnavailable, result.Reason);
        Assert.Null(shell.LastStartInfo);
    }

    [Fact]
    public async Task OpensInstallerThroughWindowsShell()
    {
        FakeProcessShell shell = new();
        WindowsUpdateInstallerLauncher launcher = new(shell, _ => true);

        UpdateInstallerLaunchResult result = await launcher.LaunchAsync(@"C:\cache\Switchify-PC-Setup-0.2.0-x64.exe");

        Assert.True(result.Ok);
        Assert.NotNull(shell.LastStartInfo);
        Assert.Equal(@"C:\cache\Switchify-PC-Setup-0.2.0-x64.exe", shell.LastStartInfo.FileName);
        Assert.True(shell.LastStartInfo.UseShellExecute);
        Assert.Equal(ProcessWindowStyle.Normal, shell.LastStartInfo.WindowStyle);
        Assert.Equal(@"C:\cache", shell.LastStartInfo.WorkingDirectory);
    }

    [Fact]
    public async Task ReturnsLaunchFailedWhenShellReturnsFalse()
    {
        FakeProcessShell shell = new() { StartResult = false };
        WindowsUpdateInstallerLauncher launcher = new(shell, _ => true);

        UpdateInstallerLaunchResult result = await launcher.LaunchAsync(@"C:\cache\setup.exe");

        Assert.False(result.Ok);
        Assert.Equal(UpdateInstallFailureReason.InstallerLaunchFailed, result.Reason);
    }

    [Fact]
    public async Task ReturnsLaunchFailedWhenShellThrows()
    {
        FakeProcessShell shell = new() { ThrowOnStart = true };
        WindowsUpdateInstallerLauncher launcher = new(shell, _ => true);

        UpdateInstallerLaunchResult result = await launcher.LaunchAsync(@"C:\cache\setup.exe");

        Assert.False(result.Ok);
        Assert.Equal(UpdateInstallFailureReason.InstallerLaunchFailed, result.Reason);
    }

    private sealed class FakeProcessShell : IProcessShell
    {
        public ProcessStartInfo? LastStartInfo { get; private set; }
        public bool StartResult { get; init; } = true;
        public bool ThrowOnStart { get; init; }

        public bool Start(ProcessStartInfo startInfo)
        {
            LastStartInfo = startInfo;
            if (ThrowOnStart) throw new InvalidOperationException("Could not start process.");
            return StartResult;
        }
    }
}
