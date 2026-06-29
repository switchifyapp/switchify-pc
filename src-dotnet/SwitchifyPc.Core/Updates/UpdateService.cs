namespace SwitchifyPc.Core.Updates;

public interface IUpdateBackend
{
    Task<UpdateCheckOutcome> CheckForUpdatesAsync(string currentVersion, CancellationToken cancellationToken = default);

    Task<UpdateDownloadOutcome> DownloadUpdateAsync(
        AvailableUpdate update,
        IProgress<UpdateDownloadSnapshot> progress,
        CancellationToken cancellationToken = default);
}

public interface IUpdateInstallerLauncher
{
    Task<UpdateInstallerLaunchResult> LaunchAsync(string? installerPath, CancellationToken cancellationToken = default);
}

public interface IUpdatePollScheduler
{
    IDisposable ScheduleOnce(TimeSpan delay, Func<Task> callback);

    IDisposable ScheduleRecurring(TimeSpan interval, Func<Task> callback);
}

public sealed class TimerUpdatePollScheduler : IUpdatePollScheduler
{
    public IDisposable ScheduleOnce(TimeSpan delay, Func<Task> callback)
    {
        Timer? timer = null;
        timer = new Timer(_ =>
        {
            timer?.Dispose();
            _ = callback();
        }, null, delay, Timeout.InfiniteTimeSpan);

        return timer;
    }

    public IDisposable ScheduleRecurring(TimeSpan interval, Func<Task> callback)
    {
        return new Timer(_ => _ = callback(), null, interval, interval);
    }
}

public sealed class UpdateService
{
    public static readonly TimeSpan PollInterval = TimeSpan.FromHours(1);
    public static readonly TimeSpan InitialPollDelay = TimeSpan.FromSeconds(30);

    private readonly bool isPackaged;
    private readonly string platform;
    private readonly IUpdateBackend backend;
    private readonly IUpdateInstallerLauncher installerLauncher;
    private readonly IUpdatePollScheduler scheduler;
    private readonly Func<DateTimeOffset> now;
    private readonly Action<UpdateState> onStateChanged;
    private readonly object gate = new();

    private UpdateState state;
    private string? downloadedInstallerPath;
    private AvailableUpdate? availableUpdate;
    private Task<UpdateState>? checkingTask;
    private Task<UpdateState>? downloadTask;
    private UpdateOperation operation = UpdateOperation.Idle;
    private IDisposable? initialPoll;
    private IDisposable? recurringPoll;

    public UpdateService(UpdateServiceOptions options)
    {
        ArgumentNullException.ThrowIfNull(options);
        isPackaged = options.IsPackaged;
        platform = options.Platform;
        backend = options.Backend;
        installerLauncher = options.InstallerLauncher;
        scheduler = options.Scheduler ?? new TimerUpdatePollScheduler();
        now = options.Now ?? (() => DateTimeOffset.UtcNow);
        onStateChanged = options.OnStateChanged ?? (_ => { });
        state = UpdateState.CreateInitial(options.CurrentVersion);
    }

    public UpdateState GetState()
    {
        lock (gate)
        {
            return state;
        }
    }

    public void StartAutomaticUpdateChecks()
    {
        if (UnsupportedReason() is not null) return;

        lock (gate)
        {
            if (initialPoll is not null || recurringPoll is not null) return;
            initialPoll = scheduler.ScheduleOnce(InitialPollDelay, RunAutomaticUpdateCheckAsync);
            recurringPoll = scheduler.ScheduleRecurring(PollInterval, RunAutomaticUpdateCheckAsync);
        }
    }

    public void StopAutomaticUpdateChecks()
    {
        IDisposable? oneShot;
        IDisposable? recurring;
        lock (gate)
        {
            oneShot = initialPoll;
            recurring = recurringPoll;
            initialPoll = null;
            recurringPoll = null;
        }

        oneShot?.Dispose();
        recurring?.Dispose();
    }

