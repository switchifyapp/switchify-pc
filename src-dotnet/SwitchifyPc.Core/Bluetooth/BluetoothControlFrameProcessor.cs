using SwitchifyPc.Core.Control;
using SwitchifyPc.Protocol;

namespace SwitchifyPc.Core.Bluetooth;

public sealed record BluetoothControlFrameResult(
    bool MessageComplete,
    string? ErrorReason,
    IReadOnlyList<BluetoothFrame> ResponseFrames)
{
    public static BluetoothControlFrameResult Incomplete(string? reason = null) => new(false, reason, []);

    public static BluetoothControlFrameResult Complete(IReadOnlyList<BluetoothFrame> responseFrames) => new(true, null, responseFrames);

    public static BluetoothControlFrameResult Error(string reason) => new(true, reason, []);
}

public sealed class BluetoothControlFrameProcessor
{
    private readonly ControlSession controlSession;
    private readonly int maxResponseFramePayloadBytes;
    private readonly int maxMessageBytes;
    private readonly double partialTimeoutMs;
    private readonly Func<double> now;
    private readonly Dictionary<string, BluetoothFrameReassembler> reassemblers = new(StringComparer.Ordinal);

    public BluetoothControlFrameProcessor(
        ControlSession controlSession,
        int maxResponseFramePayloadBytes = BluetoothFrameCodec.DefaultBluetoothFramePayloadBytes,
        int maxMessageBytes = BluetoothFrameCodec.DefaultBluetoothMaxMessageBytes,
        double partialTimeoutMs = BluetoothFrameCodec.DefaultBluetoothPartialTimeoutMs,
        Func<double>? now = null)
    {
        this.controlSession = controlSession;
        this.maxResponseFramePayloadBytes = maxResponseFramePayloadBytes;
        this.maxMessageBytes = maxMessageBytes;
        this.partialTimeoutMs = partialTimeoutMs;
        this.now = now ?? (() => DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());
    }

    public async Task<BluetoothControlFrameResult> AcceptAsync(
        string connectionId,
        BluetoothFrame frame,
        CancellationToken cancellationToken = default)
    {
        BluetoothFrameReassembler reassembler = ReassemblerFor(connectionId);
        BluetoothFrameReassemblyResult reassembly = reassembler.Accept(frame);
        if (!reassembly.Ok)
        {
            return reassembly.Reason == "incomplete"
                ? BluetoothControlFrameResult.Incomplete(reassembly.Reason)
                : BluetoothControlFrameResult.Error(reassembly.Reason ?? "invalid_frame");
        }

        ControlSessionResult sessionResult = await controlSession.ProcessMessageAsync(reassembly.Message ?? "", cancellationToken).ConfigureAwait(false);
        if (!sessionResult.HasResponse)
        {
            return BluetoothControlFrameResult.Complete([]);
        }

        IReadOnlyList<BluetoothFrame> responseFrames = BluetoothFrameCodec.CreateFrames(
            sessionResult.ResponseJson!,
            maxPayloadBytes: maxResponseFramePayloadBytes,
            maxMessageBytes: maxMessageBytes);
        return BluetoothControlFrameResult.Complete(responseFrames);
    }

    public void RemoveConnection(string connectionId)
    {
        reassemblers.Remove(connectionId);
    }

    public int ClearExpired()
    {
        return reassemblers.Values.Sum(reassembler => reassembler.ClearExpired());
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
