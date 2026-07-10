using SwitchifyPc.Core.Startup;
using SwitchifyPc.Windows.Startup;

namespace SwitchifyPc.Tests;

public sealed class SystemStartupServiceTests
{
    private const string MainExecutablePath = "C:\\Program Files\\Switchify PC\\Switchify PC.exe";
    private const string LauncherPath = "C:\\Program Files\\Switchify PC\\Switchify PC Startup.exe";
    private const string ExpectedLauncherCommand = "\"C:\\Program Files\\Switchify PC\\Switchify PC Startup.exe\"";
    private const string LegacyMainCommand = "\"C:\\Program Files\\Switchify PC\\Switchify PC.exe\" --start-hidden";

    [Fact]
    public void ShouldStartHiddenOnlyOnWindowsWithHiddenArg()
    {
        Assert.True(SystemStartupService.ShouldStartHidden(["Switchify PC.exe", "--start-hidden"], "win32"));
        Assert.False(SystemStartupService.ShouldStartHidden(["Switchify PC.exe", "--start-hidden"], "darwin"));
        Assert.False(SystemStartupService.ShouldStartHidden(["Switchify PC.exe"], "win32"));
    }

    [Fact]
    public async Task UnsupportedPlatformsDoNotReadTaskOrRegistry()
    {
        FakeStartupTask task = new();
        FakeStartupRegistry registry = new();
        SystemStartupService service = CreateService(task, registry, platform: "darwin", isPackaged: true);

        SystemStartupSettings settings = await service.GetSettingsAsync();
        await service.SetStartWithSystemAsync(true);

        Assert.False(settings.Supported);
        Assert.Equal("unsupported_platform", settings.Reason);
        Assert.Empty(task.Calls);
        Assert.Empty(registry.Calls);
    }

    [Fact]
    public async Task UnsupportedUnpackagedWindowsDoesNotReadTaskOrRegistry()
    {
        FakeStartupTask task = new();
        FakeStartupRegistry registry = new();

        SystemStartupSettings settings = await CreateService(task, registry, isPackaged: false).GetSettingsAsync();

        Assert.False(settings.Supported);
        Assert.Equal("unpackaged", settings.Reason);
        Assert.Empty(task.Calls);
        Assert.Empty(registry.Calls);
    }

    [Fact]
    public async Task ReportsEnabledOnlyForMatchingLauncherRegistrationThatWindowsHasNotDisabled()
    {
        SystemStartupSettings enabled = await CreateService(
            new FakeStartupTask(),
            new FakeStartupRegistry(new StartupRegistrySnapshot(ExpectedLauncherCommand, "enabled"))).GetSettingsAsync();
        SystemStartupSettings missingApproval = await CreateService(
            new FakeStartupTask(),
            new FakeStartupRegistry(new StartupRegistrySnapshot(ExpectedLauncherCommand, "missing"))).GetSettingsAsync();
        SystemStartupSettings disabled = await CreateService(
            new FakeStartupTask(),
            new FakeStartupRegistry(new StartupRegistrySnapshot(ExpectedLauncherCommand, "disabled"))).GetSettingsAsync();
        SystemStartupSettings stale = await CreateService(
            new FakeStartupTask(),
            new FakeStartupRegistry(new StartupRegistrySnapshot(LegacyMainCommand, "enabled"))).GetSettingsAsync();

        Assert.True(enabled.StartWithSystem);
        Assert.True(missingApproval.StartWithSystem);
        Assert.False(disabled.StartWithSystem);
        Assert.False(stale.StartWithSystem);
        Assert.Equal(LauncherPath, enabled.TaskRegistration?.ExpectedExecutablePath);
        Assert.Empty(enabled.TaskRegistration?.ExpectedArguments ?? []);
    }

    [Fact]
    public async Task EnablesAndDisablesThroughRunEntryAndRemovesLegacyTask()
    {
        FakeStartupTask task = new();
        FakeStartupRegistry registry = new();
        SystemStartupService service = CreateService(task, registry);

        await service.SetStartWithSystemAsync(true);
        await service.SetStartWithSystemAsync(false);

        Assert.Contains($"set:{SystemStartupService.StartupValueName}:{ExpectedLauncherCommand}", registry.Calls);
        Assert.Contains($"delete:{SystemStartupService.StartupValueName}", registry.Calls);
        Assert.Equal(2, task.Calls.Count(call => call == $"delete:{SystemStartupService.StartupTaskName}"));
        Assert.DoesNotContain(task.Calls, call => call.StartsWith("set:", StringComparison.Ordinal));
    }

    [Fact]
    public async Task MigratesEnabledLegacyTaskBeforeDeletingIt()
    {
        FakeStartupTask task = new(LegacyTask(enabled: true));
        FakeStartupRegistry registry = new();

        await CreateService(task, registry).RepairLegacyStartupRegistrationAsync();

        Assert.Equal(
            [$"get:{SystemStartupService.StartupValueName}", $"set:{SystemStartupService.StartupValueName}:{ExpectedLauncherCommand}"],
            registry.Calls);
        Assert.Equal(
            [$"get:{SystemStartupService.StartupTaskName}", $"delete:{SystemStartupService.StartupTaskName}"],
            task.Calls);
    }

