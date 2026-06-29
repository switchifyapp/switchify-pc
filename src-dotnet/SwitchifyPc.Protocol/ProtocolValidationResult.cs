using System.Text.Json;

namespace SwitchifyPc.Protocol;

public sealed record ProtocolValidationResult
{
    private ProtocolValidationResult(bool ok, JsonElement? value, string? error, string? message)
    {
        Ok = ok;
        Value = value;
        Error = error;
        Message = message;
    }

    public bool Ok { get; }
    public JsonElement? Value { get; }
    public string? Error { get; }
    public string? Message { get; }

    public static ProtocolValidationResult Valid(JsonElement value)
    {
        return new ProtocolValidationResult(true, value.Clone(), null, null);
    }

    public static ProtocolValidationResult Invalid(string error, string message)
    {
        return new ProtocolValidationResult(false, null, error, message);
    }
}
