using SwitchifyPc.Core.Bluetooth;
using SwitchifyPc.Core.Updates;

namespace SwitchifyPc.Core.Ui;

public enum DesktopUiState
{
    Loading,
    Starting,
    WaitingForDevice,
    Connected,
    ServerError,
    NotRunning
}

public sealed record BluetoothPrimaryCopy(string Title, string Body, string Tone);

public sealed record UpdateBannerCopy(string Title, string Body, string Tone, string ButtonText);

public static class MainWindowCopy
{
    public static string StatusBadgeTone(DesktopUiState state)
    {
        return state switch
        {
            DesktopUiState.Connected => "connected",
            DesktopUiState.ServerError => "error",
            DesktopUiState.Loading or DesktopUiState.Starting or DesktopUiState.WaitingForDevice or DesktopUiState.NotRunning => "waiting",
            _ => "ready"
        };
    }

    public static string StatusBadgeLabel(DesktopUiState state)
    {
        return state switch
        {
            DesktopUiState.Connected => "Connected",
            DesktopUiState.ServerError => "Needs attention",
            DesktopUiState.WaitingForDevice => "Waiting",
            DesktopUiState.NotRunning => "Not running",
            DesktopUiState.Loading or DesktopUiState.Starting => "Starting...",
            _ => "Ready"
        };
    }

    public static BluetoothPrimaryCopy BluetoothPrimary(DesktopUiState state, BluetoothStatus? bluetooth)
    {
        if (state == DesktopUiState.Connected)
        {
            return new BluetoothPrimaryCopy(
                "Your device is connected",
                "You can control this PC from Switchify over Bluetooth.",
                "ready");
        }

        if (state == DesktopUiState.WaitingForDevice)
        {
            return new BluetoothPrimaryCopy(
                "Waiting for your device",
                "Open Switchify near this PC to reconnect over Bluetooth.",
                "working");
        }

        if (bluetooth?.System.RadioState is "off" or "disabled")
        {
            return new BluetoothPrimaryCopy(
                "Bluetooth is off",
                "Turn on Bluetooth in Windows. Switchify PC will reconnect automatically.",
                "error");
        }

        if (bluetooth?.Status == "unavailable")
        {
            if (bluetooth.Reason == "permission_denied")
            {
                return new BluetoothPrimaryCopy(
                    "Bluetooth permission denied",
                    "Allow Bluetooth access for Switchify PC, then restart the app.",
                    "error");
            }

            if (bluetooth.Reason == "adapter_off")
            {
                return new BluetoothPrimaryCopy(
                    "Bluetooth needs attention",
                    "Turn on Bluetooth in Windows. Switchify PC will update when it is available.",
                    "error");
            }

            return new BluetoothPrimaryCopy(
                "Bluetooth needs attention",
                "Turn on Bluetooth in Windows. Switchify PC will reconnect automatically.",
                "error");
        }

        if (state == DesktopUiState.ServerError || bluetooth?.Status == "error")
        {
            return new BluetoothPrimaryCopy(
                "Bluetooth needs attention",
                "Restart Switchify PC. If this keeps happening, open troubleshooting details.",
                "error");
        }

        if (state is DesktopUiState.Loading or DesktopUiState.Starting || bluetooth?.Status == "starting")
        {
            return new BluetoothPrimaryCopy(
                "Getting Bluetooth ready...",
                bluetooth?.System.RadioState == "on"
                    ? "Switchify PC is restarting nearby device connection."
                    : "Switchify PC is preparing nearby device connection.",
                "working");
        }

        if (state == DesktopUiState.NotRunning || bluetooth?.Status is "stopped" or "disabled")
        {
            return new BluetoothPrimaryCopy(
                "Switchify PC is not running",
                "Restart the app to connect your device over Bluetooth.",
                "error");
        }

        return new BluetoothPrimaryCopy(
            "Ready for Bluetooth",
            "Open Switchify on your Android device while you are near this PC.",
            "ready");
    }

    public static string BluetoothStatusLabel(BluetoothStatus? status)
    {
        if (status is null) return "Bluetooth status unknown.";
        if (!status.System.AdapterPresent) return "Bluetooth adapter not found.";
        if (status.System.RadioState == "off") return "Bluetooth radio off.";
        if (status.System.RadioState == "disabled") return "Bluetooth radio disabled.";
        return status.Status switch
        {
            "ready" => "Bluetooth ready.",
            "connected" => status.ConnectedClientCount == 1 ? "Bluetooth device connected." : "Bluetooth devices connected.",
            "starting" => "Starting Bluetooth...",
            "stopped" => "Bluetooth stopped.",
            "disabled" => "Bluetooth disabled.",
            "unavailable" when status.Reason == "adapter_off" => "Bluetooth is off.",
            "unavailable" when status.Reason == "permission_denied" => "Bluetooth permission denied.",
            "unavailable" => "Bluetooth unavailable.",
            _ => "Bluetooth needs attention."
        };
    }

