using System.Text.Json;
using System.Text.Json.Nodes;
using SwitchifyPc.Core.Bluetooth;
using SwitchifyPc.Core.Control;
using SwitchifyPc.Core.Input;
using SwitchifyPc.Core.Pairing;
using SwitchifyPc.Protocol;

namespace SwitchifyPc.Tests;

public sealed class BluetoothControlFrameProcessorTests
{
    private const string DeviceId = "android-1";
    private const string Token = "shared-token";
    private const double Now = 1_000_000;

    [Fact]
    public async Task ReassemblesRequestAndFramesAckResponse()
    {
        FakeInputAdapter adapter = new();
        BluetoothControlFrameProcessor processor = new(CreateSession(adapter), maxResponseFramePayloadBytes: 20);
        IReadOnlyList<BluetoothFrame> frames = BluetoothFrameCodec.CreateFrames(SignedCommand("keyboard.key", new { key = "Meta" }), "incoming-1", maxPayloadBytes: 40);

        BluetoothControlFrameResult incomplete = BluetoothControlFrameResult.Incomplete();
        BluetoothControlFrameResult complete = BluetoothControlFrameResult.Incomplete();
        for (int index = 0; index < frames.Count; index++)
        {
            BluetoothControlFrameResult result = await processor.AcceptAsync("ble", frames[index]);
            if (index == 0)
            {
                incomplete = result;
            }

            complete = result;
        }

        Assert.False(incomplete.MessageComplete);
        Assert.True(complete.MessageComplete);
        Assert.Null(complete.ErrorReason);
        Assert.NotEmpty(complete.ResponseFrames);
        Assert.Equal(["pressKey:Meta"], adapter.Calls);
        AssertResponseType(complete.ResponseFrames, "ack");
    }

    [Fact]
    public async Task NoAckRequestsCompleteWithoutResponseFrames()
    {
        FakeInputAdapter adapter = new();
        BluetoothControlFrameProcessor processor = new(CreateSession(adapter));
        IReadOnlyList<BluetoothFrame> frames = BluetoothFrameCodec.CreateFrames(SignedCommand("mouse.move", new { dx = 4, dy = 5 }, responseMode: "none"), "incoming-1");

        BluetoothControlFrameResult result = BluetoothControlFrameResult.Incomplete();
        foreach (BluetoothFrame frame in frames)
        {
            result = await processor.AcceptAsync("ble", frame);
        }

        Assert.True(result.MessageComplete);
        Assert.Empty(result.ResponseFrames);
        Assert.Equal(["moveMouseBy:4,5"], adapter.Calls);
    }

    [Fact]
    public async Task InvalidFrameReturnsErrorWithoutSessionExecution()
    {
        FakeInputAdapter adapter = new();
        BluetoothControlFrameProcessor processor = new(CreateSession(adapter));
        BluetoothFrame invalid = new(99, "incoming-1", 0, true, 2, "e30=");

        BluetoothControlFrameResult result = await processor.AcceptAsync("ble", invalid);

        Assert.True(result.MessageComplete);
        Assert.Equal("invalid_frame", result.ErrorReason);
        Assert.Empty(result.ResponseFrames);
        Assert.Empty(adapter.Calls);
    }

    [Fact]
    public async Task RemoveConnectionDropsPartialMessages()
    {
        BluetoothControlFrameProcessor processor = new(CreateSession(new FakeInputAdapter()));
        IReadOnlyList<BluetoothFrame> frames = BluetoothFrameCodec.CreateFrames(SignedCommand("keyboard.key", new { key = "Meta" }), "incoming-1", maxPayloadBytes: 40);

        Assert.False((await processor.AcceptAsync("ble", frames[0])).MessageComplete);
        processor.RemoveConnection("ble");
        BluetoothControlFrameResult result = await processor.AcceptAsync("ble", frames[^1]);

        Assert.False(result.MessageComplete);
        Assert.Equal("incomplete", result.ErrorReason);
    }

    private static void AssertResponseType(IReadOnlyList<BluetoothFrame> frames, string expectedType)
    {
        BluetoothFrameReassembler reassembler = new();
        BluetoothFrameReassemblyResult result = BluetoothFrameReassemblyResult.Incomplete("incomplete");
        foreach (BluetoothFrame frame in frames)
        {
            result = reassembler.Accept(frame);
        }

        Assert.True(result.Ok);
        using JsonDocument response = JsonDocument.Parse(result.Message!);
        Assert.Equal(expectedType, response.RootElement.GetProperty("type").GetString());
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
            Capabilities: TestPointerCapabilities());

        return new ControlSession(
            new CommandAuthValidator(store, () => Now),
            new DesktopCommandExecutor(adapter),
            new FixedPointerProfileProvider(profile));
    }

    private static PointerCapabilities TestPointerCapabilities()
    {
        return new PointerCapabilities(
            true,
            ProtocolConstants.NoAckControlCommandTypes.ToArray(),
            ProtocolConstants.CommandTypes.ToArray(),
            new MouseRepeatCapabilities(true, true, 250, 250, 250, 100, 2000));
    }

    private static string SignedCommand(string type, object payload, string id = "request-1", string? responseMode = null)
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

        if (responseMode is not null)
        {
            command["responseMode"] = responseMode;
        }

        using JsonDocument unsignedDocument = JsonDocument.Parse(command.ToJsonString());
        command["auth"] = CommandAuth.CreateCommandAuthProof(unsignedDocument.RootElement, Token);
        return command.ToJsonString();
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
