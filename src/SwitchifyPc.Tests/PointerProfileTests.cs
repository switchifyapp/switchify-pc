using SwitchifyPc.Core.Input;
using SwitchifyPc.Protocol;
using InputPoint = SwitchifyPc.Core.Input.Point;

namespace SwitchifyPc.Tests;

public sealed class PointerProfileTests
{
    [Fact]
    public void CreatesStableBaselineDeltasForScaledDisplay()
    {
        PointerMovementProfile profile = PointerProfile.Create(new PointerProfileInput(
            new InputPoint(100, 100),
            new DisplayInfo(new Bounds(0, 0, 1280, 720), 1.5)));

        Assert.Equal("0:0:1280:720:1.5", profile.DisplayId);
        Assert.Equal(new RecommendedDeltas(32, 86, 187), profile.RecommendedDeltas);
        Assert.True(profile.Capabilities.NoAckMouseMove);
        Assert.Contains("keyboard.textStream.char", profile.Capabilities.NoAckCommands);
        Assert.Contains("keyboard.textStream.chunk", profile.Capabilities.NoAckCommands);
        Assert.Contains("keyboard.modifierDown", profile.Capabilities.NoAckCommands);
        Assert.Contains("keyboard.modifierUp", profile.Capabilities.NoAckCommands);
        Assert.Contains("keyboard.textStream.open", profile.Capabilities.SupportedCommands);
        Assert.Contains("keyboard.modifierDown", profile.Capabilities.SupportedCommands);
        Assert.Contains("keyboard.modifierUp", profile.Capabilities.SupportedCommands);
        Assert.Equal(250, profile.Capabilities.MouseRepeat.IntervalMs);
        Assert.Equal(250, profile.Capabilities.MouseRepeat.MoveIntervalMs);
        Assert.Equal(250, profile.Capabilities.MouseRepeat.ScrollIntervalMs);
        Assert.Equal(1000, profile.Capabilities.MouseRepeat.AccelerationDurationMs);
        Assert.Equal([0, 500, 1000, 2000], profile.Capabilities.MouseRepeat.AccelerationDurationOptionsMs);
        Assert.Equal(25, profile.Capabilities.MouseRepeat.AccelerationInitialScalePercent);
        Assert.True(profile.Capabilities.PointerSpeed.Supported);
        Assert.True(profile.Capabilities.PointerSpeed.SetSupported);
        Assert.Equal(100, profile.Capabilities.PointerSpeed.ScalePercent);
        Assert.Equal(5, profile.Capabilities.PointerSpeed.MinScalePercent);
        Assert.Equal(225, profile.Capabilities.PointerSpeed.MaxScalePercent);
        Assert.Equal(5, profile.Capabilities.PointerSpeed.StepPercent);
        Assert.Equal(128, profile.Capabilities.PointerSpeed.BaseMoveDelta);
        Assert.Equal(128, profile.Capabilities.PointerSpeed.EffectiveMoveDelta);
        Assert.True(profile.Capabilities.DisplayNavigation.Supported);
        Assert.Equal(1, profile.Capabilities.DisplayNavigation.DisplayCount);
        Assert.Contains("pointer.display.move", profile.Capabilities.SupportedCommands);
    }

    [Fact]
    public void PreservesCurrentFeelOn1080pAtOneX()
    {
        RecommendedDeltas deltas = PointerProfile.Create(new PointerProfileInput(
            new InputPoint(100, 100),
            new DisplayInfo(new Bounds(0, 0, 1920, 1080), 1))).RecommendedDeltas;

        Assert.Equal(new RecommendedDeltas(49, 130, 281), deltas);
    }

    [Fact]
    public void DividesStableBaselineDeltasByScaleFactor()
    {
        RecommendedDeltas deltas = PointerProfile.Create(new PointerProfileInput(
            new InputPoint(100, 100),
            new DisplayInfo(new Bounds(0, 0, 3840, 2160), 2))).RecommendedDeltas;

        Assert.Equal(new RecommendedDeltas(24, 65, 140), deltas);
    }

    [Fact]
    public void KeepsRecommendedDeltasBelowMax()
    {
        RecommendedDeltas deltas = PointerProfile.Create(new PointerProfileInput(
            new InputPoint(100, 100),
            new DisplayInfo(new Bounds(0, 0, 3840, 2160), 0.25),
            ProtocolConstants.MaxPointerDelta)).RecommendedDeltas;

        Assert.True(deltas.Small <= ProtocolConstants.MaxPointerDelta);
        Assert.True(deltas.Medium <= ProtocolConstants.MaxPointerDelta);
        Assert.True(deltas.Large <= ProtocolConstants.MaxPointerDelta);
    }

    [Fact]
    public void FallsBackForInvalidScaleAndKeepsNegativeDisplayCoordinates()
    {
        PointerMovementProfile invalidScale = PointerProfile.Create(new PointerProfileInput(
            new InputPoint(100, 100),
            new DisplayInfo(new Bounds(0, 0, 1280, 720), 0)));
        PointerMovementProfile negativeDisplay = PointerProfile.Create(new PointerProfileInput(
            new InputPoint(-100, 100),
            new DisplayInfo(new Bounds(-1920, 0, 1920, 1080), 1.25)));

        Assert.Equal(1, invalidScale.ScaleFactor);
        Assert.Equal("0:0:1280:720:1", invalidScale.DisplayId);
        Assert.Equal("-1920:0:1920:1080:1.25", negativeDisplay.DisplayId);
    }
}
