using System.Xml.Linq;
using SwitchifyPc.Core.Startup;

namespace SwitchifyPc.Windows.Startup;

public sealed class WindowsStartupTask : IStartupTask
{
    public const string StartupTaskName = "Switchify PC";
    public const string StartHiddenArg = "--start-hidden";

    private readonly CommandRunner commandRunner;
    private readonly Action<string> warn;

    public WindowsStartupTask(CommandRunner? commandRunner = null, Action<string>? warn = null)
    {
        this.commandRunner = commandRunner ?? RunCommandAsync;
        this.warn = warn ?? Console.WriteLine;
    }

    public async Task<StartupTaskSnapshot> GetAsync(string taskName)
    {
        string? lastRunResult = await QueryLastRunResultAsync(taskName);

        try
        {
            CommandResult result = await commandRunner("schtasks.exe", ["/Query", "/TN", taskName, "/XML"]);
            StartupTaskSnapshot snapshot = ParseTaskXml(result.Stdout, lastRunResult);
            return snapshot with { LastRunResult = lastRunResult };
        }
        catch (Exception error) when (IsMissingTaskError(error))
        {
            return new StartupTaskSnapshot(false, false, null, [], lastRunResult);
        }
        catch (Exception error) when (error is System.Xml.XmlException or InvalidOperationException)
        {
            warn("Switchify startup task could not be read.");
            return new StartupTaskSnapshot(true, false, null, [], lastRunResult);
        }
    }

    public async Task SetAsync(string taskName, string executablePath, IReadOnlyList<string> args)
    {
        await commandRunner("schtasks.exe",
        [
            "/Create",
            "/TN",
            taskName,
            "/SC",
            "ONLOGON",
            "/TR",
            StartupCommandFor(executablePath, args),
            "/RL",
            "LIMITED",
            "/F"
        ]);
    }

    public async Task DeleteAsync(string taskName)
    {
        try
        {
            await commandRunner("schtasks.exe", ["/Delete", "/TN", taskName, "/F"]);
        }
        catch (Exception error) when (IsMissingTaskError(error))
        {
        }
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

    public static StartupTaskSnapshot ParseTaskXml(string xml, string? lastRunResult = null)
    {
        XDocument document = XDocument.Parse(xml);
        XElement root = document.Root ?? throw new InvalidOperationException("Task XML is empty.");
        string? enabledText = FirstDescendantValue(root, "Settings", "Enabled") ?? FirstDescendantValue(root, "Enabled");
        string? command = FirstDescendantValue(root, "Actions", "Exec", "Command") ?? FirstDescendantValue(root, "Command");
        string? argumentsText = FirstDescendantValue(root, "Actions", "Exec", "Arguments") ?? FirstDescendantValue(root, "Arguments");

        IReadOnlyList<string> arguments = SplitArguments(argumentsText);
        if (arguments.Count == 0 && TrySplitCombinedCommand(command, out string? splitCommand, out IReadOnlyList<string> splitArguments))
        {
            command = splitCommand;
            arguments = splitArguments;
        }

        return new StartupTaskSnapshot(
            Exists: true,
            Enabled: string.Equals(enabledText, "true", StringComparison.OrdinalIgnoreCase),
            ExecutablePath: string.IsNullOrWhiteSpace(command) ? null : command,
            Arguments: arguments,
            LastRunResult: lastRunResult);
    }

    public static string? ParseLastRunResult(string output)
    {
        foreach (string line in output.Split(["\r\n", "\n"], StringSplitOptions.RemoveEmptyEntries))
        {
            int separator = line.IndexOf(':', StringComparison.Ordinal);
            if (separator <= 0) continue;

            string key = line[..separator].Trim();
            if (!key.Equals("Last Result", StringComparison.OrdinalIgnoreCase)) continue;

            return line[(separator + 1)..].Trim();
        }

        return null;
    }

    private async Task<string?> QueryLastRunResultAsync(string taskName)
    {
        try
        {
            CommandResult result = await commandRunner("schtasks.exe", ["/Query", "/TN", taskName, "/FO", "LIST", "/V"]);
            return ParseLastRunResult(result.Stdout);
        }
        catch
        {
            return null;
        }
    }

    private static string? FirstDescendantValue(XElement root, params string[] path)
    {
        IEnumerable<XElement> current = [root];
        foreach (string segment in path)
        {
            XElement[] previous = current.ToArray();
            current = previous
                .SelectMany(element => element.Elements())
                .Where(element => element.Name.LocalName == segment)
                .ToArray();
        }

        return current.FirstOrDefault()?.Value.Trim();
    }

    private static IReadOnlyList<string> SplitArguments(string? value)
    {
        return string.IsNullOrWhiteSpace(value)
            ? []
            : value.Split(' ', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
    }

    private static bool TrySplitCombinedCommand(string? command, out string? executablePath, out IReadOnlyList<string> args)
    {
        executablePath = command;
        args = [];
        if (string.IsNullOrWhiteSpace(command) || !command.StartsWith('"')) return false;

        int closingQuote = command.IndexOf('"', 1);
        if (closingQuote <= 1) return false;

        executablePath = command[1..closingQuote];
        args = SplitArguments(command[(closingQuote + 1)..]);
        return true;
    }

    private static bool IsMissingTaskError(Exception error)
    {
        string output = error.Message.ToLowerInvariant();
        return error is RegistryCommandException { ExitCode: 1 } &&
            (output.Contains("cannot find", StringComparison.Ordinal) ||
             output.Contains("does not exist", StringComparison.Ordinal) ||
             output.Contains("unable to find", StringComparison.Ordinal));
    }

    private static async Task<CommandResult> RunCommandAsync(string file, IReadOnlyList<string> args)
    {
        return await WindowsStartupRegistry.RunCommandAsync(file, args);
    }
}
