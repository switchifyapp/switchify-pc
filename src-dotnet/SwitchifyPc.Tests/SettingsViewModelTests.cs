using SwitchifyPc.Core.Settings;
using SwitchifyPc.Core.Startup;
using SwitchifyPc.Core.Ui;
using SwitchifyPc.Core.Updates;
using SwitchifyPc.Core.Pairing;

namespace SwitchifyPc.Tests;

public sealed class SettingsViewModelTests
{
    [Fact]
    public void ShowsUnsupportedStartupMessageByDefault()
    {
        SettingsViewModel viewModel = new();

        Assert.False(viewModel.StartWithSystem);
        Assert.False(viewModel.StartWithSystemSupported);
        Assert.Equal("Start with system is available in the installed app.", viewModel.StartWithSystemMessage);
    }

    [Fact]
    public void MapsStartupSettingsAndDiagnostics()
    {
        SettingsViewModel viewModel = new();

        viewModel.SetStartupSettings(new SystemStartupSettings(
            Supported: true,
            StartWithSystem: true,
            StartsHidden: true,
            Reason: null,
            Registration: new StartupRegistration(
                ExpectedCommand: "\"C:\\Program Files\\Switchify PC\\Switchify PC.exe\" --start-hidden",
                RegisteredCommand: "\"C:\\Program Files\\Switchify PC\\Switchify PC.exe\" --start-hidden",
                StartupApproved: "enabled")));

        Assert.True(viewModel.StartWithSystem);
        Assert.True(viewModel.StartWithSystemSupported);
        Assert.Equal("Switchify PC will start hidden when you sign in.", viewModel.StartWithSystemMessage);

        viewModel.SetStartupSettings(new SystemStartupSettings(
            Supported: true,
            StartWithSystem: false,
            StartsHidden: true,
            Reason: null,
            Registration: new StartupRegistration(
                ExpectedCommand: "new",
                RegisteredCommand: "old",
                StartupApproved: "enabled")));

        Assert.Equal("Start with system is registered to an older app path. Turn it off and on again to repair it.", viewModel.StartWithSystemMessage);
    }

    [Fact]
    public void MapsPointerMovementScaleAndDerivedPercentages()
    {
        SettingsViewModel viewModel = new();

        viewModel.SetPointerMovementSettings(new PointerMovementSettings(150));

        Assert.Equal(150, viewModel.PointerScalePercent);
        Assert.Equal("7%", viewModel.PointerSmall);
        Assert.Equal("18%", viewModel.PointerMedium);
        Assert.Equal("39%", viewModel.PointerLarge);
    }

    [Fact]
    public void MapsCursorOverlaySettings()
    {
        SettingsViewModel viewModel = new();

        viewModel.SetCursorOverlaySettings(new CursorOverlaySettings(
            Enabled: false,
            Size: "large",
            Visibility: "whileControlling",
            Crosshairs: true,
            Color: "blue"));

        Assert.False(viewModel.CursorOverlayEnabled);
        Assert.Equal("large", viewModel.CursorOverlaySize);
        Assert.Equal("While controlling", viewModel.CursorOverlayVisibility);
        Assert.True(viewModel.CursorOverlayCrosshairs);
        Assert.Equal("Blue", viewModel.CursorOverlayColor);
    }

    [Fact]
    public void MapsPairedDevicesWithoutTokens()
    {
        SettingsViewModel viewModel = new();

        Assert.False(viewModel.HasPairedDevices);
        Assert.Equal("No paired devices.", viewModel.PairedDevicesMessage);

        viewModel.SetPairedDevices(
        [
            new PairedDeviceView("device-1", "Pixel 9", 3_723_000, 3_724_000),
            new PairedDeviceView("device-2", "Galaxy Tab", 3_725_000, null)
        ]);

        Assert.True(viewModel.HasPairedDevices);
        Assert.Equal("2 paired devices.", viewModel.PairedDevicesMessage);
        Assert.Equal(2, viewModel.PairedDevices.Count);
        Assert.Equal("Pixel 9", viewModel.PairedDevices[0].DeviceName);
        Assert.Equal("01:02:03", viewModel.PairedDevices[0].PairedAt);
        Assert.Equal("01:02:04", viewModel.PairedDevices[0].LastSeenAt);
        Assert.Equal("Not yet.", viewModel.PairedDevices[1].LastSeenAt);
        Assert.DoesNotContain("token", string.Join("\n", viewModel.PairedDevices), StringComparison.OrdinalIgnoreCase);
    }

    [Fact]
    public void MapsUpdateMessagesAndActions()
    {
        SettingsViewModel viewModel = new();
        UpdateState available = UpdateState.CreateInitial("0.1.20") with
        {
            Info = UpdateState.CreateInitial("0.1.20").Info with
            {
                Status = UpdateCheckStatus.UpdateAvailable,
                LatestVersion = "0.2.0"
            }
        };
        UpdateState downloaded = available with
        {
            Download = new UpdateDownloadProgress(UpdateDownloadStatus.Downloaded, 10, 10, 100)
        };

        viewModel.SetUpdateState(available);
        Assert.Equal("Update available: v0.2.0.", viewModel.UpdateStatusMessage);
        Assert.True(viewModel.CanDownloadUpdate);
        Assert.False(viewModel.CanInstallUpdate);

        viewModel.SetUpdateState(downloaded);
        Assert.Equal("Update downloaded and ready to install.", viewModel.UpdateDownloadMessage);
        Assert.False(viewModel.CanDownloadUpdate);
        Assert.True(viewModel.CanInstallUpdate);
    }

    [Fact]
    public void MapsUpdateInstallFailureMessages()
    {
        Assert.Equal("The update is not downloaded yet.", SettingsViewModel.InstallMessage(UpdateInstallFailureReason.NotDownloaded));
        Assert.Equal("Updates are only available in the installed app.", SettingsViewModel.InstallMessage(UpdateInstallFailureReason.NotPackaged));
        Assert.Equal("Updates are only supported on Windows.", SettingsViewModel.InstallMessage(UpdateInstallFailureReason.NotSupported));
        Assert.Equal("The update was cancelled.", SettingsViewModel.InstallMessage(UpdateInstallFailureReason.Cancelled));
        Assert.Equal(
            "The downloaded installer could not be found. Download the update again.",
            SettingsViewModel.InstallMessage(UpdateInstallFailureReason.InstallerUnavailable));
        Assert.Equal(
            "The update installer could not be opened. Download the update again or run the installer manually.",
            SettingsViewModel.InstallMessage(UpdateInstallFailureReason.InstallerLaunchFailed));
        Assert.Equal("The update installer could not be opened.", SettingsViewModel.InstallMessage(null));
    }

    [Fact]
    public void RaisesChangedProperties()
    {
        SettingsViewModel viewModel = new();
        List<string?> changed = [];
        viewModel.PropertyChanged += (_, eventArgs) => changed.Add(eventArgs.PropertyName);

        viewModel.SetPointerMovementSettings(new PointerMovementSettings(50));

        Assert.Contains(nameof(SettingsViewModel.PointerScalePercent), changed);
        Assert.Contains(nameof(SettingsViewModel.PointerSmall), changed);
        Assert.Contains(nameof(SettingsViewModel.PointerMedium), changed);
        Assert.Contains(nameof(SettingsViewModel.PointerLarge), changed);
    }
}
