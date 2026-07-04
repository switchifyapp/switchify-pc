using System.Text.Json;
using System.Text.Json.Nodes;
using SwitchifyPc.Core.Bluetooth;
using SwitchifyPc.Core.Control;
using SwitchifyPc.Core.Input;
using SwitchifyPc.Core.Pairing;
using SwitchifyPc.Protocol;

namespace SwitchifyPc.Tests;

public sealed class BluetoothRemoteFrameProcessorTests
{
    private const string DeviceId = "android-1";
    private const string Token = "shared-token";
    private const double Now = 1_000_000;

    [Fact]
    public async Task ReassemblesCommandAndFramesAckToSameConnection()
    {
        TestContext context = CreateContext();
        BluetoothRemoteFrameProcessor processor = new(context.Session, maxResponseFramePayloadBytes: 20);
        IReadOnlyList<BluetoothFrame> frames = BluetoothFrameCodec.CreateFrames(
            SignedCommand("keyboard.key", new { key = "Meta" }),
            "incoming-1",
            maxPayloadBytes: 40);

        BluetoothRemoteFrameResult result = BluetoothRemoteFrameResult.Incomplete();
        foreach (BluetoothFrame frame in frames)
        {
            result = await processor.AcceptAsync("ble-1", frame);
        }

        Assert.True(result.MessageComplete);
        Assert.Equal("ble-1", result.AuthenticatedConnectionId);
        Assert.Equal(DeviceId, result.AuthenticatedDeviceId);
        BluetoothRemoteFrameOutput output = Assert.Single(result.OutgoingMessages);
        Assert.Equal("ble-1", output.ConnectionId);
        Assert.False(output.CloseConnection);
        AssertResponseType(output.ResponseFrames, "ack");
        Assert.Equal(["pressKey:Meta"], context.Adapter.Calls);
    }

    [Fact]
    public async Task PairingRequestCompletesWithoutImmediateResponseAndAcceptFramesCompleteResponse()
    {
        TestContext context = CreateContext(createToken: () => "new-token");
        BluetoothRemoteFrameProcessor processor = new(context.Session, maxResponseFramePayloadBytes: 24);
        IReadOnlyList<BluetoothFrame> frames = BluetoothFrameCodec.CreateFrames(PairingRequest(), "incoming-1", maxPayloadBytes: 40);

        BluetoothRemoteFrameResult result = BluetoothRemoteFrameResult.Incomplete();
        foreach (BluetoothFrame frame in frames)
        {
            result = await processor.AcceptAsync("ble-1", frame, remoteAddress: "192.168.1.50");
        }

        Assert.True(result.MessageComplete);
        Assert.Empty(result.OutgoingMessages);
        Assert.Equal("192.168.1.50", Assert.Single(context.ApprovalManager.ListPendingRequestViews()).RemoteAddress);

        IReadOnlyList<BluetoothRemoteFrameOutput> acceptOutputs = await processor.AcceptPairingRequestAsync("request-1");

        BluetoothRemoteFrameOutput output = Assert.Single(acceptOutputs);
        Assert.Equal("ble-1", output.ConnectionId);
        Assert.False(output.CloseConnection);
        using JsonDocument response = Reassemble(output.ResponseFrames);
        Assert.Equal("pairing.complete", response.RootElement.GetProperty("type").GetString());
        Assert.Equal("new-token", response.RootElement.GetProperty("payload").GetProperty("token").GetString());
    }

    [Fact]
    public async Task ReassembledCommandFromPreviouslyUsedDevicePropagatesAuthenticatedConnection()
    {
        TestContext context = CreateContext(lastSeenAt: Now - 500);
        BluetoothRemoteFrameProcessor processor = new(context.Session, maxResponseFramePayloadBytes: 20);
        IReadOnlyList<BluetoothFrame> frames = BluetoothFrameCodec.CreateFrames(
            SignedCommand("keyboard.key", new { key = "Meta" }),
            "incoming-1",
            maxPayloadBytes: 40);

        BluetoothRemoteFrameResult result = BluetoothRemoteFrameResult.Incomplete();
        foreach (BluetoothFrame frame in frames)
        {
            result = await processor.AcceptAsync("ble-1", frame);
        }

        Assert.True(result.MessageComplete);
        Assert.Equal("ble-1", result.AuthenticatedConnectionId);
        Assert.Equal(DeviceId, result.AuthenticatedDeviceId);
        BluetoothRemoteFrameOutput output = Assert.Single(result.OutgoingMessages);
        Assert.Equal("ble-1", output.ConnectionId);
        AssertResponseType(output.ResponseFrames, "ack");
    }

