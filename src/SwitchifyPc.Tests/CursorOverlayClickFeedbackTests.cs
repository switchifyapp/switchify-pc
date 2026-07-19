using SwitchifyPc.Windows.CursorOverlay;

namespace SwitchifyPc.Tests;

public sealed class CursorOverlayClickFeedbackTests
{
    [Fact]
    public void LandingFeedbackUsesStaticHalo()
    {
        Assert.Equal(0.7f, CursorOverlayStaticFeedback.LandingHaloProgress);
    }

    [Fact]
    public void StaticFeedbackUsesFixedVisibilityDurations()
    {
        Assert.Equal(300, CursorOverlayFeedbackTiming.LandingDurationMs);
        Assert.Equal(550, CursorOverlayFeedbackTiming.DoubleClickDurationMs);
    }

    [Theory]
    [InlineData(1, 33.6, 36.95, 3, 2)]
    [InlineData(1.5, 50.4, 55.425, 4.5, 3)]
    [InlineData(2, 67.2, 73.9, 6, 4)]
    public void LandingGeometryScalesWithMonitorDpi(
        double scale,
        float expectedCore,
        float expectedHaloRadius,
        float expectedStroke,
        float expectedShadow)
    {
        CursorOverlayVisualTokens tokens = CursorOverlayVisualTokens.Create(128, scale);

        Assert.Equal(expectedCore, tokens.LandingCoreDiameter);
        Assert.InRange(
            CursorOverlayStaticFeedback.ResolveLandingHaloRadius(tokens),
            expectedHaloRadius - 0.001f,
            expectedHaloRadius + 0.001f);
        Assert.Equal(expectedStroke, tokens.LandingHaloStroke);
        Assert.Equal(expectedShadow, tokens.ShadowOffset);
    }
}