    public static string BluetoothSystemRadioState(BluetoothStatus? status)
    {
        if (status is null) return "Bluetooth radio state unknown.";
        if (!status.System.AdapterPresent) return "Bluetooth adapter not found.";
        return status.System.RadioState switch
        {
            "on" => "Bluetooth radio on.",
            "off" => "Bluetooth radio off.",
            "disabled" => "Bluetooth radio disabled.",
            _ => "Bluetooth radio state unknown."
        };
    }

    public static string BluetoothSystemCapabilities(BluetoothStatus? status)
    {
        BluetoothSystemStatus? system = status?.System;
        if (system is null || system.IsLowEnergySupported is null || system.IsPeripheralRoleSupported is null)
        {
            return "Bluetooth capabilities unknown.";
        }

        return system.IsLowEnergySupported.Value && system.IsPeripheralRoleSupported.Value
            ? "Bluetooth LE peripheral supported."
            : "Bluetooth LE peripheral not supported.";
    }

    public static string Timestamp(double? value)
    {
        if (value is null or <= 0) return "Not yet.";

        try
        {
            DateTimeOffset timestamp = DateTimeOffset.FromUnixTimeMilliseconds(Convert.ToInt64(value.Value));
            return timestamp.ToLocalTime().ToString("HH:mm:ss");
        }
        catch (ArgumentOutOfRangeException)
        {
            return "Not yet.";
        }
    }

    public static string BluetoothDiagnosticEvent(string? diagnosticEvent)
    {
        return diagnosticEvent switch
        {
            "advertising_started" => "Advertising started.",
            "advertising_restarted" => "Advertising restarted.",
            "system_radio_on" => "Bluetooth turned on.",
            "system_radio_off" => "Bluetooth turned off.",
            "subscribed" => "Device subscribed.",
            "unsubscribed" => "Device unsubscribed.",
            "unsubscribe_grace_started" => "Waiting for Bluetooth reconnect.",
            "unsubscribe_grace_cancelled" => "Bluetooth reconnect resumed.",
            "unsubscribe_grace_timed_out" => "Bluetooth reconnect timed out.",
            "write_received" => "Message received.",
            _ => "Not recorded."
        };
    }

    public static string BluetoothEventSummary(BluetoothStatus? status)
    {
        if (status?.LastEvent is null) return "Not recorded.";
        return $"{BluetoothDiagnosticEvent(status.LastEvent)} {Timestamp(status.LastEventAt)}";
    }

    public static string BluetoothRecentEvents(BluetoothStatus? status)
    {
        IReadOnlyList<BluetoothDiagnosticRecord> events = status?.RecentEvents ?? [];
        if (events.Count == 0) return "Not recorded.";
        return string.Join(" ", events.Select(item => $"{BluetoothDiagnosticEvent(item.Event)} {Timestamp(item.At)}"));
    }

    public static string BluetoothDisconnectReason(string? reason)
    {
        return reason switch
        {
            "notification_unsubscribed" => "Bluetooth connection lost.",
            "client_requested" => "Android device disconnected.",
            "pc_requested" => "Disconnected from this PC.",
            "helper_stopped" => "Bluetooth helper stopped.",
            "helper_error" => "Bluetooth helper error.",
            "adapter_off" => "Bluetooth was turned off.",
            _ => "Not recorded."
        };
    }

    public static string BluetoothDisconnectSummary(BluetoothStatus? status)
    {
        if (status?.LastDisconnectReason is null) return "Not recorded.";
        return $"{BluetoothDisconnectReason(status.LastDisconnectReason)} {Timestamp(status.LastDisconnectAt)}";
    }

    public static string BluetoothRecentError(BluetoothStatus? status)
    {
        return string.IsNullOrWhiteSpace(status?.LastError)
            ? "No recent errors."
            : status.LastError;
    }

    public static UpdateBannerCopy? UpdateBanner(UpdateState? state)
    {
        if (state is null) return null;
        if (state.Download.Status == UpdateDownloadStatus.Downloaded)
        {
            return new UpdateBannerCopy(
                "Update ready to install",
                "The update has been downloaded and is ready to install.",
                "downloaded",
                "Open updates");
        }

        if (state.Info.Status == UpdateCheckStatus.UpdateAvailable)
        {
            return new UpdateBannerCopy(
                "Update available",
                "A new Switchify PC update is ready to download.",
                "available",
                "Open updates");
        }

        return null;
    }
}
