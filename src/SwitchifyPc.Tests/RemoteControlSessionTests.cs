using System.Text.Json;
using System.Text.Json.Nodes;
using SwitchifyPc.Core.Control;
using SwitchifyPc.Core.Input;
using SwitchifyPc.Core.Pairing;
using SwitchifyPc.Protocol;

namespace SwitchifyPc.Tests;

public sealed class RemoteControlSessionTests
{
    private const string DeviceId = "android-1";
    private const string Token = "shared-token";
    private const double Now = 1_000_000;

    [Fact]
    public async Task PairingRequestCreatesPendingApprovalWithoutImmediateResponse()
    {
        TestContext context = CreateContext();

        RemoteSessionResult result = await context.Session.ProcessMessageAsync(
            "ble-1",
            PairingRequest(),
            remoteAddress: "192.168.1.50");

        Assert.Empty(result.OutgoingMessages);
        Assert.Null(result.AuthenticatedConnectionId);
        Assert.Null(result.AuthenticatedDeviceId);
        Assert.Null(result.AuthFailureReason);
        PendingPairingApprovalView approval = Assert.Single(context.ApprovalManager.ListPendingRequestViews());
        Assert.Equal("request-1", approval.RequestId);
        Assert.Equal("Pixel 9", approval.DeviceName);
        Assert.Equal("610717", approval.VerificationCode);
        Assert.Equal("192.168.1.50", approval.RemoteAddress);
        Assert.Equal(1, context.PendingChangeCount);
    }

    [Fact]
    public async Task AcceptingPairingRequestReturnsPairingCompleteToOriginalConnection()
    {
        TestContext context = CreateContext(createToken: () => "new-token");
        await context.Session.ProcessMessageAsync("ble-1", PairingRequest());

        RemoteSessionResult result = await context.Session.AcceptPairingRequestAsync("request-1");

        RemoteSessionOutgoingMessage outgoing = Assert.Single(result.OutgoingMessages);
        Assert.Equal("ble-1", outgoing.ConnectionId);
        Assert.False(outgoing.CloseConnection);
        using JsonDocument response = JsonDocument.Parse(outgoing.ResponseJson);
        Assert.Equal("pairing.complete", response.RootElement.GetProperty("type").GetString());
        Assert.Equal("desktop-1", response.RootElement.GetProperty("payload").GetProperty("desktopId").GetString());
        Assert.Equal(DeviceId, response.RootElement.GetProperty("payload").GetProperty("deviceId").GetString());
        Assert.Equal("new-token", response.RootElement.GetProperty("payload").GetProperty("token").GetString());
        Assert.True(ProtocolValidator.ValidateProtocolResponse(response.RootElement).Ok);
        Assert.Null(result.AuthenticatedConnectionId);
        Assert.Null(result.AuthenticatedDeviceId);
        Assert.Null(result.AuthFailureReason);
        Assert.Empty(context.ApprovalManager.ListPendingRequests());
    }

    [Fact]
    public async Task RejectingPairingRequestReturnsErrorAndClosesOriginalConnection()
    {
        TestContext context = CreateContext();
        await context.Session.ProcessMessageAsync("ble-1", PairingRequest());

        RemoteSessionResult result = context.Session.RejectPairingRequest("request-1");

        RemoteSessionOutgoingMessage outgoing = Assert.Single(result.OutgoingMessages);
        Assert.Equal("ble-1", outgoing.ConnectionId);
        Assert.True(outgoing.CloseConnection);
        AssertError(outgoing.ResponseJson, "request-1", "invalid_auth", "pairing_rejected");
        Assert.Empty(context.ApprovalManager.ListPendingRequests());
    }

    [Fact]
    public async Task ReplacingPendingRequestExpiresPreviousConnection()
    {
        TestContext context = CreateContext();
        await context.Session.ProcessMessageAsync("ble-1", PairingRequest(requestId: "request-1"));

        RemoteSessionResult result = await context.Session.ProcessMessageAsync("ble-2", PairingRequest(requestId: "request-2"));

        RemoteSessionOutgoingMessage outgoing = Assert.Single(result.OutgoingMessages);
        Assert.Equal("ble-1", outgoing.ConnectionId);
        Assert.True(outgoing.CloseConnection);
        AssertError(outgoing.ResponseJson, "request-1", "invalid_auth", "pairing_request_expired");
        PendingPairingApproval pending = Assert.Single(context.ApprovalManager.ListPendingRequests());
        Assert.Equal("request-2", pending.RequestId);
    }