    public Task<UpdateState> CheckForUpdatesAsync(CancellationToken cancellationToken = default)
    {
        UpdateFailureReason? unsupported = UnsupportedReason();
        if (unsupported is not null)
        {
            SetState(GetState() with
            {
                Info = GetState().Info with
                {
                    CheckedAt = now(),
                    Status = UpdateCheckStatus.CheckFailed,
                    Reason = unsupported
                },
                Download = UpdateState.CreateIdleDownload()
            });
            return Task.FromResult(GetState());
        }

        lock (gate)
        {
            if (checkingTask is { IsCompleted: false }) return checkingTask;
            checkingTask = null;
            operation = UpdateOperation.Checking;
            downloadedInstallerPath = null;
            availableUpdate = null;
            state = state with
            {
                Info = state.Info with
                {
                    LatestVersion = null,
                    ReleaseName = null,
                    ReleaseNotes = null,
                    CheckedAt = null,
                    Status = UpdateCheckStatus.Checking,
                    Reason = null
                },
                Download = UpdateState.CreateIdleDownload()
            };
            onStateChanged(state);
            checkingTask = RunCheckAsync(cancellationToken);
            return checkingTask;
        }
    }

    public Task<UpdateState> DownloadUpdateAsync(CancellationToken cancellationToken = default)
    {
        UpdateFailureReason? unsupported = UnsupportedReason();
        if (unsupported is not null)
        {
            SetState(GetState() with
            {
                Download = UpdateState.CreateIdleDownload() with
                {
                    Status = UpdateDownloadStatus.DownloadFailed,
                    Reason = unsupported
                }
            });
            return Task.FromResult(GetState());
        }

        lock (gate)
        {
            if (downloadTask is { IsCompleted: false }) return downloadTask;
            downloadTask = null;
            if (state.Info.Status != UpdateCheckStatus.UpdateAvailable || availableUpdate is null)
            {
                state = state with
                {
                    Download = UpdateState.CreateIdleDownload() with
                    {
                        Status = UpdateDownloadStatus.DownloadFailed,
                        Reason = UpdateFailureReason.NotAvailable
                    }
                };
                onStateChanged(state);
                return Task.FromResult(state);
            }

            operation = UpdateOperation.Downloading;
            downloadedInstallerPath = null;
            state = state with
            {
                Download = new UpdateDownloadProgress(UpdateDownloadStatus.Downloading, 0, null, null)
            };
            onStateChanged(state);
            downloadTask = RunDownloadAsync(availableUpdate, cancellationToken);
            return downloadTask;
        }
    }

    public async Task<UpdateInstallResult> InstallDownloadedUpdateAsync(CancellationToken cancellationToken = default)
    {
        UpdateFailureReason? unsupported = UnsupportedReason();
        if (unsupported == UpdateFailureReason.NotPackaged) return UpdateInstallResult.Failure(UpdateInstallFailureReason.NotPackaged);
        if (unsupported == UpdateFailureReason.NotSupported) return UpdateInstallResult.Failure(UpdateInstallFailureReason.NotSupported);

        string? installerPath;
        lock (gate)
        {
            if (state.Download.Status != UpdateDownloadStatus.Downloaded) return UpdateInstallResult.Failure(UpdateInstallFailureReason.NotDownloaded);
            installerPath = downloadedInstallerPath;
        }

        UpdateInstallerLaunchResult result = await installerLauncher.LaunchAsync(installerPath, cancellationToken).ConfigureAwait(false);
        return result.Ok
            ? UpdateInstallResult.Success()
            : UpdateInstallResult.Failure(result.Reason ?? UpdateInstallFailureReason.InstallerLaunchFailed);
    }

    private async Task<UpdateState> RunCheckAsync(CancellationToken cancellationToken)
    {
        try
        {
            UpdateCheckOutcome outcome = await backend.CheckForUpdatesAsync(GetState().Info.CurrentVersion, cancellationToken).ConfigureAwait(false);
            if (outcome.FailureReason is not null)
            {
                SetState(GetState() with
                {
                    Info = GetState().Info with
                    {
                        CheckedAt = now(),
                        Status = UpdateCheckStatus.CheckFailed,
                        Reason = outcome.FailureReason
                    }
                });
                return GetState();
            }

            if (outcome.UpdateAvailable && outcome.Update is not null)
            {
                lock (gate)
                {
                    availableUpdate = outcome.Update;
                }

                SetState(GetState() with
                {
                    Info = GetState().Info with
                    {
                        LatestVersion = outcome.Update.Version,
                        ReleaseName = outcome.Update.ReleaseName,
                        ReleaseNotes = outcome.Update.ReleaseNotes,
                        CheckedAt = now(),
                        Status = UpdateCheckStatus.UpdateAvailable,
                        Reason = null
                    },
                    Download = UpdateState.CreateIdleDownload()
                });
                return GetState();
            }

            SetState(GetState() with
            {
                Info = GetState().Info with
                {
                    LatestVersion = GetState().Info.CurrentVersion,
                    CheckedAt = now(),
                    Status = UpdateCheckStatus.UpToDate,
                    Reason = null
                },
                Download = UpdateState.CreateIdleDownload()
            });
            return GetState();
        }
        catch
        {
            SetState(GetState() with
            {
                Info = GetState().Info with
                {
                    CheckedAt = now(),
                    Status = UpdateCheckStatus.CheckFailed,
                    Reason = UpdateFailureReason.NetworkError
                }
            });
            return GetState();
        }
        finally
        {
            lock (gate)
            {
                operation = UpdateOperation.Idle;
                checkingTask = null;
            }
        }
    }

