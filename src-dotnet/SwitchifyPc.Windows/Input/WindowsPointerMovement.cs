using SwitchifyPc.Core.Settings;

namespace SwitchifyPc.Windows.Input;

public sealed record PointerDisplayBounds(double X, double Y, double Width, double Height);
public sealed record PointerDisplay(PointerDisplayBounds Bounds, double ScaleFactor);
public sealed record PointerDelta(double Dx, double Dy);
public sealed record PointerPosition(double X, double Y);

public static class WindowsPointerMovement
{
    public const double NativeScrollDeltaMultiplier = 4;
    public const double ReferencePointerShortEdge = 1080;

    private static readonly IReadOnlyDictionary<PointerMovementSizeKey, double> BaselinePointerDeltas =
        new Dictionary<PointerMovementSizeKey, double>
        {
            [PointerMovementSizeKey.Small] = 48,
            [PointerMovementSizeKey.Medium] = 128,
            [PointerMovementSizeKey.Large] = 280
        };

    private static readonly double SmallMediumPointerBoundary = (BaselinePointerDeltas[PointerMovementSizeKey.Small] + BaselinePointerDeltas[PointerMovementSizeKey.Medium]) / 2;
    private static readonly double MediumLargePointerBoundary = (BaselinePointerDeltas[PointerMovementSizeKey.Medium] + BaselinePointerDeltas[PointerMovementSizeKey.Large]) / 2;

    public static PointerPosition CalculateDisplayNormalizedMouseTarget(
        PointerPosition current,
        PointerDelta delta,
        PointerDisplay display,
        PointerMovementSettings? movementSettings = null)
    {
        double scaleFactor = double.IsFinite(display.ScaleFactor) && display.ScaleFactor > 0 ? display.ScaleFactor : 1;
        double shortEdge = double.IsFinite(display.Bounds.Width) && display.Bounds.Width > 0 &&
            double.IsFinite(display.Bounds.Height) && display.Bounds.Height > 0
                ? Math.Min(display.Bounds.Width, display.Bounds.Height)
                : ReferencePointerShortEdge;
        PointerMovementSettings settings = PointerMovementSettingsModel.Normalize(movementSettings ?? PointerMovementSettingsModel.Default);
        PointerMovementSizeKey size = InferPointerMovementSize(delta);
        double baselineFraction = PointerMovementSettingsModel.FractionFor(PointerMovementSettingsModel.Default, size);
        double movementScale = PointerMovementSettingsModel.FractionFor(settings, size) / baselineFraction;
        double multiplier = scaleFactor * (shortEdge / ReferencePointerShortEdge) * movementScale;

        return new PointerPosition(
            X: JsRound(current.X + delta.Dx * multiplier),
            Y: JsRound(current.Y + delta.Dy * multiplier));
    }

    public static PointerMovementSizeKey InferPointerMovementSize(PointerDelta delta)
    {
        double magnitude = Math.Max(Math.Abs(delta.Dx), Math.Abs(delta.Dy));
        if (magnitude <= SmallMediumPointerBoundary) return PointerMovementSizeKey.Small;
        if (magnitude <= MediumLargePointerBoundary) return PointerMovementSizeKey.Medium;
        return PointerMovementSizeKey.Large;
    }

    public static PointerDelta CalculateNativeScrollDelta(PointerDelta delta, double multiplier = NativeScrollDeltaMultiplier)
    {
        double effectiveMultiplier = double.IsFinite(multiplier) && multiplier > 0 ? multiplier : 1;
        return new PointerDelta(
            Dx: ScaleScrollAxis(delta.Dx, effectiveMultiplier),
            Dy: ScaleScrollAxis(delta.Dy, effectiveMultiplier));
    }

    private static double ScaleScrollAxis(double value, double multiplier)
    {
        if (value == 0) return 0;
        double scaledValue = value * multiplier;
        double roundedValue = Math.Round(Math.Abs(scaledValue), MidpointRounding.AwayFromZero);
        return Math.Sign(scaledValue) * Math.Max(1, roundedValue);
    }

    private static double JsRound(double value)
    {
        return Math.Floor(value + 0.5);
    }
}
