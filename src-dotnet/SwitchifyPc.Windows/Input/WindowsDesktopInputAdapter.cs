using System.Runtime.InteropServices;
using SwitchifyPc.Core.Input;
using SwitchifyPc.Core.Settings;

namespace SwitchifyPc.Windows.Input;

public interface IWindowsNativeInput
{
    PointerPosition GetCursorPosition();
    PointerDisplay GetDisplayForPosition(PointerPosition position);
    void MoveCursorTo(PointerPosition position);
    void SetMouseButtonDown(string button, bool down);
    void Scroll(PointerDelta delta);
    void SetKeyDown(ushort virtualKey, bool down);
    void TypeUnicodeText(string text);
    void ControlWindow(string action);
}

public sealed class WindowsDesktopInputAdapter : IDesktopInputAdapter
{
    private readonly IWindowsNativeInput nativeInput;
    private PointerMovementSettings pointerMovementSettings;

    public WindowsDesktopInputAdapter(
        IWindowsNativeInput? nativeInput = null,
        PointerMovementSettings? pointerMovementSettings = null)
    {
        this.nativeInput = nativeInput ?? new SendInputWindowsNativeInput();
        this.pointerMovementSettings = PointerMovementSettingsModel.Normalize(pointerMovementSettings ?? PointerMovementSettingsModel.Default);
    }

    public void SetPointerMovementSettings(PointerMovementSettings settings)
    {
        pointerMovementSettings = PointerMovementSettingsModel.Normalize(settings);
    }

    public Task MoveMouseByAsync(double dx, double dy, CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        PointerPosition current = nativeInput.GetCursorPosition();
        PointerDisplay display = nativeInput.GetDisplayForPosition(current);
        nativeInput.MoveCursorTo(WindowsPointerMovement.CalculateDisplayNormalizedMouseTarget(current, new PointerDelta(dx, dy), display, pointerMovementSettings));
        return Task.CompletedTask;
    }

    public Task SetMouseButtonDownAsync(string button, bool down, CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        nativeInput.SetMouseButtonDown(button, down);
        return Task.CompletedTask;
    }

    public async Task ClickMouseAsync(string button, CancellationToken cancellationToken = default)
    {
        await SetMouseButtonDownAsync(button, true, cancellationToken).ConfigureAwait(false);
        await SetMouseButtonDownAsync(button, false, cancellationToken).ConfigureAwait(false);
    }

    public async Task DoubleClickMouseAsync(string button, CancellationToken cancellationToken = default)
    {
        await ClickMouseAsync(button, cancellationToken).ConfigureAwait(false);
        await ClickMouseAsync(button, cancellationToken).ConfigureAwait(false);
    }

    public Task ScrollMouseAsync(double dx, double dy, CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        nativeInput.Scroll(WindowsPointerMovement.CalculateNativeScrollDelta(new PointerDelta(dx, dy)));
        return Task.CompletedTask;
    }

    public async Task PressKeyAsync(string key, CancellationToken cancellationToken = default)
    {
        ushort virtualKey = WindowsInputMapper.KeyboardVirtualKey(key);
        await TapKeyAsync(virtualKey, cancellationToken).ConfigureAwait(false);
    }

    public async Task PressShortcutAsync(IReadOnlyList<string> keys, CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        if (keys.Count == 0) return;

        ushort[] virtualKeys = keys.Select(WindowsInputMapper.KeyboardVirtualKey).ToArray();
        if (virtualKeys.Length == 1)
        {
            await TapKeyAsync(virtualKeys[0], cancellationToken).ConfigureAwait(false);
            return;
        }

        foreach (ushort virtualKey in virtualKeys)
        {
            nativeInput.SetKeyDown(virtualKey, true);
        }

        foreach (ushort virtualKey in virtualKeys.AsEnumerable().Reverse())
        {
            nativeInput.SetKeyDown(virtualKey, false);
        }
    }

    public Task TypeTextAsync(string text, CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        nativeInput.TypeUnicodeText(text);
        return Task.CompletedTask;
    }

