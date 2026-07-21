using SwitchifyPc.Core.Updates;

namespace SwitchifyPc.Tests;

public sealed class UpdateServiceTests
{
    private static readonly DateTimeOffset FixedNow = DateTimeOffset.Parse("2026-06-29T12:00:00Z");

    [Fact]
    public async Task CheckFailsInUnpackagedBuildsWithoutCallingBackend()
    {
        FakeUpdateBackend backend = new();
        UpdateService service = CreateService(backend, isPackaged: false);

        UpdateState state = await service.CheckForUpdatesAsync();

        Assert.Equal(UpdateCheckStatus.CheckFailed, state.Info.Status);
        Assert.Equal(UpdateFailureReason.NotPackaged, state.Info.Reason);
        Assert.Equal(0, backend.CheckCalls);
    }

    [Fact]
    public async Task CheckReportsAvailableUpdate()
    {
        FakeUpdateBackend backend = new()
        {
            CheckOutcome = UpdateCheckOutcome.Available(new AvailableUpdate("0.2.0", "Switchify PC 0.2.0", "Native app."))
        };
        List<UpdateState> changes = [];
        UpdateService service = CreateService(backend, onStateChanged: changes.Add);

        UpdateState state = await service.CheckForUpdatesAsync();

        Assert.Equal(UpdateCheckStatus.UpdateAvailable, state.Info.Status);
        Assert.Equal("0.2.0", state.Info.LatestVersion);
        Assert.Equal("Switchify PC 0.2.0", state.Info.ReleaseName);
        Assert.Equal("Native app.", state.Info.ReleaseNotes);
        Assert.Equal(FixedNow, state.Info.CheckedAt);
        Assert.Contains(changes, item => item.Info.Status == UpdateCheckStatus.Checking);
        Assert.Contains(changes, item => item.Info.Status == UpdateCheckStatus.UpdateAvailable);
    }

    [Fact]
    public async Task DownloadRequiresAvailableUpdate()
    {
        FakeUpdateBackend backend = new();
        UpdateService service = CreateService(backend);

        UpdateState state = await service.DownloadUpdateAsync();

        Assert.Equal(UpdateDownloadStatus.DownloadFailed, state.Download.Status);
        Assert.Equal(UpdateFailureReason.NotAvailable, state.Download.Reason);
        Assert.Equal(0, backend.DownloadCalls);
    }

    [Fact]
    public async Task DownloadCapturesInstallerPathAndProgress()
    {
        FakeUpdateBackend backend = new()
        {
            CheckOutcome = UpdateCheckOutcome.Available(new AvailableUpdate("0.2.0", null, null)),
            DownloadOutcome = UpdateDownloadOutcome.Downloaded(@"C:\cache\Switchify-PC-Setup-0.2.0-x64.exe")
        };
        List<UpdateState> changes = [];
        UpdateService service = CreateService(backend, onStateChanged: changes.Add);

        await service.CheckForUpdatesAsync();
        UpdateState state = await service.DownloadUpdateAsync();

        Assert.Equal(UpdateDownloadStatus.Downloaded, state.Download.Status);
        Assert.Equal(100, state.Download.Percent);
        Assert.Contains(changes, item => item.Download.Status == UpdateDownloadStatus.Downloading && item.Download.Percent == 40);
    }

    [Fact]
    public async Task InstallBeforeDownloadFails()
    {
        FakeInstallerLauncher launcher = new();
        UpdateService service = CreateService(new FakeUpdateBackend(), launcher: launcher);

        UpdateInstallResult result = await service.InstallDownloadedUpdateAsync();

        Assert.False(result.Ok);
        Assert.Equal(UpdateInstallFailureReason.NotDownloaded, result.Reason);
        Assert.Equal(0, launcher.LaunchCalls);
    }

