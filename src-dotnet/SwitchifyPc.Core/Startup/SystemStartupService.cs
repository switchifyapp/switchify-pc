namespace SwitchifyPc.Core.Startup;

public sealed record StartupRegistration(string ExpectedCommand, string? RegisteredCommand, string StartupApproved);

public sealed record SystemStartupSettings(
    bool Supported,
    bool StartWithSystem,
    bool StartsHidden,
    string? Reason,
    StartupRegistration? Registration = null);

public interface IStartupRegistry
{
    Task<StartupRegistrySnapshot> GetEntryAsync(string valueName);
    Task SetEntryAsync(string valueName, string command);
    Task DeleteEntryAsync(string valueName);
}

public sealed record StartupRegistrySnapshot(string? Command, string StartupApproved);

public interface ISystemStartupSettingsService
{
    Task<SystemStartupSettings> GetSettingsAsync();

    Task<SystemStartupSettings> SetStartWithSystemAsync(bool enabled);
}

public sealed class SystemStartupService : ISystemStartupSettingsService
{
    public const string StartHiddenArg = "--start-hidden";
    public const string StartupValueName = "app.switchify.pc";

    private readonly string platform;
    private readonly bool isPackaged;
    private readonly string executablePath;
    private readonly IStartupRegistry startupRegistry;
    private readonly Func<string, IReadOnlyList<string>, string> startupCommandFor;
    private readonly Action<string> warn;

    public SystemStartupService(
        string platform,
        bool isPackaged,
        string executablePath,
        IStartupRegistry startupRegistry,
        Func<string, IReadOnlyList<string>, string> startupCommandFor,
        Action<string>? warn = null)
    {
        this.platform = platform;
        this.isPackaged = isPackaged;
        this.executablePath = executablePath;
        this.startupRegistry = startupRegistry;
        this.startupCommandFor = startupCommandFor;
        this.warn = warn ?? Console.WriteLine;
    }

    public static bool ShouldStartHidden(IReadOnlyList<string> args, string platform)
    {
        return platform == "win32" && args.Contains(StartHiddenArg, StringComparer.Ordinal);
    }

    public async Task<SystemStartupSettings> GetSettingsAsync()
    {
        if (!IsSupported()) return UnsupportedSettings();

        string expectedCommand = ExpectedCommand();
        StartupRegistrySnapshot entry = await GetRegistryEntrySafelyAsync();
        return new SystemStartupSettings(
            Supported: true,
            StartWithSystem: entry.Command == expectedCommand && entry.StartupApproved != "disabled",
            StartsHidden: true,
            Reason: null,
            Registration: new StartupRegistration(expectedCommand, entry.Command, entry.StartupApproved));
    }

    public async Task<SystemStartupSettings> SetStartWithSystemAsync(bool enabled)
    {
        if (!IsSupported()) return UnsupportedSettings();
        if (enabled)
        {
            await startupRegistry.SetEntryAsync(StartupValueName, ExpectedCommand());
        }
        else
        {
            await startupRegistry.DeleteEntryAsync(StartupValueName);
        }

        return await GetSettingsAsync();
    }

    private bool IsSupported()
    {
        return platform == "win32" && isPackaged;
    }

    private string ExpectedCommand()
    {
        return startupCommandFor(executablePath, [StartHiddenArg]);
    }

    private async Task<StartupRegistrySnapshot> GetRegistryEntrySafelyAsync()
    {
        try
        {
            return await startupRegistry.GetEntryAsync(StartupValueName);
        }
        catch (Exception error)
        {
            warn(error.Message);
            return new StartupRegistrySnapshot(null, "unknown");
        }
    }

    private SystemStartupSettings UnsupportedSettings()
    {
        return new SystemStartupSettings(
            Supported: false,
            StartWithSystem: false,
            StartsHidden: true,
            Reason: platform == "win32" ? "unpackaged" : "unsupported_platform");
    }
}
