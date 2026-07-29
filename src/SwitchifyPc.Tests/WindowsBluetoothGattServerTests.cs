using SwitchifyPc.Core.Bluetooth;
using SwitchifyPc.Windows.Bluetooth;
using Windows.Devices.Radios;
using Windows.Devices.Bluetooth.GenericAttributeProfile;

namespace SwitchifyPc.Tests;

public sealed class WindowsBluetoothGattServerTests
{
    [Fact]
    public void DefaultOptionsUseProtocolBluetoothUuids()
    {
        WindowsBluetoothGattServerOptions options = WindowsBluetoothGattServerOptions.CreateDefault("Switchify PC", "desktop-1");

        Assert.Equal("Switchify PC", options.DisplayName);
        Assert.Equal("desktop-1", options.DesktopId);
        Assert.Equal(BluetoothGattProtocol.ServiceUuid, options.ServiceUuid);
        Assert.Equal(BluetoothGattProtocol.RxCharacteristicUuid, options.RxCharacteristicUuid);
        Assert.Equal(BluetoothGattProtocol.TxCharacteristicUuid, options.TxCharacteristicUuid);
        Assert.Equal(BluetoothGattProtocol.StatusCharacteristicUuid, options.StatusCharacteristicUuid);
    }

    [Fact]
    public void ServerCanBeConstructedAndDisposedWithoutBluetoothHardware()
    {
        List<BluetoothTransportEvent> events = [];

        using WindowsBluetoothGattServer server = new(events.Add);

        Assert.Empty(events);
    }

    [Theory]
    [InlineData(GattWriteOption.WriteWithResponse, true)]
    [InlineData(GattWriteOption.WriteWithoutResponse, false)]
    public void RespondsOnlyToWritesThatRequestAResponse(GattWriteOption option, bool expected)
    {
        Assert.Equal(expected, WindowsBluetoothGattServer.ShouldRespondToWrite(option));
    }

    [Theory]
    [InlineData("notification_unsubscribed")]
    [InlineData("pc_requested")]
    [InlineData("client_requested")]
    public void ClientDisconnectReasonsRestartAdvertising(string reason)
    {
        Assert.True(WindowsBluetoothGattServer.ShouldRestartAdvertisingAfterDisconnect(reason));
    }

    [Theory]
    [InlineData("adapter_off")]
    [InlineData("helper_stopped")]
    [InlineData("helper_error")]
    public void ShutdownAndRadioDisconnectReasonsDoNotRestartAdvertising(string reason)
    {
        Assert.False(WindowsBluetoothGattServer.ShouldRestartAdvertisingAfterDisconnect(reason));
    }

    [Theory]
    [InlineData(RadioState.On, "on")]
    [InlineData(RadioState.Off, "off")]
    [InlineData(RadioState.Disabled, "disabled")]
    [InlineData(RadioState.Unknown, "unknown")]
    public void SystemMonitorMapsWindowsRadioStates(RadioState state, string expected)
    {
        Assert.Equal(expected, WindowsBluetoothSystemMonitor.RadioStateToProtocol(state));
    }
}