    [Fact]
    public async Task InstallOpensDownloadedInstallerAndKeepsServiceState()
    {
        FakeUpdateBackend backend = new()
        {
            CheckOutcome = UpdateCheckOutcome.Available(new AvailableUpdate("0.2.0", null, null)),
            DownloadOutcome = UpdateDownloadOutcome.Downloaded(@"C:\cache\Switchify-PC-Setup-0.2.0-x64.exe")
        };
        FakeInstallerLauncher launcher = new();
        UpdateService service = CreateService(backend, launcher: launcher);

        await service.CheckForUpdatesAsync();
        await service.DownloadUpdateAsync();
        UpdateInstallResult result = await service.InstallDownloadedUpdateAsync();

        Assert.True(result.Ok);
        Assert.Equal(1, launcher.LaunchCalls);
        Assert.Equal(@"C:\cache\Switchify-PC-Setup-0.2.0-x64.exe", launcher.LastInstallerPath);
        Assert.Equal(new UpdateInstallerLaunchOptions(Silent: true), launcher.LastOptions);
        Assert.Equal(UpdateDownloadStatus.Downloaded, service.GetState().Download.Status);
    }

    [Fact]
    public async Task InstallReturnsLauncherFailure()
    {
        FakeUpdateBackend backend = new()
        {
            CheckOutcome = UpdateCheckOutcome.Available(new AvailableUpdate("0.2.0", null, null)),
            DownloadOutcome = UpdateDownloadOutcome.Downloaded(@"C:\cache\Switchify-PC-Setup-0.2.0-x64.exe")
        };
        FakeInstallerLauncher launcher = new()
        {
            Result = UpdateInstallerLaunchResult.Failure(UpdateInstallFailureReason.InstallerLaunchFailed)
        };
        UpdateService service = CreateService(backend, launcher: launcher);

        await service.CheckForUpdatesAsync();
        await service.DownloadUpdateAsync();
        UpdateInstallResult result = await service.InstallDownloadedUpdateAsync();

        Assert.False(result.Ok);
        Assert.Equal(UpdateInstallFailureReason.InstallerLaunchFailed, result.Reason);
    }

    [Fact]
    public async Task InstallAvailableUpdateDownloadsAndLaunchesInstaller()
    {
        FakeUpdateBackend backend = new()
        {
            CheckOutcome = UpdateCheckOutcome.Available(new AvailableUpdate("0.2.0", null, null)),
            DownloadOutcome = UpdateDownloadOutcome.Downloaded(@"C:\cache\setup.exe")
        };
        FakeInstallerLauncher launcher = new();
        List<UpdateState> changes = [];
        UpdateService service = CreateService(backend, launcher, onStateChanged: changes.Add);
        await service.CheckForUpdatesAsync();

        UpdateApplyResult result = await service.InstallAvailableUpdateAsync();

        Assert.True(result.Ok);
        Assert.Equal(1, backend.DownloadCalls);
        Assert.Equal(1, launcher.LaunchCalls);
        Assert.Contains(changes, state => state.IsApplyingUpdate);
        Assert.False(service.GetState().IsApplyingUpdate);
    }

    [Fact]
    public async Task InstallAvailableUpdateSkipsDownloadWhenAlreadyDownloaded()
    {
        FakeUpdateBackend backend = new()
        {
            CheckOutcome = UpdateCheckOutcome.Available(new AvailableUpdate("0.2.0", null, null))
        };
        FakeInstallerLauncher launcher = new();
        UpdateService service = CreateService(backend, launcher);
        await service.CheckForUpdatesAsync();
        await service.DownloadUpdateAsync();

        UpdateApplyResult result = await service.InstallAvailableUpdateAsync();

        Assert.True(result.Ok);
        Assert.Equal(1, backend.DownloadCalls);
        Assert.Equal(1, launcher.LaunchCalls);
    }

    [Fact]
    public async Task ConcurrentInstallAvailableCallsShareDownloadAndLaunch()
    {
        FakeUpdateBackend backend = new()
        {
            CheckOutcome = UpdateCheckOutcome.Available(new AvailableUpdate("0.2.0", null, null)),
            CompleteDownloadsManually = true
        };
        FakeInstallerLauncher launcher = new();
        UpdateService service = CreateService(backend, launcher);
        await service.CheckForUpdatesAsync();

        Task<UpdateApplyResult> first = service.InstallAvailableUpdateAsync();
        Task<UpdateApplyResult> second = service.InstallAvailableUpdateAsync();
        await backend.WaitForDownloadStartAsync();
        backend.CompletePendingDownload(UpdateDownloadOutcome.Downloaded(@"C:\cache\setup.exe"));

        UpdateApplyResult[] results = await Task.WhenAll(first, second);
        Assert.All(results, result => Assert.True(result.Ok));
        Assert.Equal(1, backend.DownloadCalls);
        Assert.Equal(1, launcher.LaunchCalls);
    }