    [Fact]
    public async Task NewPairedDeviceRepeatedMessagesCompleteOnSameConnection()
    {
        TestContext context = CreateContext();
        BluetoothRemoteFrameProcessor processor = new(context.Session);

        BluetoothRemoteFrameResult first = await AcceptAllFramesAsync(
            processor,
            "ble-1",
            BluetoothFrameCodec.CreateFrames(SignedCommand("keyboard.key", new { key = "Meta" }, id: "command-1")));
        BluetoothRemoteFrameResult second = await AcceptAllFramesAsync(
            processor,
            "ble-1",
            BluetoothFrameCodec.CreateFrames(SignedCommand("connection.ping", new { }, id: "command-2")));

        Assert.True(first.MessageComplete);
        Assert.True(second.MessageComplete);
        Assert.Equal("ble-1", first.AuthenticatedConnectionId);
        Assert.Equal("ble-1", second.AuthenticatedConnectionId);
    }

    [Fact]
    public async Task RejectFramesErrorAndCloseFlag()
    {
        TestContext context = CreateContext();
        BluetoothRemoteFrameProcessor processor = new(context.Session);
        foreach (BluetoothFrame frame in BluetoothFrameCodec.CreateFrames(PairingRequest()))
        {
            await processor.AcceptAsync("ble-1", frame);
        }

        IReadOnlyList<BluetoothRemoteFrameOutput> outputs = processor.RejectPairingRequest("request-1");

        BluetoothRemoteFrameOutput output = Assert.Single(outputs);
        Assert.Equal("ble-1", output.ConnectionId);
        Assert.True(output.CloseConnection);
        AssertResponseType(output.ResponseFrames, "error");
    }

    [Fact]
    public async Task ReplacementFramesExpiryToOriginalConnection()
    {
        TestContext context = CreateContext();
        BluetoothRemoteFrameProcessor processor = new(context.Session);
        foreach (BluetoothFrame frame in BluetoothFrameCodec.CreateFrames(PairingRequest(requestId: "request-1")))
        {
            await processor.AcceptAsync("ble-1", frame);
        }

        BluetoothRemoteFrameResult result = BluetoothRemoteFrameResult.Incomplete();
        foreach (BluetoothFrame frame in BluetoothFrameCodec.CreateFrames(PairingRequest(requestId: "request-2")))
        {
            result = await processor.AcceptAsync("ble-2", frame);
        }

        BluetoothRemoteFrameOutput output = Assert.Single(result.OutgoingMessages);
        Assert.Equal("ble-1", output.ConnectionId);
        Assert.True(output.CloseConnection);
        using JsonDocument response = Reassemble(output.ResponseFrames);
        Assert.Equal("pairing_request_expired", response.RootElement.GetProperty("error").GetProperty("message").GetString());
    }

    [Fact]
    public async Task RemoveConnectionDropsPendingPairingAndPartials()
    {
        TestContext context = CreateContext();
        BluetoothRemoteFrameProcessor processor = new(context.Session);
        IReadOnlyList<BluetoothFrame> frames = BluetoothFrameCodec.CreateFrames(PairingRequest(), "incoming-1", maxPayloadBytes: 40);

        Assert.False((await processor.AcceptAsync("ble-1", frames[0])).MessageComplete);
        processor.RemoveConnection("ble-1");
        BluetoothRemoteFrameResult partialResult = await processor.AcceptAsync("ble-1", frames[^1]);

        Assert.False(partialResult.MessageComplete);
        Assert.Empty(context.ApprovalManager.ListPendingRequests());
    }

