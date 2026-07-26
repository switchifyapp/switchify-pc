using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using SwitchifyPc.Core.Input;
using SwitchifyPc.Protocol;

namespace SwitchifyPc.Windows.Input;

public interface IGridSwitchNativeMessenger
{
    uint RegisterWindowMessage(string messageName);

    bool PostMessage(
        nint windowHandle,
        uint message,
        nuint wParam,
        nint lParam);
}

public sealed class WindowsGridSwitchBroadcaster : IGridSwitchBroadcaster
{
    public const string MessageName = "Sensory_SwitchInput";
    public const int NativePressedValue = 1;
    public const int NativeReleasedValue = 0;
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

        cancellationToken.ThrowIfCancellationRequested();
        Debug.WriteLine(
            $"Grid3SwitchTrace tTicks={Stopwatch.GetTimestamp()} phase=native_start " +
            $"switchId={switchId} down={down}");
        bool posted = messenger.PostMessage(
            BroadcastWindowHandle,
            messageId,
            (nuint)switchId,
            down ? NativePressedValue : NativeReleasedValue);
        if (!posted)
        {
            throw new Win32Exception(Marshal.GetLastWin32Error(), $"Could not queue Grid switch {switchId}.");
        }

        Debug.WriteLine(
            $"Grid3SwitchTrace tTicks={Stopwatch.GetTimestamp()} phase=native_queued " +
            $"switchId={switchId} down={down}");
        return Task.CompletedTask;
    }

    private sealed class GridSwitchNativeMessenger : IGridSwitchNativeMessenger
    {
        public uint RegisterWindowMessage(string messageName) => RegisterWindowMessageNative(messageName);

        public bool PostMessage(
            nint windowHandle,
            uint message,
            nuint wParam,
            nint lParam)
        {
            return PostMessageNative(
                windowHandle,
                message,
                wParam,
                lParam);
        }

        [DllImport(
            "user32.dll",
            EntryPoint = "RegisterWindowMessageW",
            CharSet = CharSet.Unicode,
            SetLastError = true)]
        private static extern uint RegisterWindowMessageNative(string messageName);

        [DllImport(
            "user32.dll",
            EntryPoint = "PostMessageW",
            CharSet = CharSet.Unicode,
            SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static extern bool PostMessageNative(
            nint windowHandle,
            uint message,
            nuint wParam,
            nint lParam);
    }
}