    [Fact]
    public async Task DisabledLegacyTaskStaysDisabledAndIsCleanedUp()
    {
        FakeStartupTask task = new(LegacyTask(enabled: false));
        FakeStartupRegistry registry = new(new StartupRegistrySnapshot(LegacyMainCommand, "enabled"));

        await CreateService(task, registry).RepairLegacyStartupRegistrationAsync();

        Assert.Contains($"delete:{SystemStartupService.StartupValueName}", registry.Calls);
        Assert.DoesNotContain(registry.Calls, call => call.StartsWith("set:", StringComparison.Ordinal));
        Assert.Contains($"delete:{SystemStartupService.StartupTaskName}", task.Calls);
    }

    [Theory]
    [InlineData("enabled", true)]
    [InlineData("missing", true)]
    [InlineData("disabled", false)]
    public async Task MigratesLegacyRunRegistrationWithoutChangingUserChoice(string startupApproved, bool expectedEnabled)
    {
        FakeStartupRegistry registry = new(new StartupRegistrySnapshot(LegacyMainCommand, startupApproved));

        await CreateService(new FakeStartupTask(), registry).RepairLegacyStartupRegistrationAsync();

        string expectedCall = expectedEnabled
            ? $"set:{SystemStartupService.StartupValueName}:{ExpectedLauncherCommand}"
            : $"delete:{SystemStartupService.StartupValueName}";
        Assert.Contains(expectedCall, registry.Calls);
    }

    [Fact]
    public async Task HealthyLauncherRegistrationCleansRecognizedTaskWithoutRewritingRegistry()
    {
        FakeStartupTask task = new(LegacyTask(enabled: true));
        FakeStartupRegistry registry = new(new StartupRegistrySnapshot(ExpectedLauncherCommand, "enabled"));

        await CreateService(task, registry).RepairLegacyStartupRegistrationAsync();

        Assert.DoesNotContain(registry.Calls, call => call.StartsWith("set:", StringComparison.Ordinal));
        Assert.Contains($"delete:{SystemStartupService.StartupTaskName}", task.Calls);
    }

    [Fact]
    public async Task DoesNotModifyUnrecognizedTaskOrRegistration()
    {
        FakeStartupTask task = new(new StartupTaskSnapshot(true, true, "C:\\Other\\App.exe", [], "0"));
        FakeStartupRegistry registry = new(new StartupRegistrySnapshot("\"C:\\Other\\App.exe\"", "enabled"));

        await CreateService(task, registry).RepairLegacyStartupRegistrationAsync();

        Assert.Single(task.Calls);
        Assert.Single(registry.Calls);
    }

    private static StartupTaskSnapshot LegacyTask(bool enabled)
    {
        return new StartupTaskSnapshot(true, enabled, MainExecutablePath, [SystemStartupService.StartHiddenArg], "-2147024156");
    }

    private static SystemStartupService CreateService(
        FakeStartupTask task,
        FakeStartupRegistry registry,
        string platform = "win32",
        bool isPackaged = true)
    {
        return new SystemStartupService(
            platform,
            isPackaged,
            MainExecutablePath,
            LauncherPath,
            task,
            registry,
            WindowsStartupRegistry.StartupCommandFor);
    }

    private sealed class FakeStartupTask : IStartupTask
    {
        private readonly StartupTaskSnapshot snapshot;
        public List<string> Calls { get; } = [];

        public FakeStartupTask(StartupTaskSnapshot? snapshot = null)
        {
            this.snapshot = snapshot ?? new StartupTaskSnapshot(false, false, null, [], null);
        }

        public Task<StartupTaskSnapshot> GetAsync(string taskName)
        {
            Calls.Add($"get:{taskName}");
            return Task.FromResult(snapshot);
        }

        public Task SetAsync(string taskName, string executablePath, IReadOnlyList<string> args)
        {
            Calls.Add($"set:{taskName}:{executablePath}:{string.Join(" ", args)}");
            return Task.CompletedTask;
        }

        public Task DeleteAsync(string taskName)
        {
            Calls.Add($"delete:{taskName}");
            return Task.CompletedTask;
        }
    }

    private sealed class FakeStartupRegistry : IStartupRegistry
    {
        private readonly StartupRegistrySnapshot initial;
        private StartupRegistrySnapshot current;
        public List<string> Calls { get; } = [];

        public FakeStartupRegistry(StartupRegistrySnapshot? entry = null)
        {
            initial = entry ?? new StartupRegistrySnapshot(null, "missing");
            current = initial;
        }

        public Task<StartupRegistrySnapshot> GetEntryAsync(string valueName)
        {
            Calls.Add($"get:{valueName}");
            return Task.FromResult(current);
        }

        public Task SetEntryAsync(string valueName, string command)
        {
            Calls.Add($"set:{valueName}:{command}");
            current = new StartupRegistrySnapshot(command, "enabled");
            return Task.CompletedTask;
        }

        public Task DeleteEntryAsync(string valueName)
        {
            Calls.Add($"delete:{valueName}");
            current = new StartupRegistrySnapshot(null, "missing");
            return Task.CompletedTask;
        }
    }
}