    [Fact]
    public async Task ExpiringPendingRequestsReturnsCloseMessages()
    {
        double currentTime = Now;
        TestContext context = CreateContext(now: () => currentTime);
        await context.Session.ProcessMessageAsync("ble-1", PairingRequest());
        currentTime += PairingApprovalManager.PairingApprovalRequestTtlMs + 1;

        RemoteSessionResult result = context.Session.ExpirePendingPairingRequests();

        RemoteSessionOutgoingMessage outgoing = Assert.Single(result.OutgoingMessages);
        Assert.Equal("ble-1", outgoing.ConnectionId);
        Assert.True(outgoing.CloseConnection);
        AssertError(outgoing.ResponseJson, "request-1", "invalid_auth", "pairing_request_expired");
    }

    [Fact]
    public async Task RemovesPendingApprovalWhenConnectionIsRemoved()
    {
        TestContext context = CreateContext();
        await context.Session.ProcessMessageAsync("ble-1", PairingRequest());

        context.Session.RemoveConnection("ble-1");

        Assert.Empty(context.ApprovalManager.ListPendingRequests());
        Assert.Equal(2, context.PendingChangeCount);
    }

    [Fact]
    public async Task PairingRequestWithMismatchedDesktopReturnsError()
    {
        TestContext context = CreateContext();

        RemoteSessionResult result = await context.Session.ProcessMessageAsync(
            "ble-1",
            PairingRequest(desktopId: "wrong-desktop"));

        RemoteSessionOutgoingMessage outgoing = Assert.Single(result.OutgoingMessages);
        Assert.Equal("ble-1", outgoing.ConnectionId);
        AssertError(outgoing.ResponseJson, "request-1", "invalid_auth", "pairing_mismatch");
        Assert.Empty(context.ApprovalManager.ListPendingRequests());
    }

    [Fact]
    public async Task DelegatesAuthenticatedCommandsToCommandSession()
    {
        TestContext context = CreateContext();

        RemoteSessionResult result = await context.Session.ProcessMessageAsync(
            "ble-1",
            SignedCommand("keyboard.key", new { key = "Meta" }));

        Assert.Equal("ble-1", result.AuthenticatedConnectionId);
        Assert.Equal(DeviceId, result.AuthenticatedDeviceId);
        Assert.Null(result.AuthFailureReason);
        RemoteSessionOutgoingMessage outgoing = Assert.Single(result.OutgoingMessages);
        Assert.Equal("ble-1", outgoing.ConnectionId);
        using JsonDocument response = JsonDocument.Parse(outgoing.ResponseJson);
        Assert.Equal("ack", response.RootElement.GetProperty("type").GetString());
        Assert.Equal(["pressKey:Meta"], context.Adapter.Calls);
    }

    [Fact]
    public async Task UnknownAuthenticatedCommandReportsAuthFailureWithoutMarkingConnectionAuthenticated()
    {
        TestContext context = CreateContext();

        RemoteSessionResult result = await context.Session.ProcessMessageAsync(
            "ble-1",
            SignedCommand("connection.ping", new { }, deviceId: "android-unknown", token: "unknown-token"));

        RemoteSessionOutgoingMessage outgoing = Assert.Single(result.OutgoingMessages);
        Assert.Equal("ble-1", outgoing.ConnectionId);
        AssertError(outgoing.ResponseJson, "command-1", "unknown_device", "Command authentication failed.");
        Assert.Null(result.AuthenticatedConnectionId);
        Assert.Null(result.AuthenticatedDeviceId);
        Assert.Equal("unknown_device", result.AuthFailureReason);
    }

    [Fact]
    public async Task AuthenticatedCommandsUpdateLastSeenAt()
    {
        TestContext context = CreateContext();

        await context.Session.ProcessMessageAsync(
            "ble-1",
            SignedCommand("keyboard.key", new { key = "Meta" }, id: "command-1"));
        await context.Session.ProcessMessageAsync(
            "ble-1",
            SignedCommand("connection.ping", new { }, id: "command-2"));

        Assert.Equal(Now, (await context.Store.LoadAsync()).PairedDevices[0].LastSeenAt);
    }

