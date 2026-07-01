using System.Diagnostics;
using System.IO;
using SwitchifyPc.Core.Updates;

namespace SwitchifyPc.Windows.Updates;

public interface IProcessShell
{
    bool Start(ProcessStartInfo startInfo);
}

public sealed class ProcessShell : IProcessShell
{
    public bool Start(ProcessStartInfo startInfo)
    {
        using Process? process = Process.Start(startInfo);
        return process is not null;
    }
}

public sealed class WindowsUpdateInstallerLauncher : IUpdateInstallerLauncher
{
    private readonly IProcessShell processShell;
    private readonly Func<string, bool> fileExists;

    public WindowsUpdateInstallerLauncher(IProcessShell? processShell = null, Func<string, bool>? fileExists = null)
    {
        this.processShell = processShell ?? new ProcessShell();
        this.fileExists = fileExists ?? File.Exists;
    }

    public Task<UpdateInstallerLaunchResult> LaunchAsync(
        string? installerPath,
        UpdateInstallerLaunchOptions options,
        CancellationToken cancellationToken = default)
    {
        if (string.IsNullOrWhiteSpace(installerPath) || !fileExists(installerPath))
        {
            return Task.FromResult(UpdateInstallerLaunchResult.Failure(UpdateInstallFailureReason.InstallerUnavailable));
        }

        try
        {
            cancellationToken.ThrowIfCancellationRequested();
            ProcessStartInfo startInfo = new()
            {
                FileName = installerPath,
                Arguments = options.Silent ? "/S" : string.Empty,
                UseShellExecute = true,
                WindowStyle = ProcessWindowStyle.Normal,
                WorkingDirectory = Path.GetDirectoryName(installerPath) ?? string.Empty
            };

            return Task.FromResult(
                processShell.Start(startInfo)
                    ? UpdateInstallerLaunchResult.Success()
                    : UpdateInstallerLaunchResult.Failure(UpdateInstallFailureReason.InstallerLaunchFailed));
        }
        catch (OperationCanceledException)
        {
            throw;
        }
        catch
        {
            return Task.FromResult(UpdateInstallerLaunchResult.Failure(UpdateInstallFailureReason.InstallerLaunchFailed));
        }
    }
}