    private async Task<UpdateState> RunDownloadAsync(AvailableUpdate update, CancellationToken cancellationToken)
    {
        try
        {
            InlineProgress<UpdateDownloadSnapshot> progress = new(snapshot =>
            {
                SetState(GetState() with
                {
                    Download = new UpdateDownloadProgress(
                        UpdateDownloadStatus.Downloading,
                        snapshot.DownloadedBytes,
                        snapshot.TotalBytes,
                        snapshot.Percent is null ? null : Math.Clamp(snapshot.Percent.Value, 0, 100))
                });
            });

            UpdateDownloadOutcome outcome = await backend.DownloadUpdateAsync(update, progress, cancellationToken).ConfigureAwait(false);
            if (outcome.FailureReason is not null || string.IsNullOrWhiteSpace(outcome.InstallerPath))
            {
                lock (gate)
                {
                    downloadedInstallerPath = null;
                }

                SetState(GetState() with
                {
                    Download = GetState().Download with
                    {
                        Status = UpdateDownloadStatus.DownloadFailed,
                        Reason = outcome.FailureReason ?? UpdateFailureReason.InvalidUpdate
                    }
                });
                return GetState();
            }

            lock (gate)
            {
                downloadedInstallerPath = outcome.InstallerPath;
            }

            SetState(GetState() with
            {
                Download = GetState().Download with
                {
                    Status = UpdateDownloadStatus.Downloaded,
                    Percent = 100,
                    Reason = null
                }
            });
            return GetState();
        }
        catch
        {
            lock (gate)
            {
                downloadedInstallerPath = null;
            }

            SetState(GetState() with
            {
                Download = GetState().Download with
                {
                    Status = UpdateDownloadStatus.DownloadFailed,
                    Reason = UpdateFailureReason.NetworkError
                }
            });
            return GetState();
        }
        finally
        {
            lock (gate)
            {
                operation = UpdateOperation.Idle;
                downloadTask = null;
            }
        }
    }

    private async Task RunAutomaticUpdateCheckAsync()
    {
        lock (gate)
        {
            if (operation != UpdateOperation.Idle) return;
            if (state.Download.Status is UpdateDownloadStatus.Downloading or UpdateDownloadStatus.Downloaded) return;
        }

        await CheckForUpdatesAsync().ConfigureAwait(false);
    }

    private void SetState(UpdateState next)
    {
        lock (gate)
        {
            state = next;
        }

        onStateChanged(next);
    }

    private UpdateFailureReason? UnsupportedReason()
    {
        if (!isPackaged) return UpdateFailureReason.NotPackaged;
        if (!string.Equals(platform, "win32", StringComparison.OrdinalIgnoreCase)) return UpdateFailureReason.NotSupported;
        return null;
    }

    private enum UpdateOperation
    {
        Idle,
        Checking,
        Downloading
    }

    private sealed class InlineProgress<T>(Action<T> handler) : IProgress<T>
    {
        public void Report(T value)
        {
            handler(value);
        }
    }
}

public sealed record UpdateServiceOptions(
    string CurrentVersion,
    bool IsPackaged,
    string Platform,
    IUpdateBackend Backend,
    IUpdateInstallerLauncher InstallerLauncher,
    IUpdatePollScheduler? Scheduler = null,
    Func<DateTimeOffset>? Now = null,
    Action<UpdateState>? OnStateChanged = null);