    [Fact]
    public async Task ApplyRemainsDeduplicatedUntilFinalStateCallbackReturns()
    {
        FakeUpdateBackend backend = new()
        {
            CheckOutcome = UpdateCheckOutcome.Available(new AvailableUpdate("0.2.0", null, null))
        };
        FakeInstallerLauncher launcher = new();
        using ManualResetEventSlim finalStateEntered = new();
        using ManualResetEventSlim releaseFinalState = new();
        UpdateService service = CreateService(backend, launcher, onStateChanged: state =>
        {
            if (!state.IsApplyingUpdate && state.Download.Status == UpdateDownloadStatus.Downloaded)
            {
                finalStateEntered.Set();
                releaseFinalState.Wait(TimeSpan.FromSeconds(10));
            }
        });
        await service.CheckForUpdatesAsync();

        Task<UpdateApplyResult> first = service.InstallAvailableUpdateAsync();
        Assert.True(finalStateEntered.Wait(TimeSpan.FromSeconds(10)));

        Task<UpdateApplyResult> second = service.InstallAvailableUpdateAsync();

        Assert.Same(first, second);
        Assert.Equal(1, backend.DownloadCalls);
        Assert.Equal(1, launcher.LaunchCalls);
        releaseFinalState.Set();
        UpdateApplyResult[] results = await Task.WhenAll(first, second);
        Assert.All(results, result => Assert.True(result.Ok));
    }

    [Fact]
    public async Task InstallAvailableUpdateDistinguishesDownloadAndInstallFailures()
    {
        FakeUpdateBackend downloadBackend = new()
        {
            CheckOutcome = UpdateCheckOutcome.Available(new AvailableUpdate("0.2.0", null, null)),
            DownloadOutcome = UpdateDownloadOutcome.Failed(UpdateFailureReason.NetworkError)
        };
        UpdateService downloadService = CreateService(downloadBackend);
        await downloadService.CheckForUpdatesAsync();

        UpdateApplyResult downloadFailure = await downloadService.InstallAvailableUpdateAsync();

        Assert.Equal(UpdateApplyFailureStage.Download, downloadFailure.FailureStage);
        Assert.Equal(UpdateFailureReason.NetworkError, downloadFailure.DownloadFailureReason);
        Assert.Null(downloadFailure.InstallFailureReason);
        Assert.Equal(UpdateDownloadStatus.DownloadFailed, downloadService.GetState().Download.Status);

        FakeUpdateBackend installBackend = new()
        {
            CheckOutcome = UpdateCheckOutcome.Available(new AvailableUpdate("0.2.0", null, null))
        };
        FakeInstallerLauncher launcher = new()
        {
            Result = UpdateInstallerLaunchResult.Failure(UpdateInstallFailureReason.InstallerLaunchFailed)
        };
        UpdateService installService = CreateService(installBackend, launcher);
        await installService.CheckForUpdatesAsync();

        UpdateApplyResult installFailure = await installService.InstallAvailableUpdateAsync();

        Assert.Equal(UpdateApplyFailureStage.Install, installFailure.FailureStage);
        Assert.Null(installFailure.DownloadFailureReason);
        Assert.Equal(UpdateInstallFailureReason.InstallerLaunchFailed, installFailure.InstallFailureReason);
        Assert.Equal(UpdateDownloadStatus.Downloaded, installService.GetState().Download.Status);
        Assert.Equal(UpdateApplyFailureStage.Install, installService.GetState().LastApplyResult?.FailureStage);
        Assert.Equal(
            UpdateInstallFailureReason.InstallerLaunchFailed,
            installService.GetState().LastApplyResult?.InstallFailureReason);
    }

    [Fact]
    public async Task RetryAfterLaunchFailureReusesDownloadedInstaller()
    {
        FakeUpdateBackend backend = new()
        {
            CheckOutcome = UpdateCheckOutcome.Available(new AvailableUpdate("0.2.0", null, null))
        };
        FakeInstallerLauncher launcher = new()
        {
            Result = UpdateInstallerLaunchResult.Failure(UpdateInstallFailureReason.InstallerLaunchFailed)
        };
        UpdateService service = CreateService(backend, launcher);
        await service.CheckForUpdatesAsync();
        await service.InstallAvailableUpdateAsync();
        launcher.Result = UpdateInstallerLaunchResult.Success();

        UpdateApplyResult retry = await service.InstallAvailableUpdateAsync();

        Assert.True(retry.Ok);
        Assert.Equal(1, backend.DownloadCalls);
        Assert.Equal(2, launcher.LaunchCalls);
    }

