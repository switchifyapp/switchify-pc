using SwitchifyPc.Core.Control;
using SwitchifyPc.Core.Input;
using SwitchifyPc.Core.Settings;
using SwitchifyPc.Protocol;

namespace SwitchifyPc.Windows.Input;

public sealed class WindowsPointerProfileProvider : IPointerProfileProvider
{
    private readonly IWindowsNativeInput nativeInput;
    private readonly IPointerMovementSettingsStore settingsStore;

    public WindowsPointerProfileProvider(IWindowsNativeInput nativeInput, IPointerMovementSettingsStore settingsStore)
    {
        this.nativeInput = nativeInput;
        this.settingsStore = settingsStore;
    }

    public PointerMovementProfile GetPointerProfile()
    {
        PointerPosition current = nativeInput.GetCursorPosition();
        PointerDisplay display = nativeInput.GetDisplayForPosition(current);
        PointerMovementSettings settings = settingsStore.Load();
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
                SupportedCommands: ProtocolConstants.CommandTypes.ToArray()));
    }

    private static int DeltaFor(double shortEdge, PointerMovementSettings settings, PointerMovementSizeKey size)
    {
        return (int)Math.Round(shortEdge * PointerMovementSettingsModel.FractionFor(settings, size));
    }
}
