using SwitchifyPc.Protocol;

namespace SwitchifyPc.Tests;

public sealed class BluetoothFrameTests
{
    [Fact]
    public void RoundTripsSingleFrameMessage()
    {
        BluetoothFrame frame = BluetoothFrameCodec.CreateFrames("""{"type":"connection.ping"}""", "message-1", 512).Single();
        BluetoothFrameReassembler reassembler = new();

        BluetoothFrameReassemblyResult result = reassembler.Accept(frame);

        Assert.True(result.Ok);
        Assert.Equal("""{"type":"connection.ping"}""", result.Message);
    }

    [Fact]
    public void RoundTripsMultiFrameMessage()
    {
        IReadOnlyList<BluetoothFrame> frames = BluetoothFrameCodec.CreateFrames("abcdefghijklmnopqrstuvwxyz", "message-1", 5);
        BluetoothFrameReassembler reassembler = new();

        foreach (BluetoothFrame frame in frames.Take(frames.Count - 1))
        {
            Assert.Equal("incomplete", reassembler.Accept(frame).Reason);
        }

        BluetoothFrameReassemblyResult result = reassembler.Accept(frames[^1]);

        Assert.True(result.Ok);
        Assert.Equal("abcdefghijklmnopqrstuvwxyz", result.Message);
    }

    [Fact]
    public void WaitsForMissingChunks()
    {
        IReadOnlyList<BluetoothFrame> frames = BluetoothFrameCodec.CreateFrames("abcdefghijklmnopqrstuvwxyz", "message-1", 5);
        BluetoothFrameReassembler reassembler = new();

        Assert.Equal("incomplete", reassembler.Accept(frames[0]).Reason);
        Assert.Equal("incomplete", reassembler.Accept(frames[^1]).Reason);
    }

    [Fact]
    public void RejectsOversizedMessagesAndInvalidVersions()
    {
        Assert.Throws<InvalidOperationException>(() => BluetoothFrameCodec.CreateFrames("too large", maxMessageBytes: 3));
        Assert.Equal("message_too_large", BluetoothFrameCodec.Validate(CreateFrame(totalBytes: 10), 3).Reason);
        Assert.Equal("invalid_frame", BluetoothFrameCodec.Validate(CreateFrame(version: 2)).Reason);
    }

    [Fact]
    public void ExpiresPartialMessages()
    {
        double now = 1000;
        BluetoothFrame first = BluetoothFrameCodec.CreateFrames("abcdefghijklmnopqrstuvwxyz", "message-1", 5)[0];
        BluetoothFrameReassembler reassembler = new(partialTimeoutMs: 100, now: () => now);

        Assert.Equal("incomplete", reassembler.Accept(first).Reason);
        now = 1101;

        Assert.Equal(1, reassembler.ClearExpired());
    }

    private static BluetoothFrame CreateFrame(int version = BluetoothFrameCodec.BluetoothFrameVersion, int totalBytes = 3)
    {
        return new BluetoothFrame(version, "message-1", 0, true, totalBytes, Convert.ToBase64String(System.Text.Encoding.UTF8.GetBytes("abc")));
    }
}
