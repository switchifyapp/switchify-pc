using System.Text.Json;
using System.Text.Json.Nodes;
using SwitchifyPc.Core.Control;
using SwitchifyPc.Core.Input;
using SwitchifyPc.Core.Pairing;
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
        using JsonDocument response = JsonDocument.Parse(result.ResponseJson!);
        Assert.Equal("ack", response.RootElement.GetProperty("type").GetString());
        Assert.Equal(["pressKey:Meta"], adapter.Calls);
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
        Assert.True(ProtocolValidator.ValidateProtocolResponse(response.RootElement).Ok);
    }

    [Fact]
    public async Task DisconnectingReleasesHeldMouseButtons()
    {
        FakeInputAdapter adapter = new();
        ControlSession session = CreateSession(adapter);

        await session.ProcessMessageAsync(SignedCommand("mouse.dragStart", new { button = "left" }, id: "request-1"));
        ControlSessionResult result = await session.ProcessMessageAsync(SignedCommand("connection.disconnecting", new { }, id: "request-2"));

        Assert.True(result.HasResponse);
        Assert.Equal(
            [
                "setMouseButtonDown:left:True",
                "setMouseButtonDown:left:False"
            ],
            adapter.Calls);
    }

    [Fact]
    public async Task ConvertsExecutorFailuresToProtocolErrors()
    {
        FakeInputAdapter adapter = new() { ThrowOnPressKey = true };
        ControlSession session = CreateSession(adapter);

        ControlSessionResult result = await session.ProcessMessageAsync(SignedCommand("keyboard.key", new { key = "Meta" }));

        AssertError(result, "request-1", "adapter_failure");
    }

    private static ControlSession CreateSession(FakeInputAdapter adapter)
    {
        MemoryPairingStore store = new(new PairingState(
            DesktopId: "desktop-1",
            PairedDevices:
            [
                new PairedDevice(DeviceId, "Phone", Token, PairedAt: 1, LastSeenAt: null)
            ]));

        PointerMovementProfile profile = new(
            DisplayId: "display-1",
            ScaleFactor: 1,
            Bounds: new Bounds(0, 0, 1920, 1080),
            MaxDelta: ProtocolConstants.MaxPointerDelta,
            RecommendedDeltas: new RecommendedDeltas(49, 130, 281),
            Capabilities: new PointerCapabilities(true, ProtocolConstants.NoAckControlCommandTypes.ToArray(), ProtocolConstants.CommandTypes.ToArray()));

        return new ControlSession(
            new CommandAuthValidator(store, () => Now),
            new DesktopCommandExecutor(adapter),
            new FixedPointerProfileProvider(profile));
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
}
