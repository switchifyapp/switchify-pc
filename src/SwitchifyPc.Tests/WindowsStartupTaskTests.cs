using SwitchifyPc.Core.Startup;
using SwitchifyPc.Windows.Startup;

namespace SwitchifyPc.Tests;

public sealed class WindowsStartupTaskTests
{
    [Fact]
    public async Task CreatesScheduledTaskForCurrentUserLogon()
    {
        List<IReadOnlyList<string>> calls = [];
        WindowsStartupTask task = new((_, args) =>
        {
            calls.Add(args);
            return Task.FromResult(new CommandResult("", ""));
        });

        await task.SetAsync("Switchify PC", "C:\\Program Files\\Switchify PC\\Switchify PC.exe", ["--start-hidden"]);

        IReadOnlyList<string> create = calls.Single();
        Assert.Contains("/Create", create);
        Assert.Contains("/TN", create);
        Assert.Contains("Switchify PC", create);
        Assert.Contains("/SC", create);
        Assert.Contains("ONLOGON", create);
        Assert.Contains("/TR", create);
        Assert.Contains("\"C:\\Program Files\\Switchify PC\\Switchify PC.exe\" --start-hidden", create);
        Assert.Contains("/RL", create);
        Assert.Contains("LIMITED", create);
        Assert.Contains("/F", create);
    }

    [Fact]
    public async Task DeletesScheduledTaskAndIgnoresMissingTask()
    {
        List<IReadOnlyList<string>> calls = [];
        WindowsStartupTask task = new((_, args) =>
        {
            calls.Add(args);
            throw new RegistryCommandException(1, "", "ERROR: The system cannot find the file specified.");
        });

        await task.DeleteAsync("Switchify PC");

        IReadOnlyList<string> delete = calls.Single();
        Assert.Contains("/Delete", delete);
        Assert.Contains("/TN", delete);
        Assert.Contains("Switchify PC", delete);
        Assert.Contains("/F", delete);
    }

    [Fact]
    public async Task MissingScheduledTaskReturnsMissingSnapshot()
    {
        WindowsStartupTask task = new((_, _) => throw new RegistryCommandException(1, "", "ERROR: The system cannot find the file specified."));

        StartupTaskSnapshot snapshot = await task.GetAsync("Switchify PC");

        Assert.False(snapshot.Exists);
        Assert.False(snapshot.Enabled);
        Assert.Null(snapshot.ExecutablePath);
        Assert.Empty(snapshot.Arguments);
    }

    [Fact]
    public void ParsesEnabledScheduledTaskXml()
    {
        StartupTaskSnapshot snapshot = WindowsStartupTask.ParseTaskXml(TaskXml(enabled: true), "0");

        Assert.True(snapshot.Exists);
        Assert.True(snapshot.Enabled);
        Assert.Equal("C:\\Program Files\\Switchify PC\\Switchify PC.exe", snapshot.ExecutablePath);
        Assert.Equal(["--start-hidden"], snapshot.Arguments);
        Assert.Equal("0", snapshot.LastRunResult);
    }

    [Fact]
    public void ParsesDisabledScheduledTaskXml()
    {
        StartupTaskSnapshot snapshot = WindowsStartupTask.ParseTaskXml(TaskXml(enabled: false));

        Assert.True(snapshot.Exists);
        Assert.False(snapshot.Enabled);
    }

    [Fact]
    public void ParsesCombinedQuotedCommandWhenArgumentsAreMissing()
    {
        string xml = """
        <Task>
          <Settings><Enabled>true</Enabled></Settings>
          <Actions><Exec><Command>"C:\Program Files\Switchify PC\Switchify PC.exe" --start-hidden</Command></Exec></Actions>
        </Task>
        """;

        StartupTaskSnapshot snapshot = WindowsStartupTask.ParseTaskXml(xml);

        Assert.Equal("C:\\Program Files\\Switchify PC\\Switchify PC.exe", snapshot.ExecutablePath);
        Assert.Equal(["--start-hidden"], snapshot.Arguments);
    }

    [Fact]
    public async Task MalformedXmlDoesNotThrowDuringRead()
    {
        WindowsStartupTask task = new((_, args) =>
        {
            if (args.Contains("/XML"))
            {
                return Task.FromResult(new CommandResult("not xml", ""));
            }

            return Task.FromResult(new CommandResult("", ""));
        });

        StartupTaskSnapshot snapshot = await task.GetAsync("Switchify PC");

        Assert.True(snapshot.Exists);
        Assert.False(snapshot.Enabled);
    }

    [Fact]
    public void ParsesLastRunResultFromListOutput()
    {
        string output = """
        TaskName: \Switchify PC
        Last Run Time: 30/06/2026 15:00:00
        Last Result: 0
        """;

        Assert.Equal("0", WindowsStartupTask.ParseLastRunResult(output));
    }

    private static string TaskXml(bool enabled)
    {
        return $$"""
        <Task xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
          <Settings>
            <Enabled>{{enabled.ToString().ToLowerInvariant()}}</Enabled>
          </Settings>
          <Actions>
            <Exec>
              <Command>C:\Program Files\Switchify PC\Switchify PC.exe</Command>
              <Arguments>--start-hidden</Arguments>
            </Exec>
          </Actions>
        </Task>
        """;
    }
}
