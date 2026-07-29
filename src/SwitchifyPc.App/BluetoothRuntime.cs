using System.Windows.Threading;
using SwitchifyPc.Core.Bluetooth;
using SwitchifyPc.Core.Pairing;

namespace SwitchifyPc.App;

internal sealed class BluetoothRuntime : IDisposable
{
    private readonly PairingApprovalManager pairingApprovalManager;
    private readonly BluetoothStatusTracker statusTracker;
    private readonly IBluetoothRemoteFrameProcessor frameProcessor;
    private readonly IBluetoothTransportServer server;
    private readonly Func<Func<Task>, Task> dispatchAsync;
    private readonly Func<Task> endControlSessionAsync;
    private readonly Action<string, string?, string?> recordDiagnostic;
    private readonly Action pairingApprovalsChanged;
    private readonly SemaphoreSlim messageProcessing = new(1, 1);
    private readonly SemaphoreSlim outputProcessing = new(1, 1);
    private readonly object connectionSync = new();
    private readonly HashSet<string> authenticatedConnections = new(StringComparer.Ordinal);
    private DispatcherTimer? pairingExpiryTimer;
    private bool disposed;

    public BluetoothRuntime(
        PairingApprovalManager pairingApprovalManager,
        BluetoothStatusTracker statusTracker,
        IBluetoothRemoteFrameProcessor frameProcessor,
        Func<Action<BluetoothHelperEvent>, IBluetoothTransportServer> serverFactory,
        Func<Func<Task>, Task> dispatchAsync,
        Func<Task> endControlSessionAsync,
        Action<string, string?, string?> recordDiagnostic,
        Action pairingApprovalsChanged)
    {
        this.pairingApprovalManager = pairingApprovalManager;
        this.statusTracker = statusTracker;
        this.frameProcessor = frameProcessor;
        this.dispatchAsync = dispatchAsync;
        this.endControlSessionAsync = endControlSessionAsync;
        this.recordDiagnostic = recordDiagnostic;
        this.pairingApprovalsChanged = pairingApprovalsChanged;
        server = serverFactory(HandleTransportEvent);
    }

    public BluetoothStatus Status => statusTracker.Status;

    public IReadOnlyList<PendingPairingApprovalView> PendingPairingApprovals =>
        pairingApprovalManager.ListPendingRequestViews();

    public async Task StartAsync(string displayName, string desktopId)
    {
        ObjectDisposedException.ThrowIf(disposed, this);
        statusTracker.SetStarting();
        recordDiagnostic("bluetooth.starting", null, null);
        StartPairingExpiryTimer();
        await server.StartAsync(displayName, desktopId).ConfigureAwait(false);
        recordDiagnostic("bluetooth.start.completed", null, null);
    }

    public void DisconnectAll()
    {
        server.DisconnectAll();
    }

    public async Task AcceptPairingRequestAsync(string requestId)
    {
        await SendOutputsAsync(await frameProcessor.AcceptPairingRequestAsync(requestId).ConfigureAwait(false))
            .ConfigureAwait(false);
        pairingApprovalsChanged();
    }

    public void RejectPairingRequest(string requestId)
    {
        _ = SendOutputsAsync(frameProcessor.RejectPairingRequest(requestId));
        pairingApprovalsChanged();
    }

    public async Task ExpirePendingPairingRequestsAsync()
    {
        await SendOutputsAsync(frameProcessor.ExpirePendingPairingRequests()).ConfigureAwait(false);
        frameProcessor.ClearExpiredPartials();
        pairingApprovalsChanged();
    }

    public void Dispose()
    {
        if (disposed) return;
        disposed = true;
        pairingExpiryTimer?.Stop();
        pairingExpiryTimer = null;
        server.Dispose();
        messageProcessing.Dispose();
        outputProcessing.Dispose();
        lock (connectionSync)
        {
            authenticatedConnections.Clear();
        }
    }

    private void HandleTransportEvent(BluetoothHelperEvent transportEvent)
    {
        if (transportEvent is BluetoothMessageEvent message)
        {
            _ = ProcessMessageAsync(message);
            return;
        }

        _ = dispatchAsync(() => ProcessTransportEventAsync(transportEvent));
    }

