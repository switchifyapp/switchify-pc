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
        Assert.Equal(
            [
                new CursorOverlayEvent(CursorOverlayEventKind.Move),
                new CursorOverlayEvent(CursorOverlayEventKind.Click, "left"),
                new CursorOverlayEvent(CursorOverlayEventKind.DoubleClick, "middle"),
                new CursorOverlayEvent(CursorOverlayEventKind.Click, "right"),
                new CursorOverlayEvent(CursorOverlayEventKind.Scroll, Dx: 0, Dy: -3)
            ],
            overlay.Events);
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
                "setKeyDown:Meta:True",
                "setKeyDown:Meta:False",
                "setKeyDown:Ctrl:True",
                "setKeyDown:C:True",
                "setKeyDown:C:False",
                "setKeyDown:Ctrl:False",
                "typeText:Hello",
                "mediaControl:playPause",
                "controlWindow:switchNext"
            ],
            adapter.Calls);
        Assert.Equal(7, overlay.HideCount);
    }

    [Fact]
    public async Task KeyboardShortcutsUseTemporaryKeyDownAndUp()
    {
        FakeInputAdapter adapter = new();
        DesktopCommandExecutor executor = new(adapter);

        await executor.ExecuteAsync(Command("keyboard.shortcut", new { keys = new[] { "Ctrl", "C" } }));

        Assert.Equal(
            [
                "setKeyDown:Ctrl:True",
                "setKeyDown:C:True",
                "setKeyDown:C:False",
                "setKeyDown:Ctrl:False"
            ],
            adapter.Calls);
    }

    [Fact]
    public async Task KeyboardShortcutsDoNotReleaseHeldModifiers()
    {
        FakeInputAdapter adapter = new();
        FakeModifierOverlay modifierOverlay = new();
        DesktopCommandExecutor executor = new(adapter, modifierOverlay: modifierOverlay);

        await executor.ExecuteAsync(Command("keyboard.modifierDown", new { key = "Ctrl" }));
        await executor.ExecuteAsync(Command("keyboard.shortcut", new { keys = new[] { "Ctrl", "C" } }));
        await executor.ExecuteAsync(Command("keyboard.modifierDown", new { key = "Shift" }));
        await executor.ExecuteAsync(Command("keyboard.shortcut", new { keys = new[] { "Ctrl", "Shift", "Z" } }));
        await executor.ExecuteAsync(Command("keyboard.shortcut", new { keys = new[] { "Ctrl", "Alt", "F" } }));

        Assert.Equal(
            [
                "setKeyDown:Ctrl:True",
                "setKeyDown:C:True",
                "setKeyDown:C:False",
                "setKeyDown:Shift:True",
                "setKeyDown:Z:True",
                "setKeyDown:Z:False",
                "setKeyDown:Alt:True",
                "setKeyDown:F:True",
                "setKeyDown:F:False",
                "setKeyDown:Alt:False"
            ],
            adapter.Calls);
        Assert.Equal(["Ctrl", "Shift"], modifierOverlay.Changes.Last());

        await executor.ReleaseHeldInputsAsync();

        Assert.EndsWith("setKeyDown:Shift:False", adapter.Calls[^2]);
        Assert.EndsWith("setKeyDown:Ctrl:False", adapter.Calls[^1]);
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
                "typeText: ok"
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
        Assert.Equal(
            [
                new CursorOverlayEvent(CursorOverlayEventKind.Drag, "left"),
                new CursorOverlayEvent(CursorOverlayEventKind.Drag, "left")
            ],
            overlay.Events);
        Assert.Equal([true, false], overlay.DragActiveChanges);
        Assert.Equal(1, overlay.HideCount);
    }

    [Fact]
    public async Task ClickReleasesActiveDragBeforeClick()
    {
        FakeInputAdapter adapter = new();
        FakeCursorOverlay overlay = new();
        DesktopCommandExecutor executor = new(adapter, overlay);

        await executor.ExecuteAsync(Command("mouse.dragStart", new { button = "left" }));
        await executor.ExecuteAsync(Command("mouse.click", new { button = "left" }));

        Assert.Equal(
            [
                "setMouseButtonDown:left:True",
                "setMouseButtonDown:left:False",
                "clickMouse:left"
            ],
            adapter.Calls);
        Assert.Equal([true, false], overlay.DragActiveChanges);
        Assert.Equal(
            [
                new CursorOverlayEvent(CursorOverlayEventKind.Drag, "left"),
                new CursorOverlayEvent(CursorOverlayEventKind.Click, "left")
            ],
            overlay.Events);
    }

    [Fact]
    public async Task RightClickReleasesActiveDragBeforeClick()
    {
        FakeInputAdapter adapter = new();
        FakeCursorOverlay overlay = new();
        DesktopCommandExecutor executor = new(adapter, overlay);

        await executor.ExecuteAsync(Command("mouse.dragStart", new { button = "left" }));
        await executor.ExecuteAsync(Command("mouse.rightClick", new { }));

        Assert.Equal(
            [
                "setMouseButtonDown:left:True",
                "setMouseButtonDown:left:False",
                "clickMouse:right"
            ],
            adapter.Calls);
        Assert.Equal([true, false], overlay.DragActiveChanges);
    }

    [Fact]
    public async Task DoubleClickReleasesActiveDragBeforeClick()
    {
        FakeInputAdapter adapter = new();
        FakeCursorOverlay overlay = new();
        DesktopCommandExecutor executor = new(adapter, overlay);

        await executor.ExecuteAsync(Command("mouse.dragStart", new { button = "left" }));
        await executor.ExecuteAsync(Command("mouse.doubleClick", new { button = "middle" }));

        Assert.Equal(
            [
                "setMouseButtonDown:left:True",
                "setMouseButtonDown:left:False",
                "doubleClickMouse:middle"
            ],
            adapter.Calls);
        Assert.Equal([true, false], overlay.DragActiveChanges);
    }

    [Fact]
    public async Task MoveDoesNotReleaseActiveDrag()
    {
        FakeInputAdapter adapter = new();
        FakeCursorOverlay overlay = new();
        DesktopCommandExecutor executor = new(adapter, overlay);

        await executor.ExecuteAsync(Command("mouse.dragStart", new { button = "left" }));
        await executor.ExecuteAsync(Command("mouse.move", new { dx = 5, dy = 0 }));

        Assert.Equal(
            [
                "setMouseButtonDown:left:True",
                "moveMouseBy:5,0"
            ],
            adapter.Calls);
        Assert.Equal([true], overlay.DragActiveChanges);
        Assert.Equal(
            [
                new CursorOverlayEvent(CursorOverlayEventKind.Drag, "left"),
                new CursorOverlayEvent(CursorOverlayEventKind.Drag, "left")
            ],
            overlay.Events);
    }

    [Fact]
    public async Task ScrollDoesNotReleaseActiveDrag()
    {
        FakeInputAdapter adapter = new();
        FakeCursorOverlay overlay = new();
        DesktopCommandExecutor executor = new(adapter, overlay);

        await executor.ExecuteAsync(Command("mouse.dragStart", new { button = "left" }));
        await executor.ExecuteAsync(Command("mouse.scroll", new { dx = 0, dy = -3 }));

        Assert.Equal(
            [
                "setMouseButtonDown:left:True",
                "scrollMouse:0,-3"
            ],
            adapter.Calls);
        Assert.Equal([true], overlay.DragActiveChanges);
        Assert.Equal(
            [
                new CursorOverlayEvent(CursorOverlayEventKind.Drag, "left"),
                new CursorOverlayEvent(CursorOverlayEventKind.Scroll, Dx: 0, Dy: -3)
            ],
            overlay.Events);
    }

    [Fact]
    public async Task TracksModifierStateAndUpdatesOverlay()
    {
        FakeInputAdapter adapter = new();
        FakeModifierOverlay modifierOverlay = new();
        DesktopCommandExecutor executor = new(adapter, modifierOverlay: modifierOverlay);

        await executor.ExecuteAsync(Command("keyboard.modifierDown", new { key = "Shift" }));
        await executor.ExecuteAsync(Command("keyboard.modifierDown", new { key = "Ctrl" }));
        await executor.ExecuteAsync(Command("keyboard.modifierDown", new { key = "Meta" }));
        await executor.ExecuteAsync(Command("keyboard.modifierDown", new { key = "Ctrl" }));
        await executor.ExecuteAsync(Command("keyboard.modifierUp", new { key = "Ctrl" }));
        await executor.ExecuteAsync(Command("keyboard.modifierUp", new { key = "Alt" }));
        await executor.ExecuteAsync(Command("keyboard.modifierUp", new { key = "Shift" }));
        await executor.ExecuteAsync(Command("keyboard.modifierUp", new { key = "Meta" }));

        Assert.Equal(
            [
                "setKeyDown:Shift:True",
                "setKeyDown:Ctrl:True",
                "setKeyDown:Meta:True",
                "setKeyDown:Ctrl:False",
                "setKeyDown:Shift:False",
                "setKeyDown:Meta:False"
            ],
            adapter.Calls);
        Assert.Equal(
            [
                ["Shift"],
                ["Ctrl", "Shift"],
                ["Ctrl", "Shift", "Start"],
                ["Shift", "Start"],
                ["Start"],
                []
            ],
            modifierOverlay.Changes);
    }

    [Fact]
    public async Task ReleaseHeldInputsReleasesDragAndModifiers()
    {
        FakeInputAdapter adapter = new();
        FakeCursorOverlay cursorOverlay = new();
        FakeModifierOverlay modifierOverlay = new();
        DesktopCommandExecutor executor = new(adapter, cursorOverlay, modifierOverlay: modifierOverlay);

        await executor.ExecuteAsync(Command("mouse.dragStart", new { button = "left" }));
        await executor.ExecuteAsync(Command("keyboard.modifierDown", new { key = "Ctrl" }));
        await executor.ExecuteAsync(Command("keyboard.modifierDown", new { key = "Meta" }));
        await executor.ReleaseHeldInputsAsync();

        Assert.Equal(
            [
                "setMouseButtonDown:left:True",
                "setKeyDown:Ctrl:True",
                "setKeyDown:Meta:True",
                "setMouseButtonDown:left:False",
                "setKeyDown:Meta:False",
                "setKeyDown:Ctrl:False"
            ],
            adapter.Calls);
        Assert.Equal([true, false], cursorOverlay.DragActiveChanges);
        Assert.Equal(["Ctrl"], modifierOverlay.Changes[0]);
        Assert.Equal(["Ctrl", "Start"], modifierOverlay.Changes[1]);
        Assert.Equal(["Ctrl"], modifierOverlay.Changes[2]);
        Assert.Equal([], modifierOverlay.Changes[3]);
    }

    [Fact]
    public async Task ModifierFailuresKeepOverlayConsistent()
    {
        FakeInputAdapter downFailure = new() { ThrowOnSetKeyDown = true };
        FakeModifierOverlay downOverlay = new();
        DesktopCommandExecutor downExecutor = new(downFailure, modifierOverlay: downOverlay);

        CommandExecutionResult down = await downExecutor.ExecuteAsync(Command("keyboard.modifierDown", new { key = "Ctrl" }));

        Assert.False(down.Ok);
        Assert.Empty(downOverlay.Changes);

        FakeInputAdapter upFailure = new();
        FakeModifierOverlay upOverlay = new();
        DesktopCommandExecutor upExecutor = new(upFailure, modifierOverlay: upOverlay);
        await upExecutor.ExecuteAsync(Command("keyboard.modifierDown", new { key = "Ctrl" }));
        upFailure.ThrowOnSetKeyDown = true;

        CommandExecutionResult up = await upExecutor.ExecuteAsync(Command("keyboard.modifierUp", new { key = "Ctrl" }));

        Assert.False(up.Ok);
        Assert.Equal([["Ctrl"], []], upOverlay.Changes);
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

    [Fact]
    public async Task MovesPointerToDisplayAndShowsCursorOverlay()
    {
        FakeInputAdapter adapter = new();
        FakeCursorOverlay overlay = new();
        DesktopCommandExecutor executor = new(adapter, overlay);

        CommandExecutionResult result = await executor.ExecuteAsync(Command("pointer.display.move", new { direction = "right" }));

        Assert.True(result.Ok);
        Assert.Contains("movePointerToDisplay:right", adapter.Calls);
        Assert.Equal(CursorOverlayEventKind.Move, Assert.Single(overlay.Events).Kind);
        Assert.Equal(1, overlay.ActiveCount);
    }

    [Fact]
    public async Task RejectsDisplayNavigationDuringActiveDrag()
    {
        FakeInputAdapter adapter = new();
        DesktopCommandExecutor executor = new(adapter);
        await executor.ExecuteAsync(Command("mouse.dragStart", new { button = "left" }));

        CommandExecutionResult result = await executor.ExecuteAsync(Command("pointer.display.move", new { direction = "right" }));

        Assert.False(result.Ok);
        Assert.Equal("drag_active", result.Code);
        Assert.DoesNotContain("movePointerToDisplay:right", adapter.Calls);
    }

    [Fact]
    public async Task ConvertsDisplayNavigationAdapterFailureToStructuredResult()
    {
        FakeInputAdapter adapter = new() { ThrowOnMoveToDisplay = true };
        DesktopCommandExecutor executor = new(adapter);

        CommandExecutionResult result = await executor.ExecuteAsync(Command("pointer.display.move", new { direction = "up" }));

        Assert.False(result.Ok);
        Assert.Equal("adapter_failure", result.Code);
        Assert.Equal("Monitor move failed.", result.Message);
    }

    [Fact]
    public void EndControlSessionHidesCursorOverlaySession()
    {
        FakeCursorOverlay overlay = new();
        FakeModifierOverlay modifierOverlay = new();
        DesktopCommandExecutor executor = new(new FakeInputAdapter(), overlay, modifierOverlay: modifierOverlay);

        executor.EndControlSession();

        Assert.Equal(1, overlay.EndSessionCount);
        Assert.Equal(1, modifierOverlay.EndSessionCount);
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
        public bool ThrowOnSetKeyDown { get; set; }
        public bool ThrowOnMoveToDisplay { get; set; }

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

        public Task MovePointerToDisplayAsync(string direction, CancellationToken cancellationToken = default)
        {
            if (ThrowOnMoveToDisplay)
            {
                throw new DesktopInputException("adapter_failure", "Monitor move failed.");
            }

            Calls.Add($"movePointerToDisplay:{direction}");
            return Task.CompletedTask;
        }

        public Task PressKeyAsync(string key, CancellationToken cancellationToken = default)
        {
            Calls.Add($"pressKey:{key}");
            return Task.CompletedTask;
        }

        public Task SetKeyDownAsync(string key, bool down, CancellationToken cancellationToken = default)
        {
            if (ThrowOnSetKeyDown)
            {
                throw new DesktopInputException("adapter_failure", "Set key failed.");
            }

            Calls.Add($"setKeyDown:{key}:{down}");
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
        public List<CursorOverlayEvent> Events { get; } = [];
        public List<bool> DragActiveChanges { get; } = [];
        public int ActiveCount { get; private set; }
        public int HideCount { get; private set; }
        public int EndSessionCount { get; private set; }

        public void Show(CursorOverlayEvent cursorEvent)
        {
            Events.Add(cursorEvent);
        }

        public void Hide()
        {
            HideCount += 1;
        }

        public void EndControlSession()
        {
            EndSessionCount += 1;
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

    private sealed class FakeModifierOverlay : IModifierKeyOverlayNotifier
    {
        public List<IReadOnlyList<string>> Changes { get; } = [];
        public int EndSessionCount { get; private set; }

        public void SetActiveModifiers(IReadOnlyCollection<string> activeModifiers)
        {
            Changes.Add(activeModifiers.ToArray());
        }

        public void EndControlSession()
        {
            EndSessionCount += 1;
        }
    }
}
