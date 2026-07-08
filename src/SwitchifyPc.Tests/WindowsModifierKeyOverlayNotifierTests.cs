using SwitchifyPc.Windows.Input;
using SwitchifyPc.Windows.ModifierOverlay;

namespace SwitchifyPc.Tests;

public sealed class WindowsModifierKeyOverlayNotifierTests
{
    [Fact]
    public void CanConstructAndDisposeWithoutCreatingOverlayWindow()
    {
        using WindowsModifierKeyOverlayNotifier notifier = new(new FakeNativeInput());
    }

    [Fact]
    public void EndControlSessionCanBeCalledBeforeShow()
    {
        using WindowsModifierKeyOverlayNotifier notifier = new(new FakeNativeInput());

        notifier.EndControlSession();
    }

    [Fact]
    public void CanSetActiveModifiersAndClearWithoutThrowing()
    {
        using WindowsModifierKeyOverlayNotifier notifier = new(new FakeNativeInput());

        notifier.SetActiveModifiers(["Ctrl", "Start"]);
        notifier.SetActiveModifiers([]);
    }

    private sealed class FakeNativeInput : IWindowsNativeInput
    {
        public PointerPosition GetCursorPosition() => new(10, 10);

        public PointerDisplay GetDisplayForPosition(PointerPosition position) => new(new PointerDisplayBounds(0, 0, 1920, 1080), 1);

        public void MoveCursorTo(PointerPosition position) { }

        public void MoveCursorBy(PointerDelta delta) { }

        public void SetMouseButtonDown(string button, bool down) { }

        public void Scroll(PointerDelta delta) { }

        public void SetKeyDown(ushort virtualKey, bool down) { }

        public void TypeUnicodeText(string text) { }

        public void ControlWindow(string action) { }
    }
}
