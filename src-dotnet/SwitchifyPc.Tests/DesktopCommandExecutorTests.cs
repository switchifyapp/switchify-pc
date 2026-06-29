using System.Text.Json;
using SwitchifyPc.Core.Input;
using SwitchifyPc.Protocol;

namespace SwitchifyPc.Tests;

public sealed class DesktopCommandExecutorTests
{
    [Fact]
    public async Task MapsMouseCommandsToAdapter()
    {
        FakeInputAdapter adapter = new();
        FakeCursorOverlay overlay = new();
        DesktopCommandExecutor executor = new(adapter, overlay);

        await executor.ExecuteAsync(Command("mouse.move", new { dx = 10, dy = -4 }));
        await executor.ExecuteAsync(Command("mouse.click", new { button = "left" }));
        await executor.ExecuteAsync(Command("mouse.doubleClick", new { button = "middle" }));
        await executor.ExecuteAsync(Command("mouse.rightClick", new { }));
        await executor.ExecuteAsync(Command("mouse.scroll", new { dx = 0, dy = -3 }));

        Assert.Equal(
            [
                "moveMouseBy:10,-4",
                "clickMouse:left",
                "doubleClickMouse:middle",
                "clickMouse:right",
                "scrollMouse:0,-3"
            ],
            adapter.Calls);
        Assert.Equal(["move", "click", "click", "click"], overlay.Events);
        Assert.Equal(5, overlay.ActiveCount);
    }

    [Fact]
    public async Task MapsKeyboardTextMediaWindowAndPingCommands()
    {
        FakeInputAdapter adapter = new();
        FakeCursorOverlay overlay = new();
        DesktopCommandExecutor executor = new(adapter, overlay);

        await executor.ExecuteAsync(Command("keyboard.key", new { key = "Meta" }));
        await executor.ExecuteAsync(Command("keyboard.shortcut", new { keys = new[] { "Meta" } }));
        await executor.ExecuteAsync(Command("keyboard.shortcut", new { keys = new[] { "Ctrl", "C" } }));
        await executor.ExecuteAsync(Command("keyboard.typeText", new { text = "Hello" }));
        await executor.ExecuteAsync(Command("media.control", new { action = "playPause" }));
        await executor.ExecuteAsync(Command("window.control", new { action = "switchNext" }));
        CommandExecutionResult ping = await executor.ExecuteAsync(Command("connection.ping", new { }));

        Assert.True(ping.Ok);
        Assert.Equal(
            [
                "pressKey:Meta",
                "pressShortcut:Meta",
                "pressShortcut:Ctrl+C",
                "typeText:Hello",
                "mediaControl:playPause",
                "controlWindow:switchNext"
            ],
            adapter.Calls);
        Assert.Equal(7, overlay.HideCount);
    }

    [Fact]
    public async Task StreamsTextCharactersKeysAndChunks()
    {
        FakeInputAdapter adapter = new();
        DesktopCommandExecutor executor = new(adapter);

        Assert.True((await executor.ExecuteAsync(Command("keyboard.textStream.open", new { streamId = "stream-1" }))).Ok);
        Assert.True((await executor.ExecuteAsync(Command("keyboard.textStream.char", new { streamId = "stream-1", seq = 0, text = "H" }, "none"))).Ok);
        Assert.True((await executor.ExecuteAsync(Command("keyboard.textStream.key", new { streamId = "stream-1", seq = 1, key = "Meta" }, "none"))).Ok);
        Assert.True((await executor.ExecuteAsync(Command("keyboard.textStream.chunk", new { streamId = "stream-1", seq = 2, text = " ok" }))).Ok);
        Assert.True((await executor.ExecuteAsync(Command("keyboard.textStream.close", new { streamId = "stream-1", expectedCount = 3 }))).Ok);

        Assert.Equal(
            [
                "typeCharacter:H",
                "pressKey:Meta",
                "typeCharacter: ",
                "typeCharacter:o",
                "typeCharacter:k"
            ],
            adapter.Calls);
    }

    [Fact]
    public async Task RejectsTextStreamSequenceMismatchesAndMissingStreams()
    {
        DesktopCommandExecutor executor = new(new FakeInputAdapter());

        CommandExecutionResult missing = await executor.ExecuteAsync(Command("keyboard.textStream.char", new { streamId = "stream-1", seq = 0, text = "H" }, "none"));
        await executor.ExecuteAsync(Command("keyboard.textStream.open", new { streamId = "stream-1" }));
        CommandExecutionResult mismatch = await executor.ExecuteAsync(Command("keyboard.textStream.char", new { streamId = "stream-1", seq = 1, text = "H" }, "none"));
        CommandExecutionResult close = await executor.ExecuteAsync(Command("keyboard.textStream.close", new { streamId = "stream-1", expectedCount = 2 }));

        Assert.Equal("Text stream is not open.", missing.Message);
        Assert.Equal("Text stream sequence mismatch.", mismatch.Message);
        Assert.Equal("Text stream sequence mismatch.", close.Message);
    }

