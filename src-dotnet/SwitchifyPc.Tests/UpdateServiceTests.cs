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
        public UpdateDownloadOutcome DownloadOutcome { get; init; } = UpdateDownloadOutcome.Downloaded(@"C:\cache\setup.exe");
        public bool CompleteChecksManually { get; init; }

        private TaskCompletionSource<UpdateCheckOutcome>? pendingCheck;
        private readonly TaskCompletionSource checkStarted = new(TaskCreationOptions.RunContinuationsAsynchronously);

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
            return Task.FromResult(DownloadOutcome);
        }
    }

    private sealed class FakeInstallerLauncher : IUpdateInstallerLauncher
    {
        public int LaunchCalls { get; private set; }
        public string? LastInstallerPath { get; private set; }
        public UpdateInstallerLaunchResult Result { get; init; } = UpdateInstallerLaunchResult.Success();

        public Task<UpdateInstallerLaunchResult> LaunchAsync(string? installerPath, CancellationToken cancellationToken = default)
        {
            LaunchCalls++;
            LastInstallerPath = installerPath;
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
