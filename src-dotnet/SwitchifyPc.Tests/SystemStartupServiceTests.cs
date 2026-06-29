using SwitchifyPc.Core.Startup;
using SwitchifyPc.Windows.Startup;

namespace SwitchifyPc.Tests;

public sealed class SystemStartupServiceTests
{
    private const string ExpectedCommand = "\"C:\\Program Files\\Switchify PC\\Switchify PC.exe\" --start-hidden";

    [Fact]
    public void ShouldStartHiddenOnlyOnWindowsWithHiddenArg()
    {
        Assert.True(SystemStartupService.ShouldStartHidden(["Switchify PC.exe", "--start-hidden"], "win32"));
        Assert.False(SystemStartupService.ShouldStartHidden(["Switchify PC.exe", "--start-hidden"], "darwin"));
        Assert.False(SystemStartupService.ShouldStartHidden(["Switchify PC.exe"], "win32"));
    }

    [Fact]
    public async Task UnsupportedPlatformsDoNotReadRegistry()
    {
        FakeStartupRegistry registry = new();
        SystemStartupService service = CreateService(registry, platform: "darwin", isPackaged: true);

        SystemStartupSettings settings = await service.GetSettingsAsync();
        await service.SetStartWithSystemAsync(true);

        Assert.False(settings.Supported);
        Assert.Equal("unsupported_platform", settings.Reason);
        Assert.Empty(registry.Calls);
    }

    [Fact]
    public async Task ReportsEnabledWhenExpectedCommandMatchesAndStartupApprovedIsNotDisabled()
    {
        FakeStartupRegistry registry = new(new StartupRegistrySnapshot(ExpectedCommand, "missing"));
        SystemStartupService service = CreateService(registry);

        SystemStartupSettings settings = await service.GetSettingsAsync();

        Assert.True(settings.StartWithSystem);
        Assert.Equal(ExpectedCommand, settings.Registration?.ExpectedCommand);
        Assert.Equal("missing", settings.Registration?.StartupApproved);
    }

    [Fact]
    public async Task ReportsDisabledForDisabledStartupApprovedMissingCommandOrStaleCommand()
    {
        Assert.False((await CreateService(new FakeStartupRegistry(new StartupRegistrySnapshot(ExpectedCommand, "disabled"))).GetSettingsAsync()).StartWithSystem);
        Assert.False((await CreateService(new FakeStartupRegistry(new StartupRegistrySnapshot(null, "missing"))).GetSettingsAsync()).StartWithSystem);
        Assert.False((await CreateService(new FakeStartupRegistry(new StartupRegistrySnapshot("\"C:\\Old\\Switchify PC.exe\" --start-hidden", "enabled"))).GetSettingsAsync()).StartWithSystem);
    }

    [Fact]
    public async Task EnablesAndDisablesStartupThroughRegistry()
    {
        FakeStartupRegistry registry = new();
        SystemStartupService service = CreateService(registry);

        await service.SetStartWithSystemAsync(true);
        await service.SetStartWithSystemAsync(false);

        Assert.Contains($"set:app.switchify.pc:{ExpectedCommand}", registry.Calls);
        Assert.Contains("delete:app.switchify.pc", registry.Calls);
    }

    private static SystemStartupService CreateService(FakeStartupRegistry registry, string platform = "win32", bool isPackaged = true)
    {
        return new SystemStartupService(
            platform,
            isPackaged,
            "C:\\Program Files\\Switchify PC\\Switchify PC.exe",
            registry,
            WindowsStartupRegistry.StartupCommandFor);
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