    private static TestContext CreateContext(Func<string>? createToken = null, double? lastSeenAt = null)
    {
        MemoryPairingStore store = new(new PairingState(
            DesktopId: "desktop-1",
            PairedDevices:
            [
                new PairedDevice(DeviceId, "Phone", Token, PairedAt: 1, lastSeenAt)
            ]));
        FakeInputAdapter adapter = new();
        PairingApprovalManager approvalManager = new(store, () => Now, createToken);
        PointerMovementProfile profile = new(
            DisplayId: "display-1",
            ScaleFactor: 1,
            Bounds: new Bounds(0, 0, 1920, 1080),
            MaxDelta: ProtocolConstants.MaxPointerDelta,
            RecommendedDeltas: new RecommendedDeltas(49, 130, 281),
            Capabilities: TestPointerCapabilities());
        ControlSession controlSession = new(
            new CommandAuthValidator(store, () => Now),
            new DesktopCommandExecutor(adapter),
            new FixedPointerProfileProvider(profile));
        RemoteControlSession session = new(
            new PairingManager(store),
            approvalManager,
            controlSession);

        return new TestContext(session, approvalManager, adapter);
    }

    private static PointerCapabilities TestPointerCapabilities()
    {
        return new PointerCapabilities(
            true,
            ProtocolConstants.NoAckControlCommandTypes.ToArray(),
            ProtocolConstants.CommandTypes.ToArray(),
            new MouseRepeatCapabilities(true, true, 250, 250, 250, 100, 2000));
    }

    private static JsonDocument Reassemble(IReadOnlyList<BluetoothFrame> frames)
    {
        BluetoothFrameReassembler reassembler = new();
        BluetoothFrameReassemblyResult result = BluetoothFrameReassemblyResult.Incomplete("incomplete");
        foreach (BluetoothFrame frame in frames)
        {
            result = reassembler.Accept(frame);
        }

        Assert.True(result.Ok);
        return JsonDocument.Parse(result.Message!);
    }

    private static async Task<BluetoothRemoteFrameResult> AcceptAllFramesAsync(
        BluetoothRemoteFrameProcessor processor,
        string connectionId,
        IReadOnlyList<BluetoothFrame> frames)
    {
        BluetoothRemoteFrameResult result = BluetoothRemoteFrameResult.Incomplete();
        foreach (BluetoothFrame frame in frames)
        {
            result = await processor.AcceptAsync(connectionId, frame);
        }

        return result;
    }

    private static void AssertResponseType(IReadOnlyList<BluetoothFrame> frames, string expectedType)
    {
        using JsonDocument response = Reassemble(frames);
        Assert.Equal(expectedType, response.RootElement.GetProperty("type").GetString());
    }

    private static string PairingRequest(string requestId = "request-1")
    {
        JsonObject request = new()
        {
            ["version"] = ProtocolConstants.ProtocolVersion,
            ["id"] = requestId,
            ["type"] = "pairing.request",
            ["payload"] = new JsonObject
            {
                ["deviceId"] = DeviceId,
                ["deviceName"] = "Pixel 9",
                ["desktopId"] = "desktop-1",
                ["requestNonce"] = "nonce-1"
            }
        };
        return request.ToJsonString();
    }

    private static string SignedCommand(string type, object payload, string id = "command-1")
    {
        JsonObject command = new()
        {
            ["version"] = ProtocolConstants.ProtocolVersion,
            ["id"] = id,
            ["deviceId"] = DeviceId,
            ["timestamp"] = Now,
            ["type"] = type,
            ["payload"] = JsonSerializer.SerializeToNode(payload),
            ["auth"] = ""
        };

        using JsonDocument unsignedDocument = JsonDocument.Parse(command.ToJsonString());
        command["auth"] = CommandAuth.CreateCommandAuthProof(unsignedDocument.RootElement, Token);
        return command.ToJsonString();
    }

    private sealed record TestContext(
        RemoteControlSession Session,
        PairingApprovalManager ApprovalManager,
        FakeInputAdapter Adapter);

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
}
