using SwitchifyPc.Core.Updates;

namespace SwitchifyPc.Core.Ui;

public static class UpdateUiCopy
{
    public static string ApplyFailureMessage(UpdateApplyResult result)
    {
        if (result.FailureStage == UpdateApplyFailureStage.Download)
        {
            return result.DownloadFailureReason switch
            {
                UpdateFailureReason.NotPackaged => "Updates are only available in the installed app.",
                UpdateFailureReason.NotSupported => "Updates are only supported on Windows.",
                UpdateFailureReason.NotAvailable => "No update is currently available.",
                UpdateFailureReason.InvalidUpdate => "The downloaded update was invalid. Try again.",
                _ => "The update could not be downloaded. Check your connection and try again."
            };
        }

        return InstallFailureMessage(result.InstallFailureReason);
    }

    public static string InstallFailureMessage(UpdateInstallFailureReason? reason)
    {
        return reason switch
        {
            UpdateInstallFailureReason.NotDownloaded => "The update is not downloaded yet.",
            UpdateInstallFailureReason.NotPackaged => "Updates are only available in the installed app.",
            UpdateInstallFailureReason.NotSupported => "Updates are only supported on Windows.",
            UpdateInstallFailureReason.Cancelled => "The update was cancelled.",
            UpdateInstallFailureReason.InstallerUnavailable => "The downloaded installer could not be found. Try the update again.",
            UpdateInstallFailureReason.InstallerLaunchFailed => "The update installer could not be opened. Try again.",
            _ => "The update installer could not be opened."
        };
    }
}
