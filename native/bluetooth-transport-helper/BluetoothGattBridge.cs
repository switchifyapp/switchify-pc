using System.Runtime.InteropServices.WindowsRuntime;
using System.Text;
using System.Text.Json;
using Windows.Devices.Bluetooth;
using Windows.Devices.Bluetooth.GenericAttributeProfile;
using Windows.Devices.Radios;
using Windows.Storage.Streams;

namespace SwitchifyBluetoothTransport;

internal sealed class BluetoothGattBridge
{
    private const string ConnectionId = "ble";
    private static readonly TimeSpan DisconnectGracePeriod = TimeSpan.FromSeconds(10);
    private static readonly TimeSpan SystemStatusPollInterval = TimeSpan.FromSeconds(5);

    private GattServiceProvider? serviceProvider;
    private GattLocalCharacteristic? txCharacteristic;
    private GattLocalCharacteristic? rxCharacteristic;
    private GattLocalCharacteristic? statusCharacteristic;
    private string statusPayload = "{}";
    private bool connected;
    private CancellationTokenSource? disconnectGrace;
    private StartCommand? activeStartCommand;
    private BluetoothAdapter? currentAdapter;
    private Radio? currentRadio;
    private CancellationTokenSource? systemStatusPolling;
    private string? lastSystemStatusKey;
    private bool restartInProgress;

    public async Task StartAsync(StartCommand command)
    {
        Stop();
        activeStartCommand = command;
        var snapshot = await StartSystemStatusMonitoringAsync();
        EmitSystemStatus(snapshot, force: true);

        if (!snapshot.AdapterPresent)
        {
            HelperProtocol.WriteEvent(new { type = "unavailable", reason = "unsupported" });
            return;
        }

        if (snapshot.IsLowEnergySupported != true || snapshot.IsPeripheralRoleSupported != true)
        {
            HelperProtocol.WriteEvent(new { type = "unavailable", reason = "unsupported" });
            return;
        }

        if (snapshot.RadioState is "off" or "disabled")
        {
            HelperProtocol.WriteEvent(new { type = "unavailable", reason = "adapter_off" });
            return;
        }

        await StartAdvertisingAsync(command, restarted: false);
    }

    public void Stop()
    {
        StopSystemStatusMonitoring();
        activeStartCommand = null;
        CancelDisconnectGrace();
        if (connected)
        {
            EmitDisconnected("helper_stopped");
        }

        StopAdvertisingOnly();
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

        CancelDisconnectGrace();
        EmitDisconnected("pc_requested");
    }

    private async Task StartAdvertisingAsync(StartCommand command, bool restarted)
    {
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

        HelperProtocol.WriteEvent(new { type = "diagnostic", @event = restarted ? "advertising_restarted" : "advertising_started" });
        HelperProtocol.WriteEvent(new { type = "ready" });
    }

    private void StopAdvertisingOnly()
    {
        serviceProvider?.StopAdvertising();
        serviceProvider = null;
        rxCharacteristic = null;
        txCharacteristic = null;
        statusCharacteristic = null;
    }

    private async Task<AdapterSnapshot> StartSystemStatusMonitoringAsync()
    {
        StopSystemStatusMonitoring();
        systemStatusPolling = new CancellationTokenSource();
        var token = systemStatusPolling.Token;
        var snapshot = await ReadAdapterSnapshotAsync();

        _ = Task.Run(async () =>
        {
            while (!token.IsCancellationRequested)
            {
                try
                {
                    await Task.Delay(SystemStatusPollInterval, token);
                    if (token.IsCancellationRequested)
                    {
                        return;
                    }

                    var current = await ReadAdapterSnapshotAsync();
                    await HandleSystemStatusChangeAsync(current);
                }
                catch (OperationCanceledException)
                {
                    return;
                }
                catch
                {
                    var unknown = new AdapterSnapshot(false, "unknown", null, null);
                    await HandleSystemStatusChangeAsync(unknown);
                }
            }
        }, token);

        return snapshot;
    }

    private void StopSystemStatusMonitoring()
    {
        systemStatusPolling?.Cancel();
        systemStatusPolling?.Dispose();
        systemStatusPolling = null;
        DetachRadioStateChanged();
        currentAdapter = null;
        lastSystemStatusKey = null;
    }

    private async Task<AdapterSnapshot> ReadAdapterSnapshotAsync()
    {
        try
        {
            var adapter = await BluetoothAdapter.GetDefaultAsync();
            if (adapter is null)
            {
                DetachRadioStateChanged();
                currentAdapter = null;
                return new AdapterSnapshot(false, "unknown", null, null);
            }

            currentAdapter = adapter;
            var radio = await adapter.GetRadioAsync();
            if (!ReferenceEquals(currentRadio, radio))
            {
                DetachRadioStateChanged();
                if (radio is not null)
                {
                    AttachRadioStateChanged(radio);
                }
            }

            return new AdapterSnapshot(
                true,
                RadioStateToProtocol(radio?.State),
                adapter.IsLowEnergySupported,
                adapter.IsPeripheralRoleSupported
            );
        }
        catch
        {
            return new AdapterSnapshot(false, "unknown", null, null);
        }
    }

    private void AttachRadioStateChanged(Radio radio)
    {
        currentRadio = radio;
        currentRadio.StateChanged += OnRadioStateChanged;
    }

    private void DetachRadioStateChanged()
    {
        if (currentRadio is not null)
        {
            currentRadio.StateChanged -= OnRadioStateChanged;
            currentRadio = null;
        }
    }

