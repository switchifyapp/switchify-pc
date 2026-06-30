using SwitchifyPc.Windows.Startup;

namespace SwitchifyPc.Tests;

public sealed class WindowsStartupRegistryTests
{
    [Fact]
    public void StartupCommandForQuotesExecutableAndRejectsQuotes()
    {
        Assert.Equal(
            "\"C:\\Program Files\\Switchify PC\\Switchify PC.exe\" --start-hidden",
            WindowsStartupRegistry.StartupCommandFor("C:\\Program Files\\Switchify PC\\Switchify PC.exe", ["--start-hidden"]));
        Assert.Throws<InvalidOperationException>(() => WindowsStartupRegistry.StartupCommandFor("C:\\Bad\"Path\\app.exe", []));
        Assert.Throws<InvalidOperationException>(() => WindowsStartupRegistry.StartupCommandFor("C:\\app.exe", ["--bad\"arg"]));
    }

    [Theory]
    [InlineData("020000000000000000000000", "enabled")]
    [InlineData("03 00 00 00 00 00 00 00 00 00 00 00", "disabled")]
    [InlineData("ff000000", "unknown")]
    [InlineData(null, "unknown")]
    public void ParsesStartupApprovedState(string? value, string expected)
    {
        Assert.Equal(expected, WindowsStartupRegistry.StartupApprovedStateFromHex(value));
    }

    [Fact]
    public async Task ReadsRunCommandAndStartupApprovedState()
    {
        List<string> calls = [];
        WindowsStartupRegistry registry = new(async (file, args) =>
        {
            calls.Add($"{file} {string.Join(" ", args)}");
            if (args.Contains(WindowsStartupRegistry.RunKey))
            {
                return new CommandResult("    app.switchify.pc    REG_SZ    \"C:\\Program Files\\Switchify PC\\Switchify PC.exe\" --start-hidden", "");
            }

            return new CommandResult("    app.switchify.pc    REG_BINARY    020000000000000000000000", "");
        });

        StartupRegistryEntry entry = await registry.GetEntryAsync("app.switchify.pc");

        Assert.Equal("\"C:\\Program Files\\Switchify PC\\Switchify PC.exe\" --start-hidden", entry.Command);
        Assert.Equal("enabled", entry.StartupApproved);
        Assert.Equal(2, calls.Count);
    }

    [Fact]
    public async Task TreatsMissingValuesAsNullAndMissing()
    {
        WindowsStartupRegistry registry = new((_, _) => throw new RegistryCommandException(1, "", "unable to find"));

        StartupRegistryEntry entry = await registry.GetEntryAsync("app.switchify.pc");

        Assert.Null(entry.Command);
        Assert.Equal("missing", entry.StartupApproved);
    }

    [Fact]
    public async Task WritesAndDeletesRunAndStartupApprovedValues()
    {
        List<string> calls = [];
        WindowsStartupRegistry registry = new((file, args) =>
        {
            calls.Add($"{file} {string.Join(" ", args)}");
            return Task.FromResult(new CommandResult("", ""));
        });

        await registry.SetEntryAsync("app.switchify.pc", "\"C:\\app.exe\" --start-hidden");
        await registry.DeleteEntryAsync("app.switchify.pc");

        Assert.Contains(calls, call => call.Contains("add", StringComparison.Ordinal) && call.Contains(WindowsStartupRegistry.RunKey, StringComparison.Ordinal));
        Assert.Contains(calls, call => call.Contains("add", StringComparison.Ordinal) && call.Contains(WindowsStartupRegistry.StartupApprovedRunKey, StringComparison.Ordinal));
        Assert.Contains(calls, call => call.Contains("delete", StringComparison.Ordinal) && call.Contains(WindowsStartupRegistry.RunKey, StringComparison.Ordinal));
        Assert.Contains(calls, call => call.Contains("delete", StringComparison.Ordinal) && call.Contains(WindowsStartupRegistry.StartupApprovedRunKey, StringComparison.Ordinal));
    }
}
