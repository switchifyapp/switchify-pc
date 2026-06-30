using SwitchifyPc.Core.Bluetooth;
using SwitchifyPc.Windows.Bluetooth;

namespace SwitchifyPc.Tests;

public sealed class WindowsBluetoothGattServerTests
{
    [Fact]
    public void DefaultOptionsUseProtocolBluetoothUuids()
    {
        WindowsBluetoothGattServerOptions options = WindowsBluetoothGattServerOptions.CreateDefault("Switchify PC", "desktop-1");

        Assert.Equal("Switchify PC", options.DisplayName);
        Assert.Equal("desktop-1", options.DesktopId);
        Assert.Equal(BluetoothHelperProtocol.ServiceUuid, options.ServiceUuid);
        Assert.Equal(BluetoothHelperProtocol.RxCharacteristicUuid, options.RxCharacteristicUuid);
        Assert.Equal(BluetoothHelperProtocol.TxCharacteristicUuid, options.TxCharacteristicUuid);
        Assert.Equal(BluetoothHelperProtocol.StatusCharacteristicUuid, options.StatusCharacteristicUuid);
    }

    [Fact]
    public void ServerCanBeConstructedAndDisposedWithoutBluetoothHardware()
    {
        List<BluetoothHelperEvent> events = [];

        using WindowsBluetoothGattServer server = new(events.Add);

        Assert.Empty(events);
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
}