    private async void OnRadioStateChanged(Radio sender, object args)
    {
        var snapshot = await ReadAdapterSnapshotAsync();
        await HandleSystemStatusChangeAsync(snapshot);
    }

    private void EmitSystemStatus(AdapterSnapshot snapshot, bool force = false)
    {
        var key = $"{snapshot.AdapterPresent}|{snapshot.RadioState}|{snapshot.IsLowEnergySupported}|{snapshot.IsPeripheralRoleSupported}";
        if (!force && key == lastSystemStatusKey)
        {
            return;
        }

        lastSystemStatusKey = key;
        HelperProtocol.WriteEvent(new
        {
            type = "systemStatus",
            adapterPresent = snapshot.AdapterPresent,
            radioState = snapshot.RadioState,
            isLowEnergySupported = snapshot.IsLowEnergySupported,
            isPeripheralRoleSupported = snapshot.IsPeripheralRoleSupported
        });
    }

    private async Task HandleSystemStatusChangeAsync(AdapterSnapshot snapshot)
    {
        var previousKey = lastSystemStatusKey;
        EmitSystemStatus(snapshot);
        var changed = previousKey != lastSystemStatusKey;
        if (!changed)
        {
            return;
        }

        if (!snapshot.AdapterPresent || snapshot.IsLowEnergySupported != true || snapshot.IsPeripheralRoleSupported != true)
        {
            if (connected)
            {
                EmitDisconnected("adapter_off");
            }
            StopAdvertisingOnly();
            HelperProtocol.WriteEvent(new { type = "unavailable", reason = "unsupported" });
            return;
        }

        if (snapshot.RadioState is "off" or "disabled")
        {
            HelperProtocol.WriteEvent(new { type = "diagnostic", @event = "system_radio_off" });
            if (connected)
            {
                EmitDisconnected("adapter_off");
            }
            StopAdvertisingOnly();
            HelperProtocol.WriteEvent(new { type = "unavailable", reason = "adapter_off" });
            return;
        }

        if (snapshot.RadioState == "on")
        {
            HelperProtocol.WriteEvent(new { type = "diagnostic", @event = "system_radio_on" });
            await RestartAdvertisingIfPossibleAsync();
        }
    }

    private async Task RestartAdvertisingIfPossibleAsync()
    {
        if (restartInProgress || activeStartCommand is null || serviceProvider is not null)
        {
            return;
        }

        restartInProgress = true;
        try
        {
            await StartAdvertisingAsync(activeStartCommand, restarted: true);
        }
        finally
        {
            restartInProgress = false;
        }
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
            if (hasSubscribers)
            {
                MarkConnected("subscribed");
            }
            else if (!hasSubscribers && connected)
            {
                HelperProtocol.WriteEvent(new { type = "diagnostic", @event = "unsubscribed" });
                StartDisconnectGrace();
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

            request.Respond();
            MarkConnected("write_received");
            HelperProtocol.WriteEvent(new { type = "message", connectionId = ConnectionId, frame });
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

    private void MarkConnected(string eventName)
    {
        CancelDisconnectGrace();
        if (!connected)
        {
            connected = true;
            HelperProtocol.WriteEvent(new { type = "connected", connectionId = ConnectionId, label = "Bluetooth device" });
        }
        HelperProtocol.WriteEvent(new { type = "diagnostic", @event = eventName });
    }

    private void StartDisconnectGrace()
    {
        if (disconnectGrace is not null)
        {
            return;
        }

        disconnectGrace = new CancellationTokenSource();
        var grace = disconnectGrace;
        var token = grace.Token;
        HelperProtocol.WriteEvent(new { type = "diagnostic", @event = "unsubscribe_grace_started" });
        _ = Task.Run(async () =>
        {
            try
            {
                await Task.Delay(DisconnectGracePeriod, token);
                if (!token.IsCancellationRequested && connected)
                {
                    HelperProtocol.WriteEvent(new { type = "diagnostic", @event = "unsubscribe_grace_timed_out" });
                    EmitDisconnected("notification_unsubscribed");
                }
            }
            catch (OperationCanceledException)
            {
                // Expected when the client writes or resubscribes before the grace period expires.
            }
            finally
            {
                if (ReferenceEquals(disconnectGrace, grace))
                {
                    grace.Dispose();
                    disconnectGrace = null;
                }
            }
        });
    }

    private void CancelDisconnectGrace()
    {
        var pending = disconnectGrace;
        if (pending is null)
        {
            return;
        }

        disconnectGrace = null;
        pending.Cancel();
        pending.Dispose();
        HelperProtocol.WriteEvent(new { type = "diagnostic", @event = "unsubscribe_grace_cancelled" });
    }

    private void EmitDisconnected(string reason)
    {
        connected = false;
        disconnectGrace?.Dispose();
        disconnectGrace = null;
        HelperProtocol.WriteEvent(new { type = "disconnected", connectionId = ConnectionId, reason });
    }

    private static string RadioStateToProtocol(RadioState? state)
    {
        return state switch
        {
            RadioState.On => "on",
            RadioState.Off => "off",
            RadioState.Disabled => "disabled",
            _ => "unknown"
        };
    }

    private sealed record AdapterSnapshot(
        bool AdapterPresent,
        string RadioState,
        bool? IsLowEnergySupported,
        bool? IsPeripheralRoleSupported
    );
}