    [Fact]
    public async Task DelegatesShortcutTextMediaAndWindowCommandsToCommandSession()
    {
        TestContext context = CreateContext();

        RemoteSessionResult shortcut = await context.Session.ProcessMessageAsync(
            "ble-1",
            SignedCommand("keyboard.shortcut", new { keys = new[] { "Meta" } }, id: "command-1"));
        RemoteSessionResult text = await context.Session.ProcessMessageAsync(
            "ble-1",
            SignedCommand("keyboard.typeText", new { text = "Hello" }, id: "command-2"));
        RemoteSessionResult media = await context.Session.ProcessMessageAsync(
            "ble-1",
            SignedCommand("media.control", new { action = "playPause" }, id: "command-3"));
        RemoteSessionResult window = await context.Session.ProcessMessageAsync(
            "ble-1",
            SignedCommand("window.control", new { action = "showDesktop" }, id: "command-4"));

        Assert.All(
            new[] { shortcut, text, media, window },
            result => Assert.Equal("ble-1", Assert.Single(result.OutgoingMessages).ConnectionId));
        Assert.Equal(
            [
                "pressShortcut:Meta",
                "typeText:Hello",
                "mediaControl:playPause",
                "controlWindow:showDesktop"
            ],
            context.Adapter.Calls);
    }

    private static TestContext CreateContext(Func<double>? now = null, Func<string>? createToken = null, double? lastSeenAt = null)
    {
        MemoryPairingStore store = new(new PairingState(
            DesktopId: "desktop-1",
            PairedDevices:
            [
                new PairedDevice(DeviceId, "Phone", Token, PairedAt: 1, lastSeenAt)
            ]));
        FakeInputAdapter adapter = new();
        PairingApprovalManager approvalManager = new(store, now ?? (() => Now), createToken);
        int pendingChangeCount = 0;

        PointerMovementProfile profile = new(
            DisplayId: "display-1",
            ScaleFactor: 1,
            Bounds: new Bounds(0, 0, 1920, 1080),
            MaxDelta: ProtocolConstants.MaxPointerDelta,
            RecommendedDeltas: new RecommendedDeltas(49, 130, 281),
            Capabilities: TestPointerCapabilities());

        ControlSession commandSession = new(
            new CommandAuthValidator(store, () => Now),
            new DesktopCommandExecutor(adapter),
            new FixedPointerProfileProvider(profile));

        RemoteControlSession session = new(
            new PairingManager(store),
            approvalManager,
            commandSession,
            () => pendingChangeCount += 1);

        return new TestContext(session, approvalManager, adapter, store, () => pendingChangeCount);
    }

    private static PointerCapabilities TestPointerCapabilities()
    {
        return new PointerCapabilities(
            true,
            ProtocolConstants.NoAckControlCommandTypes.ToArray(),
            ProtocolConstants.CommandTypes.ToArray(),
            new MouseRepeatCapabilities(true, true, 250, 250, 250, 100, 2000));
    }

    private static string PairingRequest(
        string requestId = "request-1",
        string deviceId = DeviceId,
        string deviceName = "Pixel 9",
        string desktopId = "desktop-1",
        string requestNonce = "nonce-1")
    {
        JsonObject request = new()
        {
            ["version"] = ProtocolConstants.ProtocolVersion,
            ["id"] = requestId,
            ["type"] = "pairing.request",
            ["payload"] = new JsonObject
            {
                ["deviceId"] = deviceId,
                ["deviceName"] = deviceName,
                ["desktopId"] = desktopId,
                ["requestNonce"] = requestNonce
            }
        };
        return request.ToJsonString();
    }

    private static string SignedCommand(
        string type,
        object payload,
        string id = "command-1",
        string deviceId = DeviceId,
        string token = Token)
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

        using JsonDocument unsignedDocument = JsonDocument.Parse(command.ToJsonString());
        command["auth"] = CommandAuth.CreateCommandAuthProof(unsignedDocument.RootElement, token);
        return command.ToJsonString();
    }

    private static void AssertError(string responseJson, string? id, string code, string message)
    {
        using JsonDocument response = JsonDocument.Parse(responseJson);
        Assert.Equal("error", response.RootElement.GetProperty("type").GetString());
        if (id is null)
        {
            Assert.Equal(JsonValueKind.Null, response.RootElement.GetProperty("id").ValueKind);
        }
        else
        {
            Assert.Equal(id, response.RootElement.GetProperty("id").GetString());
        }

        JsonElement error = response.RootElement.GetProperty("error");
        Assert.Equal(code, error.GetProperty("code").GetString());
        Assert.Equal(message, error.GetProperty("message").GetString());
    }

    private sealed record TestContext(
        RemoteControlSession Session,
        PairingApprovalManager ApprovalManager,
        FakeInputAdapter Adapter,
        MemoryPairingStore Store,
        Func<int> GetPendingChangeCount)
    {
        public int PendingChangeCount => GetPendingChangeCount();
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
}
