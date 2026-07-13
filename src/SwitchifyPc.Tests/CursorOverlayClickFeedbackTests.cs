using SwitchifyPc.Windows.CursorOverlay;

namespace SwitchifyPc.Tests;

public sealed class CursorOverlayClickFeedbackTests
{
    [Fact]
    public void LandingAnimationUsesCoreHaloAndFadePhases()
    {
        CursorOverlayLandingFrame start = CursorOverlayLandingFrame.Resolve(0);
        CursorOverlayLandingFrame coreComplete = CursorOverlayLandingFrame.Resolve(66);
        CursorOverlayLandingFrame haloComplete = CursorOverlayLandingFrame.Resolve(216);
        CursorOverlayLandingFrame halfwayThroughFade = CursorOverlayLandingFrame.Resolve(258);
        CursorOverlayLandingFrame end = CursorOverlayLandingFrame.Resolve(300);

        Assert.Equal(0, start.CoreScale);
        Assert.Equal(0, start.HaloProgress);
        Assert.Equal(1, start.Opacity);
        Assert.Equal(1, coreComplete.CoreScale);
        Assert.Equal(1, haloComplete.HaloProgress);
        Assert.Equal(1, haloComplete.Opacity);
        Assert.Equal(0.5f, halfwayThroughFade.Opacity, precision: 3);
        Assert.Equal(0, end.Opacity);
    }

    [Fact]
    public void StaticLandingIsVisibleWithoutMotion()
    {
        Assert.Equal(1, CursorOverlayLandingFrame.Static.CoreScale);
        Assert.InRange(CursorOverlayLandingFrame.Static.HaloProgress, 0.69f, 0.70f);
        Assert.Equal(1, CursorOverlayLandingFrame.Static.Opacity);
    }

    [Fact]
    public void DoubleClickStartsSecondPulseBeforeFirstPulseCompletes()
    {
        Assert.Equal(300, CursorOverlayFeedbackTiming.LandingDurationMs);
        Assert.Equal(250, CursorOverlayFeedbackTiming.DoubleClickIntervalMs);
        Assert.Equal(550, CursorOverlayFeedbackTiming.DoubleClickDurationMs);
        Assert.True(
            CursorOverlayFeedbackTiming.DoubleClickIntervalMs <
            CursorOverlayFeedbackTiming.LandingDurationMs);
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
