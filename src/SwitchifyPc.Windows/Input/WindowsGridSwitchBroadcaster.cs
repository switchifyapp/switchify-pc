using System.ComponentModel;
using System.Runtime.InteropServices;
using SwitchifyPc.Core.Input;
using SwitchifyPc.Protocol;

namespace SwitchifyPc.Windows.Input;

public interface IGridSwitchNativeMessenger
{
    uint RegisterWindowMessage(string messageName);

    bool SendMessageTimeout(
        nint windowHandle,
        uint message,
        nuint wParam,
        nint lParam,
        uint flags,
        uint timeoutMilliseconds);
}

public sealed class WindowsGridSwitchBroadcaster : IGridSwitchBroadcaster
{
    public const string MessageName = "Sensory_SwitchInput";
    public const int NativePressedValue = 1;
    public const int NativeReleasedValue = 0;
    public const uint SendFlags = 0x0001 | 0x0002;
    public const uint TimeoutMilliseconds = 1_000;
    public static readonly nint BroadcastWindowHandle = new(0xffff);

    private readonly IGridSwitchNativeMessenger messenger;
    private readonly uint messageId;

    public WindowsGridSwitchBroadcaster(IGridSwitchNativeMessenger? messenger = null)
    {
        if (!OperatingSystem.IsWindows())
        {
            throw new PlatformNotSupportedException("Grid 3 switch input is only available on Windows.");
        }

        this.messenger = messenger ?? new GridSwitchNativeMessenger();
        messageId = this.messenger.RegisterWindowMessage(MessageName);
        if (messageId == 0)
        {
            throw new Win32Exception(Marshal.GetLastWin32Error(), $"Could not register {MessageName}.");
        }
    }

    public Task SetSwitchStateAsync(int switchId, bool down, CancellationToken cancellationToken = default)
    {
        if (switchId is < ProtocolConstants.MinimumGridSwitchId or > ProtocolConstants.MaximumGridSwitchId)
        {
            throw new ArgumentOutOfRangeException(nameof(switchId));
        }

        return Task.Run(() =>
        {
            cancellationToken.ThrowIfCancellationRequested();
            bool sent = messenger.SendMessageTimeout(
                BroadcastWindowHandle,
                messageId,
                (nuint)switchId,
                down ? NativePressedValue : NativeReleasedValue,
                SendFlags,
                TimeoutMilliseconds);
            if (!sent)
            {
                throw new Win32Exception(Marshal.GetLastWin32Error(), $"Could not broadcast Grid switch {switchId}.");
            }

            cancellationToken.ThrowIfCancellationRequested();
        }, cancellationToken);
    }

    private sealed class GridSwitchNativeMessenger : IGridSwitchNativeMessenger
    {
        public uint RegisterWindowMessage(string messageName) => RegisterWindowMessageNative(messageName);

        public bool SendMessageTimeout(
            nint windowHandle,
            uint message,
            nuint wParam,
            nint lParam,
            uint flags,
            uint timeoutMilliseconds)
        {
            return SendMessageTimeoutNative(
                windowHandle,
                message,
                wParam,
                lParam,
                flags,
                timeoutMilliseconds,
                out _) != 0;
        }

        [DllImport(
            "user32.dll",
            EntryPoint = "RegisterWindowMessageW",
            CharSet = CharSet.Unicode,
            SetLastError = true)]
        private static extern uint RegisterWindowMessageNative(string messageName);

        [DllImport(
            "user32.dll",
            EntryPoint = "SendMessageTimeoutW",
            CharSet = CharSet.Unicode,
            SetLastError = true)]
        private static extern nint SendMessageTimeoutNative(
            nint windowHandle,
            uint message,
            nuint wParam,
            nint lParam,
            uint flags,
            uint timeoutMilliseconds,
            out nuint result);
    }
}
