using System.Text.Json;
using SwitchifyPc.Protocol;

namespace SwitchifyPc.Core.Bluetooth;

public abstract record BluetoothHelperEvent(string Type);
public sealed record BluetoothReadyEvent() : BluetoothHelperEvent("ready");
public sealed record BluetoothUnavailableEvent(string Reason) : BluetoothHelperEvent("unavailable");
public sealed record BluetoothConnectedEvent(string ConnectionId, string Label) : BluetoothHelperEvent("connected");
public sealed record BluetoothMessageEvent(string ConnectionId, BluetoothFrame Frame) : BluetoothHelperEvent("message");
public sealed record BluetoothDisconnectedEvent(string ConnectionId, string Reason) : BluetoothHelperEvent("disconnected");
public sealed record BluetoothDiagnosticEvent(string Event) : BluetoothHelperEvent("diagnostic");
public sealed record BluetoothSystemStatusEvent(
    bool AdapterPresent,
    string RadioState,
    bool? IsLowEnergySupported,
    bool? IsPeripheralRoleSupported) : BluetoothHelperEvent("systemStatus");
public sealed record BluetoothErrorEvent(string Reason) : BluetoothHelperEvent("error");

public static class BluetoothHelperProtocol
{
    public static readonly Guid ServiceUuid = Guid.Parse("7a78f7e8-1d6d-4d92-9ef0-1f89d3db21f4");
    public static readonly Guid RxCharacteristicUuid = Guid.Parse("7a78f7e9-1d6d-4d92-9ef0-1f89d3db21f4");
    public static readonly Guid TxCharacteristicUuid = Guid.Parse("7a78f7ea-1d6d-4d92-9ef0-1f89d3db21f4");
    public static readonly Guid StatusCharacteristicUuid = Guid.Parse("7a78f7eb-1d6d-4d92-9ef0-1f89d3db21f4");

    public static bool TryParseEvent(string json, out BluetoothHelperEvent? helperEvent)
    {
        helperEvent = null;
        try
        {
            using JsonDocument document = JsonDocument.Parse(json);
            JsonElement root = document.RootElement;
            if (root.ValueKind != JsonValueKind.Object || !TryGetString(root, "type", out string type))
            {
                return false;
            }

            helperEvent = type switch
            {
                "ready" => new BluetoothReadyEvent(),
                "unavailable" => ParseUnavailable(root),
                "connected" => ParseConnected(root),
                "message" => ParseMessage(root),
                "disconnected" => ParseDisconnected(root),
                "diagnostic" => ParseDiagnostic(root),
                "systemStatus" => ParseSystemStatus(root),
                "error" => ParseError(root),
                _ => null
            };

            return helperEvent is not null;
        }
        catch (JsonException)
        {
            return false;
        }
    }

    private static BluetoothHelperEvent? ParseUnavailable(JsonElement root)
    {
        return TryGetString(root, "reason", out string reason) && BluetoothStatusModel.UnavailableReasons.Contains(reason)
            ? new BluetoothUnavailableEvent(reason)
            : null;
    }

    private static BluetoothHelperEvent? ParseConnected(JsonElement root)
    {
        return TryGetString(root, "connectionId", out string connectionId) &&
            TryGetString(root, "label", out string label)
                ? new BluetoothConnectedEvent(connectionId, label)
                : null;
    }

    private static BluetoothHelperEvent? ParseMessage(JsonElement root)
    {
        if (!TryGetString(root, "connectionId", out string connectionId) ||
            !root.TryGetProperty("frame", out JsonElement frameElement) ||
            frameElement.ValueKind != JsonValueKind.Object)
        {
            return null;
        }

        try
        {
            BluetoothFrame? frame = JsonSerializer.Deserialize<BluetoothFrame>(frameElement.GetRawText(), FrameJsonOptions);
            return frame is not null && BluetoothFrameCodec.Validate(frame).Reason == "incomplete"
                ? new BluetoothMessageEvent(connectionId, frame)
                : null;
        }
        catch (JsonException)
        {
            return null;
        }
    }

    private static BluetoothHelperEvent? ParseDisconnected(JsonElement root)
    {
        return TryGetString(root, "connectionId", out string connectionId) &&
            TryGetString(root, "reason", out string reason) &&
            BluetoothStatusModel.DisconnectReasons.Contains(reason)
                ? new BluetoothDisconnectedEvent(connectionId, reason)
                : null;
    }

    private static BluetoothHelperEvent? ParseDiagnostic(JsonElement root)
    {
        return TryGetString(root, "event", out string diagnosticEvent) && BluetoothStatusModel.DiagnosticEvents.Contains(diagnosticEvent)
            ? new BluetoothDiagnosticEvent(diagnosticEvent)
            : null;
    }

    private static BluetoothHelperEvent? ParseSystemStatus(JsonElement root)
    {
        return TryGetBoolean(root, "adapterPresent", out bool adapterPresent) &&
            TryGetString(root, "radioState", out string radioState) &&
            BluetoothStatusModel.SystemRadioStates.Contains(radioState) &&
            TryGetOptionalBoolean(root, "isLowEnergySupported", out bool? isLowEnergySupported) &&
            TryGetOptionalBoolean(root, "isPeripheralRoleSupported", out bool? isPeripheralRoleSupported)
                ? new BluetoothSystemStatusEvent(adapterPresent, radioState, isLowEnergySupported, isPeripheralRoleSupported)
                : null;
    }

    private static BluetoothHelperEvent? ParseError(JsonElement root)
    {
        return TryGetString(root, "reason", out string reason) ? new BluetoothErrorEvent(reason) : null;
    }

    private static bool TryGetString(JsonElement value, string propertyName, out string result)
    {
        result = "";
        return value.TryGetProperty(propertyName, out JsonElement property) &&
            property.ValueKind == JsonValueKind.String &&
            !string.IsNullOrEmpty(result = property.GetString() ?? "");
    }

    private static bool TryGetBoolean(JsonElement value, string propertyName, out bool result)
    {
        result = false;
        if (!value.TryGetProperty(propertyName, out JsonElement property)) return false;
        if (property.ValueKind == JsonValueKind.True)
        {
            result = true;
            return true;
        }

        if (property.ValueKind == JsonValueKind.False)
        {
            result = false;
            return true;
        }

        return false;
    }

    private static bool TryGetOptionalBoolean(JsonElement value, string propertyName, out bool? result)
    {
        result = null;
        if (!value.TryGetProperty(propertyName, out JsonElement property)) return false;
        if (property.ValueKind == JsonValueKind.Null) return true;
        if (property.ValueKind == JsonValueKind.True)
        {
            result = true;
            return true;
        }

        if (property.ValueKind == JsonValueKind.False)
        {
            result = false;
            return true;
        }

        return false;
    }

    private static readonly JsonSerializerOptions FrameJsonOptions = new()
    {
        PropertyNameCaseInsensitive = true
    };
}
