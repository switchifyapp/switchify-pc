using SwitchifyPc.Core.Settings;

namespace SwitchifyPc.Core.Input;

public sealed class PointerSpeedController
{
    private readonly IPointerMovementSettingsStore settingsStore;
    private readonly Action<PointerMovementSettings> applyLiveSettings;

    public PointerSpeedController(
        IPointerMovementSettingsStore settingsStore,
        Action<PointerMovementSettings> applyLiveSettings)
    {
        this.settingsStore = settingsStore;
        this.applyLiveSettings = applyLiveSettings;
    }

    public PointerMovementSettings Current()
    {
        return settingsStore.Load();
    }

    public PointerMovementSettings SetScalePercent(double scalePercent)
    {
        PointerMovementSettings saved = settingsStore.Save(new PointerMovementSettings(scalePercent));
        applyLiveSettings(saved);
        return saved;
    }
}
