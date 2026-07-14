using SwitchifyPc.Protocol;
using SwitchifyPc.Core.Settings;

namespace SwitchifyPc.Core.Input;

public sealed record Point(double X, double Y);
public sealed record Bounds(double X, double Y, double Width, double Height);
public sealed record DisplayInfo(Bounds Bounds, double ScaleFactor);
public sealed record PointerProfileInput(Point Cursor, DisplayInfo Display, int? MaxDelta = null);
public sealed record RecommendedDeltas(int Small, int Medium, int Large);
public sealed record MouseRepeatCapabilities(
    bool Supported,
    bool Enabled,
    int IntervalMs,
    int MoveIntervalMs,
    int ScrollIntervalMs,
    int MinIntervalMs,
    int MaxIntervalMs,
    int AccelerationDurationMs = MouseRepeatSettingsModel.DefaultAccelerationDurationMs,
    IReadOnlyList<int>? AccelerationDurationOptionsMs = null,
    int AccelerationInitialScalePercent = MouseRepeatSettingsModel.AccelerationInitialScalePercent);
public sealed record PointerSpeedCapabilities(
    bool Supported,
    bool SetSupported,
    double ScalePercent,
    double MinScalePercent,
    double MaxScalePercent,
    double StepPercent,
    int BaseMoveDelta,
    int EffectiveMoveDelta);
public sealed record DisplayNavigationCapabilities(bool Supported, int DisplayCount);
public sealed record PointerCapabilities(
    bool NoAckMouseMove,
    IReadOnlyList<string> NoAckCommands,
    IReadOnlyList<string> SupportedCommands,
    MouseRepeatCapabilities MouseRepeat,
    PointerSpeedCapabilities PointerSpeed,
    DisplayNavigationCapabilities DisplayNavigation);
public sealed record PointerMovementProfile(
    string DisplayId,
    double ScaleFactor,
    Bounds Bounds,
    int MaxDelta,
    RecommendedDeltas RecommendedDeltas,
    PointerCapabilities Capabilities);

public static class PointerProfile
{
    private const double ReferencePointerShortEdge = 1080;

    private static readonly IReadOnlyDictionary<PointerMovementSizeKey, double> TargetReferenceNativeDeltas =
        Enum.GetValues<PointerMovementSizeKey>().ToDictionary(
            size => size,
            size => ReferencePointerShortEdge * PointerMovementSettingsModel.FractionFor(PointerMovementSettingsModel.Default, size));

    public static PointerMovementProfile Create(PointerProfileInput input)
    {
        Bounds bounds = NormalizeBounds(input.Display.Bounds);
        double scaleFactor = double.IsFinite(input.Display.ScaleFactor) && input.Display.ScaleFactor > 0 ? input.Display.ScaleFactor : 1;
        int maxDelta = input.MaxDelta ?? ProtocolConstants.MaxPointerDelta;

        return new PointerMovementProfile(
            DisplayId: $"{FormatNumber(bounds.X)}:{FormatNumber(bounds.Y)}:{FormatNumber(bounds.Width)}:{FormatNumber(bounds.Height)}:{FormatNumber(scaleFactor)}",
            ScaleFactor: scaleFactor,
            Bounds: bounds,
            MaxDelta: maxDelta,
            RecommendedDeltas: new RecommendedDeltas(
                Small: ToLogicalDelta(TargetReferenceNativeDeltas[PointerMovementSizeKey.Small], scaleFactor, maxDelta),
                Medium: ToLogicalDelta(TargetReferenceNativeDeltas[PointerMovementSizeKey.Medium], scaleFactor, maxDelta),
                Large: ToLogicalDelta(TargetReferenceNativeDeltas[PointerMovementSizeKey.Large], scaleFactor, maxDelta)),
            Capabilities: new PointerCapabilities(
                NoAckMouseMove: true,
                NoAckCommands: ProtocolConstants.NoAckControlCommandTypes.ToArray(),
                SupportedCommands:
                [
                    .. ProtocolConstants.NoAckControlCommandTypes,
                    "mouse.repeat.start",
                    "mouse.repeat.stop",
                    "keyboard.textStream.open",
                    "keyboard.textStream.chunk",
                    "keyboard.textStream.close",
                    "connection.ping",
                    "pointer.profile",
                    "pointer.display.move"
                ],
                MouseRepeat: new MouseRepeatCapabilities(
                    Supported: true,
                    Enabled: MouseRepeatSettingsModel.Default.Enabled,
                    IntervalMs: MouseRepeatSettingsModel.Default.MoveIntervalMs,
                    MoveIntervalMs: MouseRepeatSettingsModel.Default.MoveIntervalMs,
                    ScrollIntervalMs: MouseRepeatSettingsModel.Default.ScrollIntervalMs,
                    MinIntervalMs: MouseRepeatSettingsModel.MinIntervalMs,
                    MaxIntervalMs: MouseRepeatSettingsModel.MaxIntervalMs,
                    AccelerationDurationMs: MouseRepeatSettingsModel.Default.AccelerationDurationMs,
                    AccelerationDurationOptionsMs: MouseRepeatSettingsModel.AccelerationDurationOptionsMs,
                    AccelerationInitialScalePercent: MouseRepeatSettingsModel.AccelerationInitialScalePercent),
                PointerSpeed: PointerSpeedFor(PointerMovementSettingsModel.Default),
                DisplayNavigation: new DisplayNavigationCapabilities(Supported: true, DisplayCount: 1)));
    }

    private static Bounds NormalizeBounds(Bounds bounds)
    {
        return new Bounds(
            X: FiniteOr(bounds.X, 0),
            Y: FiniteOr(bounds.Y, 0),
            Width: PositiveFiniteOr(bounds.Width, 1),
            Height: PositiveFiniteOr(bounds.Height, 1));
    }

    private static double FiniteOr(double value, double fallback)
    {
        return double.IsFinite(value) ? value : fallback;
    }

    private static double PositiveFiniteOr(double value, double fallback)
    {
        return double.IsFinite(value) && value > 0 ? value : fallback;
    }

    private static int ToLogicalDelta(double nativePixels, double scaleFactor, int maxDelta)
    {
        return Clamp((int)Math.Round(nativePixels / scaleFactor, MidpointRounding.AwayFromZero), 1, maxDelta);
    }

    public static PointerSpeedCapabilities PointerSpeedFor(PointerMovementSettings settings)
    {
        double scalePercent = PointerMovementSettingsModel.ScalePercentFor(settings);
        return new PointerSpeedCapabilities(
            Supported: true,
            SetSupported: true,
            ScalePercent: scalePercent,
            MinScalePercent: PointerMovementSettingsModel.PointerMovementScaleMin,
            MaxScalePercent: PointerMovementSettingsModel.PointerMovementScaleMax,
            StepPercent: PointerMovementSettingsModel.PointerMovementScaleStep,
            BaseMoveDelta: PointerMovementSettingsModel.BaseMoveDelta,
            EffectiveMoveDelta: Clamp(
                (int)Math.Round(PointerMovementSettingsModel.BaseMoveDelta * (scalePercent / 100), MidpointRounding.AwayFromZero),
                1,
                ProtocolConstants.MaxPointerDelta));
    }

    private static int Clamp(int value, int min, int max)
    {
        return Math.Min(max, Math.Max(min, value));
    }

    private static string FormatNumber(double value)
    {
        return value.ToString("G15", System.Globalization.CultureInfo.InvariantCulture);
    }
}
