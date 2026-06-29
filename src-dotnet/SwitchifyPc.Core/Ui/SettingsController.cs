using SwitchifyPc.Core.Settings;
using SwitchifyPc.Core.Startup;
using SwitchifyPc.Core.Updates;
using SwitchifyPc.Core.Pairing;

namespace SwitchifyPc.Core.Ui;

public sealed class SettingsController
{
    private readonly SettingsViewModel viewModel;
    private readonly ISystemStartupSettingsService startupSettings;
    private readonly IPointerMovementSettingsStore pointerMovementSettings;
    private readonly ICursorOverlaySettingsStore cursorOverlaySettings;
    private readonly IUpdateSettingsService updates;
    private readonly IPairingStore pairingStore;

    public SettingsController(
        SettingsViewModel viewModel,
        ISystemStartupSettingsService startupSettings,
        IPointerMovementSettingsStore pointerMovementSettings,
        ICursorOverlaySettingsStore cursorOverlaySettings,
        IUpdateSettingsService updates,
        IPairingStore pairingStore)
    {
        this.viewModel = viewModel;
        this.startupSettings = startupSettings;
        this.pointerMovementSettings = pointerMovementSettings;
        this.cursorOverlaySettings = cursorOverlaySettings;
        this.updates = updates;
        this.pairingStore = pairingStore;
    }

    public SettingsViewModel ViewModel => viewModel;

    public async Task LoadAsync()
    {
        viewModel.SetStartupSettings(await startupSettings.GetSettingsAsync().ConfigureAwait(false));
        viewModel.SetPointerMovementSettings(pointerMovementSettings.Load());
        viewModel.SetCursorOverlaySettings(cursorOverlaySettings.Load());
        viewModel.SetUpdateState(updates.GetState());
        viewModel.SetPairedDevices(PairingStateHelpers.ToPairedDeviceViews(
            await pairingStore.LoadAsync().ConfigureAwait(false)));
    }

    public async Task SetStartWithSystemAsync(bool enabled)
    {
        viewModel.SetStartupSettings(await startupSettings.SetStartWithSystemAsync(enabled).ConfigureAwait(false));
    }

    public PointerMovementSettings SetPointerScalePercent(double scalePercent)
    {
        PointerMovementSettings saved = pointerMovementSettings.Save(new PointerMovementSettings(scalePercent));
        viewModel.SetPointerMovementSettings(saved);
        return saved;
    }

    public CursorOverlaySettings SetCursorOverlayEnabled(bool enabled)
    {
        return SaveCursorOverlay(CurrentCursorOverlaySettings() with { Enabled = enabled });
    }

    public CursorOverlaySettings SetCursorOverlaySize(string size)
    {
        return SaveCursorOverlay(CurrentCursorOverlaySettings() with { Size = size });
    }

    public CursorOverlaySettings SetCursorOverlayVisibility(string visibility)
    {
        return SaveCursorOverlay(CurrentCursorOverlaySettings() with { Visibility = visibility });
    }

    public CursorOverlaySettings SetCursorOverlayCrosshairs(bool crosshairs)
    {
        return SaveCursorOverlay(CurrentCursorOverlaySettings() with { Crosshairs = crosshairs });
    }

    public CursorOverlaySettings SetCursorOverlayColor(string color)
    {
        return SaveCursorOverlay(CurrentCursorOverlaySettings() with { Color = color });
    }

    public async Task<UpdateState> CheckForUpdatesAsync(CancellationToken cancellationToken = default)
    {
        UpdateState state = await updates.CheckForUpdatesAsync(cancellationToken).ConfigureAwait(false);
        viewModel.SetUpdateState(state);
        return state;
    }

    public async Task<UpdateState> DownloadUpdateAsync(CancellationToken cancellationToken = default)
    {
        UpdateState state = await updates.DownloadUpdateAsync(cancellationToken).ConfigureAwait(false);
        viewModel.SetUpdateState(state);
        return state;
    }

    public async Task<UpdateInstallResult> InstallDownloadedUpdateAsync(CancellationToken cancellationToken = default)
    {
        UpdateInstallResult result = await updates.InstallDownloadedUpdateAsync(cancellationToken).ConfigureAwait(false);
        viewModel.SetUpdateState(updates.GetState());
        return result;
    }

    public void ApplyUpdateState(UpdateState state)
    {
        viewModel.SetUpdateState(state);
    }

    private CursorOverlaySettings SaveCursorOverlay(CursorOverlaySettings settings)
    {
        CursorOverlaySettings saved = cursorOverlaySettings.Save(settings);
        viewModel.SetCursorOverlaySettings(saved);
        return saved;
    }

    private CursorOverlaySettings CurrentCursorOverlaySettings()
    {
        return viewModel.CursorOverlaySettings;
    }
}
