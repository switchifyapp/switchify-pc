namespace SwitchifyPc.Core.Bluetooth;

public sealed record BluetoothSystemStatus(
    bool AdapterPresent,
    string RadioState,
    bool? IsLowEnergySupported,
    bool? IsPeripheralRoleSupported,
    double? LastCheckedAt,
    double? LastChangedAt);

public sealed record BluetoothDiagnosticRecord(string Event, double At);

public sealed record BluetoothStatus(
    string Status,
    string? Reason,
    int ConnectedClientCount,
    string? LastError,
    string? LastEvent,
    double? LastEventAt,
    IReadOnlyList<BluetoothDiagnosticRecord> RecentEvents,
    string? LastDisconnectReason,
    double? LastDisconnectAt,
    BluetoothSystemStatus System);

public static class BluetoothStatusModel
{
    public static readonly IReadOnlySet<string> TransportStatuses = new HashSet<string>(StringComparer.Ordinal)
    {
        "disabled",
        "starting",
        "ready",
        "unavailable",
        "connected",
        "error",
        "stopped"
    };

    public static readonly IReadOnlySet<string> SystemRadioStates = new HashSet<string>(StringComparer.Ordinal)
    {
        "on",
        "off",
        "disabled",
        "unknown"
    };

    public static readonly IReadOnlySet<string> UnavailableReasons = new HashSet<string>(StringComparer.Ordinal)
    {
        "unsupported",
        "permission_denied",
        "adapter_off",
        "startup_failed"
    };

    public static readonly IReadOnlySet<string> DisconnectReasons = new HashSet<string>(StringComparer.Ordinal)
    {
        "notification_unsubscribed",
        "client_requested",
        "pc_requested",
        "helper_stopped",
        "helper_error",
        "adapter_off"
    };

    public static readonly IReadOnlySet<string> DiagnosticEvents = new HashSet<string>(StringComparer.Ordinal)
    {
        "advertising_started",
        "advertising_restarted",
        "system_radio_on",
        "system_radio_off",
        "subscribed",
        "unsubscribed",
        "unsubscribe_grace_started",
        "unsubscribe_grace_cancelled",
        "unsubscribe_grace_timed_out",
        "write_received"
    };

    public static readonly BluetoothSystemStatus DefaultSystemStatus = new(
        AdapterPresent: false,
        RadioState: "unknown",
        IsLowEnergySupported: null,
        IsPeripheralRoleSupported: null,
        LastCheckedAt: null,
        LastChangedAt: null);

    public static readonly BluetoothStatus DefaultStatus = new(
        Status: "disabled",
        Reason: null,
        ConnectedClientCount: 0,
        LastError: null,
        LastEvent: null,
        LastEventAt: null,
        RecentEvents: [],
        LastDisconnectReason: null,
        LastDisconnectAt: null,
        System: DefaultSystemStatus);
}
