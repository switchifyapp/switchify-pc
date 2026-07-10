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
    public const string StartupLauncherFileName = "Switchify PC Startup.exe";

    private readonly string platform;
    private readonly bool isPackaged;
    private readonly string mainExecutablePath;
    private readonly string startupLauncherPath;
    private readonly IStartupTask startupTask;
    private readonly IStartupRegistry startupRegistry;
    private readonly Func<string, IReadOnlyList<string>, string> startupCommandFor;
    private readonly Action<string> warn;

    public SystemStartupService(
        string platform,
        bool isPackaged,
        string mainExecutablePath,
        string startupLauncherPath,
        IStartupTask startupTask,
        IStartupRegistry startupRegistry,
        Func<string, IReadOnlyList<string>, string> startupCommandFor,
        Action<string>? warn = null)
    {
        this.platform = platform;
        this.isPackaged = isPackaged;
        this.mainExecutablePath = mainExecutablePath;
        this.startupLauncherPath = startupLauncherPath;
        this.startupTask = startupTask;
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

        string expectedCommand = ExpectedLauncherCommand();
        IReadOnlyList<string> expectedTaskArguments = [];
        StartupTaskSnapshot task = await GetTaskSafelyAsync();
        StartupRegistrySnapshot entry = await GetRegistryEntrySafelyAsync();
        bool registrationMatches = entry.Command == expectedCommand && entry.StartupApproved != "disabled";

        return new SystemStartupSettings(
            Supported: true,
            StartWithSystem: registrationMatches,
            StartsHidden: true,
            Reason: null,
            Registration: new StartupRegistration(expectedCommand, entry.Command, entry.StartupApproved),
            TaskRegistration: new StartupTaskRegistration(
                StartupTaskName,
                task.Exists,
                task.Enabled,
                startupLauncherPath,
                task.ExecutablePath,
                expectedTaskArguments,
                task.Arguments,
                task.LastRunResult));
    }

    public async Task<SystemStartupSettings> SetStartWithSystemAsync(bool enabled)
    {
        if (!IsSupported()) return UnsupportedSettings();
        if (enabled)
        {
            await startupRegistry.SetEntryAsync(StartupValueName, ExpectedLauncherCommand());
            await DeleteLegacyTaskSafelyAsync();
        }
        else
        {
            await startupRegistry.DeleteEntryAsync(StartupValueName);
            await DeleteLegacyTaskSafelyAsync();
        }

        return await GetSettingsAsync();
    }

    public async Task RepairLegacyStartupRegistrationAsync()
    {
        if (!IsSupported()) return;

        StartupTaskSnapshot task = await GetTaskSafelyAsync();
        StartupRegistrySnapshot registration = await GetRegistryEntrySafelyAsync();

        if (registration.Command == ExpectedLauncherCommand())
        {
            if (IsRecognizedLegacyTask(task)) await DeleteLegacyTaskSafelyAsync();
            return;
        }

        if (IsRecognizedLegacyTask(task))
        {
            if (task.Enabled)
            {
                await startupRegistry.SetEntryAsync(StartupValueName, ExpectedLauncherCommand());
            }
            else
            {
                await startupRegistry.DeleteEntryAsync(StartupValueName);
            }

            await DeleteLegacyTaskSafelyAsync();
            return;
        }

        if (registration.Command == LegacyMainCommand())
        {
            if (registration.StartupApproved != "disabled")
            {
                await startupRegistry.SetEntryAsync(StartupValueName, ExpectedLauncherCommand());
            }
            else
            {
                await startupRegistry.DeleteEntryAsync(StartupValueName);
            }
        }
    }

    private bool IsSupported()
    {
        return platform == "win32" && isPackaged;
    }

    private string ExpectedLauncherCommand()
    {
        return startupCommandFor(startupLauncherPath, []);
    }

    private string LegacyMainCommand()
    {
        return startupCommandFor(mainExecutablePath, [StartHiddenArg]);
    }

    private bool IsRecognizedLegacyTask(StartupTaskSnapshot task)
    {
        return task.Exists &&
            string.Equals(task.ExecutablePath, mainExecutablePath, StringComparison.Ordinal) &&
            task.Arguments.SequenceEqual([StartHiddenArg], StringComparer.Ordinal);
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
            return await startupRegistry.GetEntryAsync(StartupValueName);
        }
        catch (Exception error)
        {
            warn(error.Message);
            return new StartupRegistrySnapshot(null, "unknown");
        }
    }

    private async Task DeleteLegacyTaskSafelyAsync()
    {
        try
        {
            await startupTask.DeleteAsync(StartupTaskName);
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
