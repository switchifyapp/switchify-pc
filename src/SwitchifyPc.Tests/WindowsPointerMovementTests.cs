using SwitchifyPc.Core.Settings;
using SwitchifyPc.Windows.Input;

namespace SwitchifyPc.Tests;

public sealed class WindowsPointerMovementTests
{
    [Fact]
    public void InfersPointerMovementSizesFromBaselineDeltas()
    {
        Assert.Equal(PointerMovementSizeKey.Small, WindowsPointerMovement.InferPointerMovementSize(new PointerDelta(48, 0)));
        Assert.Equal(PointerMovementSizeKey.Medium, WindowsPointerMovement.InferPointerMovementSize(new PointerDelta(128, 0)));
        Assert.Equal(PointerMovementSizeKey.Large, WindowsPointerMovement.InferPointerMovementSize(new PointerDelta(280, 0)));
        Assert.Equal(PointerMovementSizeKey.Medium, WindowsPointerMovement.InferPointerMovementSize(new PointerDelta(0, 128)));
    }

    [Fact]
    public void AppliesDisplayScaleAndMovementScale()
    {
        PointerPosition current = new(100, 100);
        PointerDisplay display1080p = new(new PointerDisplayBounds(0, 0, 1920, 1080), 1);
        PointerDisplay display4k = new(new PointerDisplayBounds(0, 0, 3840, 2160), 2);

        Assert.Equal(new PointerPosition(148, 100), WindowsPointerMovement.CalculateDisplayNormalizedMouseTarget(current, new PointerDelta(48, 0), display1080p));
        Assert.Equal(new PointerPosition(196, 100), WindowsPointerMovement.CalculateDisplayNormalizedMouseTarget(current, new PointerDelta(48, 0), display1080p, new PointerMovementSettings(200)));
        Assert.Equal(new PointerPosition(164, 100), WindowsPointerMovement.CalculateDisplayNormalizedMouseTarget(current, new PointerDelta(128, 0), display1080p, new PointerMovementSettings(50)));
        Assert.Equal(new PointerPosition(868, 100), WindowsPointerMovement.CalculateDisplayNormalizedMouseTarget(current, new PointerDelta(128, 0), display4k, new PointerMovementSettings(150)));
    }

    [Fact]
    public void FallsBackToReferenceShortEdgeForInvalidDisplayData()
    {
        PointerPosition target = WindowsPointerMovement.CalculateDisplayNormalizedMouseTarget(
            new PointerPosition(100, 100),
            new PointerDelta(48, 0),
            new PointerDisplay(new PointerDisplayBounds(0, 0, 0, double.NaN), 1),
            new PointerMovementSettings(200));

        Assert.Equal(new PointerPosition(196, 100), target);
    }

    [Fact]
    public void CalculatesNativeScrollDelta()
    {
        Assert.Equal(new PointerDelta(0, -12), WindowsPointerMovement.CalculateNativeScrollDelta(new PointerDelta(0, -3)));
        Assert.Equal(new PointerDelta(1, 0), WindowsPointerMovement.CalculateNativeScrollDelta(new PointerDelta(0.1, 0)));
        Assert.Equal(new PointerDelta(0, 1), WindowsPointerMovement.CalculateNativeScrollDelta(new PointerDelta(0, 0.1)));
        Assert.Equal(new PointerDelta(0, -3), WindowsPointerMovement.CalculateNativeScrollDelta(new PointerDelta(0, -3), 0));
    }
}
