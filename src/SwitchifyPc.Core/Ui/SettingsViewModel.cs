using System.ComponentModel;
using System.Runtime.CompilerServices;
using SwitchifyPc.Core.Pairing;
using SwitchifyPc.Core.Settings;
using SwitchifyPc.Core.Startup;
using SwitchifyPc.Core.Updates;

namespace SwitchifyPc.Core.Ui;

public sealed record PairedDeviceSettingsView(
    string DeviceId,
    string DeviceName,
    string PairedAt,
    string LastSeenAt);

public sealed class SettingsViewModel : INotifyPropertyChanged
{
    private SystemStartupSettings startupSettings = new(
        Supported: false,
        StartWithSystem: false,
        StartsHidden: true,
        Reason: "unpackaged");
    private PointerMovementSettings pointerMovementSettings = PointerMovementSettingsModel.Default;
    private CursorOverlaySettings cursorOverlaySettings = CursorOverlaySettingsModel.Default;
    private UpdateState updateState = UpdateState.CreateInitial("0.2.0");
    private IReadOnlyList<PairedDeviceSettingsView> pairedDevices = [];

    public event PropertyChangedEventHandler? PropertyChanged;

    public bool StartWithSystem => startupSettings.StartWithSystem;

    public bool StartWithSystemSupported => startupSettings.Supported;

    public string StartWithSystemMessage
    {
        get
        {
            if (!startupSettings.Supported)
            {
                return startupSettings.Reason == "unpackaged"
                    ? "Start with system is available in the installed app."
                    : "Start with system is not available on this platform.";
            }

            if (startupSettings.Registration?.StartupApproved == "disabled")
            {
                return "Start with system is disabled in Windows Startup settings.";
            }

            if (!startupSettings.StartWithSystem &&
                !string.IsNullOrWhiteSpace(startupSettings.Registration?.RegisteredCommand) &&
                startupSettings.Registration.RegisteredCommand != startupSettings.Registration.ExpectedCommand)
            {
                return "Start with system is registered to an older app path. Turn it off and on again to repair it.";
            }

            return startupSettings.StartWithSystem
                ? "Switchify PC will start hidden when you sign in."
                : "Switchify PC will not start when you sign in.";
        }
    }

    public IReadOnlyList<PairedDeviceSettingsView> PairedDevices => pairedDevices;

    public bool HasPairedDevices => pairedDevices.Count > 0;

    public string PairedDevicesMessage => pairedDevices.Count == 0
        ? "No paired devices."
        : pairedDevices.Count == 1
            ? "1 paired device."
            : $"{pairedDevices.Count} paired devices.";

    public double PointerScalePercent => PointerMovementSettingsModel.ScalePercentFor(pointerMovementSettings);

    public bool IsPointerScale25 => Math.Abs(PointerScalePercent - 25) < 0.1;

    public bool IsPointerScale50 => Math.Abs(PointerScalePercent - 50) < 0.1;

    public bool IsPointerScale75 => Math.Abs(PointerScalePercent - 75) < 0.1;

    public bool IsPointerScale100 => Math.Abs(PointerScalePercent - 100) < 0.1;

    public PointerMovementSettings PointerMovementSettings => pointerMovementSettings;

    public string PointerSmall => $"{PointerMovementSettingsModel.PercentageFor(pointerMovementSettings, PointerMovementSizeKey.Small):0.#}%";

    public string PointerMedium => $"{PointerMovementSettingsModel.PercentageFor(pointerMovementSettings, PointerMovementSizeKey.Medium):0.#}%";

    public string PointerLarge => $"{PointerMovementSettingsModel.PercentageFor(pointerMovementSettings, PointerMovementSizeKey.Large):0.#}%";

    public bool CursorOverlayEnabled => cursorOverlaySettings.Enabled;

    public CursorOverlaySettings CursorOverlaySettings => cursorOverlaySettings;

    public string CursorOverlaySize => cursorOverlaySettings.Size;

    public bool IsCursorOverlaySizeSmall => cursorOverlaySettings.Size == "small";

    public bool IsCursorOverlaySizeMedium => cursorOverlaySettings.Size == "medium";

    public bool IsCursorOverlaySizeLarge => cursorOverlaySettings.Size == "large";

    public string CursorOverlayVisibility => cursorOverlaySettings.Visibility == "whileControlling" ? "While controlling" : "On input";

    public bool IsCursorOverlayVisibilityOnInput => cursorOverlaySettings.Visibility == "onInput";

    public bool IsCursorOverlayVisibilityWhileControlling => cursorOverlaySettings.Visibility == "whileControlling";

    public bool CursorOverlayCrosshairs => cursorOverlaySettings.Crosshairs;

    public string CursorOverlayColor => CursorOverlaySettingsModel.Colors[cursorOverlaySettings.Color].Label;

    public bool IsCursorOverlayColorRed => cursorOverlaySettings.Color == "red";

    public bool IsCursorOverlayColorGreen => cursorOverlaySettings.Color == "green";

    public bool IsCursorOverlayColorBlue => cursorOverlaySettings.Color == "blue";

    public bool IsCursorOverlayColorYellow => cursorOverlaySettings.Color == "yellow";

    public bool IsCursorOverlayColorWhite => cursorOverlaySettings.Color == "white";

    public string UpdateStatusMessage => UpdateCheckMessage(updateState.Info);

    public string? UpdateDownloadMessage => DownloadMessage(updateState.Download);

    public bool CanDownloadUpdate =>
        updateState.Info.Status == UpdateCheckStatus.UpdateAvailable &&
        updateState.Download.Status is UpdateDownloadStatus.Idle or UpdateDownloadStatus.DownloadFailed;

