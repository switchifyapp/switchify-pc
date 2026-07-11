using System.Text.Json;
using System.Text.Json.Nodes;
using SwitchifyPc.Core.Control;
using SwitchifyPc.Core.Input;
using SwitchifyPc.Core.Pairing;
using SwitchifyPc.Core.Settings;
using SwitchifyPc.Protocol;

namespace SwitchifyPc.Tests;

public sealed class ControlSessionTests
{
    private const string DeviceId = "android-1";
    private const string Token = "shared-token";
    private const double Now = 1_000_000;

    [Fact]
    public async Task AuthenticatesAndRoutesCommandToExecutor()
    {
        FakeInputAdapter adapter = new();
        ControlSession session = CreateSession(adapter);

        ControlSessionResult result = await session.ProcessMessageAsync(SignedCommand("keyboard.key", new { key = "Meta" }));

        Assert.True(result.HasResponse);
        Assert.True(result.HasAuthenticatedDevice);
        Assert.Equal(DeviceId, result.AuthenticatedDeviceId);
        Assert.False(result.AuthenticatedDeviceWasPreviouslyUsed);
        using JsonDocument response = JsonDocument.Parse(result.ResponseJson!);
        Assert.Equal("ack", response.RootElement.GetProperty("type").GetString());
        Assert.Equal(["pressKey:Meta"], adapter.Calls);
    }

    [Fact]
    public async Task PreservesPreviouslyUsedDeviceMetadataForAuthenticatedCommands()
    {
        FakeInputAdapter adapter = new();
        ControlSession session = CreateSession(adapter, lastSeenAt: Now - 500);

        ControlSessionResult result = await session.ProcessMessageAsync(SignedCommand("keyboard.key", new { key = "Meta" }));

        Assert.True(result.HasAuthenticatedDevice);
        Assert.Equal(DeviceId, result.AuthenticatedDeviceId);
        Assert.True(result.AuthenticatedDeviceWasPreviouslyUsed);
    }

    [Fact]
    public async Task SuppressesResponseForNoAckCommands()
    {
        FakeInputAdapter adapter = new();
        ControlSession session = CreateSession(adapter);

        ControlSessionResult result = await session.ProcessMessageAsync(SignedCommand("mouse.move", new { dx = 10, dy = -2 }, responseMode: "none"));

        Assert.False(result.HasResponse);
        Assert.Equal(["moveMouseBy:10,-2"], adapter.Calls);
    }

    [Fact]
    public async Task AuthenticatedModifierCommandsRouteToExecutor()
    {
        FakeInputAdapter adapter = new();
        ControlSession session = CreateSession(adapter);

        ControlSessionResult down = await session.ProcessMessageAsync(SignedCommand("keyboard.modifierDown", new { key = "Ctrl" }));
        ControlSessionResult up = await session.ProcessMessageAsync(SignedCommand("keyboard.modifierUp", new { key = "Ctrl" }, id: "request-2"));

        Assert.True(down.HasResponse);
        Assert.True(up.HasResponse);
        Assert.Equal(["setKeyDown:Ctrl:True", "setKeyDown:Ctrl:False"], adapter.Calls);
    }

    [Fact]
    public async Task RejectsInvalidJsonAndMalformedPayloads()
    {
        ControlSession session = CreateSession(new FakeInputAdapter());

        ControlSessionResult invalidJson = await session.ProcessMessageAsync("{");
        ControlSessionResult invalidPayload = await session.ProcessMessageAsync(SignedCommand("keyboard.key", new { key = "Win" }, signBeforeMutation: true));

        AssertError(invalidJson, null, "invalid_json");
        AssertError(invalidPayload, "request-1", "invalid_payload");
    }

    [Fact]
    public async Task RejectsUnknownDevicesAndInvalidAuth()
    {
        ControlSession session = CreateSession(new FakeInputAdapter());

        ControlSessionResult unknown = await session.ProcessMessageAsync(SignedCommand("keyboard.key", new { key = "Meta" }, deviceId: "unknown"));
        ControlSessionResult invalidAuth = await session.ProcessMessageAsync(SignedCommand("keyboard.key", new { key = "Meta" }, authOverride: "bad-proof"));

        AssertError(unknown, "request-1", "unknown_device");
        AssertError(invalidAuth, "request-1", "invalid_auth");
        Assert.False(unknown.HasAuthenticatedDevice);
        Assert.False(invalidAuth.HasAuthenticatedDevice);
    }

