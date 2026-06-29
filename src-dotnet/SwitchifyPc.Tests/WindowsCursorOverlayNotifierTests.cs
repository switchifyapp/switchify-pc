using SwitchifyPc.Core.Input;
using SwitchifyPc.Core.Settings;
using SwitchifyPc.Windows.CursorOverlay;
using SwitchifyPc.Windows.Input;

namespace SwitchifyPc.Tests;

public sealed class WindowsCursorOverlayNotifierTests
{
    [Fact]
    public void CanConstructAndDisposeWithoutCreatingOverlayWindow()
    {
        using WindowsCursorOverlayNotifier notifier = new(new FakeNativeInput(), new FakeCursorOverlaySettings());

        notifier.Hide();
    }

    private sealed class FakeCursorOverlaySettings : ICursorOverlaySettingsStore
    {
        public CursorOverlaySettings Load() => CursorOverlaySettingsModel.Default with { Enabled = false };

        public CursorOverlaySettings Save(CursorOverlaySettings settings) => CursorOverlaySettingsModel.Normalize(settings);
    }

    private sealed class FakeNativeInput : IWindowsNativeInput
    {
        public PointerPosition GetCursorPosition() => new(100, 200);

        public PointerDisplay GetDisplayForPosition(PointerPosition position) =>
            new(new PointerDisplayBounds(0, 0, 1920, 1080), 1);

        public void MoveCursorTo(PointerPosition position) { }

        public void SetMouseButtonDown(string button, bool down) { }

        public void Scroll(PointerDelta delta) { }

        public void SetKeyDown(ushort virtualKey, bool down) { }

        public void TypeUnicodeText(string text) { }

        public void ControlWindow(string action) { }
    }
}
