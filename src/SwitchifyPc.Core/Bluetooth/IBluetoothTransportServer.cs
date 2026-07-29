using SwitchifyPc.Protocol;

namespace SwitchifyPc.Core.Bluetooth;

public interface IBluetoothTransportServer : IDisposable
{
    Task StartAsync(string displayName, string desktopId);
    Task SendAsync(string connectionId, BluetoothFrame frame);
    void Disconnect(string connectionId);
    void DisconnectAll();
    void Stop();
}
