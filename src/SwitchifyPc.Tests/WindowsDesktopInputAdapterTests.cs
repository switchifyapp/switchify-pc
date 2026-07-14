using SwitchifyPc.Core.Input;
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

        Assert.Equal(new PointerDelta(34, 0), native.MovedBy);
        Assert.Null(native.MovedTo);
    }

    [Fact]
    public async Task CachesDisplayLookupForMouseMoves()
    {
        double now = 1000;
        FakeNativeInput native = new()
        {
            CursorPosition = new PointerPosition(100, 100),
            Display = new PointerDisplay(new PointerDisplayBounds(0, 0, 1920, 1080), 1)
        };
        WindowsDesktopInputAdapter adapter = new(native, new PointerMovementSettings(100), () => now);

        await adapter.MoveMouseByAsync(48, 0);
        await adapter.MoveMouseByAsync(48, 0);
        now += 1001;
        await adapter.MoveMouseByAsync(48, 0);

        Assert.Equal(2, native.CursorPositionReads);
        Assert.Equal(2, native.DisplayReads);
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

        await adapter.ScrollMouseAsync(0, -5);

        Assert.Equal(new PointerDelta(0, -2), native.Scrolled);
    }

    [Fact]
    public async Task ScrollsSmallNonZeroDeltaByAtLeastOneDetent()
    {
        FakeNativeInput native = new();
        WindowsDesktopInputAdapter adapter = new(native);

        await adapter.ScrollMouseAsync(0.1, 0);

        Assert.Equal(new PointerDelta(1, 0), native.Scrolled);
    }

    [Fact]
    public async Task MovesPointerToCenterOfSelectedDisplay()
    {
        PointerDisplay source = new(new PointerDisplayBounds(0, 0, 1920, 1080), 1);
        PointerDisplay target = new(new PointerDisplayBounds(1920, -360, 2560, 1440), 1);
        FakeNativeInput native = new()
        {
            CursorPosition = new PointerPosition(100, 100),
            Display = source,
            Displays = [source, target]
        };
        WindowsDesktopInputAdapter adapter = new(native);

        await adapter.MovePointerToDisplayAsync("right");

        Assert.Equal(new PointerPosition(3200, 360), native.MovedTo);
    }

    [Fact]
    public async Task RejectsUnavailableDisplayDirection()
    {
        FakeNativeInput native = new();
        WindowsDesktopInputAdapter adapter = new(native);

        DesktopInputException error = await Assert.ThrowsAsync<DesktopInputException>(() => adapter.MovePointerToDisplayAsync("left"));

        Assert.Equal("no_display_in_direction", error.Code);
        Assert.Equal("No monitor to the left.", error.Message);
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
    public async Task MapsUppercaseShortcutLettersToVirtualKeys()
    {
        FakeNativeInput native = new();
        WindowsDesktopInputAdapter adapter = new(native);

        await adapter.PressShortcutAsync(["A", "M", "Z"]);

        Assert.Equal(
            [
                "key:65:down",
                "key:77:down",
                "key:90:down",
                "key:90:up",
                "key:77:up",
                "key:65:up"
            ],
            native.Calls);
    }

    [Fact]
    public async Task RejectsLowercaseShortcutLetters()
    {
        WindowsDesktopInputAdapter adapter = new(new FakeNativeInput());

        await Assert.ThrowsAsync<ArgumentOutOfRangeException>(() => adapter.PressShortcutAsync(["a"]));
    }

    [Fact]
    public async Task SetsModifierKeysDownAndUp()
    {
        FakeNativeInput native = new();
        WindowsDesktopInputAdapter adapter = new(native);

        await adapter.SetKeyDownAsync("Ctrl", true);
        await adapter.SetKeyDownAsync("Alt", true);
        await adapter.SetKeyDownAsync("Shift", true);
        await adapter.SetKeyDownAsync("Meta", true);
        await adapter.SetKeyDownAsync("Meta", false);
        await adapter.SetKeyDownAsync("Shift", false);
        await adapter.SetKeyDownAsync("Alt", false);
        await adapter.SetKeyDownAsync("Ctrl", false);

        Assert.Equal(
            [
                "key:17:down",
                "key:18:down",
                "key:16:down",
                "key:91:down",
                "key:91:up",
                "key:16:up",
                "key:18:up",
                "key:17:up"
            ],
            native.Calls);
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

    [Theory]
    [InlineData("minimizeFocused")]
    [InlineData("maximizeFocused")]
    public async Task DelegatesExplicitWindowStateActionsWithoutSynthesizingKeys(string action)
    {
        FakeNativeInput native = new();
        WindowsDesktopInputAdapter adapter = new(native);

        await adapter.ControlWindowAsync(action);

        Assert.Equal([$"window:{action}"], native.Calls);
    }

    [Fact]
    public void RejectsUnsupportedNativeWindowAction()
    {
        SendInputWindowsNativeInput native = new();

        DesktopInputException error = Assert.Throws<DesktopInputException>(() => native.ControlWindow("unsupported"));

        Assert.Equal("unsupported_command", error.Code);
    }

    private sealed class FakeNativeInput : IWindowsNativeInput
    {
        public PointerPosition CursorPosition { get; init; } = new(0, 0);
        public PointerDisplay Display { get; init; } = new(new PointerDisplayBounds(0, 0, 1080, 1080), 1);
        public IReadOnlyList<PointerDisplay>? Displays { get; init; }
        public PointerPosition? MovedTo { get; private set; }
        public PointerDelta? MovedBy { get; private set; }
        public PointerDelta? Scrolled { get; private set; }
        public int CursorPositionReads { get; private set; }
        public int DisplayReads { get; private set; }
        public List<string> Calls { get; } = [];

        public PointerPosition GetCursorPosition()
        {
            CursorPositionReads += 1;
            return CursorPosition;
        }

        public PointerDisplay GetDisplayForPosition(PointerPosition position)
        {
            DisplayReads += 1;
            return Display;
        }

        public IReadOnlyList<PointerDisplay> GetDisplays() => Displays ?? [Display];

        public void MoveCursorTo(PointerPosition position)
        {
            MovedTo = position;
            Calls.Add($"move:{position.X}:{position.Y}");
        }

        public void MoveCursorBy(PointerDelta delta)
        {
            MovedBy = delta;
            Calls.Add($"moveBy:{delta.Dx}:{delta.Dy}");
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
