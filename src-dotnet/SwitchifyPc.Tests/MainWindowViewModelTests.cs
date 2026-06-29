using SwitchifyPc.Core.Bluetooth;
using SwitchifyPc.Core.Pairing;
using SwitchifyPc.Core.Ui;
using SwitchifyPc.Core.Updates;

namespace SwitchifyPc.Tests;

public sealed class MainWindowViewModelTests
{
    [Fact]
    public void DefaultsToStartingBluetoothCopy()
    {
        MainWindowViewModel viewModel = new();

        Assert.Equal("Switchify PC", viewModel.AppTitle);
        Assert.Equal("Getting Bluetooth ready...", viewModel.StatusTitle);
        Assert.Equal("Switchify PC is preparing nearby device connection.", viewModel.StatusBody);
        Assert.Equal("Bluetooth radio state unknown.", viewModel.SystemBluetooth);
        Assert.False(viewModel.HasUpdateBanner);
    }

    [Fact]
    public void UpdatesBluetoothPropertiesAndRaisesNotifications()
    {
        MainWindowViewModel viewModel = new();
        List<string?> changed = [];
        viewModel.PropertyChanged += (_, eventArgs) => changed.Add(eventArgs.PropertyName);

        viewModel.SetBluetoothState(
            DesktopUiState.Starting,
            BluetoothStatusModel.DefaultStatus with
            {
                Status = "unavailable",
                Reason = "adapter_off",
                System = BluetoothStatusModel.DefaultSystemStatus with
                {
                    AdapterPresent = true,
                    RadioState = "off",
                    LastCheckedAt = 3_723_000,
                    LastChangedAt = 3_723_000
                },
                LastEvent = "system_radio_off",
                LastDisconnectReason = "adapter_off"
            });

        Assert.Equal("Bluetooth is off", viewModel.StatusTitle);
        Assert.Equal("Bluetooth radio off.", viewModel.SystemBluetooth);
        Assert.Equal("01:02:03", viewModel.BluetoothLastChecked);
        Assert.Equal("01:02:03", viewModel.BluetoothLastChanged);
        Assert.Equal("Bluetooth turned off.", viewModel.LastBluetoothEvent);
        Assert.Equal("Bluetooth was turned off.", viewModel.LastDisconnectReason);
        Assert.Contains(nameof(MainWindowViewModel.StatusTitle), changed);
        Assert.Contains(nameof(MainWindowViewModel.BluetoothStatus), changed);
        Assert.Contains(nameof(MainWindowViewModel.BluetoothLastChecked), changed);
        Assert.Contains(nameof(MainWindowViewModel.BluetoothLastChanged), changed);
    }

    [Fact]
    public void UpdatesUpdateBannerProperties()
    {
        MainWindowViewModel viewModel = new();
        List<string?> changed = [];
        viewModel.PropertyChanged += (_, eventArgs) => changed.Add(eventArgs.PropertyName);
        UpdateState state = UpdateState.CreateInitial("0.1.20") with
        {
            Info = UpdateState.CreateInitial("0.1.20").Info with
            {
                Status = UpdateCheckStatus.UpdateAvailable,
                LatestVersion = "0.2.0"
            }
        };

        viewModel.SetUpdateState(state);

        Assert.True(viewModel.HasUpdateBanner);
        Assert.Equal("Update available", viewModel.UpdateBannerTitle);
        Assert.Equal("Open updates", viewModel.UpdateBannerButtonText);
        Assert.Contains(nameof(MainWindowViewModel.HasUpdateBanner), changed);
        Assert.Contains(nameof(MainWindowViewModel.UpdateBannerTitle), changed);
    }

    [Fact]
    public void UpdatesPairingApprovalProperties()
    {
        MainWindowViewModel viewModel = new();
        List<string?> changed = [];
        viewModel.PropertyChanged += (_, eventArgs) => changed.Add(eventArgs.PropertyName);

        viewModel.SetPairingApprovals([
            new PendingPairingApprovalView(
                "approval-1",
                "Pixel 9",
                "123456",
                1_724_000_000_000,
                1_724_000_120_000,
                "192.168.1.50")
        ]);

        Assert.True(viewModel.HasPairingApprovals);
        PendingPairingApprovalView approval = Assert.Single(viewModel.PairingApprovals);
        Assert.Equal("approval-1", approval.RequestId);
        Assert.Equal("Pixel 9", approval.DeviceName);
        Assert.Equal("123456", approval.VerificationCode);
        Assert.Contains(nameof(MainWindowViewModel.HasPairingApprovals), changed);
        Assert.Contains(nameof(MainWindowViewModel.PairingApprovals), changed);
    }

    [Fact]
    public void ClonesPairingApprovalList()
    {
        MainWindowViewModel viewModel = new();
        List<PendingPairingApprovalView> approvals =
        [
            new("approval-1", "Pixel 9", "123456", 1, 2, null)
        ];

        viewModel.SetPairingApprovals(approvals);
        approvals.Clear();

        Assert.True(viewModel.HasPairingApprovals);
        Assert.Single(viewModel.PairingApprovals);
    }
}
