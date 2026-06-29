using SwitchifyPc.Core.Bluetooth;
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
                System = BluetoothStatusModel.DefaultSystemStatus with { AdapterPresent = true, RadioState = "off" },
                LastEvent = "system_radio_off",
                LastDisconnectReason = "adapter_off"
            });

        Assert.Equal("Bluetooth is off", viewModel.StatusTitle);
        Assert.Equal("Bluetooth radio off.", viewModel.SystemBluetooth);
        Assert.Equal("Bluetooth turned off.", viewModel.LastBluetoothEvent);
        Assert.Equal("Bluetooth was turned off.", viewModel.LastDisconnectReason);
        Assert.Contains(nameof(MainWindowViewModel.StatusTitle), changed);
        Assert.Contains(nameof(MainWindowViewModel.BluetoothStatus), changed);
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
}
