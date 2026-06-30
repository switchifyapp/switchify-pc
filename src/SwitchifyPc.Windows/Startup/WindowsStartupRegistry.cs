using System.Diagnostics;
using System.Text.RegularExpressions;
using SwitchifyPc.Core.Startup;

namespace SwitchifyPc.Windows.Startup;

public sealed record StartupRegistryEntry(string? Command, string StartupApproved);

public sealed record CommandResult(string Stdout, string Stderr);

public delegate Task<CommandResult> CommandRunner(string file, IReadOnlyList<string> args);

public sealed class WindowsStartupRegistry : IStartupRegistry
{
    public const string RunKey = @"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    public const string StartupApprovedRunKey = @"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
    public const string StartupApprovedEnabledHex = "020000000000000000000000";

    private readonly CommandRunner commandRunner;

    public WindowsStartupRegistry(CommandRunner? commandRunner = null)
    {
        this.commandRunner = commandRunner ?? RunCommandAsync;
    }

    public async Task<StartupRegistryEntry> GetEntryAsync(string valueName)
    {
        string? command = await QueryRunCommandAsync(valueName);
        string startupApproved = await QueryStartupApprovedAsync(valueName);
        return new StartupRegistryEntry(command, startupApproved);
    }

    async Task<StartupRegistrySnapshot> IStartupRegistry.GetEntryAsync(string valueName)
    {
        StartupRegistryEntry entry = await GetEntryAsync(valueName);
        return new StartupRegistrySnapshot(entry.Command, entry.StartupApproved);
    }

    public async Task SetEntryAsync(string valueName, string command)
    {
        await commandRunner("reg.exe", ["add", RunKey, "/v", valueName, "/t", "REG_SZ", "/d", command, "/f"]);
        await commandRunner("reg.exe", ["add", StartupApprovedRunKey, "/v", valueName, "/t", "REG_BINARY", "/d", StartupApprovedEnabledHex, "/f"]);
    }

    public async Task DeleteEntryAsync(string valueName)
    {
        await IgnoreMissingValueAsync(commandRunner("reg.exe", ["delete", RunKey, "/v", valueName, "/f"]));
        await IgnoreMissingValueAsync(commandRunner("reg.exe", ["delete", StartupApprovedRunKey, "/v", valueName, "/f"]));
    }

    public static string StartupCommandFor(string executablePath, IReadOnlyList<string> args)
    {
        if (executablePath.Contains('"', StringComparison.Ordinal))
        {
            throw new InvalidOperationException("Startup executable path cannot contain quotes.");
        }

        if (args.Any(arg => arg.Contains('"', StringComparison.Ordinal)))
        {
            throw new InvalidOperationException("Startup arguments cannot contain quotes.");
        }

        return string.Join(" ", new[] { $"\"{executablePath}\"" }.Concat(args));
    }

    public static string StartupApprovedStateFromHex(string? value)
    {
        if (string.IsNullOrWhiteSpace(value)) return "unknown";
        string firstByte = Regex.Replace(value, "\\s+", "").ToLowerInvariant()[..Math.Min(2, Regex.Replace(value, "\\s+", "").Length)];
        return firstByte switch
        {
            "02" => "enabled",
            "03" => "disabled",
            _ => "unknown"
        };
    }

    internal static string? ParseRegistryValue(string stdout, string valueName, string valueType)
    {
        string escapedName = Regex.Escape(valueName);
        Match match = Regex.Match(stdout, $"^\\s*{escapedName}\\s+{valueType}\\s+(.+?)\\s*$", RegexOptions.IgnoreCase | RegexOptions.Multiline);
        return match.Success ? match.Groups[1].Value.Trim() : null;
    }

    private async Task<string?> QueryRunCommandAsync(string valueName)
    {
        try
        {
            CommandResult result = await commandRunner("reg.exe", ["query", RunKey, "/v", valueName]);
            return ParseRegistryValue(result.Stdout, valueName, "REG_SZ");
        }
        catch (Exception error) when (IsMissingRegistryValueError(error))
        {
            return null;
        }
    }

    private async Task<string> QueryStartupApprovedAsync(string valueName)
    {
        try
        {
            CommandResult result = await commandRunner("reg.exe", ["query", StartupApprovedRunKey, "/v", valueName]);
            return StartupApprovedStateFromHex(ParseRegistryValue(result.Stdout, valueName, "REG_BINARY"));
        }
        catch (Exception error) when (IsMissingRegistryValueError(error))
        {
            return "missing";
        }
    }

    private static async Task IgnoreMissingValueAsync(Task promise)
    {
        try
        {
            await promise;
        }
        catch (Exception error) when (IsMissingRegistryValueError(error))
        {
        }
    }

    private static bool IsMissingRegistryValueError(Exception error)
    {
        string output = $"{error.Message}".ToLowerInvariant();
        return error is RegistryCommandException { ExitCode: 1 } || output.Contains("unable to find", StringComparison.Ordinal) || output.Contains("cannot find", StringComparison.Ordinal);
    }

    private static async Task<CommandResult> RunCommandAsync(string file, IReadOnlyList<string> args)
    {
        ProcessStartInfo startInfo = new(file)
        {
            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            CreateNoWindow = true
        };
        foreach (string arg in args)
        {
            startInfo.ArgumentList.Add(arg);
        }

        using Process process = Process.Start(startInfo) ?? throw new InvalidOperationException("Could not start registry command.");
        string stdout = await process.StandardOutput.ReadToEndAsync();
        string stderr = await process.StandardError.ReadToEndAsync();
        await process.WaitForExitAsync();
        if (process.ExitCode != 0)
        {
            throw new RegistryCommandException(process.ExitCode, stdout, stderr);
        }

        return new CommandResult(stdout, stderr);
    }
}

public sealed class RegistryCommandException : Exception
{
    public RegistryCommandException(int exitCode, string stdout, string stderr) : base($"{stdout}\n{stderr}")
    {
        ExitCode = exitCode;
        Stdout = stdout;
        Stderr = stderr;
    }

    public int ExitCode { get; }
    public string Stdout { get; }
    public string Stderr { get; }
}