    private async Task ProcessTransportEventAsync(BluetoothHelperEvent transportEvent)
    {
        switch (transportEvent)
        {
            case BluetoothReadyEvent:
                statusTracker.SetReady();
                recordDiagnostic("bluetooth.ready", "ready", null);
                break;
            case BluetoothUnavailableEvent unavailable:
                statusTracker.SetUnavailable(unavailable.Reason);
                recordDiagnostic("bluetooth.unavailable", "unavailable", unavailable.Reason);
                break;
            case BluetoothConnectedEvent:
                statusTracker.RecordDiagnostic("transport_connected");
                recordDiagnostic("bluetooth.connected", "connected", null);
                break;
            case BluetoothDisconnectedEvent disconnected:
                await HandleDisconnectedAsync(disconnected).ConfigureAwait(false);
                break;
            case BluetoothDiagnosticEvent diagnostic:
                statusTracker.RecordDiagnostic(diagnostic.Event);
                recordDiagnostic("bluetooth.diagnostic", null, diagnostic.Event);
                break;
            case BluetoothSystemStatusEvent systemStatus:
                statusTracker.SetSystemStatus(systemStatus);
                break;
            case BluetoothErrorEvent error:
                statusTracker.SetError(error.Reason);
                recordDiagnostic("bluetooth.error", "error", error.Reason);
                break;
        }
    }

    private async Task HandleDisconnectedAsync(BluetoothDisconnectedEvent disconnected)
    {
        BluetoothStatus status = statusTracker.RemoveConnection(disconnected.ConnectionId, disconnected.Reason);
        recordDiagnostic("bluetooth.disconnected", status.Status, disconnected.Reason);
        lock (connectionSync)
        {
            authenticatedConnections.Remove(disconnected.ConnectionId);
        }
        frameProcessor.RemoveConnection(disconnected.ConnectionId);
        if (status.ConnectedClientCount == 0)
        {
            await endControlSessionAsync().ConfigureAwait(false);
        }
    }

    private async Task ProcessMessageAsync(BluetoothMessageEvent message)
    {
        BluetoothRemoteFrameResult result;
        await messageProcessing.WaitAsync().ConfigureAwait(false);
        try
        {
            result = await frameProcessor.AcceptAsync(message.ConnectionId, message.Frame).ConfigureAwait(false);
        }
        finally
        {
            messageProcessing.Release();
        }

        if (!result.MessageComplete) return;
        if (result.ErrorReason is not null)
        {
            recordDiagnostic("bluetooth.message.error", "error", result.ErrorReason);
            await dispatchAsync(() =>
            {
                statusTracker.SetError(result.ErrorReason);
                return Task.CompletedTask;
            }).ConfigureAwait(false);
            return;
        }

        await SendOutputsAsync(result.OutgoingMessages).ConfigureAwait(false);
        await UpdateAuthenticationStateAsync(result).ConfigureAwait(false);
    }

    private async Task UpdateAuthenticationStateAsync(BluetoothRemoteFrameResult result)
    {
        if (result.AuthenticatedConnectionId is not null)
        {
            string connectionId = result.AuthenticatedConnectionId;
            bool firstAuthenticatedMessage;
            lock (connectionSync)
            {
                firstAuthenticatedMessage = authenticatedConnections.Add(connectionId);
            }

            if (firstAuthenticatedMessage)
            {
                recordDiagnostic("bluetooth.authenticated", "connected", null);
                await dispatchAsync(() =>
                {
                    statusTracker.AddConnection(connectionId);
                    return Task.CompletedTask;
                }).ConfigureAwait(false);
            }
            return;
        }

        if (result.AuthFailureReason is null) return;
        recordDiagnostic("bluetooth.auth.rejected", null, result.AuthFailureReason);
        await dispatchAsync(() =>
        {
            statusTracker.RecordDiagnostic("unauthenticated_command_rejected");
            string? message = AuthFailureMessage(result.AuthFailureReason);
            if (message is not null)
            {
                statusTracker.SetError(message);
            }
            return Task.CompletedTask;
        }).ConfigureAwait(false);
    }

    private async Task SendOutputsAsync(IReadOnlyList<BluetoothRemoteFrameOutput> outputs)
    {
        await outputProcessing.WaitAsync().ConfigureAwait(false);
        try
        {
            foreach (BluetoothRemoteFrameOutput output in outputs)
            {
                foreach (var frame in output.ResponseFrames)
                {
                    await server.SendAsync(output.ConnectionId, frame).ConfigureAwait(false);
                }
                if (output.CloseConnection)
                {
                    server.Disconnect(output.ConnectionId);
                }
            }
        }
        finally
        {
            outputProcessing.Release();
        }
    }

    private void StartPairingExpiryTimer()
    {
        pairingExpiryTimer = new DispatcherTimer { Interval = TimeSpan.FromSeconds(5) };
        pairingExpiryTimer.Tick += async (_, _) => await ExpirePendingPairingRequestsAsync();
        pairingExpiryTimer.Start();
    }

    internal static string? AuthFailureMessage(string reason)
    {
        return reason switch
        {
            "unknown_device" => "Bluetooth device is not approved in Switchify. Open Switchify on Android and request access.",
            "invalid_auth" => "Switchify access expired. Request access again from Android.",
            "expired_timestamp" => "Switchify command timestamp was stale. Check the device time and reconnect.",
            _ => null
        };
    }
}
