using System.Text;
using System.Text.RegularExpressions;

namespace SwitchifyPc.Protocol;

public sealed record BluetoothFrame(
    int Version,
    string MessageId,
    int Sequence,
    bool IsFinal,
    int TotalBytes,
    string PayloadBase64);

public sealed record BluetoothFrameReassemblyResult(bool Ok, string? Message = null, string? Reason = null)
{
    public static BluetoothFrameReassemblyResult Complete(string message) => new(true, message);
    public static BluetoothFrameReassemblyResult Incomplete(string reason) => new(false, null, reason);
}

public static partial class BluetoothFrameCodec
{
    public const int BluetoothFrameVersion = 1;
    public const int DefaultBluetoothFramePayloadBytes = 160;
    public const int DefaultBluetoothMaxMessageBytes = 16 * 1024;
    public const int DefaultBluetoothPartialTimeoutMs = 10_000;

    public static IReadOnlyList<BluetoothFrame> CreateFrames(
        string message,
        string? messageId = null,
        int maxPayloadBytes = DefaultBluetoothFramePayloadBytes,
        int maxMessageBytes = DefaultBluetoothMaxMessageBytes)
    {
        byte[] payload = Encoding.UTF8.GetBytes(message);
        if (maxPayloadBytes <= 0) throw new InvalidOperationException("Bluetooth frame payload size must be positive.");
        if (payload.Length > maxMessageBytes) throw new InvalidOperationException("Bluetooth message is too large.");

        string id = messageId ?? Guid.NewGuid().ToString();
        List<BluetoothFrame> frames = [];
        for (int offset = 0, sequence = 0; offset < payload.Length || sequence == 0; offset += maxPayloadBytes, sequence += 1)
        {
            int count = Math.Min(maxPayloadBytes, Math.Max(0, payload.Length - offset));
            byte[] chunk = payload.Skip(offset).Take(count).ToArray();
            frames.Add(new BluetoothFrame(
                Version: BluetoothFrameVersion,
                MessageId: id,
                Sequence: sequence,
                IsFinal: offset + maxPayloadBytes >= payload.Length,
                TotalBytes: payload.Length,
                PayloadBase64: Convert.ToBase64String(chunk)));
        }

        return frames;
    }

    public static BluetoothFrameReassemblyResult Validate(BluetoothFrame frame, int maxMessageBytes = DefaultBluetoothMaxMessageBytes)
    {
        return GetValidationError(frame, maxMessageBytes) ?? BluetoothFrameReassemblyResult.Incomplete("incomplete");
    }

    internal static BluetoothFrameReassemblyResult? GetValidationError(BluetoothFrame frame, int maxMessageBytes = DefaultBluetoothMaxMessageBytes)
    {
        if (frame.Version != BluetoothFrameVersion ||
            string.IsNullOrEmpty(frame.MessageId) ||
            frame.Sequence < 0 ||
            frame.TotalBytes < 0 ||
            frame.TotalBytes > maxMessageBytes ||
            frame.PayloadBase64 is null)
        {
            return BluetoothFrameReassemblyResult.Incomplete(frame.TotalBytes > maxMessageBytes ? "message_too_large" : "invalid_frame");
        }

        return IsValidBase64(frame.PayloadBase64)
            ? null
            : BluetoothFrameReassemblyResult.Incomplete("invalid_frame");
    }

    private static bool IsValidBase64(string value)
    {
        if (value.Length == 0) return true;
        return value.Length % 4 == 0 && Base64Regex().IsMatch(value);
    }

    [GeneratedRegex("^[A-Za-z0-9+/]+={0,2}$")]
    private static partial Regex Base64Regex();
}

public sealed class BluetoothFrameReassembler
{
    private readonly int maxMessageBytes;
    private readonly double partialTimeoutMs;
    private readonly Func<double> now;
    private readonly Dictionary<string, PartialBluetoothMessage> partialMessages = new(StringComparer.Ordinal);

    public BluetoothFrameReassembler(
        int maxMessageBytes = BluetoothFrameCodec.DefaultBluetoothMaxMessageBytes,
        double partialTimeoutMs = BluetoothFrameCodec.DefaultBluetoothPartialTimeoutMs,
        Func<double>? now = null)
    {
        this.maxMessageBytes = maxMessageBytes;
        this.partialTimeoutMs = partialTimeoutMs;
        this.now = now ?? (() => DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());
    }

    public BluetoothFrameReassemblyResult Accept(BluetoothFrame frame)
    {
        BluetoothFrameReassemblyResult? validation = BluetoothFrameCodec.GetValidationError(frame, maxMessageBytes);
        if (validation is not null) return validation;

        ExpirePartials();

        if (!partialMessages.TryGetValue(frame.MessageId, out PartialBluetoothMessage? partial))
        {
            partial = new PartialBluetoothMessage(frame.TotalBytes, now());
            partialMessages[frame.MessageId] = partial;
        }

        if (partial.TotalBytes != frame.TotalBytes)
        {
            partialMessages.Remove(frame.MessageId);
            return BluetoothFrameReassemblyResult.Incomplete("invalid_frame");
        }

        partial.Chunks.TryAdd(frame.Sequence, Convert.FromBase64String(frame.PayloadBase64));
        if (!frame.IsFinal) return BluetoothFrameReassemblyResult.Incomplete("incomplete");

        List<byte[]> chunks = [];
        int totalBytes = 0;
        for (int sequence = 0; partial.Chunks.ContainsKey(sequence); sequence += 1)
        {
            byte[] chunk = partial.Chunks[sequence];
            chunks.Add(chunk);
            totalBytes += chunk.Length;
            if (totalBytes > partial.TotalBytes)
            {
                partialMessages.Remove(frame.MessageId);
                return BluetoothFrameReassemblyResult.Incomplete("invalid_frame");
            }
        }

        if (totalBytes != partial.TotalBytes) return BluetoothFrameReassemblyResult.Incomplete("incomplete");

        partialMessages.Remove(frame.MessageId);
        byte[] payload = new byte[totalBytes];
        int offset = 0;
        foreach (byte[] chunk in chunks)
        {
            Buffer.BlockCopy(chunk, 0, payload, offset, chunk.Length);
            offset += chunk.Length;
        }

        return BluetoothFrameReassemblyResult.Complete(Encoding.UTF8.GetString(payload));
    }

    public int ClearExpired()
    {
        return ExpirePartials();
    }

    private int ExpirePartials()
    {
        double deadline = now() - partialTimeoutMs;
        string[] expired = partialMessages
            .Where(entry => entry.Value.CreatedAt <= deadline)
            .Select(entry => entry.Key)
            .ToArray();
        foreach (string key in expired)
        {
            partialMessages.Remove(key);
        }

        return expired.Length;
    }

    private sealed record PartialBluetoothMessage(int TotalBytes, double CreatedAt)
    {
        public Dictionary<int, byte[]> Chunks { get; } = [];
    }
}
