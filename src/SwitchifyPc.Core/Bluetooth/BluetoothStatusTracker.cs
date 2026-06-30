namespace SwitchifyPc.Core.Bluetooth;

public sealed class BluetoothStatusTracker
{
    private readonly Func<double> now;
    private readonly Action<BluetoothStatus>? onStatusChanged;
    private readonly HashSet<string> connections = new(StringComparer.Ordinal);

    public BluetoothStatusTracker(Func<double>? now = null, Action<BluetoothStatus>? onStatusChanged = null)
    {
        this.now = now ?? (() => DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());
        this.onStatusChanged = onStatusChanged;
        Status = BluetoothStatusModel.DefaultStatus with { Status = OperatingSystem.IsWindows() ? "stopped" : "disabled" };
    }

    public BluetoothStatus Status { get; private set; }

    public BluetoothStatus SetStarting()
    {
        return SetStatus(Status with { Status = "starting", Reason = null, LastError = null });
    }

    public BluetoothStatus SetReady()
    {
        return SetStatus(Status with { Status = "ready", Reason = null, LastError = null });
    }

    public BluetoothStatus SetUnavailable(string reason)
    {
        return SetStatus(Status with { Status = "unavailable", Reason = reason, ConnectedClientCount = 0 });
    }

    public BluetoothStatus SetError(string message)
    {
        return SetStatus(Status with { Status = "error", LastError = SafeBluetoothError(message) });
    }

    public BluetoothStatus AddConnection(string connectionId)
    {
        connections.Add(connectionId);
        return SetStatus(Status with { Status = "connected", ConnectedClientCount = connections.Count, Reason = null });
    }

    public BluetoothStatus RemoveConnection(string connectionId, string? reason = null)
    {
        if (!connections.Remove(connectionId)) return Status;
        return SetStatus(Status with
        {
            Status = connections.Count > 0 ? "connected" : "ready",
            ConnectedClientCount = connections.Count,
            LastDisconnectReason = reason,
            LastDisconnectAt = reason is null ? Status.LastDisconnectAt : now()
        });
    }

    public BluetoothStatus RemoveAllConnections(string reason)
    {
        foreach (string connectionId in connections.ToArray())
        {
            RemoveConnection(connectionId, reason);
        }

        return Status;
    }

    public BluetoothStatus RecordDiagnostic(string diagnosticEvent)
    {
        double at = now();
        return SetStatus(Status with
        {
            LastEvent = diagnosticEvent,
            LastEventAt = at,
            RecentEvents = Status.RecentEvents
                .Concat([new BluetoothDiagnosticRecord(diagnosticEvent, at)])
                .TakeLast(5)
                .ToArray()
        });
    }

    public BluetoothStatus SetSystemStatus(BluetoothSystemStatusEvent systemStatus)
    {
        double at = now();
        BluetoothSystemStatus previous = Status.System;
        bool changed = previous.LastCheckedAt is null ||
            previous.AdapterPresent != systemStatus.AdapterPresent ||
            previous.RadioState != systemStatus.RadioState ||
            previous.IsLowEnergySupported != systemStatus.IsLowEnergySupported ||
            previous.IsPeripheralRoleSupported != systemStatus.IsPeripheralRoleSupported;

        BluetoothSystemStatus system = new(
            AdapterPresent: systemStatus.AdapterPresent,
            RadioState: systemStatus.RadioState,
            IsLowEnergySupported: systemStatus.IsLowEnergySupported,
            IsPeripheralRoleSupported: systemStatus.IsPeripheralRoleSupported,
            LastCheckedAt: at,
            LastChangedAt: changed ? at : previous.LastChangedAt);

        if (!systemStatus.AdapterPresent)
        {
            RemoveAllConnections("adapter_off");
            return SetStatus(Status with { System = system, Status = "unavailable", Reason = "unsupported", ConnectedClientCount = 0 });
        }

        if (systemStatus.RadioState is "off" or "disabled")
        {
            RemoveAllConnections("adapter_off");
            return SetStatus(Status with { System = system, Status = "unavailable", Reason = "adapter_off", ConnectedClientCount = 0 });
        }

        if (systemStatus.RadioState == "on" && Status.Status == "unavailable" && Status.Reason == "adapter_off")
        {
            return SetStatus(Status with { System = system, Status = "starting", Reason = null, LastError = null });
        }

        return SetStatus(Status with { System = system });
    }

    private BluetoothStatus SetStatus(BluetoothStatus status)
    {
        Status = status;
        onStatusChanged?.Invoke(status);
        return status;
    }

    private static string SafeBluetoothError(string message)
    {
        return message.Length > 300 ? $"{message[..297]}..." : message;
    }
}