    [Fact]
    public async Task RetryAfterInstallerBecomesUnavailableDownloadsItAgain()
    {
        FakeUpdateBackend backend = new()
        {
            CheckOutcome = UpdateCheckOutcome.Available(new AvailableUpdate("0.2.0", null, null))
        };
        FakeInstallerLauncher launcher = new()
        {
            Result = UpdateInstallerLaunchResult.Failure(UpdateInstallFailureReason.InstallerUnavailable)
        };
        UpdateService service = CreateService(backend, launcher);
        await service.CheckForUpdatesAsync();

        UpdateApplyResult first = await service.InstallAvailableUpdateAsync();

        Assert.Equal(UpdateInstallFailureReason.InstallerUnavailable, first.InstallFailureReason);
        Assert.Equal(UpdateDownloadStatus.Idle, service.GetState().Download.Status);
        Assert.Equal(UpdateInstallFailureReason.InstallerUnavailable, service.GetState().LastApplyResult?.InstallFailureReason);

        launcher.Result = UpdateInstallerLaunchResult.Success();
        UpdateApplyResult retry = await service.InstallAvailableUpdateAsync();

        Assert.True(retry.Ok);
        Assert.Equal(2, backend.DownloadCalls);
        Assert.Equal(2, launcher.LaunchCalls);
        Assert.Null(service.GetState().LastApplyResult);
    }

    [Fact]
    public async Task InstallAvailableUpdateReportsUnsupportedAndUnavailable()
    {
        UpdateApplyResult unsupported = await CreateService(new FakeUpdateBackend(), isPackaged: false)
            .InstallAvailableUpdateAsync();
        UpdateApplyResult unavailable = await CreateService(new FakeUpdateBackend())
            .InstallAvailableUpdateAsync();

        Assert.Equal(UpdateApplyFailureStage.Download, unsupported.FailureStage);
        Assert.Equal(UpdateFailureReason.NotPackaged, unsupported.DownloadFailureReason);
        Assert.Equal(UpdateFailureReason.NotAvailable, unavailable.DownloadFailureReason);
    }

    [Fact]
    public async Task InstallAvailableUpdateCancellationClearsApplyingAndDownloadState()
    {
        FakeUpdateBackend backend = new()
        {
            CheckOutcome = UpdateCheckOutcome.Available(new AvailableUpdate("0.2.0", null, null)),
            CompleteDownloadsManually = true
        };
        UpdateService service = CreateService(backend);
        await service.CheckForUpdatesAsync();
        using CancellationTokenSource cancellation = new();

        Task<UpdateApplyResult> applying = service.InstallAvailableUpdateAsync(cancellation.Token);
        await backend.WaitForDownloadStartAsync();
        cancellation.Cancel();

        await Assert.ThrowsAnyAsync<OperationCanceledException>(() => applying);
        Assert.False(service.GetState().IsApplyingUpdate);
        Assert.Equal(UpdateDownloadStatus.Idle, service.GetState().Download.Status);
        Assert.Null(service.GetState().Download.Reason);
    }

    [Fact]
    public async Task AutomaticAndManualChecksSkipWhileApplying()
    {
        FakeUpdateBackend backend = new()
        {
            CheckOutcome = UpdateCheckOutcome.Available(new AvailableUpdate("0.2.0", null, null)),
            CompleteDownloadsManually = true
        };
        FakeUpdateScheduler scheduler = new();
        UpdateService service = CreateService(backend, scheduler: scheduler);
        await service.CheckForUpdatesAsync();
        service.StartAutomaticUpdateChecks();

        Task<UpdateApplyResult> applying = service.InstallAvailableUpdateAsync();
        await backend.WaitForDownloadStartAsync();
        await scheduler.Recurring[0].RunAsync();
        UpdateState manual = await service.CheckForUpdatesAsync();

        Assert.True(manual.IsApplyingUpdate);
        Assert.Equal(1, backend.CheckCalls);
        backend.CompletePendingDownload(UpdateDownloadOutcome.Downloaded(@"C:\cache\setup.exe"));
        await applying;
    }

