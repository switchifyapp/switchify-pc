using SwitchifyPc.Core.Bluetooth;

namespace SwitchifyPc.Tests;

public sealed class BluetoothGattProtocolTests
{
    [Fact]
    public void UsesStableProtocolUuids()
    {
        Assert.Equal(Guid.Parse("7a78f7e8-1d6d-4d92-9ef0-1f89d3db21f4"), BluetoothGattProtocol.ServiceUuid);
        Assert.Equal(Guid.Parse("7a78f7e9-1d6d-4d92-9ef0-1f89d3db21f4"), BluetoothGattProtocol.RxCharacteristicUuid);
        Assert.Equal(Guid.Parse("7a78f7ea-1d6d-4d92-9ef0-1f89d3db21f4"), BluetoothGattProtocol.TxCharacteristicUuid);
        Assert.Equal(Guid.Parse("7a78f7eb-1d6d-4d92-9ef0-1f89d3db21f4"), BluetoothGattProtocol.StatusCharacteristicUuid);
    }
}
