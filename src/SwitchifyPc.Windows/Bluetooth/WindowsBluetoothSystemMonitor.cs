using Windows.Devices.Bluetooth;
using Windows.Devices.Radios;

namespace SwitchifyPc.Windows.Bluetooth;

internal sealed record WindowsBluetoothAdapterSnapshot(
    bool AdapterPresent,
    string RadioState,
    bool? IsLowEnergySupported,
    bool? IsPeripheralRoleSupported);

internal sealed class WindowsBluetoothSystemMonitor : IDisposable
{
    private static readonly TimeSpan PollInterval = TimeSpan.FromSeconds(5);
    private Radio? currentRadio;
    private CancellationTokenSource? polling;
    private Func<WindowsBluetoothAdapterSnapshot, Task>? onStatusChanged;

    public async Task<WindowsBluetoothAdapterSnapshot> StartAsync(
        Func<WindowsBluetoothAdapterSnapshot, Task> statusChanged)
    {
        Stop();
        onStatusChanged = statusChanged;
        polling = new CancellationTokenSource();
        CancellationToken token = polling.Token;
        WindowsBluetoothAdapterSnapshot snapshot = await ReadAsync().ConfigureAwait(false);

        _ = Task.Run(async () =>
        {
            while (!token.IsCancellationRequested)
            {
                try
                {
                    await Task.Delay(PollInterval, token).ConfigureAwait(false);
                    if (token.IsCancellationRequested) return;
                    await NotifyStatusChangedAsync(await ReadAsync().ConfigureAwait(false)).ConfigureAwait(false);
                }
                catch (OperationCanceledException)
                {
                    return;
                }
                catch
                {
                    await NotifyStatusChangedAsync(UnavailableSnapshot()).ConfigureAwait(false);
                }
            }
        }, token);

        return snapshot;
    }

    public async Task<WindowsBluetoothAdapterSnapshot> ReadAsync()
    {
        try
        {
            BluetoothAdapter? adapter = await BluetoothAdapter.GetDefaultAsync();
            if (adapter is null)
            {
                DetachRadio();
                return UnavailableSnapshot();
            }

            Radio? radio = await adapter.GetRadioAsync();
            if (!ReferenceEquals(currentRadio, radio))
            {
                DetachRadio();
                if (radio is not null)
                {
                    currentRadio = radio;
                    currentRadio.StateChanged += OnRadioStateChanged;
                }
            }

            return new WindowsBluetoothAdapterSnapshot(
                true,
                RadioStateToProtocol(radio?.State),
                adapter.IsLowEnergySupported,
                adapter.IsPeripheralRoleSupported);
        }
        catch
        {
            return UnavailableSnapshot();
        }
    }

    public void Stop()
    {
        polling?.Cancel();
        polling?.Dispose();
        polling = null;
        onStatusChanged = null;
        DetachRadio();
    }

    public void Dispose()
    {
        Stop();
    }

    internal static string RadioStateToProtocol(RadioState? state)
    {
        return state switch
        {
            RadioState.On => "on",
            RadioState.Off => "off",
            RadioState.Disabled => "disabled",
            _ => "unknown"
        };
    }

    private async void OnRadioStateChanged(Radio sender, object args)
    {
        await NotifyStatusChangedAsync(await ReadAsync().ConfigureAwait(false)).ConfigureAwait(false);
    }

    private Task NotifyStatusChangedAsync(WindowsBluetoothAdapterSnapshot snapshot)
    {
        return onStatusChanged?.Invoke(snapshot) ?? Task.CompletedTask;
    }

    private void DetachRadio()
    {
        if (currentRadio is null) return;
        currentRadio.StateChanged -= OnRadioStateChanged;
        currentRadio = null;
    }

    private static WindowsBluetoothAdapterSnapshot UnavailableSnapshot() =>
        new(false, "unknown", null, null);
}