    [Fact]
    public async Task AutomaticPollingRunsInitialAndHourlyChecks()
    {
        FakeUpdateBackend backend = new();
        FakeUpdateScheduler scheduler = new();
        UpdateService service = CreateService(backend, scheduler: scheduler);

        service.StartAutomaticUpdateChecks();
        ScheduledCallback initial = Assert.Single(scheduler.OneShot);
        ScheduledCallback recurring = Assert.Single(scheduler.Recurring);

        await initial.RunAsync();
        await recurring.RunAsync();

        Assert.Equal(2, backend.CheckCalls);
        Assert.Equal(UpdateService.InitialPollDelay, initial.Delay);
        Assert.Equal(UpdateService.PollInterval, recurring.Delay);
    }

    [Fact]
    public void AutomaticPollingDoesNotStartDuplicateTimers()
    {
        FakeUpdateScheduler scheduler = new();
        UpdateService service = CreateService(new FakeUpdateBackend(), scheduler: scheduler);

        service.StartAutomaticUpdateChecks();
        service.StartAutomaticUpdateChecks();

        Assert.Single(scheduler.OneShot);
        Assert.Single(scheduler.Recurring);
    }

    [Fact]
    public async Task StopAutomaticPollingDisposesScheduledChecks()
    {
        FakeUpdateBackend backend = new();
        FakeUpdateScheduler scheduler = new();
        UpdateService service = CreateService(backend, scheduler: scheduler);

        service.StartAutomaticUpdateChecks();
        service.StopAutomaticUpdateChecks();
        await scheduler.OneShot[0].RunAsync();
        await scheduler.Recurring[0].RunAsync();

        Assert.Equal(0, backend.CheckCalls);
    }

    [Fact]
    public async Task AutomaticPollingSkipsOverlappingChecks()
    {
        FakeUpdateBackend backend = new() { CompleteChecksManually = true };
        FakeUpdateScheduler scheduler = new();
        UpdateService service = CreateService(backend, scheduler: scheduler);
        service.StartAutomaticUpdateChecks();

        Task firstCheck = scheduler.OneShot[0].RunAsync();
        await backend.WaitForCheckStartAsync();
        await scheduler.Recurring[0].RunAsync();
        backend.CompletePendingCheck(UpdateCheckOutcome.UpToDate());
        await firstCheck;

        Assert.Equal(1, backend.CheckCalls);
    }

    [Fact]
    public void AutomaticPollingDoesNotStartWhenUnsupported()
    {
        FakeUpdateScheduler scheduler = new();
        UpdateService service = CreateService(new FakeUpdateBackend(), scheduler: scheduler, platform: "darwin");

        service.StartAutomaticUpdateChecks();

        Assert.Empty(scheduler.OneShot);
        Assert.Empty(scheduler.Recurring);
    }

    [Fact]
    public async Task AutomaticPollingSkipsWhenUpdateIsDownloaded()
    {
        FakeUpdateBackend backend = new()
        {
            CheckOutcome = UpdateCheckOutcome.Available(new AvailableUpdate("0.2.0", null, null)),
            DownloadOutcome = UpdateDownloadOutcome.Downloaded(@"C:\cache\Switchify-PC-Setup-0.2.0-x64.exe")
        };
        FakeUpdateScheduler scheduler = new();
        UpdateService service = CreateService(backend, scheduler: scheduler);

        await service.CheckForUpdatesAsync();
        await service.DownloadUpdateAsync();
        service.StartAutomaticUpdateChecks();
        await scheduler.Recurring[0].RunAsync();

        Assert.Equal(1, backend.CheckCalls);
    }

    private static UpdateService CreateService(
        FakeUpdateBackend backend,
        FakeInstallerLauncher? launcher = null,
        FakeUpdateScheduler? scheduler = null,
        bool isPackaged = true,
        string platform = "win32",
        Action<UpdateState>? onStateChanged = null)
    {
        return new UpdateService(new UpdateServiceOptions(
            CurrentVersion: "0.1.20",
            IsPackaged: isPackaged,
            Platform: platform,
            Backend: backend,
            InstallerLauncher: launcher ?? new FakeInstallerLauncher(),
            Scheduler: scheduler,
            Now: () => FixedNow,
            OnStateChanged: onStateChanged));
    }