    [Fact]
    public async Task ReturnsPointerProfileResponse()
    {
        ControlSession session = CreateSession(new FakeInputAdapter());

        ControlSessionResult result = await session.ProcessMessageAsync(SignedCommand("pointer.profile", new { }));

        Assert.True(result.HasResponse);
        using JsonDocument response = JsonDocument.Parse(result.ResponseJson!);
        Assert.Equal("pointer.profile", response.RootElement.GetProperty("type").GetString());
        JsonElement payload = response.RootElement.GetProperty("payload");
        Assert.Equal("display-1", payload.GetProperty("displayId").GetString());
        JsonElement mouseRepeat = payload.GetProperty("capabilities").GetProperty("mouseRepeat");
        Assert.Equal(1000, mouseRepeat.GetProperty("accelerationDurationMs").GetInt32());
        Assert.Equal([0, 500, 1000, 2000], mouseRepeat.GetProperty("accelerationDurationOptionsMs").EnumerateArray().Select(value => value.GetInt32()));
        Assert.Equal(25, mouseRepeat.GetProperty("accelerationInitialScalePercent").GetInt32());
        Assert.True(ProtocolValidator.ValidateProtocolResponse(response.RootElement).Ok);
    }

    [Fact]
    public async Task AuthenticatedPointerSpeedSetSavesAndAppliesLiveSettings()
    {
        FakePointerSettings pointerSettings = new(PointerMovementSettingsModel.Default);
        List<PointerMovementSettings> applied = [];
        ControlSession session = CreateSession(
            new FakeInputAdapter(),
            pointerSettings: pointerSettings,
            applyPointerSettings: applied.Add);

        ControlSessionResult result = await session.ProcessMessageAsync(SignedCommand("pointer.speed.set", new { scalePercent = 227 }));

        Assert.True(result.HasResponse);
        Assert.Equal(new PointerMovementSettings(225), pointerSettings.Saved);
        Assert.Equal([new PointerMovementSettings(225)], applied);
    }

    [Fact]
    public async Task UnauthenticatedPointerSpeedSetDoesNotSaveSettings()
    {
        FakePointerSettings pointerSettings = new(PointerMovementSettingsModel.Default);
        List<PointerMovementSettings> applied = [];
        ControlSession session = CreateSession(
            new FakeInputAdapter(),
            pointerSettings: pointerSettings,
            applyPointerSettings: applied.Add);

        ControlSessionResult result = await session.ProcessMessageAsync(SignedCommand("pointer.speed.set", new { scalePercent = 125 }, authOverride: "bad-proof"));

        AssertError(result, "request-1", "invalid_auth");
        Assert.Null(pointerSettings.Saved);
        Assert.Empty(applied);
    }

    [Fact]
    public async Task AuthenticatedRepeatStartExecutesInitialCommandAndStopAcknowledges()
    {
        FakeInputAdapter adapter = new();
        ControlSession session = CreateSession(adapter, mouseRepeatSettings: new FakeMouseRepeatSettings(MouseRepeatSettingsModel.Default));

        ControlSessionResult start = await session.ProcessMessageAsync(SignedCommand(
            "mouse.repeat.start",
            new
            {
                command = new
                {
                    type = "mouse.move",
                    payload = new { dx = 4, dy = 5 }
                }
            }));
        ControlSessionResult stop = await session.ProcessMessageAsync(SignedCommand("mouse.repeat.stop", new { }, id: "request-2"));

        Assert.True(start.HasResponse);
        Assert.True(stop.HasResponse);
        Assert.Equal(["moveMouseBy:1,1.25"], adapter.Calls);
    }

    [Fact]
    public async Task DisconnectingReleasesHeldInputs()
    {
        FakeInputAdapter adapter = new();
        FakeCursorOverlay overlay = new();
        ControlSession session = CreateSession(adapter, overlay);

        await session.ProcessMessageAsync(SignedCommand("mouse.dragStart", new { button = "left" }, id: "request-1"));
        await session.ProcessMessageAsync(SignedCommand("keyboard.modifierDown", new { key = "Ctrl" }, id: "request-2"));
        ControlSessionResult result = await session.ProcessMessageAsync(SignedCommand("connection.disconnecting", new { }, id: "request-3"));

        Assert.True(result.HasResponse);
        Assert.Equal(
            [
                "setMouseButtonDown:left:True",
                "setKeyDown:Ctrl:True",
                "setMouseButtonDown:left:False",
                "setKeyDown:Ctrl:False"
            ],
            adapter.Calls);
        Assert.Equal([true, false], overlay.DragActiveChanges);
        Assert.Equal(2, overlay.HideCount);
        Assert.Equal(1, overlay.EndSessionCount);
    }

    [Fact]
    public async Task ConvertsExecutorFailuresToProtocolErrors()
    {
        FakeInputAdapter adapter = new() { ThrowOnPressKey = true };
        ControlSession session = CreateSession(adapter);

        ControlSessionResult result = await session.ProcessMessageAsync(SignedCommand("keyboard.key", new { key = "Meta" }));

        AssertError(result, "request-1", "adapter_failure");
    }

