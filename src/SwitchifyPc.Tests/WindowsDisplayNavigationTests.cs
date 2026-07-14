using SwitchifyPc.Windows.Input;

namespace SwitchifyPc.Tests;

public sealed class WindowsDisplayNavigationTests
{
    private static readonly PointerDisplay Source = Display(0, 0, 1920, 1080);

    [Theory]
    [InlineData("left", -1280, 0)]
    [InlineData("right", 1920, 0)]
    [InlineData("up", 0, -900)]
    [InlineData("down", 0, 1080)]
    public void FindsDisplayInRequestedDirection(string direction, double expectedX, double expectedY)
    {
        PointerDisplay destination = direction switch
        {
            "left" => Display(-1280, 0, 1280, 1024),
            "right" => Display(1920, 0, 2560, 1440),
            "up" => Display(0, -900, 1600, 900),
            _ => Display(0, 1080, 1366, 768)
        };

        PointerDisplay? target = WindowsDisplayNavigation.FindTarget(Source, [Source, destination], direction);

        Assert.NotNull(target);
        Assert.Equal(expectedX, target.Bounds.X);
        Assert.Equal(expectedY, target.Bounds.Y);
    }

    [Fact]
    public void SelectsNearestCenterInRequestedHalfPlane()
    {
        PointerDisplay nearDiagonal = Display(1920, 1080, 1280, 720);
        PointerDisplay farRight = Display(4000, 0, 1920, 1080);

        PointerDisplay? target = WindowsDisplayNavigation.FindTarget(Source, [Source, farRight, nearDiagonal], "right");

        Assert.Equal(nearDiagonal, target);
    }

    [Fact]
    public void UsesBoundsAsDeterministicTieBreaker()
    {
        PointerDisplay upper = Display(1920, -1080, 1920, 1080);
        PointerDisplay lower = Display(1920, 1080, 1920, 1080);

        PointerDisplay? target = WindowsDisplayNavigation.FindTarget(Source, [Source, lower, upper], "right");

        Assert.Equal(upper, target);
    }

    [Fact]
    public void ReturnsNullWhenNoDisplayExistsInDirection()
    {
        Assert.Null(WindowsDisplayNavigation.FindTarget(Source, [Source, Display(-1920, 0, 1920, 1080)], "right"));
        Assert.Null(WindowsDisplayNavigation.FindTarget(Source, [Source], "left"));
    }

    [Fact]
    public void CalculatesCenterForNegativeOriginAndMixedResolution()
    {
        PointerDisplay display = Display(-2560, -1440, 2560, 1440);

        Assert.Equal(new PointerPosition(-1280, -720), WindowsDisplayNavigation.Center(display));
    }

    private static PointerDisplay Display(double x, double y, double width, double height) =>
        new(new PointerDisplayBounds(x, y, width, height), 1);
}