    public bool CanInstallUpdate => updateState.Download.Status == UpdateDownloadStatus.Downloaded;

    public void SetStartupSettings(SystemStartupSettings settings)
    {
        startupSettings = settings;
        OnPropertyChanged(nameof(StartWithSystem));
        OnPropertyChanged(nameof(StartWithSystemSupported));
        OnPropertyChanged(nameof(StartWithSystemMessage));
    }

    public void SetPairedDevices(IReadOnlyList<PairedDeviceView> devices)
    {
        pairedDevices = devices
            .Select(device => new PairedDeviceSettingsView(
                device.DeviceId,
                device.DeviceName,
                MainWindowCopy.Timestamp(device.PairedAt),
                MainWindowCopy.Timestamp(device.LastSeenAt)))
            .ToArray();
        OnPropertyChanged(nameof(PairedDevices));
        OnPropertyChanged(nameof(HasPairedDevices));
        OnPropertyChanged(nameof(PairedDevicesMessage));
    }

    public void SetPointerMovementSettings(PointerMovementSettings settings)
    {
        pointerMovementSettings = PointerMovementSettingsModel.Normalize(settings);
        OnPropertyChanged(nameof(PointerMovementSettings));
        OnPropertyChanged(nameof(PointerScalePercent));
        OnPropertyChanged(nameof(IsPointerScale25));
        OnPropertyChanged(nameof(IsPointerScale50));
        OnPropertyChanged(nameof(IsPointerScale75));
        OnPropertyChanged(nameof(IsPointerScale100));
        OnPropertyChanged(nameof(PointerSmall));
        OnPropertyChanged(nameof(PointerMedium));
        OnPropertyChanged(nameof(PointerLarge));
    }

    public void SetCursorOverlaySettings(CursorOverlaySettings settings)
    {
        cursorOverlaySettings = CursorOverlaySettingsModel.Normalize(settings);
        OnPropertyChanged(nameof(CursorOverlaySettings));
        OnPropertyChanged(nameof(CursorOverlayEnabled));
        OnPropertyChanged(nameof(CursorOverlaySize));
        OnPropertyChanged(nameof(IsCursorOverlaySizeSmall));
        OnPropertyChanged(nameof(IsCursorOverlaySizeMedium));
        OnPropertyChanged(nameof(IsCursorOverlaySizeLarge));
        OnPropertyChanged(nameof(CursorOverlayVisibility));
        OnPropertyChanged(nameof(IsCursorOverlayVisibilityOnInput));
        OnPropertyChanged(nameof(IsCursorOverlayVisibilityWhileControlling));
        OnPropertyChanged(nameof(CursorOverlayCrosshairs));
        OnPropertyChanged(nameof(CursorOverlayColor));
        OnPropertyChanged(nameof(IsCursorOverlayColorRed));
        OnPropertyChanged(nameof(IsCursorOverlayColorGreen));
        OnPropertyChanged(nameof(IsCursorOverlayColorBlue));
        OnPropertyChanged(nameof(IsCursorOverlayColorYellow));
        OnPropertyChanged(nameof(IsCursorOverlayColorWhite));
    }

    public void SetUpdateState(UpdateState state)
    {
        updateState = state;
        OnPropertyChanged(nameof(UpdateStatusMessage));
        OnPropertyChanged(nameof(UpdateDownloadMessage));
        OnPropertyChanged(nameof(CanDownloadUpdate));
        OnPropertyChanged(nameof(CanInstallUpdate));
    }

    public static string UpdateCheckMessage(UpdateInfo info)
    {
        if (info.Reason == UpdateFailureReason.NotPackaged) return "Updates are only available in packaged builds.";
        if (info.Reason == UpdateFailureReason.NotSupported) return "Updates are not available on this platform.";

        return info.Status switch
        {
            UpdateCheckStatus.NotChecked => "Not checked yet.",
            UpdateCheckStatus.Checking => "Checking...",
            UpdateCheckStatus.UpToDate => "Switchify PC is up to date.",
            UpdateCheckStatus.UpdateAvailable => $"Update available: v{info.LatestVersion ?? "unknown"}.",
            UpdateCheckStatus.CheckFailed => "Could not check for updates.",
            _ => "Not checked yet."
        };
    }

    public static string? DownloadMessage(UpdateDownloadProgress download)
    {
        if (download.Status == UpdateDownloadStatus.Idle) return null;
        if (download.Status == UpdateDownloadStatus.Downloading)
        {
            return download.Percent is null ? "Downloading..." : $"Downloading {download.Percent}%.";
        }

        if (download.Status == UpdateDownloadStatus.Downloaded) return "Update downloaded and ready to install.";
        if (download.Reason == UpdateFailureReason.NotPackaged) return "Updates are only available in packaged builds.";
        if (download.Reason == UpdateFailureReason.NotSupported) return "Updates are not available on this platform.";
        return "Could not download the update.";
    }

    public static string InstallMessage(UpdateInstallFailureReason? reason)
    {
        return reason switch
        {
            UpdateInstallFailureReason.NotDownloaded => "The update is not downloaded yet.",
            UpdateInstallFailureReason.NotPackaged => "Updates are only available in the installed app.",
            UpdateInstallFailureReason.NotSupported => "Updates are only supported on Windows.",
            UpdateInstallFailureReason.Cancelled => "The update was cancelled.",
            UpdateInstallFailureReason.InstallerUnavailable => "The downloaded installer could not be found. Download the update again.",
            UpdateInstallFailureReason.InstallerLaunchFailed => "The update installer could not be opened. Download the update again or run the installer manually.",
            _ => "The update installer could not be opened."
        };
    }

    private void OnPropertyChanged([CallerMemberName] string? propertyName = null)
    {
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
    }
}
