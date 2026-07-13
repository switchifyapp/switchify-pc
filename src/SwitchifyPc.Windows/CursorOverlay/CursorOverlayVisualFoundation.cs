using SwitchifyPc.Core.Input;

namespace SwitchifyPc.Windows.CursorOverlay;

internal sealed record CursorOverlayVisualTokens(
    int WindowSize,
    float RingDiameter,
    float RingStroke,
    float GlowStroke,
    int CrosshairThickness,
    float LandingCoreDiameter,
    float LandingHaloDiameter,
    float LandingHaloStroke,
    float ShadowOffset,
    float ProgressDiameter,
    float ProgressStroke,
    float ScrollTrackLength,
    float ScrollStroke,
    float ScrollHeadSize)
{
    private const double DefaultDpi = 96;

    public static CursorOverlayVisualTokens Create(int logicalWindowSize, double scaleFactor)
    {
        int normalizedLogicalSize = logicalWindowSize > 0 ? logicalWindowSize : 128;
        double scale = NormalizeScale(scaleFactor);
        int windowSize = Scale(normalizedLogicalSize, scale);
        return new CursorOverlayVisualTokens(
            WindowSize: windowSize,
            RingDiameter: (float)(normalizedLogicalSize * 0.5625 * scale),
            RingStroke: (float)Math.Max(4 * scale, normalizedLogicalSize * 0.039 * scale),
            GlowStroke: (float)Math.Max(18 * scale, normalizedLogicalSize * 0.1875 * scale),
            CrosshairThickness: Math.Max(1, Scale(2, scale)),
            LandingCoreDiameter: (float)(normalizedLogicalSize * 0.375 * scale),
            LandingHaloDiameter: (float)(normalizedLogicalSize * 0.6875 * scale),
            LandingHaloStroke: (float)Math.Max(2 * scale, normalizedLogicalSize * 0.0234375 * scale),
            ShadowOffset: (float)Math.Max(1, 2 * scale),
            ProgressDiameter: (float)(normalizedLogicalSize * 0.78125 * scale),
            ProgressStroke: (float)Math.Max(3 * scale, normalizedLogicalSize * 0.03125 * scale),
            ScrollTrackLength: (float)(normalizedLogicalSize * 0.4375 * scale),
            ScrollStroke: (float)Math.Max(3 * scale, normalizedLogicalSize * 0.046875 * scale),
            ScrollHeadSize: (float)(normalizedLogicalSize * 0.09375 * scale));
    }

    public static double ScaleFromDpi(uint dpi)
    {
        return dpi == 0 ? 1 : NormalizeScale(dpi / DefaultDpi);
    }

    private static double NormalizeScale(double scaleFactor)
    {
        return double.IsFinite(scaleFactor) && scaleFactor > 0 ? scaleFactor : 1;
    }

    private static int Scale(int value, double scaleFactor)
    {
        return Math.Max(1, (int)Math.Round(value * scaleFactor, MidpointRounding.AwayFromZero));
    }
}

internal static class CursorOverlayFeedbackTiming
{
    public const int LandingDurationMs = 300;
    public const int DoubleClickIntervalMs = 250;
    public const int DoubleClickDurationMs = LandingDurationMs + DoubleClickIntervalMs;
    public const int StaticDoubleClickGapStartsMs = 170;
}

internal sealed record CursorOverlayLandingFrame(float CoreScale, float HaloProgress, float Opacity)
{
    public static CursorOverlayLandingFrame Resolve(double elapsedMs)
    {
        float progress = (float)Math.Clamp(
            elapsedMs / CursorOverlayFeedbackTiming.LandingDurationMs,
            0,
            1);
        float coreScale = Math.Min(progress / 0.22f, 1);
        float haloProgress = Math.Min(progress / 0.72f, 1);
        float opacity = progress >= 1
            ? 0
            : progress < 0.72f
                ? 1
                : Math.Max(0, 1 - ((progress - 0.72f) / 0.28f));
        return new CursorOverlayLandingFrame(coreScale, haloProgress, opacity);
    }

    public static CursorOverlayLandingFrame Static { get; } = Resolve(150);
}

internal static class CursorOverlayRepeatProgress
{
    public static float Resolve(int durationMs, TimeSpan elapsed)
    {
        if (durationMs <= 0) return 1;
        double progress = Math.Clamp(elapsed.TotalMilliseconds / durationMs, 0, 1);
        return (float)(progress * progress * (3 - (2 * progress)));
    }
}

internal sealed record CursorOverlayDirection(float X, float Y)
{
    public static CursorOverlayDirection Resolve(double dx, double dy)
    {
        double magnitude = Math.Sqrt((dx * dx) + (dy * dy));
        return magnitude > 0
            ? new CursorOverlayDirection((float)(-dx / magnitude), (float)(-dy / magnitude))
            : new CursorOverlayDirection(0, -1);
    }
}

internal static class CursorOverlayRepeatOwnership
{
    public static bool CanEnd(MouseRepeatFeedback? active, Guid generationId)
    {
        return active?.GenerationId == generationId;
    }
}

internal interface ICursorOverlayMotionPolicy
{
    bool AnimationsEnabled();
}

internal sealed class WindowsCursorOverlayMotionPolicy : ICursorOverlayMotionPolicy
{
    private readonly Func<(bool Success, bool Enabled)> readSetting;

    public WindowsCursorOverlayMotionPolicy(Func<(bool Success, bool Enabled)>? readSetting = null)
    {
        this.readSetting = readSetting ?? ReadSetting;
    }

    public bool AnimationsEnabled()
    {
        try
        {
            (bool success, bool enabled) = readSetting();
            return !success || enabled;
        }
        catch (Exception error) when (error is DllNotFoundException or EntryPointNotFoundException)
        {
            return true;
        }
    }

    private static (bool Success, bool Enabled) ReadSetting()
    {
        bool success = CursorOverlayNativeMethods.SystemParametersInfo(
            CursorOverlayNativeMethods.SPI_GETCLIENTAREAANIMATION,
            0,
            out bool enabled,
            0);
        return (success, enabled);
    }
}

internal sealed class CursorOverlayGenerationTracker
{
    private long current;

    public long Next() => Interlocked.Increment(ref current);

    public bool IsCurrent(long generation) => Volatile.Read(ref current) == generation;

    public void Invalidate() => Interlocked.Increment(ref current);
}

internal static class CursorOverlayDpi
{
    public static double ScaleForPoint(double x, double y)
    {
        try
        {
            CursorOverlayNativeMethods.Point point = new(
                (int)Math.Round(x),
                (int)Math.Round(y));
            IntPtr monitor = CursorOverlayNativeMethods.MonitorFromPoint(
                point,
                CursorOverlayNativeMethods.MONITOR_DEFAULTTONEAREST);
            if (monitor == IntPtr.Zero ||
                CursorOverlayNativeMethods.GetDpiForMonitor(
                    monitor,
                    CursorOverlayNativeMethods.MDT_EFFECTIVE_DPI,
                    out uint dpiX,
                    out _) != 0)
            {
                return 1;
            }

            return CursorOverlayVisualTokens.ScaleFromDpi(dpiX);
        }
        catch (Exception error) when (error is DllNotFoundException or EntryPointNotFoundException)
        {
            return 1;
        }
    }
}
