using SwitchifyPc.Core.Settings;
using SwitchifyPc.Core.Startup;
using SwitchifyPc.Core.Updates;
using SwitchifyPc.Core.Pairing;
using SwitchifyPc.Core.Diagnostics;

namespace SwitchifyPc.Core.Ui;

public sealed class SettingsController
{
    private readonly SettingsViewModel viewModel;
    private readonly ISystemStartupSettingsService startupSettings;
    private readonly IPointerMovementSettingsStore pointerMovementSettings;
    private readonly IMouseRepeatSettingsStore mouseRepeatSettings;
    private readonly ICursorOverlaySettingsStore cursorOverlaySettings;
    private readonly IUpdateSettingsService updates;
    private readonly IPairingStore pairingStore;
    private readonly ITelemetrySettingsStore? telemetrySettings;
    private readonly ITelemetryReporter? telemetryReporter;

    public SettingsController(
        SettingsViewModel viewModel,
        ISystemStartupSettingsService startupSettings,
        IPointerMovementSettingsStore pointerMovementSettings,
        IMouseRepeatSettingsStore mouseRepeatSettings,
        ICursorOverlaySettingsStore cursorOverlaySettings,
        IUpdateSettingsService updates,
        IPairingStore pairingStore,
        ITelemetrySettingsStore? telemetrySettings = null,
        ITelemetryReporter? telemetryReporter = null)
    {
        this.viewModel = viewModel;
        this.startupSettings = startupSettings;
        this.pointerMovementSettings = pointerMovementSettings;
        this.mouseRepeatSettings = mouseRepeatSettings;
        this.cursorOverlaySettings = cursorOverlaySettings;
        this.updates = updates;
        this.pairingStore = pairingStore;
        this.telemetrySettings = telemetrySettings;
        this.telemetryReporter = telemetryReporter;
    }

    public SettingsViewModel ViewModel => viewModel;

    public async Task LoadAsync()
    {
        viewModel.SetStartupSettings(await startupSettings.GetSettingsAsync().ConfigureAwait(false));
        viewModel.SetPointerMovementSettings(pointerMovementSettings.Load());
        viewModel.SetMouseRepeatSettings(mouseRepeatSettings.Load());
        viewModel.SetCursorOverlaySettings(cursorOverlaySettings.Load());
        viewModel.SetUpdateState(updates.GetState());
        if (telemetrySettings is not null) viewModel.SetTelemetrySettings(telemetrySettings.Load());
        await RefreshPairedDevicesAsync().ConfigureAwait(false);
    }

    public TelemetrySettings SetShareDiagnosticData(bool enabled)
    {
        telemetryReporter?.SetEnabled(enabled);
        TelemetrySettings saved = telemetrySettings?.Load() ?? new(false, false, Guid.Empty.ToString("D"));
        viewModel.SetTelemetrySettings(saved);
        return saved;
    }

    public MouseRepeatSettings SetMouseRepeatEnabled(bool enabled)
    {
        MouseRepeatSettings saved = mouseRepeatSettings.Save(CurrentMouseRepeatSettings() with { Enabled = enabled });
        viewModel.SetMouseRepeatSettings(saved);
        return saved;
    }

    public MouseRepeatSettings SetMouseRepeatMoveIntervalMs(int intervalMs)
    {
        MouseRepeatSettings saved = mouseRepeatSettings.Save(CurrentMouseRepeatSettings() with { MoveIntervalMs = intervalMs });
        viewModel.SetMouseRepeatSettings(saved);
        return saved;
    }

    public MouseRepeatSettings SetMouseRepeatScrollIntervalMs(int intervalMs)
    {
        MouseRepeatSettings saved = mouseRepeatSettings.Save(CurrentMouseRepeatSettings() with { ScrollIntervalMs = intervalMs });
        viewModel.SetMouseRepeatSettings(saved);
        return saved;
    }

    public MouseRepeatSettings SetMouseRepeatAccelerationDurationMs(int durationMs)
    {
        MouseRepeatSettings saved = mouseRepeatSettings.Save(CurrentMouseRepeatSettings() with { AccelerationDurationMs = durationMs });
        viewModel.SetMouseRepeatSettings(saved);
        return saved;
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

    public async Task<UpdateApplyResult> InstallAvailableUpdateAsync(CancellationToken cancellationToken = default)
    {
        UpdateApplyResult result = await updates.InstallAvailableUpdateAsync(cancellationToken).ConfigureAwait(false);
        viewModel.SetUpdateState(updates.GetState());
        return result;
    }

    public void ApplyUpdateState(UpdateState state)
    {
        viewModel.SetUpdateState(state);
    }

    public async Task<bool> ForgetPairedDeviceAsync(string deviceId, CancellationToken cancellationToken = default)
    {
        if (string.IsNullOrWhiteSpace(deviceId)) return false;

        PairingState state = await pairingStore.LoadAsync(cancellationToken).ConfigureAwait(false);
        if (PairingStateHelpers.FindPairedDevice(state, deviceId) is null)
        {
            await RefreshPairedDevicesAsync(cancellationToken).ConfigureAwait(false);
            return false;
        }

        PairingState next = PairingStateHelpers.RemovePairedDevice(state, deviceId);
        await pairingStore.SaveAsync(next, cancellationToken).ConfigureAwait(false);
        viewModel.SetPairedDevices(PairingStateHelpers.ToPairedDeviceViews(next));
        return true;
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

    private MouseRepeatSettings CurrentMouseRepeatSettings()
    {
        return viewModel.MouseRepeatSettings;
    }

    private async Task RefreshPairedDevicesAsync(CancellationToken cancellationToken = default)
    {
        viewModel.SetPairedDevices(PairingStateHelpers.ToPairedDeviceViews(
            await pairingStore.LoadAsync(cancellationToken).ConfigureAwait(false)));
    }
}
