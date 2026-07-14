namespace SwitchifyPc.Windows.Input;

public static class WindowsDisplayNavigation
{
    public static PointerDisplay? FindTarget(
        PointerDisplay source,
        IReadOnlyList<PointerDisplay> displays,
        string direction)
    {
        PointerPosition sourceCenter = Center(source);

        return displays
            .Where(display => !HasSameBounds(display, source))
            .Select(display => new Candidate(display, Center(display)))
            .Where(candidate => IsInDirection(sourceCenter, candidate.Center, direction))
            .OrderBy(candidate => DistanceSquared(sourceCenter, candidate.Center))
            .ThenBy(candidate => candidate.Display.Bounds.X)
            .ThenBy(candidate => candidate.Display.Bounds.Y)
            .ThenBy(candidate => candidate.Display.Bounds.Width)
            .ThenBy(candidate => candidate.Display.Bounds.Height)
            .Select(candidate => candidate.Display)
            .FirstOrDefault();
    }

    public static PointerPosition Center(PointerDisplay display)
    {
        return new PointerPosition(
            display.Bounds.X + (display.Bounds.Width / 2),
            display.Bounds.Y + (display.Bounds.Height / 2));
    }

    private static bool IsInDirection(PointerPosition source, PointerPosition candidate, string direction)
    {
        return direction switch
        {
            "left" => candidate.X < source.X,
            "right" => candidate.X > source.X,
            "up" => candidate.Y < source.Y,
            "down" => candidate.Y > source.Y,
            _ => false
        };
    }

    private static bool HasSameBounds(PointerDisplay first, PointerDisplay second)
    {
        return first.Bounds == second.Bounds;
    }

    private static double DistanceSquared(PointerPosition source, PointerPosition candidate)
    {
        double dx = candidate.X - source.X;
        double dy = candidate.Y - source.Y;
        return (dx * dx) + (dy * dy);
    }

    private sealed record Candidate(PointerDisplay Display, PointerPosition Center);
}
