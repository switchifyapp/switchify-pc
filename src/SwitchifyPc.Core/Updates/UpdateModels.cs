namespace SwitchifyPc.Core.Updates;

public enum UpdateCheckStatus
{
    NotChecked,
    Checking,
    UpToDate,
    UpdateAvailable,
    CheckFailed
}

public enum UpdateDownloadStatus
{
    Idle,
    Downloading,
    Downloaded,
    DownloadFailed
}

public enum UpdateFailureReason
{
    NetworkError,
    InvalidUpdate,
    NotPackaged,
    NotSupported,
    NotAvailable
}

public enum UpdateInstallFailureReason
{
    NotDownloaded,
    NotPackaged,
    NotSupported,
    Cancelled,
    InstallerUnavailable,
    InstallerLaunchFailed
}

public enum UpdateApplyFailureStage
{
    Download,
    Install
}

public sealed record UpdateInfo(
    string CurrentVersion,
    string? LatestVersion,
    string? ReleaseName,
    string? ReleaseNotes,
    DateTimeOffset? CheckedAt,
    UpdateCheckStatus Status,
    UpdateFailureReason? Reason = null);

public sealed record UpdateDownloadProgress(
    UpdateDownloadStatus Status,
    long DownloadedBytes,
    long? TotalBytes,
    int? Percent,
    UpdateFailureReason? Reason = null);

public sealed record UpdateState(
    UpdateInfo Info,
    UpdateDownloadProgress Download,
    bool IsApplyingUpdate = false,
    UpdateApplyResult? LastApplyResult = null)
{
    public static UpdateState CreateInitial(string currentVersion) =>
        new(
            new UpdateInfo(currentVersion, null, null, null, null, UpdateCheckStatus.NotChecked),
            CreateIdleDownload());

    public static UpdateDownloadProgress CreateIdleDownload() =>
        new(UpdateDownloadStatus.Idle, 0, null, null);
}

public sealed record AvailableUpdate(
    string Version,
    string? ReleaseName,
    string? ReleaseNotes,
    string? InstallerAssetName = null,
    string? InstallerDownloadUrl = null);

public sealed record UpdateCheckOutcome(bool UpdateAvailable, AvailableUpdate? Update, UpdateFailureReason? FailureReason = null)
{
    public static UpdateCheckOutcome Available(AvailableUpdate update) => new(true, update);
    public static UpdateCheckOutcome UpToDate() => new(false, null);
    public static UpdateCheckOutcome Failed(UpdateFailureReason reason) => new(false, null, reason);
}

public sealed record UpdateDownloadSnapshot(long DownloadedBytes, long? TotalBytes, int? Percent);

public sealed record UpdateDownloadOutcome(string? InstallerPath, UpdateFailureReason? FailureReason = null)
{
    public static UpdateDownloadOutcome Downloaded(string installerPath) => new(installerPath);
    public static UpdateDownloadOutcome Failed(UpdateFailureReason reason) => new(null, reason);
}

public sealed record UpdateInstallResult(bool Ok, UpdateInstallFailureReason? Reason = null)
{
    public static UpdateInstallResult Success() => new(true);
    public static UpdateInstallResult Failure(UpdateInstallFailureReason reason) => new(false, reason);
}

public sealed record UpdateApplyResult
{
    private UpdateApplyResult(
        bool ok,
        UpdateApplyFailureStage? failureStage = null,
        UpdateFailureReason? downloadFailureReason = null,
        UpdateInstallFailureReason? installFailureReason = null)
    {
        Ok = ok;
        FailureStage = failureStage;
        DownloadFailureReason = downloadFailureReason;
        InstallFailureReason = installFailureReason;
    }

    public bool Ok { get; }

    public UpdateApplyFailureStage? FailureStage { get; }

    public UpdateFailureReason? DownloadFailureReason { get; }

    public UpdateInstallFailureReason? InstallFailureReason { get; }

    public static UpdateApplyResult Success() => new(true);

    public static UpdateApplyResult DownloadFailure(UpdateFailureReason reason) =>
        new(false, UpdateApplyFailureStage.Download, downloadFailureReason: reason);

    public static UpdateApplyResult InstallFailure(UpdateInstallFailureReason reason) =>
        new(false, UpdateApplyFailureStage.Install, installFailureReason: reason);
}

public sealed record UpdateInstallerLaunchResult(bool Ok, UpdateInstallFailureReason? Reason = null)
{
    public static UpdateInstallerLaunchResult Success() => new(true);
    public static UpdateInstallerLaunchResult Failure(UpdateInstallFailureReason reason) => new(false, reason);
}

public sealed record UpdateInstallerLaunchOptions(bool Silent);
