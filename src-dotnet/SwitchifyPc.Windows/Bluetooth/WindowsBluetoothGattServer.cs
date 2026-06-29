using System.Runtime.InteropServices.WindowsRuntime;
using System.Text;
using System.Text.Json;
using SwitchifyPc.Core.Bluetooth;
using SwitchifyPc.Protocol;
using Windows.Devices.Bluetooth;
using Windows.Devices.Bluetooth.GenericAttributeProfile;
using Windows.Devices.Radios;
using Windows.Storage.Streams;

namespace SwitchifyPc.Windows.Bluetooth;

public sealed record WindowsBluetoothGattServerOptions(
    string DisplayName,
    string DesktopId,
    Guid ServiceUuid,
    Guid RxCharacteristicUuid,
    Guid TxCharacteristicUuid,
    Guid StatusCharacteristicUuid)
{
    public static WindowsBluetoothGattServerOptions CreateDefault(string displayName, string desktopId) =>
        new(
            displayName,
            desktopId,
            BluetoothHelperProtocol.ServiceUuid,
            BluetoothHelperProtocol.RxCharacteristicUuid,
            BluetoothHelperProtocol.TxCharacteristicUuid,
            BluetoothHelperProtocol.StatusCharacteristicUuid);
}

public sealed class WindowsBluetoothGattServer : IDisposable
{
    private const string ConnectionId = "ble";
    private static readonly TimeSpan DisconnectGracePeriod = TimeSpan.FromSeconds(10);
    private static readonly TimeSpan SystemStatusPollInterval = TimeSpan.FromSeconds(5);

    private readonly Action<BluetoothHelperEvent> emit;
    private GattServiceProvider? serviceProvider;
    private GattLocalCharacteristic? txCharacteristic;
    private GattLocalCharacteristic? rxCharacteristic;
    private GattLocalCharacteristic? statusCharacteristic;
    private string statusPayload = "{}";
    private bool connected;
    private CancellationTokenSource? disconnectGrace;
    private WindowsBluetoothGattServerOptions? activeOptions;
    private BluetoothAdapter? currentAdapter;
    private Radio? currentRadio;
    private CancellationTokenSource? systemStatusPolling;
    private string? lastSystemStatusKey;
    private bool restartInProgress;
    private bool disposed;

    public WindowsBluetoothGattServer(Action<BluetoothHelperEvent> emit)
    {
        this.emit = emit;
    }

    public async Task StartAsync(WindowsBluetoothGattServerOptions options)
    {
        ObjectDisposedException.ThrowIf(disposed, this);
        Stop();
        activeOptions = options;
        AdapterSnapshot snapshot = await StartSystemStatusMonitoringAsync().ConfigureAwait(false);
        EmitSystemStatus(snapshot, force: true);

        if (!snapshot.AdapterPresent)
        {
            emit(new BluetoothUnavailableEvent("unsupported"));
            return;
        }

        if (snapshot.IsLowEnergySupported != true || snapshot.IsPeripheralRoleSupported != true)
        {
            emit(new BluetoothUnavailableEvent("unsupported"));
            return;
        }

        if (snapshot.RadioState is "off" or "disabled")
        {
            emit(new BluetoothUnavailableEvent("adapter_off"));
            return;
        }

        await StartAdvertisingAsync(options, restarted: false).ConfigureAwait(false);
    }

    public void Stop()
    {
        StopSystemStatusMonitoring();
        activeOptions = null;
        CancelDisconnectGrace();
        if (connected)
        {
            EmitDisconnected("helper_stopped");
        }

        StopAdvertisingOnly();
    }

    public async Task SendAsync(string connectionId, BluetoothFrame frame)
    {
        ObjectDisposedException.ThrowIf(disposed, this);
        if (connectionId != ConnectionId || txCharacteristic is null)
        {
            return;
        }

        string json = JsonSerializer.Serialize(frame, FrameJsonOptions);
        byte[] bytes = Encoding.UTF8.GetBytes(json);
        await txCharacteristic.NotifyValueAsync(bytes.AsBuffer());
    }

    public void Disconnect(string connectionId)
    {
        ObjectDisposedException.ThrowIf(disposed, this);
        if (connectionId != ConnectionId || !connected)
        {
            return;
        }

        CancelDisconnectGrace();
        EmitDisconnected("pc_requested");
    }

    public void Dispose()
    {
        if (disposed) return;
        disposed = true;
        Stop();
        disconnectGrace?.Dispose();
        systemStatusPolling?.Dispose();
    }

