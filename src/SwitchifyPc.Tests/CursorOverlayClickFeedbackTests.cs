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
    [InlineData(1, 48, 88, 3, 2)]
    [InlineData(1.5, 72, 132, 4.5, 3)]
    [InlineData(2, 96, 176, 6, 4)]
    public void LandingGeometryScalesWithMonitorDpi(
        double scale,
        float expectedCore,
        float expectedHalo,
        float expectedStroke,
        float expectedShadow)
    {
        CursorOverlayVisualTokens tokens = CursorOverlayVisualTokens.Create(128, scale);

        Assert.Equal(expectedCore, tokens.LandingCoreDiameter);
        Assert.Equal(expectedHalo, tokens.LandingHaloDiameter);
        Assert.Equal(expectedStroke, tokens.LandingHaloStroke);
        Assert.Equal(expectedShadow, tokens.ShadowOffset);
    }
}