    public Task TypeCharacterAsync(string text, CancellationToken cancellationToken = default)
    {
        return TypeTextAsync(text, cancellationToken);
    }

    public async Task MediaControlAsync(string action, CancellationToken cancellationToken = default)
    {
        ushort virtualKey = WindowsInputMapper.MediaVirtualKey(action);
        await TapKeyAsync(virtualKey, cancellationToken).ConfigureAwait(false);
    }

    public async Task ControlWindowAsync(string action, CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        IReadOnlyList<ushort> shortcut = WindowsInputMapper.WindowControlShortcut(action);
        if (shortcut.Count > 0)
        {
            foreach (ushort virtualKey in shortcut)
            {
                nativeInput.SetKeyDown(virtualKey, true);
            }

            foreach (ushort virtualKey in shortcut.Reverse())
            {
                nativeInput.SetKeyDown(virtualKey, false);
            }

            return;
        }

        nativeInput.ControlWindow(action);
        await Task.CompletedTask;
    }

    private Task TapKeyAsync(ushort virtualKey, CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        nativeInput.SetKeyDown(virtualKey, true);
        nativeInput.SetKeyDown(virtualKey, false);
        return Task.CompletedTask;
    }
}

public sealed class SendInputWindowsNativeInput : IWindowsNativeInput
{
    private const uint InputMouse = 0;
    private const uint InputKeyboard = 1;
    private const uint MouseEventFLeftDown = 0x0002;
    private const uint MouseEventFLeftUp = 0x0004;
    private const uint MouseEventFRightDown = 0x0008;
    private const uint MouseEventFRightUp = 0x0010;
    private const uint MouseEventFMiddleDown = 0x0020;
    private const uint MouseEventFMiddleUp = 0x0040;
    private const uint MouseEventFWheel = 0x0800;
    private const uint MouseEventFHWheel = 0x01000;
    private const uint KeyEventFKeyUp = 0x0002;
    private const uint KeyEventFUnicode = 0x0004;
    private const int WheelDelta = 120;

    public PointerPosition GetCursorPosition()
    {
        return GetCursorPos(out NativePoint point)
            ? new PointerPosition(point.X, point.Y)
            : throw new DesktopInputException("adapter_failure", "Could not read cursor position.");
    }

    public PointerDisplay GetDisplayForPosition(PointerPosition position)
    {
        System.Drawing.Point point = new((int)Math.Round(position.X), (int)Math.Round(position.Y));
        System.Windows.Forms.Screen screen = System.Windows.Forms.Screen.FromPoint(point);
        return new PointerDisplay(
            new PointerDisplayBounds(screen.Bounds.X, screen.Bounds.Y, screen.Bounds.Width, screen.Bounds.Height),
            ScaleFactor: 1);
    }

    public void MoveCursorTo(PointerPosition position)
    {
        if (!SetCursorPos((int)Math.Round(position.X), (int)Math.Round(position.Y)))
        {
            throw new DesktopInputException("adapter_failure", "Could not move cursor.");
        }
    }

    public void SetMouseButtonDown(string button, bool down)
    {
        uint flags = button switch
        {
            "left" => down ? MouseEventFLeftDown : MouseEventFLeftUp,
            "right" => down ? MouseEventFRightDown : MouseEventFRightUp,
            "middle" => down ? MouseEventFMiddleDown : MouseEventFMiddleUp,
            _ => throw new DesktopInputException("unsupported_command", $"Unsupported mouse button: {button}")
        };
        SendMouse(flags, 0);
    }

    public void Scroll(PointerDelta delta)
    {
        if (delta.Dy != 0) SendMouse(MouseEventFWheel, (int)Math.Round(delta.Dy * WheelDelta));
        if (delta.Dx != 0) SendMouse(MouseEventFHWheel, (int)Math.Round(delta.Dx * WheelDelta));
    }

    public void SetKeyDown(ushort virtualKey, bool down)
    {
        SendKeyboard(virtualKey, scanCode: 0, down ? 0 : KeyEventFKeyUp);
    }

