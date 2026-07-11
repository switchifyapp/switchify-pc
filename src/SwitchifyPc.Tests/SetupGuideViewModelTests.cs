using SwitchifyPc.Core.Bluetooth;
using SwitchifyPc.Core.Pairing;
using SwitchifyPc.Core.Settings;
using SwitchifyPc.Core.Startup;
using SwitchifyPc.Core.Ui;

namespace SwitchifyPc.Tests;

public sealed class SetupGuideViewModelTests
{
    [Fact]
    public void AutoOpenRequiresUnpairedUserWhoHasNotSeenOrDismissedSetup()
    {
        Assert.True(SetupGuidePrompt.ShouldAutoOpen(MainWindowPromptSettingsModel.Default, hasPairedDevices: false));
        Assert.False(SetupGuidePrompt.ShouldAutoOpen(
            new MainWindowPromptSettings(false, SetupGuideShown: true),
            hasPairedDevices: false));
        Assert.False(SetupGuidePrompt.ShouldAutoOpen(
            new MainWindowPromptSettings(AndroidDownloadDismissed: true),
            hasPairedDevices: false));
        Assert.False(SetupGuidePrompt.ShouldAutoOpen(MainWindowPromptSettingsModel.Default, hasPairedDevices: true));
    }

    [Fact]
    public void MarksGuideShownAndCompletedWithoutChangingLegacyPreference()
    {
        MainWindowPromptSettings settings = new(AndroidDownloadDismissed: false);

        MainWindowPromptSettings shown = SetupGuidePrompt.MarkShown(settings);
        MainWindowPromptSettings completed = SetupGuidePrompt.MarkCompleted(settings);

        Assert.True(shown.SetupGuideShown);
        Assert.False(shown.SetupGuideCompleted);
        Assert.True(completed.SetupGuideShown);
        Assert.True(completed.SetupGuideCompleted);
        Assert.False(completed.AndroidDownloadDismissed);
    }

    [Fact]
    public void NavigatesFourStepsAndRequiresPairingBeforeStartup()
    {
        SetupGuideViewModel viewModel = new();

        Assert.True(viewModel.IsBluetoothStep);
        Assert.False(viewModel.CanGoBack);
        Assert.True(viewModel.MoveNext());
        Assert.True(viewModel.IsAndroidAppStep);
        Assert.True(viewModel.MoveNext());
        Assert.True(viewModel.IsPairDeviceStep);
        Assert.False(viewModel.CanGoNext);
        Assert.False(viewModel.MoveNext());

        viewModel.SetHasPairedDevices(true);

        Assert.True(viewModel.CanGoNext);
        Assert.True(viewModel.MoveNext());
        Assert.True(viewModel.IsStartupStep);
        Assert.True(viewModel.IsFinalStep);
        Assert.Equal("Finish", viewModel.NextButtonText);
        Assert.False(viewModel.MoveNext());
        Assert.True(viewModel.MoveBack());
        Assert.True(viewModel.IsPairDeviceStep);
    }

    [Fact]
    public void MapsLiveBluetoothReadiness()
    {
        SetupGuideViewModel viewModel = new();

        viewModel.SetBluetoothStatus(BluetoothStatusModel.DefaultStatus with
        {
            System = BluetoothStatusModel.DefaultSystemStatus with
            {
                AdapterPresent = true,
                RadioState = "on",
                IsLowEnergySupported = true,
                IsPeripheralRoleSupported = true
            }
        });

        Assert.True(viewModel.BluetoothReady);
        Assert.Equal("Bluetooth is ready", viewModel.BluetoothSetupTitle);

        viewModel.SetBluetoothStatus(BluetoothStatusModel.DefaultStatus with
        {
            System = BluetoothStatusModel.DefaultSystemStatus with
            {
                AdapterPresent = true,
                RadioState = "off"
            }
        });

        Assert.False(viewModel.BluetoothReady);
        Assert.Equal("Bluetooth is off", viewModel.BluetoothSetupTitle);
    }

    [Fact]
    public void ClonesAndExposesSecurePairingApprovals()
    {
        SetupGuideViewModel viewModel = new();
        List<PendingPairingApprovalView> approvals =
        [
            new("request-1", "Pixel 9", "123456", 1, 2, null)
        ];

        viewModel.SetPairingApprovals(approvals);
        approvals.Clear();

        PendingPairingApprovalView approval = Assert.Single(viewModel.PairingApprovals);
        Assert.True(viewModel.HasPairingApprovals);
        Assert.Equal("Pixel 9", approval.DeviceName);
        Assert.Equal("123456", approval.VerificationCode);
    }

    [Fact]
    public void MapsStartupSupportAndEnabledState()
    {
        SetupGuideViewModel viewModel = new();

        viewModel.SetStartupSettings(new SystemStartupSettings(
            Supported: true,
            StartWithSystem: true,
            StartsHidden: true,
            Reason: null));

        Assert.True(viewModel.StartWithSystemSupported);
        Assert.True(viewModel.StartWithSystem);
        Assert.Equal("Switchify PC will start hidden when you sign in.", viewModel.StartWithSystemMessage);
    }
}
