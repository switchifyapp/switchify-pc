using SwitchifyPc.Core.Control;
using SwitchifyPc.Protocol;

namespace SwitchifyPc.Core.Bluetooth;

public sealed record BluetoothRemoteFrameOutput(
    string ConnectionId,
    IReadOnlyList<BluetoothFrame> ResponseFrames,
    bool CloseConnection = false);

public sealed record BluetoothRemoteFrameResult(
    bool MessageComplete,
    string? ErrorReason,
    IReadOnlyList<BluetoothRemoteFrameOutput> OutgoingMessages)
{
    public static BluetoothRemoteFrameResult Incomplete(string? reason = null) => new(false, reason, []);

    public static BluetoothRemoteFrameResult Complete(IReadOnlyList<BluetoothRemoteFrameOutput> outgoingMessages) =>
        new(true, null, outgoingMessages);

    public static BluetoothRemoteFrameResult Error(string reason) => new(true, reason, []);
}

public sealed class BluetoothRemoteFrameProcessor
{
    private readonly RemoteControlSession remoteSession;
    private readonly int maxResponseFramePayloadBytes;
    private readonly int maxMessageBytes;
    private readonly double partialTimeoutMs;
    private readonly Func<double> now;
    private readonly Dictionary<string, BluetoothFrameReassembler> reassemblers = new(StringComparer.Ordinal);

    public BluetoothRemoteFrameProcessor(
        RemoteControlSession remoteSession,
        int maxResponseFramePayloadBytes = BluetoothFrameCodec.DefaultBluetoothFramePayloadBytes,
        int maxMessageBytes = BluetoothFrameCodec.DefaultBluetoothMaxMessageBytes,
        double partialTimeoutMs = BluetoothFrameCodec.DefaultBluetoothPartialTimeoutMs,
        Func<double>? now = null)
    {
        this.remoteSession = remoteSession;
        this.maxResponseFramePayloadBytes = maxResponseFramePayloadBytes;
        this.maxMessageBytes = maxMessageBytes;
        this.partialTimeoutMs = partialTimeoutMs;
        this.now = now ?? (() => DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());
    }

    public async Task<BluetoothRemoteFrameResult> AcceptAsync(
        string connectionId,
        BluetoothFrame frame,
        string? remoteAddress = null,
        CancellationToken cancellationToken = default)
    {
        BluetoothFrameReassemblyResult reassembly = ReassemblerFor(connectionId).Accept(frame);
        if (!reassembly.Ok)
        {
            return reassembly.Reason == "incomplete"
                ? BluetoothRemoteFrameResult.Incomplete(reassembly.Reason)
                : BluetoothRemoteFrameResult.Error(reassembly.Reason ?? "invalid_frame");
        }

        RemoteSessionResult sessionResult = await remoteSession.ProcessMessageAsync(
            connectionId,
            reassembly.Message ?? "",
            remoteAddress,
            cancellationToken).ConfigureAwait(false);
        return BluetoothRemoteFrameResult.Complete(FrameOutgoingMessages(sessionResult));
    }

    public Task<IReadOnlyList<BluetoothRemoteFrameOutput>> AcceptPairingRequestAsync(
        string requestId,
        CancellationToken cancellationToken = default)
    {
        return FramePairingResultAsync(remoteSession.AcceptPairingRequestAsync(requestId, cancellationToken));
    }

    public IReadOnlyList<BluetoothRemoteFrameOutput> RejectPairingRequest(string requestId)
    {
        return FrameOutgoingMessages(remoteSession.RejectPairingRequest(requestId));
    }

    public IReadOnlyList<BluetoothRemoteFrameOutput> ExpirePendingPairingRequests()
    {
        return FrameOutgoingMessages(remoteSession.ExpirePendingPairingRequests());
    }

    public void RemoveConnection(string connectionId)
    {
        reassemblers.Remove(connectionId);
        remoteSession.RemoveConnection(connectionId);
    }

    public int ClearExpiredPartials()
    {
        return reassemblers.Values.Sum(reassembler => reassembler.ClearExpired());
    }

    private async Task<IReadOnlyList<BluetoothRemoteFrameOutput>> FramePairingResultAsync(Task<RemoteSessionResult> resultTask)
    {
        return FrameOutgoingMessages(await resultTask.ConfigureAwait(false));
    }

    private IReadOnlyList<BluetoothRemoteFrameOutput> FrameOutgoingMessages(RemoteSessionResult result)
    {
        return result.OutgoingMessages
            .Select(message => new BluetoothRemoteFrameOutput(
                message.ConnectionId,
                BluetoothFrameCodec.CreateFrames(
                    message.ResponseJson,
                    maxPayloadBytes: maxResponseFramePayloadBytes,
                    maxMessageBytes: maxMessageBytes),
                message.CloseConnection))
            .ToArray();
    }

    private BluetoothFrameReassembler ReassemblerFor(string connectionId)
    {
        if (!reassemblers.TryGetValue(connectionId, out BluetoothFrameReassembler? reassembler))
        {
            reassembler = new BluetoothFrameReassembler(maxMessageBytes, partialTimeoutMs, now);
            reassemblers[connectionId] = reassembler;
        }

        return reassembler;
    }
}
