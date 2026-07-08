using SwitchifyPc.Core.Input;
using SwitchifyPc.Core.Settings;
using SwitchifyPc.Windows.CursorOverlay;
using SwitchifyPc.Windows.Input;
using System.Reflection;

namespace SwitchifyPc.Tests;

public sealed class WindowsCursorOverlayNotifierTests
{
    [Fact]
    public void CanConstructAndDisposeWithoutCreatingOverlayWindow()
    {
        using WindowsCursorOverlayNotifier notifier = new(new FakeNativeInput(), new FakeCursorOverlaySettings());

        notifier.Hide();
    }

    [Fact]
    public void CanStartFollowingBeforeOverlayWindowIsShown()
    {
        using WindowsCursorOverlayNotifier notifier = new(
            new FakeNativeInput(),
            new FakeCursorOverlaySettings(CursorOverlaySettingsModel.Default with
            {
                Enabled = true,
                Visibility = "whileControlling"
            }));

        notifier.MarkControlActive();
        Thread.Sleep(150);
    }

    [Fact]
    public void HideDoesNotStopWhileControllingOverlayUntilSessionEnds()
    {
        using WindowsCursorOverlayNotifier notifier = new(
            new FakeNativeInput(),
            new FakeCursorOverlaySettings(CursorOverlaySettingsModel.Default with
            {
                Enabled = true,
                Visibility = "whileControlling"
            }));

        notifier.MarkControlActive();
        Thread.Sleep(50);

        Assert.True(IsFollowing(notifier));

        notifier.Hide();

        Assert.True(IsFollowing(notifier));

        notifier.EndControlSession();

        Assert.False(IsFollowing(notifier));
    }

    private static bool IsFollowing(WindowsCursorOverlayNotifier notifier)
    {
        FieldInfo? field = typeof(WindowsCursorOverlayNotifier).GetField("followTimer", BindingFlags.Instance | BindingFlags.NonPublic);
        Assert.NotNull(field);
        return field.GetValue(notifier) is not null;
    }

    private sealed class FakeCursorOverlaySettings(CursorOverlaySettings? settings = null) : ICursorOverlaySettingsStore
    {
        public CursorOverlaySettings Load() => settings ?? CursorOverlaySettingsModel.Default with { Enabled = false };

        public CursorOverlaySettings Save(CursorOverlaySettings settings) => CursorOverlaySettingsModel.Normalize(settings);
    }

    private sealed class FakeNativeInput : IWindowsNativeInput
    {
        public PointerPosition GetCursorPosition() => new(100, 200);

        public PointerDisplay GetDisplayForPosition(PointerPosition position) =>
            new(new PointerDisplayBounds(0, 0, 1920, 1080), 1);

        public void MoveCursorTo(PointerPosition position) { }

        public void MoveCursorBy(PointerDelta delta) { }

        public void SetMouseButtonDown(string button, bool down) { }

        public void Scroll(PointerDelta delta) { }

        public void SetKeyDown(ushort virtualKey, bool down) { }

        public void TypeUnicodeText(string text) { }

        public void ControlWindow(string action) { }
    }
}
