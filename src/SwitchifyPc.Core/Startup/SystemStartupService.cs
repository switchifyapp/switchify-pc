namespace SwitchifyPc.Core.Startup;

public sealed record StartupRegistration(string ExpectedCommand, string? RegisteredCommand, string StartupApproved);

public sealed record StartupTaskRegistration(
    string TaskName,
    bool Exists,
    bool Enabled,
    string ExpectedExecutablePath,
    string? RegisteredExecutablePath,
    IReadOnlyList<string> ExpectedArguments,
    IReadOnlyList<string> RegisteredArguments,
    string? LastRunResult);

public sealed record SystemStartupSettings(
    bool Supported,
    bool StartWithSystem,
    bool StartsHidden,
    string? Reason,
    StartupRegistration? Registration = null,
    StartupTaskRegistration? TaskRegistration = null);

public interface IStartupRegistry
{
    Task<StartupRegistrySnapshot> GetEntryAsync(string valueName);
    Task SetEntryAsync(string valueName, string command);
    Task DeleteEntryAsync(string valueName);
}

public sealed record StartupRegistrySnapshot(string? Command, string StartupApproved);

public sealed record StartupTaskSnapshot(
    bool Exists,
    bool Enabled,
    string? ExecutablePath,
    IReadOnlyList<string> Arguments,
    string? LastRunResult);

public interface IStartupTask
{
    Task<StartupTaskSnapshot> GetAsync(string taskName);
    Task SetAsync(string taskName, string executablePath, IReadOnlyList<string> args);
    Task DeleteAsync(string taskName);
}

public interface ISystemStartupSettingsService
{
    Task<SystemStartupSettings> GetSettingsAsync();

    Task<SystemStartupSettings> SetStartWithSystemAsync(bool enabled);
}

public sealed class SystemStartupService : ISystemStartupSettingsService
{
    public const string StartHiddenArg = "--start-hidden";
    public const string StartupValueName = "app.switchify.pc";
    public const string StartupTaskName = "Switchify PC";

    private readonly string platform;
    private readonly bool isPackaged;
    private readonly string executablePath;
    private readonly IStartupTask startupTask;
    private readonly IStartupRegistry legacyStartupRegistry;
    private readonly Func<string, IReadOnlyList<string>, string> startupCommandFor;
    private readonly Action<string> warn;

    public SystemStartupService(
        string platform,
        bool isPackaged,
        string executablePath,
        IStartupTask startupTask,
        IStartupRegistry legacyStartupRegistry,
        Func<string, IReadOnlyList<string>, string> startupCommandFor,
        Action<string>? warn = null)
    {
        this.platform = platform;
        this.isPackaged = isPackaged;
        this.executablePath = executablePath;
        this.startupTask = startupTask;
        this.legacyStartupRegistry = legacyStartupRegistry;
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
        IReadOnlyList<string> expectedArguments = [StartHiddenArg];
        StartupTaskSnapshot task = await GetTaskSafelyAsync();
        StartupRegistrySnapshot entry = await GetRegistryEntrySafelyAsync();
        bool taskMatches = IsHealthyTask(task, expectedArguments);

        return new SystemStartupSettings(
            Supported: true,
            StartWithSystem: taskMatches,
            StartsHidden: true,
            Reason: null,
            Registration: new StartupRegistration(expectedCommand, entry.Command, entry.StartupApproved),
            TaskRegistration: new StartupTaskRegistration(
                StartupTaskName,
                task.Exists,
                task.Enabled,
                executablePath,
                task.ExecutablePath,
                expectedArguments,
                task.Arguments,
                task.LastRunResult));
    }

    public async Task<SystemStartupSettings> SetStartWithSystemAsync(bool enabled)
    {
        if (!IsSupported()) return UnsupportedSettings();
        if (enabled)
        {
            await startupTask.SetAsync(StartupTaskName, executablePath, [StartHiddenArg]);
            await DeleteLegacyRegistrySafelyAsync();
        }
        else
        {
            await startupTask.DeleteAsync(StartupTaskName);
            await DeleteLegacyRegistrySafelyAsync();
        }

        return await GetSettingsAsync();
    }

    public async Task RepairLegacyStartupRegistrationAsync()
    {
        if (!IsSupported()) return;

        IReadOnlyList<string> expectedArguments = [StartHiddenArg];
        StartupTaskSnapshot task = await GetTaskSafelyAsync();
        StartupRegistrySnapshot legacy = await GetRegistryEntrySafelyAsync();

        if (IsHealthyTask(task, expectedArguments))
        {
            await DeleteLegacyRegistrySafelyAsync();
            return;
        }

        if (legacy.Command == ExpectedCommand() && legacy.StartupApproved != "disabled")
        {
            await startupTask.SetAsync(StartupTaskName, executablePath, expectedArguments);
            await DeleteLegacyRegistrySafelyAsync();
        }
    }

    private bool IsSupported()
    {
        return platform == "win32" && isPackaged;
    }

    private string ExpectedCommand()
    {
        return startupCommandFor(executablePath, [StartHiddenArg]);
    }

    private bool IsHealthyTask(StartupTaskSnapshot task, IReadOnlyList<string> expectedArguments)
    {
        return task.Exists &&
            task.Enabled &&
            string.Equals(task.ExecutablePath, executablePath, StringComparison.Ordinal) &&
            task.Arguments.SequenceEqual(expectedArguments, StringComparer.Ordinal);
    }

    private async Task<StartupTaskSnapshot> GetTaskSafelyAsync()
    {
        try
        {
            return await startupTask.GetAsync(StartupTaskName);
        }
        catch (Exception error)
        {
            warn(error.Message);
            return new StartupTaskSnapshot(false, false, null, [], null);
        }
    }

    private async Task<StartupRegistrySnapshot> GetRegistryEntrySafelyAsync()
    {
        try
        {
            return await legacyStartupRegistry.GetEntryAsync(StartupValueName);
        }
        catch (Exception error)
        {
            warn(error.Message);
            return new StartupRegistrySnapshot(null, "unknown");
        }
    }

    private async Task DeleteLegacyRegistrySafelyAsync()
    {
        try
        {
            await legacyStartupRegistry.DeleteEntryAsync(StartupValueName);
        }
        catch (Exception error)
        {
            warn(error.Message);
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
