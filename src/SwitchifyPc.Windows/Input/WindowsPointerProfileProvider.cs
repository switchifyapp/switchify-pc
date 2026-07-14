using SwitchifyPc.Core.Control;
using SwitchifyPc.Core.Input;
using SwitchifyPc.Core.Settings;
using SwitchifyPc.Protocol;

namespace SwitchifyPc.Windows.Input;

public sealed class WindowsPointerProfileProvider : IPointerProfileProvider
{
    private readonly IWindowsNativeInput nativeInput;
    private readonly IPointerMovementSettingsStore settingsStore;
    private readonly IMouseRepeatSettingsStore mouseRepeatSettingsStore;

    public WindowsPointerProfileProvider(
        IWindowsNativeInput nativeInput,
        IPointerMovementSettingsStore settingsStore,
        IMouseRepeatSettingsStore? mouseRepeatSettingsStore = null)
    {
        this.nativeInput = nativeInput;
        this.settingsStore = settingsStore;
        this.mouseRepeatSettingsStore = mouseRepeatSettingsStore ?? new MemoryMouseRepeatSettingsStore(MouseRepeatSettingsModel.Default);
    }

    public PointerMovementProfile GetPointerProfile()
    {
        PointerPosition current = nativeInput.GetCursorPosition();
        PointerDisplay display = nativeInput.GetDisplayForPosition(current);
        PointerMovementSettings settings = settingsStore.Load();
        MouseRepeatSettings mouseRepeatSettings = mouseRepeatSettingsStore.Load();
        double shortEdge = Math.Min(display.Bounds.Width, display.Bounds.Height);

        return new PointerMovementProfile(
            DisplayId: $"{display.Bounds.X},{display.Bounds.Y},{display.Bounds.Width},{display.Bounds.Height}",
            ScaleFactor: display.ScaleFactor,
            Bounds: new Bounds(display.Bounds.X, display.Bounds.Y, display.Bounds.Width, display.Bounds.Height),
            MaxDelta: ProtocolConstants.MaxPointerDelta,
            RecommendedDeltas: new RecommendedDeltas(
                DeltaFor(shortEdge, settings, PointerMovementSizeKey.Small),
                DeltaFor(shortEdge, settings, PointerMovementSizeKey.Medium),
                DeltaFor(shortEdge, settings, PointerMovementSizeKey.Large)),
            Capabilities: new PointerCapabilities(
                NoAckMouseMove: true,
                NoAckCommands: ProtocolConstants.NoAckControlCommandTypes.ToArray(),
                SupportedCommands: ProtocolConstants.CommandTypes.ToArray(),
                MouseRepeat: new MouseRepeatCapabilities(
                    Supported: true,
                    Enabled: mouseRepeatSettings.Enabled,
                    IntervalMs: mouseRepeatSettings.MoveIntervalMs,
                    MoveIntervalMs: mouseRepeatSettings.MoveIntervalMs,
                    ScrollIntervalMs: mouseRepeatSettings.ScrollIntervalMs,
                    MinIntervalMs: MouseRepeatSettingsModel.MinIntervalMs,
                    MaxIntervalMs: MouseRepeatSettingsModel.MaxIntervalMs,
                    AccelerationDurationMs: mouseRepeatSettings.AccelerationDurationMs,
                    AccelerationDurationOptionsMs: MouseRepeatSettingsModel.AccelerationDurationOptionsMs,
                    AccelerationInitialScalePercent: MouseRepeatSettingsModel.AccelerationInitialScalePercent),
                PointerSpeed: PointerProfile.PointerSpeedFor(settings),
                DisplayNavigation: new DisplayNavigationCapabilities(
                    Supported: true,
                    DisplayCount: Math.Clamp(nativeInput.GetDisplays().Count, 1, 64))));
    }

    private static int DeltaFor(double shortEdge, PointerMovementSettings settings, PointerMovementSizeKey size)
    {
        return (int)Math.Round(shortEdge * PointerMovementSettingsModel.FractionFor(settings, size));
    }
}

internal sealed class MemoryMouseRepeatSettingsStore(MouseRepeatSettings settings) : IMouseRepeatSettingsStore
{
    public MouseRepeatSettings Load() => settings;

    public MouseRepeatSettings Save(MouseRepeatSettings next)
    {
        settings = MouseRepeatSettingsModel.Normalize(next);
        return settings;
    }
}