    public void TypeUnicodeText(string text)
    {
        foreach (char character in text)
        {
            SendKeyboard(virtualKey: 0, scanCode: character, KeyEventFUnicode);
            SendKeyboard(virtualKey: 0, scanCode: character, KeyEventFUnicode | KeyEventFKeyUp);
        }
    }

    public void ControlWindow(string action)
    {
        switch (action)
        {
            case "taskView":
                System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo
                {
                    FileName = "explorer.exe",
                    Arguments = "shell:::{3080F90E-D7AD-11D9-BD98-0000947B0257}",
                    UseShellExecute = true
                });
                return;
            case "showDesktop":
                SendShortcut([WindowsInputMapper.VkLeftWindows, (ushort)'D']);
                return;
            case "closeFocused":
                SendShortcut([WindowsInputMapper.VkAlt, WindowsInputMapper.VkF4]);
                return;
            case "minimizeFocused":
                SendShortcut([WindowsInputMapper.VkLeftWindows, WindowsInputMapper.VkDown]);
                return;
            case "maximizeFocused":
                SendShortcut([WindowsInputMapper.VkLeftWindows, WindowsInputMapper.VkUp]);
                return;
            default:
                throw new DesktopInputException("unsupported_command", $"Unsupported window action: {action}");
        }
    }

    private void SendShortcut(IReadOnlyList<ushort> virtualKeys)
    {
        foreach (ushort virtualKey in virtualKeys) SetKeyDown(virtualKey, true);
        foreach (ushort virtualKey in virtualKeys.Reverse()) SetKeyDown(virtualKey, false);
    }

    private static void SendMouse(uint flags, int mouseData)
    {
        NativeInput input = NativeInput.Mouse(flags, mouseData);
        SendInputOrThrow(input);
    }

    private static void SendKeyboard(ushort virtualKey, ushort scanCode, uint flags)
    {
        NativeInput input = NativeInput.Keyboard(virtualKey, scanCode, flags);
        SendInputOrThrow(input);
    }

    private static void SendInputOrThrow(NativeInput input)
    {
        NativeInput[] inputs = [input];
        uint sent = SendInput(1, inputs, Marshal.SizeOf<NativeInput>());
        if (sent != 1)
        {
            throw new DesktopInputException("adapter_failure", "Windows did not accept the input event.");
        }
    }

    [DllImport("user32.dll")]
    private static extern bool GetCursorPos(out NativePoint lpPoint);

    [DllImport("user32.dll")]
    private static extern bool SetCursorPos(int x, int y);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern uint SendInput(uint cInputs, [In] NativeInput[] pInputs, int cbSize);

    [StructLayout(LayoutKind.Sequential)]
    private readonly struct NativePoint
    {
        public readonly int X;
        public readonly int Y;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct NativeInput
    {
        public uint Type;
        public NativeInputUnion Union;

        public static NativeInput Mouse(uint flags, int mouseData) => new()
        {
            Type = InputMouse,
            Union = new NativeInputUnion
            {
                Mouse = new MouseInput
                {
                    MouseData = mouseData,
                    Flags = flags
                }
            }
        };

        public static NativeInput Keyboard(ushort virtualKey, ushort scanCode, uint flags) => new()
        {
            Type = InputKeyboard,
            Union = new NativeInputUnion
            {
                Keyboard = new KeyboardInput
                {
                    VirtualKey = virtualKey,
                    ScanCode = scanCode,
                    Flags = flags
                }
            }
        };
    }

    [StructLayout(LayoutKind.Explicit)]
    private struct NativeInputUnion
    {
        [FieldOffset(0)]
        public MouseInput Mouse;

        [FieldOffset(0)]
        public KeyboardInput Keyboard;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct MouseInput
    {
        public int Dx;
        public int Dy;
        public int MouseData;
        public uint Flags;
        public uint Time;
        public nint ExtraInfo;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct KeyboardInput
    {
        public ushort VirtualKey;
        public ushort ScanCode;
        public uint Flags;
        public uint Time;
        public nint ExtraInfo;
    }
}
