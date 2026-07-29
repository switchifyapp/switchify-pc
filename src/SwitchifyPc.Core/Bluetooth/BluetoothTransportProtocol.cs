using SwitchifyPc.Protocol;

namespace SwitchifyPc.Core.Bluetooth;

public abstract record BluetoothTransportEvent(string Type);
public sealed record BluetoothReadyEvent() : BluetoothTransportEvent("ready");
public sealed record BluetoothUnavailableEvent(string Reason) : BluetoothTransportEvent("unavailable");
public sealed record BluetoothConnectedEvent(string ConnectionId, string Label) : BluetoothTransportEvent("connected");
public sealed record BluetoothMessageEvent(string ConnectionId, BluetoothFrame Frame) : BluetoothTransportEvent("message");
public sealed record BluetoothDisconnectedEvent(string ConnectionId, string Reason) : BluetoothTransportEvent("disconnected");
public sealed record BluetoothDiagnosticEvent(string Event) : BluetoothTransportEvent("diagnostic");
public sealed record BluetoothSystemStatusEvent(
    bool AdapterPresent,
    string RadioState,
    bool? IsLowEnergySupported,
    bool? IsPeripheralRoleSupported) : BluetoothTransportEvent("systemStatus");
public sealed record BluetoothErrorEvent(string Reason) : BluetoothTransportEvent("error");

public static class BluetoothGattProtocol
{
    public static readonly Guid ServiceUuid = Guid.Parse("7a78f7e8-1d6d-4d92-9ef0-1f89d3db21f4");
    public static readonly Guid RxCharacteristicUuid = Guid.Parse("7a78f7e9-1d6d-4d92-9ef0-1f89d3db21f4");
    public static readonly Guid TxCharacteristicUuid = Guid.Parse("7a78f7ea-1d6d-4d92-9ef0-1f89d3db21f4");
    public static readonly Guid StatusCharacteristicUuid = Guid.Parse("7a78f7eb-1d6d-4d92-9ef0-1f89d3db21f4");
}
