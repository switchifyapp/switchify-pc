using SwitchifyPc.Core.Startup;
using SwitchifyPc.Windows.Startup;

namespace SwitchifyPc.Tests;

public sealed class SystemStartupServiceTests
{
    private const string ExecutablePath = "C:\\Program Files\\Switchify PC\\Switchify PC.exe";
    private const string ExpectedCommand = "\"C:\\Program Files\\Switchify PC\\Switchify PC.exe\" --start-hidden";

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
        SystemStartupService service = CreateService(task, registry, isPackaged: false);

        SystemStartupSettings settings = await service.GetSettingsAsync();

        Assert.False(settings.Supported);
        Assert.Equal("unpackaged", settings.Reason);
        Assert.Empty(task.Calls);
        Assert.Empty(registry.Calls);
    }

    [Fact]
    public async Task ReportsEnabledOnlyForHealthyScheduledTask()
    {
        SystemStartupSettings settings = await CreateService(new FakeStartupTask(HealthyTask()), new FakeStartupRegistry()).GetSettingsAsync();

        Assert.True(settings.StartWithSystem);
        Assert.Equal(SystemStartupService.StartupTaskName, settings.TaskRegistration?.TaskName);
        Assert.Equal(ExecutablePath, settings.TaskRegistration?.ExpectedExecutablePath);
        Assert.Equal(ExecutablePath, settings.TaskRegistration?.RegisteredExecutablePath);
        Assert.Equal(["--start-hidden"], settings.TaskRegistration?.RegisteredArguments);
    }

    [Fact]
    public async Task ReportsEnabledForHealthyTaskWhenWindowsXmlOmitsEnabled()
    {
        StartupTaskSnapshot taskParsedFromWindowsXml = new(
            Exists: true,
            Enabled: true,
            ExecutablePath: ExecutablePath,
            Arguments: ["--start-hidden"],
            LastRunResult: "0");

        SystemStartupSettings settings = await CreateService(new FakeStartupTask(taskParsedFromWindowsXml), new FakeStartupRegistry()).GetSettingsAsync();

        Assert.True(settings.StartWithSystem);
        Assert.True(settings.TaskRegistration?.Enabled);
    }

    [Fact]
    public async Task ReportsDisabledForMissingDisabledStalePathOrStaleArguments()
    {
        Assert.False((await CreateService(new FakeStartupTask(new StartupTaskSnapshot(false, false, null, [], null)), new FakeStartupRegistry()).GetSettingsAsync()).StartWithSystem);
        Assert.False((await CreateService(new FakeStartupTask(HealthyTask() with { Enabled = false }), new FakeStartupRegistry()).GetSettingsAsync()).StartWithSystem);
        Assert.False((await CreateService(new FakeStartupTask(HealthyTask() with { ExecutablePath = "C:\\Old\\Switchify PC.exe" }), new FakeStartupRegistry()).GetSettingsAsync()).StartWithSystem);
        Assert.False((await CreateService(new FakeStartupTask(HealthyTask() with { Arguments = ["--show"] }), new FakeStartupRegistry()).GetSettingsAsync()).StartWithSystem);
    }

    [Fact]
    public async Task EnablesAndDisablesStartupThroughScheduledTaskAndLegacyCleanup()
    {
        FakeStartupTask task = new();
        FakeStartupRegistry registry = new();
        SystemStartupService service = CreateService(task, registry);

        await service.SetStartWithSystemAsync(true);
        await service.SetStartWithSystemAsync(false);

        Assert.Contains($"set:{SystemStartupService.StartupTaskName}:{ExecutablePath}:--start-hidden", task.Calls);
        Assert.Contains($"delete:{SystemStartupService.StartupTaskName}", task.Calls);
        Assert.Equal(2, registry.Calls.Count(call => call == $"delete:{SystemStartupService.StartupValueName}"));
    }

    [Fact]
    public async Task HealthyTaskCausesLegacyRegistryCleanupDuringRepair()
    {
        FakeStartupTask task = new(HealthyTask());
        FakeStartupRegistry registry = new(new StartupRegistrySnapshot(ExpectedCommand, "enabled"));
        SystemStartupService service = CreateService(task, registry);

        await service.RepairLegacyStartupRegistrationAsync();

        Assert.Contains($"delete:{SystemStartupService.StartupValueName}", registry.Calls);
        Assert.DoesNotContain(task.Calls, call => call.StartsWith("set:", StringComparison.Ordinal));
    }

    [Fact]
    public async Task MigratesHealthyLegacyRegistryToScheduledTask()
    {
        FakeStartupTask task = new(new StartupTaskSnapshot(false, false, null, [], null));
        FakeStartupRegistry registry = new(new StartupRegistrySnapshot(ExpectedCommand, "enabled"));
        SystemStartupService service = CreateService(task, registry);

        await service.RepairLegacyStartupRegistrationAsync();

        Assert.Contains($"set:{SystemStartupService.StartupTaskName}:{ExecutablePath}:--start-hidden", task.Calls);
        Assert.Contains($"delete:{SystemStartupService.StartupValueName}", registry.Calls);
    }

    [Fact]
    public async Task DoesNotMigrateDisabledOrStaleLegacyRegistry()
    {
        FakeStartupTask disabledTask = new(new StartupTaskSnapshot(false, false, null, [], null));
        FakeStartupRegistry disabledRegistry = new(new StartupRegistrySnapshot(ExpectedCommand, "disabled"));
        await CreateService(disabledTask, disabledRegistry).RepairLegacyStartupRegistrationAsync();

        FakeStartupTask staleTask = new(new StartupTaskSnapshot(false, false, null, [], null));
        FakeStartupRegistry staleRegistry = new(new StartupRegistrySnapshot("\"C:\\Old\\Switchify PC.exe\" --start-hidden", "enabled"));
        await CreateService(staleTask, staleRegistry).RepairLegacyStartupRegistrationAsync();

        Assert.DoesNotContain(disabledTask.Calls, call => call.StartsWith("set:", StringComparison.Ordinal));
        Assert.DoesNotContain(staleTask.Calls, call => call.StartsWith("set:", StringComparison.Ordinal));
    }

    private static StartupTaskSnapshot HealthyTask()
    {
        return new StartupTaskSnapshot(true, true, ExecutablePath, ["--start-hidden"], "0");
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
            ExecutablePath,
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
        private readonly StartupRegistrySnapshot entry;
        public List<string> Calls { get; } = [];

        public FakeStartupRegistry(StartupRegistrySnapshot? entry = null)
        {
            this.entry = entry ?? new StartupRegistrySnapshot(null, "missing");
        }

        public Task<StartupRegistrySnapshot> GetEntryAsync(string valueName)
        {
            Calls.Add($"get:{valueName}");
            return Task.FromResult(entry);
        }

        public Task SetEntryAsync(string valueName, string command)
        {
            Calls.Add($"set:{valueName}:{command}");
            return Task.CompletedTask;
        }

        public Task DeleteEntryAsync(string valueName)
        {
            Calls.Add($"delete:{valueName}");
            return Task.CompletedTask;
        }
    }
}