    private static ControlSession CreateSession(
        FakeInputAdapter adapter,
        ICursorOverlayNotifier? cursorOverlay = null,
        double? lastSeenAt = null,
        IMouseRepeatSettingsStore? mouseRepeatSettings = null,
        FakePointerSettings? pointerSettings = null,
        Action<PointerMovementSettings>? applyPointerSettings = null)
    {
        MemoryPairingStore store = new(new PairingState(
            DesktopId: "desktop-1",
            PairedDevices:
            [
                new PairedDevice(DeviceId, "Phone", Token, PairedAt: 1, lastSeenAt)
            ]));

        PointerMovementProfile profile = new(
            DisplayId: "display-1",
            ScaleFactor: 1,
            Bounds: new Bounds(0, 0, 1920, 1080),
            MaxDelta: ProtocolConstants.MaxPointerDelta,
            RecommendedDeltas: new RecommendedDeltas(49, 130, 281),
            Capabilities: TestPointerCapabilities());

        DesktopCommandExecutor executor = new(adapter, cursorOverlay);
        MouseRepeatController? repeatController = mouseRepeatSettings is null
            ? null
            : new MouseRepeatController(executor, mouseRepeatSettings, (_, cancellationToken) => Task.Delay(Timeout.InfiniteTimeSpan, cancellationToken));
        PointerSpeedController? pointerSpeedController = pointerSettings is null
            ? null
            : new PointerSpeedController(pointerSettings, applyPointerSettings ?? (_ => { }));

        return new ControlSession(
            new CommandAuthValidator(store, () => Now),
            executor,
            new FixedPointerProfileProvider(profile),
            repeatController,
            pointerSpeedController);
    }

    private static PointerCapabilities TestPointerCapabilities()
    {
        return new PointerCapabilities(
            true,
            ProtocolConstants.NoAckControlCommandTypes.ToArray(),
            ProtocolConstants.CommandTypes.ToArray(),
            new MouseRepeatCapabilities(true, true, 250, 250, 250, 100, 2000),
            PointerProfile.PointerSpeedFor(PointerMovementSettingsModel.Default));
    }

    private sealed class FakeMouseRepeatSettings(MouseRepeatSettings settings) : IMouseRepeatSettingsStore
    {
        public MouseRepeatSettings Load() => settings;

        public MouseRepeatSettings Save(MouseRepeatSettings next)
        {
            settings = MouseRepeatSettingsModel.Normalize(next);
            return settings;
        }
    }

    private sealed class FakePointerSettings(PointerMovementSettings settings) : IPointerMovementSettingsStore
    {
        public PointerMovementSettings? Saved { get; private set; }

        public PointerMovementSettings Load() => settings;

        public PointerMovementSettings Save(PointerMovementSettings next)
        {
            settings = PointerMovementSettingsModel.Normalize(next);
            Saved = settings;
            return settings;
        }
    }

    private static string SignedCommand(
        string type,
        object payload,
        string id = "request-1",
        string deviceId = DeviceId,
        string? responseMode = null,
        string? authOverride = null,
        bool signBeforeMutation = false)
    {
        JsonObject command = new()
        {
            ["version"] = ProtocolConstants.ProtocolVersion,
            ["id"] = id,
            ["deviceId"] = deviceId,
            ["timestamp"] = Now,
            ["type"] = type,
            ["payload"] = JsonSerializer.SerializeToNode(payload),
            ["auth"] = ""
        };

        if (responseMode is not null)
        {
            command["responseMode"] = responseMode;
        }

        using JsonDocument unsignedDocument = JsonDocument.Parse(command.ToJsonString());
        command["auth"] = authOverride ?? CommandAuth.CreateCommandAuthProof(unsignedDocument.RootElement, Token);

        if (signBeforeMutation)
        {
            command["payload"] = JsonSerializer.SerializeToNode(payload);
        }

        return command.ToJsonString();
    }

    private static void AssertError(ControlSessionResult result, string? id, string code)
    {
        Assert.True(result.HasResponse);
        using JsonDocument response = JsonDocument.Parse(result.ResponseJson!);
        Assert.Equal("error", response.RootElement.GetProperty("type").GetString());
        if (id is null)
        {
            Assert.Equal(JsonValueKind.Null, response.RootElement.GetProperty("id").ValueKind);
        }
        else
        {
            Assert.Equal(id, response.RootElement.GetProperty("id").GetString());
        }

        Assert.Equal(code, response.RootElement.GetProperty("error").GetProperty("code").GetString());
    }

    private sealed class FakeInputAdapter : IDesktopInputAdapter
    {
        public List<string> Calls { get; } = [];
        public bool ThrowOnPressKey { get; init; }

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
            if (ThrowOnPressKey) throw new DesktopInputException("adapter_failure", "Key failed.");
            Calls.Add($"pressKey:{key}");
            return Task.CompletedTask;
        }

        public Task SetKeyDownAsync(string key, bool down, CancellationToken cancellationToken = default)
        {
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
        public List<bool> DragActiveChanges { get; } = [];
        public int HideCount { get; private set; }
        public int EndSessionCount { get; private set; }

        public void Show(string eventName)
        {
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
        }

        public void SetDragActive(bool active)
        {
            DragActiveChanges.Add(active);
        }
    }
}
