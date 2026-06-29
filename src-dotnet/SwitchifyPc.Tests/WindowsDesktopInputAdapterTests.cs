using SwitchifyPc.Core.Settings;
using SwitchifyPc.Windows.Input;

namespace SwitchifyPc.Tests;

public sealed class WindowsDesktopInputAdapterTests
{
    [Fact]
    public async Task MovesMouseUsingDisplayNormalizedPointerMath()
    {
        FakeNativeInput native = new()
        {
            CursorPosition = new PointerPosition(100, 100),
            Display = new PointerDisplay(new PointerDisplayBounds(0, 0, 1920, 1080), 1)
        };
        WindowsDesktopInputAdapter adapter = new(native, new PointerMovementSettings(200));

        await adapter.MoveMouseByAsync(48, 0);

        Assert.Equal(new PointerPosition(196, 100), native.MovedTo);
    }

    [Fact]
    public async Task ClicksAndDoubleClicksMouseButtonsInOrder()
    {
        FakeNativeInput native = new();
        WindowsDesktopInputAdapter adapter = new(native);

        await adapter.ClickMouseAsync("left");
        await adapter.DoubleClickMouseAsync("right");

        Assert.Equal(
            [
                "mouse:left:down",
                "mouse:left:up",
                "mouse:right:down",
                "mouse:right:up",
                "mouse:right:down",
                "mouse:right:up"
            ],
            native.Calls);
    }

    [Fact]
    public async Task ScrollsWithNativeDeltaScaling()
    {
        FakeNativeInput native = new();
        WindowsDesktopInputAdapter adapter = new(native);

        await adapter.ScrollMouseAsync(0.1, -3);

        Assert.Equal(new PointerDelta(1, -24), native.Scrolled);
    }

    [Fact]
    public async Task PressesMetaAsWindowsKey()
    {
        FakeNativeInput native = new();
        WindowsDesktopInputAdapter adapter = new(native);

        await adapter.PressKeyAsync("Meta");

        Assert.Equal(["key:91:down", "key:91:up"], native.Calls);
    }

    [Fact]
    public async Task PressesShortcutsDownInOrderAndUpInReverse()
    {
        FakeNativeInput native = new();
        WindowsDesktopInputAdapter adapter = new(native);

        await adapter.PressShortcutAsync(["Ctrl", "Shift", "C"]);

        Assert.Equal(
            [
                "key:17:down",
                "key:16:down",
                "key:67:down",
                "key:67:up",
                "key:16:up",
                "key:17:up"
            ],
            native.Calls);
    }

    [Fact]
    public async Task TypesUnicodeTextThroughNativeInput()
    {
        FakeNativeInput native = new();
        WindowsDesktopInputAdapter adapter = new(native);

        await adapter.TypeTextAsync("Hi");
        await adapter.TypeCharacterAsync("é");

        Assert.Equal(["text:Hi", "text:é"], native.Calls);
    }

    [Fact]
    public async Task SendsMediaControlAsVirtualKey()
    {
        FakeNativeInput native = new();
        WindowsDesktopInputAdapter adapter = new(native);

        await adapter.MediaControlAsync("playPause");

        Assert.Equal(["key:179:down", "key:179:up"], native.Calls);
    }

    [Fact]
    public async Task UsesShortcutForWindowSwitching()
    {
        FakeNativeInput native = new();
        WindowsDesktopInputAdapter adapter = new(native);

        await adapter.ControlWindowAsync("switchPrevious");

        Assert.Equal(
            [
                "key:18:down",
                "key:16:down",
                "key:9:down",
                "key:9:up",
                "key:16:up",
                "key:18:up"
            ],
            native.Calls);
    }

    [Fact]
    public async Task DelegatesNonShortcutWindowActions()
    {
        FakeNativeInput native = new();
        WindowsDesktopInputAdapter adapter = new(native);

        await adapter.ControlWindowAsync("showDesktop");

        Assert.Equal(["window:showDesktop"], native.Calls);
    }

    private sealed class FakeNativeInput : IWindowsNativeInput
    {
        public PointerPosition CursorPosition { get; init; } = new(0, 0);
        public PointerDisplay Display { get; init; } = new(new PointerDisplayBounds(0, 0, 1080, 1080), 1);
        public PointerPosition? MovedTo { get; private set; }
        public PointerDelta? Scrolled { get; private set; }
        public List<string> Calls { get; } = [];

        public PointerPosition GetCursorPosition() => CursorPosition;

        public PointerDisplay GetDisplayForPosition(PointerPosition position) => Display;

        public void MoveCursorTo(PointerPosition position)
        {
            MovedTo = position;
            Calls.Add($"move:{position.X}:{position.Y}");
        }

        public void SetMouseButtonDown(string button, bool down)
        {
            Calls.Add($"mouse:{button}:{(down ? "down" : "up")}");
        }

        public void Scroll(PointerDelta delta)
        {
            Scrolled = delta;
            Calls.Add($"scroll:{delta.Dx}:{delta.Dy}");
        }

        public void SetKeyDown(ushort virtualKey, bool down)
        {
            Calls.Add($"key:{virtualKey}:{(down ? "down" : "up")}");
        }

        public void TypeUnicodeText(string text)
        {
            Calls.Add($"text:{text}");
        }

        public void ControlWindow(string action)
        {
            Calls.Add($"window:{action}");
        }
    }
}
