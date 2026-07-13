using SwitchifyPc.Windows.CursorOverlay;

namespace SwitchifyPc.Tests;

public sealed class CursorOverlayVisualFoundationTests
{
    [Theory]
    [InlineData(1, 128, 72, 2)]
    [InlineData(1.5, 192, 108, 3)]
    [InlineData(2, 256, 144, 4)]
    public void ScalesLogicalGeometryForMonitorDpi(
        double scale,
        int expectedWindowSize,
        float expectedRingDiameter,
        int expectedCrosshairThickness)
    {
        CursorOverlayVisualTokens tokens = CursorOverlayVisualTokens.Create(128, scale);

        Assert.Equal(expectedWindowSize, tokens.WindowSize);
        Assert.Equal(expectedRingDiameter, tokens.RingDiameter);
        Assert.Equal(expectedCrosshairThickness, tokens.CrosshairThickness);
    }

    [Theory]
    [InlineData(0, 1)]
    [InlineData(96, 1)]
    [InlineData(144, 1.5)]
    [InlineData(192, 2)]
    public void ResolvesScaleFromEffectiveDpi(uint dpi, double expected)
    {
        Assert.Equal(expected, CursorOverlayVisualTokens.ScaleFromDpi(dpi));
    }

    [Theory]
    [InlineData(0)]
    [InlineData(-1)]
    [InlineData(double.NaN)]
    [InlineData(double.PositiveInfinity)]
    public void InvalidScaleFallsBackToOne(double scale)
    {
        Assert.Equal(128, CursorOverlayVisualTokens.Create(128, scale).WindowSize);
    }

    [Fact]
    public void NewGenerationInvalidatesEarlierCallbacks()
    {
        CursorOverlayGenerationTracker tracker = new();
        long first = tracker.Next();
        long second = tracker.Next();

        Assert.False(tracker.IsCurrent(first));
        Assert.True(tracker.IsCurrent(second));

        tracker.Invalidate();

        Assert.False(tracker.IsCurrent(second));
    }
}