    [Fact]
    public async Task TracksDragStateAndReleasesHeldButtons()
    {
        FakeInputAdapter adapter = new();
        FakeCursorOverlay overlay = new();
        DesktopCommandExecutor executor = new(adapter, overlay);

        await executor.ExecuteAsync(Command("mouse.dragStart", new { button = "left" }));
        await executor.ExecuteAsync(Command("mouse.move", new { dx = 5, dy = 0 }));
        await executor.ReleaseHeldMouseButtonsAsync();

        Assert.Equal(
            [
                "setMouseButtonDown:left:True",
                "moveMouseBy:5,0",
                "setMouseButtonDown:left:False"
            ],
            adapter.Calls);
        Assert.Equal(["drag", "drag"], overlay.Events);
        Assert.Equal([true, false], overlay.DragActiveChanges);
        Assert.Equal(1, overlay.HideCount);
    }

    [Fact]
    public async Task RejectsUnsafePayloads()
    {
        DesktopCommandExecutor executor = new(new FakeInputAdapter());

        Assert.Equal("unsafe_payload", (await executor.ExecuteAsync(Command("mouse.move", new { dx = 501, dy = 0 }))).Code);
        Assert.Equal("unsafe_payload", (await executor.ExecuteAsync(Command("mouse.scroll", new { dx = 0, dy = 51 }))).Code);
        Assert.Equal("unsafe_payload", (await executor.ExecuteAsync(Command("keyboard.shortcut", new { keys = Array.Empty<string>() }))).Code);
        Assert.Equal("unsafe_payload", (await executor.ExecuteAsync(Command("keyboard.typeText", new { text = new string('x', 2001) }))).Code);
    }

    [Fact]
    public async Task LeavesServerOwnedCommandsUnsupported()
    {
        FakeCursorOverlay overlay = new();
        DesktopCommandExecutor executor = new(new FakeInputAdapter(), overlay);

        CommandExecutionResult disconnecting = await executor.ExecuteAsync(Command("connection.disconnecting", new { }));
        CommandExecutionResult profile = await executor.ExecuteAsync(Command("pointer.profile", new { }));

        Assert.Equal("unsupported_command", disconnecting.Code);
        Assert.Equal("unsupported_command", profile.Code);
        Assert.Equal(2, overlay.HideCount);
    }

    private static JsonElement Command(string type, object payload, string? responseMode = null)
    {
        Dictionary<string, object?> command = new(StringComparer.Ordinal)
        {
            ["version"] = ProtocolConstants.ProtocolVersion,
            ["id"] = "request-1",
            ["deviceId"] = "android-1",
            ["timestamp"] = 1,
            ["type"] = type,
            ["payload"] = payload,
            ["auth"] = "proof"
        };
        if (responseMode is not null)
        {
            command["responseMode"] = responseMode;
        }

        return JsonSerializer.SerializeToElement(command);
    }

    private sealed class FakeInputAdapter : IDesktopInputAdapter
    {
        public List<string> Calls { get; } = [];

        public Task MoveMouseByAsync(double dx, double dy, CancellationToken cancellationToken = default)
        {
            Calls.Add($"moveMouseBy:{dx},{dy}");
            return Task.CompletedTask;
        }

        public Task SetMouseButtonDownAsync(string button, bool down, CancellationToken cancellationToken = default)
        {
            Calls.Add($"setMouseButtonDown:{button}:{down}");
            return Task.CompletedTask;
        }

        public Task ClickMouseAsync(string button, CancellationToken cancellationToken = default)
        {
            Calls.Add($"clickMouse:{button}");
            return Task.CompletedTask;
        }

        public Task DoubleClickMouseAsync(string button, CancellationToken cancellationToken = default)
        {
            Calls.Add($"doubleClickMouse:{button}");
            return Task.CompletedTask;
        }

        public Task ScrollMouseAsync(double dx, double dy, CancellationToken cancellationToken = default)
        {
            Calls.Add($"scrollMouse:{dx},{dy}");
            return Task.CompletedTask;
        }

        public Task PressKeyAsync(string key, CancellationToken cancellationToken = default)
        {
            Calls.Add($"pressKey:{key}");
            return Task.CompletedTask;
        }

        public Task PressShortcutAsync(IReadOnlyList<string> keys, CancellationToken cancellationToken = default)
        {
            Calls.Add($"pressShortcut:{string.Join("+", keys)}");
            return Task.CompletedTask;
        }

        public Task TypeTextAsync(string text, CancellationToken cancellationToken = default)
        {
            Calls.Add($"typeText:{text}");
            return Task.CompletedTask;
        }

        public Task TypeCharacterAsync(string text, CancellationToken cancellationToken = default)
        {
            Calls.Add($"typeCharacter:{text}");
            return Task.CompletedTask;
        }

        public Task MediaControlAsync(string action, CancellationToken cancellationToken = default)
        {
            Calls.Add($"mediaControl:{action}");
            return Task.CompletedTask;
        }

        public Task ControlWindowAsync(string action, CancellationToken cancellationToken = default)
        {
            Calls.Add($"controlWindow:{action}");
            return Task.CompletedTask;
        }
    }

    private sealed class FakeCursorOverlay : ICursorOverlayNotifier
    {
        public List<string> Events { get; } = [];
        public List<bool> DragActiveChanges { get; } = [];
        public int ActiveCount { get; private set; }
        public int HideCount { get; private set; }

        public void Show(string eventName)
        {
            Events.Add(eventName);
        }

        public void Hide()
        {
            HideCount += 1;
        }

        public void MarkControlActive()
        {
            ActiveCount += 1;
        }

        public void SetDragActive(bool active)
        {
            DragActiveChanges.Add(active);
        }
    }
}
