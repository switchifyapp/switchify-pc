using SwitchifyPc.App;
using SwitchifyPc.Core.Bluetooth;
using SwitchifyPc.Core.Pairing;
using SwitchifyPc.Protocol;

namespace SwitchifyPc.Tests;

public sealed class BluetoothRuntimeTests
{
    [Fact]
    public async Task StartAndDisconnectDelegateToTransport()
    {
        TestContext context = CreateContext();

        await context.Runtime.StartAsync("Switchify PC", "desktop-1");
        context.Runtime.DisconnectAll();

        Assert.Equal(("Switchify PC", "desktop-1"), context.Server.StartArguments);
        Assert.Equal(1, context.Server.DisconnectAllCalls);
        Assert.Equal("starting", context.Tracker.Status.Status);
        Assert.Contains(context.Diagnostics, item => item.EventName == "bluetooth.starting");
        Assert.Contains(context.Diagnostics, item => item.EventName == "bluetooth.start.completed");
    }

    [Fact]
    public async Task ReadyAndUnavailableEventsUpdateTrackedStatus()
    {
        TestContext context = CreateContext();
        await context.Runtime.StartAsync("Switchify PC", "desktop-1");

        context.Server.Emit(new BluetoothReadyEvent());
        Assert.Equal("ready", context.Tracker.Status.Status);

        context.Server.Emit(new BluetoothUnavailableEvent("adapter_off"));
        Assert.Equal("unavailable", context.Tracker.Status.Status);
        Assert.Equal("adapter_off", context.Tracker.Status.Reason);
    }

    [Fact]
    public async Task ExpirySendsFramesAndRefreshesApprovals()
    {
        TestContext context = CreateContext();
        BluetoothFrame frame = new(1, "response-1", 0, true, 0, "");
        context.Processor.ExpiredOutputs =
        [
            new BluetoothRemoteFrameOutput("ble", [frame], CloseConnection: true)
        ];

        await context.Runtime.ExpirePendingPairingRequestsAsync();

        Assert.Equal([("ble", frame)], context.Server.Sent);
        Assert.Equal(["ble"], context.Server.Disconnected);
        Assert.Equal(1, context.Processor.ClearExpiredCalls);
        Assert.Equal(1, context.ApprovalsChanged);
    }

    [Theory]
    [InlineData("unknown_device", "Bluetooth device is not approved")]
    [InlineData("invalid_auth", "Switchify access expired")]
    [InlineData("expired_timestamp", "timestamp was stale")]
    public void AuthFailureMessagesRemainUserFriendly(string reason, string expected)
    {
        Assert.Contains(expected, BluetoothRuntime.AuthFailureMessage(reason));
    }

    [Fact]
    public void UnknownAuthFailureHasNoUserMessage()
    {
        Assert.Null(BluetoothRuntime.AuthFailureMessage("other"));
    }

    private static TestContext CreateContext()
    {
        MemoryPairingStore store = new(new PairingState("desktop-1", []));
        PairingApprovalManager approvals = new(store);
        BluetoothStatusTracker tracker = new(now: () => 100);
        FakeFrameProcessor processor = new();
        FakeTransportServer server = new();
        List<Diagnostic> diagnostics = [];
        int approvalsChanged = 0;
        BluetoothRuntime runtime = new(
            approvals,
            tracker,
            processor,
            emit =>
            {
                server.SetEmitter(emit);
                return server;
            },
            async action => await action(),
            () => Task.CompletedTask,
            (eventName, status, reason) => diagnostics.Add(new Diagnostic(eventName, status, reason)),
            () => approvalsChanged += 1);
        return new TestContext(runtime, tracker, processor, server, diagnostics, () => approvalsChanged);
    }

    private sealed record Diagnostic(string EventName, string? Status, string? Reason);

    private sealed record TestContext(
        BluetoothRuntime Runtime,
        BluetoothStatusTracker Tracker,
        FakeFrameProcessor Processor,
        FakeTransportServer Server,
        List<Diagnostic> Diagnostics,
        Func<int> GetApprovalsChanged)
    {
        public int ApprovalsChanged => GetApprovalsChanged();
    }

    private sealed class FakeTransportServer : IBluetoothTransportServer
    {
        private Action<BluetoothTransportEvent>? emit;
        public (string DisplayName, string DesktopId)? StartArguments { get; private set; }
        public int DisconnectAllCalls { get; private set; }
        public List<(string ConnectionId, BluetoothFrame Frame)> Sent { get; } = [];
        public List<string> Disconnected { get; } = [];

        public void SetEmitter(Action<BluetoothTransportEvent> emitter) => emit = emitter;
        public void Emit(BluetoothTransportEvent transportEvent) => emit!(transportEvent);
        public Task StartAsync(string displayName, string desktopId)
        {
            StartArguments = (displayName, desktopId);
            return Task.CompletedTask;
        }
        public Task SendAsync(string connectionId, BluetoothFrame frame)
        {
            Sent.Add((connectionId, frame));
            return Task.CompletedTask;
        }
        public void Disconnect(string connectionId) => Disconnected.Add(connectionId);
        public void DisconnectAll() => DisconnectAllCalls += 1;
        public void Stop() { }
        public void Dispose() { }
    }

    private sealed class FakeFrameProcessor : IBluetoothRemoteFrameProcessor
    {
        public IReadOnlyList<BluetoothRemoteFrameOutput> ExpiredOutputs { get; set; } = [];
        public int ClearExpiredCalls { get; private set; }

        public Task<BluetoothRemoteFrameResult> AcceptAsync(
            string connectionId,
            BluetoothFrame frame,
            string? remoteAddress = null,
            CancellationToken cancellationToken = default) =>
            Task.FromResult(BluetoothRemoteFrameResult.Incomplete());
        public Task<IReadOnlyList<BluetoothRemoteFrameOutput>> AcceptPairingRequestAsync(
            string requestId,
            CancellationToken cancellationToken = default) =>
            Task.FromResult<IReadOnlyList<BluetoothRemoteFrameOutput>>([]);
        public IReadOnlyList<BluetoothRemoteFrameOutput> RejectPairingRequest(string requestId) => [];
        public IReadOnlyList<BluetoothRemoteFrameOutput> ExpirePendingPairingRequests() => ExpiredOutputs;
        public void RemoveConnection(string connectionId) { }
        public int ClearExpiredPartials()
        {
            ClearExpiredCalls += 1;
            return 0;
        }
    }
}