    private sealed class FakeUpdateBackend : IUpdateBackend
    {
        public int CheckCalls { get; private set; }
        public int DownloadCalls { get; private set; }
        public UpdateCheckOutcome CheckOutcome { get; init; } = UpdateCheckOutcome.UpToDate();
        public UpdateDownloadOutcome DownloadOutcome { get; set; } = UpdateDownloadOutcome.Downloaded(@"C:\cache\setup.exe");
        public bool CompleteChecksManually { get; init; }
        public bool CompleteDownloadsManually { get; init; }

        private TaskCompletionSource<UpdateCheckOutcome>? pendingCheck;
        private TaskCompletionSource<UpdateDownloadOutcome>? pendingDownload;
        private readonly TaskCompletionSource checkStarted = new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly TaskCompletionSource downloadStarted = new(TaskCreationOptions.RunContinuationsAsynchronously);

        public Task<UpdateCheckOutcome> CheckForUpdatesAsync(string currentVersion, CancellationToken cancellationToken = default)
        {
            CheckCalls++;
            checkStarted.TrySetResult();
            if (CompleteChecksManually)
            {
                pendingCheck = new TaskCompletionSource<UpdateCheckOutcome>(TaskCreationOptions.RunContinuationsAsynchronously);
                return pendingCheck.Task;
            }

            return Task.FromResult(CheckOutcome);
        }

        public Task WaitForCheckStartAsync()
        {
            return checkStarted.Task;
        }

        public void CompletePendingCheck(UpdateCheckOutcome outcome)
        {
            pendingCheck?.SetResult(outcome);
        }

        public Task<UpdateDownloadOutcome> DownloadUpdateAsync(
            AvailableUpdate update,
            IProgress<UpdateDownloadSnapshot> progress,
            CancellationToken cancellationToken = default)
        {
            DownloadCalls++;
            progress.Report(new UpdateDownloadSnapshot(40, 100, 40));
            downloadStarted.TrySetResult();
            if (CompleteDownloadsManually)
            {
                pendingDownload = new TaskCompletionSource<UpdateDownloadOutcome>(TaskCreationOptions.RunContinuationsAsynchronously);
                cancellationToken.Register(() => pendingDownload.TrySetCanceled(cancellationToken));
                return pendingDownload.Task;
            }

            return Task.FromResult(DownloadOutcome);
        }

        public Task WaitForDownloadStartAsync() => downloadStarted.Task;

        public void CompletePendingDownload(UpdateDownloadOutcome outcome)
        {
            pendingDownload?.TrySetResult(outcome);
        }
    }

    private sealed class FakeInstallerLauncher : IUpdateInstallerLauncher
    {
        public int LaunchCalls { get; private set; }
        public string? LastInstallerPath { get; private set; }
        public UpdateInstallerLaunchOptions? LastOptions { get; private set; }
        public UpdateInstallerLaunchResult Result { get; set; } = UpdateInstallerLaunchResult.Success();

        public Task<UpdateInstallerLaunchResult> LaunchAsync(
            string? installerPath,
            UpdateInstallerLaunchOptions options,
            CancellationToken cancellationToken = default)
        {
            LaunchCalls++;
            LastInstallerPath = installerPath;
            LastOptions = options;
            return Task.FromResult(Result);
        }
    }

    private sealed class FakeUpdateScheduler : IUpdatePollScheduler
    {
        public List<ScheduledCallback> OneShot { get; } = [];
        public List<ScheduledCallback> Recurring { get; } = [];

        public IDisposable ScheduleOnce(TimeSpan delay, Func<Task> callback)
        {
            ScheduledCallback scheduled = new(delay, callback);
            OneShot.Add(scheduled);
            return scheduled;
        }

        public IDisposable ScheduleRecurring(TimeSpan interval, Func<Task> callback)
        {
            ScheduledCallback scheduled = new(interval, callback);
            Recurring.Add(scheduled);
            return scheduled;
        }
    }

    private sealed class ScheduledCallback(TimeSpan delay, Func<Task> callback) : IDisposable
    {
        private bool disposed;

        public TimeSpan Delay { get; } = delay;

        public Task RunAsync()
        {
            return disposed ? Task.CompletedTask : callback();
        }

        public void Dispose()
        {
            disposed = true;
        }
    }
}
