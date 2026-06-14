using System.Runtime.InteropServices.WindowsRuntime;
using System.Text;
using System.Text.Json;
using Windows.Devices.Bluetooth;
using Windows.Devices.Bluetooth.GenericAttributeProfile;
using Windows.Storage.Streams;

namespace SwitchifyBluetoothTransport;

internal sealed class BluetoothGattBridge
{
    private const string ConnectionId = "ble";

    private GattServiceProvider? serviceProvider;
    private GattLocalCharacteristic? txCharacteristic;
    private GattLocalCharacteristic? rxCharacteristic;
    private GattLocalCharacteristic? statusCharacteristic;
    private string statusPayload = "{}";
    private bool connected;

    public async Task StartAsync(StartCommand command)
    {
        Stop();

        var adapter = await BluetoothAdapter.GetDefaultAsync();
        if (adapter is null)
        {
            HelperProtocol.WriteEvent(new { type = "unavailable", reason = "unsupported" });
            return;
        }

        if (!adapter.IsLowEnergySupported)
        {
            HelperProtocol.WriteEvent(new { type = "unavailable", reason = "unsupported" });
            return;
        }

        if (!adapter.IsPeripheralRoleSupported)
        {
            HelperProtocol.WriteEvent(new { type = "unavailable", reason = "unsupported" });
            return;
        }

        statusPayload = JsonSerializer.Serialize(
            new
            {
                protocolVersion = 1,
                displayName = command.DisplayName,
                desktopId = command.DesktopId
            },
            HelperProtocol.JsonOptions
        );

        var providerResult = await GattServiceProvider.CreateAsync(Guid.Parse(command.ServiceUuid));
        if (providerResult.Error != BluetoothError.Success || providerResult.ServiceProvider is null)
        {
            HelperProtocol.WriteEvent(new { type = "unavailable", reason = "startup_failed" });
            return;
        }

        serviceProvider = providerResult.ServiceProvider;
        rxCharacteristic = await CreateRxCharacteristicAsync(Guid.Parse(command.RxCharacteristicUuid));
        txCharacteristic = await CreateTxCharacteristicAsync(Guid.Parse(command.TxCharacteristicUuid));
        statusCharacteristic = await CreateStatusCharacteristicAsync(Guid.Parse(command.StatusCharacteristicUuid));

        if (rxCharacteristic is null || txCharacteristic is null || statusCharacteristic is null)
        {
            HelperProtocol.WriteEvent(new { type = "unavailable", reason = "startup_failed" });
            Stop();
            return;
        }

        serviceProvider.StartAdvertising(
            new GattServiceProviderAdvertisingParameters
            {
                IsConnectable = true,
                IsDiscoverable = true
            }
        );

        HelperProtocol.WriteEvent(new { type = "ready" });
    }

    public void Stop()
    {
        if (connected)
        {
            connected = false;
            HelperProtocol.WriteEvent(new { type = "disconnected", connectionId = ConnectionId });
        }

        serviceProvider?.StopAdvertising();
        serviceProvider = null;
        rxCharacteristic = null;
        txCharacteristic = null;
        statusCharacteristic = null;
    }

    public async Task SendAsync(SendCommand command)
    {
        if (command.ConnectionId != ConnectionId || txCharacteristic is null)
        {
            return;
        }

        var json = JsonSerializer.Serialize(command.Frame, HelperProtocol.JsonOptions);
        var bytes = Encoding.UTF8.GetBytes(json);
        await txCharacteristic.NotifyValueAsync(bytes.AsBuffer());
    }

    public void Disconnect(string connectionId)
    {
        if (connectionId != ConnectionId || !connected)
        {
            return;
        }

        connected = false;
        HelperProtocol.WriteEvent(new { type = "disconnected", connectionId = ConnectionId });
    }

