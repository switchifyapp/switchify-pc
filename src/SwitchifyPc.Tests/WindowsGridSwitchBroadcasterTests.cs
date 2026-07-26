using System.ComponentModel;
using SwitchifyPc.Windows.Input;

namespace SwitchifyPc.Tests;

public sealed class WindowsGridSwitchBroadcasterTests
{
    [Fact]
    public async Task RegistersAndMapsExactNativeMessageValues()
    {
        FakeMessenger messenger = new();
        WindowsGridSwitchBroadcaster broadcaster = new(messenger);

        await broadcaster.SetSwitchStateAsync(2, down: true);
        await broadcaster.SetSwitchStateAsync(8, down: false);

        Assert.Equal(WindowsGridSwitchBroadcaster.MessageName, messenger.RegisteredName);
        Assert.Equal(
            [
                new NativeCall(
                    WindowsGridSwitchBroadcaster.BroadcastWindowHandle,
                    42,
                    2,
                    WindowsGridSwitchBroadcaster.NativePressedValue,
                    WindowsGridSwitchBroadcaster.SendFlags,
                    WindowsGridSwitchBroadcaster.TimeoutMilliseconds),
                new NativeCall(
                    WindowsGridSwitchBroadcaster.BroadcastWindowHandle,
                    42,
                    8,
                    WindowsGridSwitchBroadcaster.NativeReleasedValue,
                    WindowsGridSwitchBroadcaster.SendFlags,
                    WindowsGridSwitchBroadcaster.TimeoutMilliseconds)
            ],
            messenger.Calls);
    }

    [Fact]
    public async Task RejectsInvalidIdsAndNativeFailure()
    {
        FakeMessenger messenger = new() { SendResult = false };
        WindowsGridSwitchBroadcaster broadcaster = new(messenger);

        await Assert.ThrowsAsync<ArgumentOutOfRangeException>(() => broadcaster.SetSwitchStateAsync(0, down: true));
        await Assert.ThrowsAsync<ArgumentOutOfRangeException>(() => broadcaster.SetSwitchStateAsync(9, down: true));
        await Assert.ThrowsAsync<Win32Exception>(() => broadcaster.SetSwitchStateAsync(1, down: true));
    }

    [Fact]
    public void FailsWhenNativeMessageCannotBeRegistered()
    {
        Assert.Throws<Win32Exception>(() => new WindowsGridSwitchBroadcaster(new FakeMessenger { MessageId = 0 }));
    }

    private sealed class FakeMessenger : IGridSwitchNativeMessenger
    {
        public uint MessageId { get; init; } = 42;
        public bool SendResult { get; init; } = true;
        public string? RegisteredName { get; private set; }
        public List<NativeCall> Calls { get; } = [];

        public uint RegisterWindowMessage(string messageName)
        {
            RegisteredName = messageName;
            return MessageId;
        }

        public bool SendMessageTimeout(
            nint windowHandle,
            uint message,
            nuint wParam,
            nint lParam,
            uint flags,
            uint timeoutMilliseconds)
        {
            Calls.Add(new NativeCall(windowHandle, message, wParam, lParam, flags, timeoutMilliseconds));
            return SendResult;
        }
    }

    private sealed record NativeCall(
        nint WindowHandle,
        uint Message,
        nuint WParam,
        nint LParam,
        uint Flags,
        uint TimeoutMilliseconds);
}
