using System.Text.Json;
using System.Text.Json.Serialization;

namespace SwitchifyBluetoothTransport;

internal static class HelperProtocol
{
    public static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull
    };

    public static void WriteEvent(object value)
    {
        Console.Out.WriteLine(JsonSerializer.Serialize(value, JsonOptions));
        Console.Out.Flush();
    }

    public static HelperCommand? ParseCommand(string line)
    {
        using var document = JsonDocument.Parse(line);
        if (!document.RootElement.TryGetProperty("type", out var typeElement))
        {
            return null;
        }

        var type = typeElement.GetString();
        return type switch
        {
            "start" => JsonSerializer.Deserialize<StartCommand>(line, JsonOptions),
            "stop" => new StopCommand(),
            "send" => JsonSerializer.Deserialize<SendCommand>(line, JsonOptions),
            "disconnect" => JsonSerializer.Deserialize<DisconnectCommand>(line, JsonOptions),
            "shutdown" => new ShutdownCommand(),
            _ => null
        };
    }
}

internal abstract record HelperCommand(string Type);

internal sealed record StartCommand(
    string ServiceUuid,
    string RxCharacteristicUuid,
    string TxCharacteristicUuid,
    string StatusCharacteristicUuid,
    string DisplayName,
    string DesktopId
) : HelperCommand("start");

internal sealed record StopCommand() : HelperCommand("stop");

internal sealed record ShutdownCommand() : HelperCommand("shutdown");

internal sealed record DisconnectCommand(string ConnectionId) : HelperCommand("disconnect");

internal sealed record SendCommand(string ConnectionId, BluetoothFrame Frame) : HelperCommand("send");

internal sealed record BluetoothFrame(
    int Version,
    string MessageId,
    int Sequence,
    bool IsFinal,
    int TotalBytes,
    string PayloadBase64
);

