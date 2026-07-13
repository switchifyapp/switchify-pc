using SwitchifyPc.Core.Input;
using SwitchifyPc.Windows.CursorOverlay;

namespace SwitchifyPc.Tests;

public sealed class CursorOverlayScrollRepeatFeedbackTests
{
    [Fact]
    public void RenderFailureGuardDisablesRenderingAndReportsOnlyOnce()
    {
        CursorOverlayRenderFailureGuard guard = new();
        List<Exception> failures = [];
        int attempts = 0;

        bool first = guard.TryRun(() => throw new OutOfMemoryException("GDI failure"), failures.Add);
        bool second = guard.TryRun(() => attempts++, failures.Add);

        Assert.False(first);
        Assert.False(second);
        Assert.True(guard.IsDisabled);
        Assert.Equal(0, attempts);
        Assert.Single(failures);
        Assert.IsType<OutOfMemoryException>(failures[0]);
    }

    [Fact]
    public void RenderFailureGuardAllowsSuccessfulRendering()
    {
        CursorOverlayRenderFailureGuard guard = new();
        int attempts = 0;

        bool rendered = guard.TryRun(() => attempts++, _ => throw new InvalidOperationException());

        Assert.True(rendered);
        Assert.False(guard.IsDisabled);
        Assert.Equal(1, attempts);
    }

    [Fact]
    public void SupersededRepeatCannotBeEndedByStaleGeneration()
    {
        Guid stale = Guid.NewGuid();
        MouseRepeatFeedback active = new(
            Guid.NewGuid(),
            MouseRepeatFeedbackKind.Move,
            8,
            0,
            1000);

        Assert.False(CursorOverlayRepeatOwnership.CanEnd(active, stale));
        Assert.True(CursorOverlayRepeatOwnership.CanEnd(active, active.GenerationId));
    }

    [Theory]
    [InlineData(0, -5, 0, 1)]
    [InlineData(0, 5, 0, -1)]
    [InlineData(4, 0, -1, 0)]
    [InlineData(-4, 0, 1, 0)]
    [InlineData(3, 4, -0.6, -0.8)]
    [InlineData(0, 0, 0, -1)]
    public void ScrollDirectionOpposesSignedVector(
        double dx,
        double dy,
        float expectedX,
        float expectedY)
    {
        CursorOverlayDirection direction = CursorOverlayDirection.Resolve(dx, dy);

        Assert.Equal(expectedX, direction.X, precision: 5);
        Assert.Equal(expectedY, direction.Y, precision: 5);
    }

    [Theory]
    [InlineData(1, 100, 4, 56, 6, 12)]
    [InlineData(1.5, 150, 6, 84, 9, 18)]
    [InlineData(2, 200, 8, 112, 12, 24)]
    public void ScrollAndProgressGeometryScaleWithMonitorDpi(
        double scale,
        float progressDiameter,
        float progressStroke,
        float trackLength,
        float trackStroke,
        float headSize)
    {
        CursorOverlayVisualTokens tokens = CursorOverlayVisualTokens.Create(128, scale);

        Assert.Equal(progressDiameter, tokens.ProgressDiameter);
        Assert.Equal(progressStroke, tokens.ProgressStroke);
        Assert.Equal(trackLength, tokens.ScrollTrackLength);
        Assert.Equal(trackStroke, tokens.ScrollStroke);
        Assert.Equal(headSize, tokens.ScrollHeadSize);
    }
}