    private async Task<GattLocalCharacteristic?> CreateRxCharacteristicAsync(Guid uuid)
    {
        if (serviceProvider is null)
        {
            return null;
        }

        var result = await serviceProvider.Service.CreateCharacteristicAsync(
            uuid,
            new GattLocalCharacteristicParameters
            {
                CharacteristicProperties = GattCharacteristicProperties.Write | GattCharacteristicProperties.WriteWithoutResponse,
                WriteProtectionLevel = GattProtectionLevel.Plain,
                UserDescription = "Switchify RX"
            }
        );

        if (result.Error != BluetoothError.Success || result.Characteristic is null)
        {
            return null;
        }

        result.Characteristic.WriteRequested += OnWriteRequested;
        return result.Characteristic;
    }

    private async Task<GattLocalCharacteristic?> CreateTxCharacteristicAsync(Guid uuid)
    {
        if (serviceProvider is null)
        {
            return null;
        }

        var result = await serviceProvider.Service.CreateCharacteristicAsync(
            uuid,
            new GattLocalCharacteristicParameters
            {
                CharacteristicProperties = GattCharacteristicProperties.Notify,
                UserDescription = "Switchify TX"
            }
        );

        if (result.Error != BluetoothError.Success || result.Characteristic is null)
        {
            return null;
        }

        result.Characteristic.SubscribedClientsChanged += (_, _) =>
        {
            var hasSubscribers = result.Characteristic.SubscribedClients.Count > 0;
            if (hasSubscribers && !connected)
            {
                connected = true;
                HelperProtocol.WriteEvent(new { type = "connected", connectionId = ConnectionId, label = "Bluetooth device" });
            }
            else if (!hasSubscribers && connected)
            {
                connected = false;
                HelperProtocol.WriteEvent(new { type = "disconnected", connectionId = ConnectionId });
            }
        };

        return result.Characteristic;
    }

    private async Task<GattLocalCharacteristic?> CreateStatusCharacteristicAsync(Guid uuid)
    {
        if (serviceProvider is null)
        {
            return null;
        }

        var result = await serviceProvider.Service.CreateCharacteristicAsync(
            uuid,
            new GattLocalCharacteristicParameters
            {
                CharacteristicProperties = GattCharacteristicProperties.Read,
                ReadProtectionLevel = GattProtectionLevel.Plain,
                UserDescription = "Switchify status"
            }
        );

        if (result.Error != BluetoothError.Success || result.Characteristic is null)
        {
            return null;
        }

        result.Characteristic.ReadRequested += OnReadRequested;
        return result.Characteristic;
    }

    private async void OnWriteRequested(GattLocalCharacteristic sender, GattWriteRequestedEventArgs args)
    {
        var deferral = args.GetDeferral();
        try
        {
            var request = await args.GetRequestAsync();
            if (request is null)
            {
                return;
            }

            var bytes = request.Value.ToArray();
            var json = Encoding.UTF8.GetString(bytes);
            var frame = JsonSerializer.Deserialize<BluetoothFrame>(json, HelperProtocol.JsonOptions);
            if (frame is null)
            {
                request.Respond();
                HelperProtocol.WriteEvent(new { type = "error", reason = "invalid_frame" });
                return;
            }

            if (!connected)
            {
                connected = true;
                HelperProtocol.WriteEvent(new { type = "connected", connectionId = ConnectionId, label = "Bluetooth device" });
            }

            HelperProtocol.WriteEvent(new { type = "message", connectionId = ConnectionId, frame });
            request.Respond();
        }
        catch
        {
            HelperProtocol.WriteEvent(new { type = "error", reason = "write_failed" });
        }
        finally
        {
            deferral.Complete();
        }
    }

    private async void OnReadRequested(GattLocalCharacteristic sender, GattReadRequestedEventArgs args)
    {
        var deferral = args.GetDeferral();
        try
        {
            var request = await args.GetRequestAsync();
            if (request is not null)
            {
                request.RespondWithValue(Encoding.UTF8.GetBytes(statusPayload).AsBuffer());
            }
        }
        catch
        {
            HelperProtocol.WriteEvent(new { type = "error", reason = "read_failed" });
        }
        finally
        {
            deferral.Complete();
        }
    }
}