    private async Task StartAdvertisingAsync(WindowsBluetoothGattServerOptions options, bool restarted)
    {
        statusPayload = JsonSerializer.Serialize(
            new
            {
                protocolVersion = ProtocolConstants.ProtocolVersion,
                displayName = options.DisplayName,
                desktopId = options.DesktopId
            },
            FrameJsonOptions);

        GattServiceProviderResult providerResult = await GattServiceProvider.CreateAsync(options.ServiceUuid);
        if (providerResult.Error != BluetoothError.Success || providerResult.ServiceProvider is null)
        {
            emit(new BluetoothUnavailableEvent("startup_failed"));
            return;
        }

        serviceProvider = providerResult.ServiceProvider;
        rxCharacteristic = await CreateRxCharacteristicAsync(options.RxCharacteristicUuid).ConfigureAwait(false);
        txCharacteristic = await CreateTxCharacteristicAsync(options.TxCharacteristicUuid).ConfigureAwait(false);
        statusCharacteristic = await CreateStatusCharacteristicAsync(options.StatusCharacteristicUuid).ConfigureAwait(false);

        if (rxCharacteristic is null || txCharacteristic is null || statusCharacteristic is null)
        {
            emit(new BluetoothUnavailableEvent("startup_failed"));
            Stop();
            return;
        }

        serviceProvider.StartAdvertising(new GattServiceProviderAdvertisingParameters
        {
            IsConnectable = true,
            IsDiscoverable = true
        });

        emit(new BluetoothDiagnosticEvent(restarted ? "advertising_restarted" : "advertising_started"));
        emit(new BluetoothReadyEvent());
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
        CancellationToken token = systemStatusPolling.Token;
        AdapterSnapshot snapshot = await ReadAdapterSnapshotAsync().ConfigureAwait(false);

        _ = Task.Run(async () =>
        {
            while (!token.IsCancellationRequested)
            {
                try
                {
                    await Task.Delay(SystemStatusPollInterval, token).ConfigureAwait(false);
                    if (token.IsCancellationRequested) return;

                    AdapterSnapshot current = await ReadAdapterSnapshotAsync().ConfigureAwait(false);
                    await HandleSystemStatusChangeAsync(current).ConfigureAwait(false);
                }
                catch (OperationCanceledException)
                {
                    return;
                }
                catch
                {
                    await HandleSystemStatusChangeAsync(new AdapterSnapshot(false, "unknown", null, null)).ConfigureAwait(false);
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
            BluetoothAdapter? adapter = await BluetoothAdapter.GetDefaultAsync();
            if (adapter is null)
            {
                DetachRadioStateChanged();
                currentAdapter = null;
                return new AdapterSnapshot(false, "unknown", null, null);
            }

            currentAdapter = adapter;
            Radio? radio = await adapter.GetRadioAsync();
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
                adapter.IsPeripheralRoleSupported);
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
        AdapterSnapshot snapshot = await ReadAdapterSnapshotAsync().ConfigureAwait(false);
        await HandleSystemStatusChangeAsync(snapshot).ConfigureAwait(false);
    }

    private void EmitSystemStatus(AdapterSnapshot snapshot, bool force = false)
    {
        string key = $"{snapshot.AdapterPresent}|{snapshot.RadioState}|{snapshot.IsLowEnergySupported}|{snapshot.IsPeripheralRoleSupported}";
        if (!force && key == lastSystemStatusKey)
        {
            return;
        }

        lastSystemStatusKey = key;
        emit(new BluetoothSystemStatusEvent(
            snapshot.AdapterPresent,
            snapshot.RadioState,
            snapshot.IsLowEnergySupported,
            snapshot.IsPeripheralRoleSupported));
    }

    private async Task HandleSystemStatusChangeAsync(AdapterSnapshot snapshot)
    {
        string? previousKey = lastSystemStatusKey;
        EmitSystemStatus(snapshot);
        bool changed = previousKey != lastSystemStatusKey;
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
            emit(new BluetoothUnavailableEvent("unsupported"));
            return;
        }

        if (snapshot.RadioState is "off" or "disabled")
        {
            emit(new BluetoothDiagnosticEvent("system_radio_off"));
            if (connected)
            {
                EmitDisconnected("adapter_off");
            }

            StopAdvertisingOnly();
            emit(new BluetoothUnavailableEvent("adapter_off"));
            return;
        }

        if (snapshot.RadioState == "on")
        {
            emit(new BluetoothDiagnosticEvent("system_radio_on"));
            await RestartAdvertisingIfPossibleAsync().ConfigureAwait(false);
        }
    }

    private async Task RestartAdvertisingIfPossibleAsync()
    {
        if (restartInProgress || activeOptions is null || serviceProvider is not null)
        {
            return;
        }

        restartInProgress = true;
        try
        {
            await StartAdvertisingAsync(activeOptions, restarted: true).ConfigureAwait(false);
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

        GattLocalCharacteristicResult result = await serviceProvider.Service.CreateCharacteristicAsync(
            uuid,
            new GattLocalCharacteristicParameters
            {
                CharacteristicProperties = GattCharacteristicProperties.Write | GattCharacteristicProperties.WriteWithoutResponse,
                WriteProtectionLevel = GattProtectionLevel.Plain,
                UserDescription = "Switchify RX"
            });

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

        GattLocalCharacteristicResult result = await serviceProvider.Service.CreateCharacteristicAsync(
            uuid,
            new GattLocalCharacteristicParameters
            {
                CharacteristicProperties = GattCharacteristicProperties.Notify,
                UserDescription = "Switchify TX"
            });

        if (result.Error != BluetoothError.Success || result.Characteristic is null)
        {
            return null;
        }

        result.Characteristic.SubscribedClientsChanged += (_, _) =>
        {
            bool hasSubscribers = result.Characteristic.SubscribedClients.Count > 0;
            if (hasSubscribers)
            {
                MarkConnected("subscribed");
            }
            else if (connected)
            {
                emit(new BluetoothDiagnosticEvent("unsubscribed"));
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

        GattLocalCharacteristicResult result = await serviceProvider.Service.CreateCharacteristicAsync(
            uuid,
            new GattLocalCharacteristicParameters
            {
                CharacteristicProperties = GattCharacteristicProperties.Read,
                ReadProtectionLevel = GattProtectionLevel.Plain,
                UserDescription = "Switchify status"
            });

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
            GattWriteRequest? request = await args.GetRequestAsync();
            if (request is null)
            {
                return;
            }

            byte[] bytes = request.Value.ToArray();
            string json = Encoding.UTF8.GetString(bytes);
            BluetoothFrame? frame = JsonSerializer.Deserialize<BluetoothFrame>(json, FrameJsonOptions);
            if (frame is null)
            {
                request.Respond();
                emit(new BluetoothErrorEvent("invalid_frame"));
                return;
            }

            request.Respond();
            MarkConnected("write_received");
            emit(new BluetoothMessageEvent(ConnectionId, frame));
        }
        catch
        {
            emit(new BluetoothErrorEvent("write_failed"));
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
            GattReadRequest? request = await args.GetRequestAsync();
            request?.RespondWithValue(Encoding.UTF8.GetBytes(statusPayload).AsBuffer());
        }
        catch
        {
            emit(new BluetoothErrorEvent("read_failed"));
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
            emit(new BluetoothConnectedEvent(ConnectionId, "Bluetooth device"));
        }

        emit(new BluetoothDiagnosticEvent(eventName));
    }

    private void StartDisconnectGrace()
    {
        if (disconnectGrace is not null)
        {
            return;
        }

        disconnectGrace = new CancellationTokenSource();
        CancellationTokenSource grace = disconnectGrace;
        CancellationToken token = grace.Token;
        emit(new BluetoothDiagnosticEvent("unsubscribe_grace_started"));
        _ = Task.Run(async () =>
        {
            try
            {
                await Task.Delay(DisconnectGracePeriod, token).ConfigureAwait(false);
                if (!token.IsCancellationRequested && connected)
                {
                    emit(new BluetoothDiagnosticEvent("unsubscribe_grace_timed_out"));
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
        CancellationTokenSource? pending = disconnectGrace;
        if (pending is null)
        {
            return;
        }

        disconnectGrace = null;
        pending.Cancel();
        pending.Dispose();
        emit(new BluetoothDiagnosticEvent("unsubscribe_grace_cancelled"));
    }

    private void EmitDisconnected(string reason)
    {
        connected = false;
        disconnectGrace?.Dispose();
        disconnectGrace = null;
        emit(new BluetoothDisconnectedEvent(ConnectionId, reason));
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

    private static readonly JsonSerializerOptions FrameJsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase
    };

    private sealed record AdapterSnapshot(
        bool AdapterPresent,
        string RadioState,
        bool? IsLowEnergySupported,
        bool? IsPeripheralRoleSupported);
}
