using System.Text.Json;
using System.Text.Json.Nodes;
using System.Text.RegularExpressions;

namespace SwitchifyPc.Protocol;

public static partial class ProtocolValidator
{
    public static ProtocolValidationResult ParseProtocolRequest(string raw)
    {
        try
        {
            using JsonDocument document = JsonDocument.Parse(raw);
            return ValidateProtocolRequest(document.RootElement);
        }
        catch (JsonException)
        {
            return Invalid("invalid_json", "Message must be valid JSON.");
        }
    }

    public static ProtocolValidationResult ValidateProtocolRequest(JsonElement value)
    {
        if (!IsObject(value)) return Invalid("invalid_message", "Message must be an object.");
        if (!HasProtocolVersion(value)) return Invalid("invalid_version", "Unsupported protocol version.");
        if (!TryGetNonEmptyString(value, "id", out _)) return Invalid("invalid_message", "Message id is required.");
        if (!TryGetNonEmptyString(value, "type", out string? type)) return Invalid("invalid_type", "Message type is required.");
        if (!TryGetObject(value, "payload", out JsonElement payload)) return Invalid("invalid_payload", "Payload must be an object.");

        if (ProtocolConstants.CommandTypes.Contains(type))
        {
            return ValidateCommandRequest(value, type, payload);
        }

        if (ProtocolConstants.PairingRequestTypes.Contains(type))
        {
            return ValidatePairingRequest(value, payload);
        }

        return Invalid("invalid_type", "Unsupported message type.");
    }

    public static ProtocolValidationResult ValidateProtocolResponse(JsonElement value)
    {
        if (!IsObject(value)) return Invalid("invalid_message", "Response must be an object.");
        if (!HasProtocolVersion(value)) return Invalid("invalid_version", "Unsupported protocol version.");
        if (!TryGetNonEmptyString(value, "type", out string? type)) return Invalid("invalid_type", "Unsupported response type.");

        return type switch
        {
            "ack" => ValidateAckResponse(value),
            "error" => ValidateErrorResponse(value),
            "pairing.complete" => ValidatePairingCompleteResponse(value),
            "pointer.profile" => ValidatePointerProfileResponse(value),
            _ => Invalid("invalid_type", "Unsupported response type.")
        };
    }

    public static JsonObject CreateAckResponse(string id)
    {
        return new JsonObject
        {
            ["version"] = ProtocolConstants.ProtocolVersion,
            ["id"] = id,
            ["type"] = "ack",
            ["ok"] = true,
            ["error"] = null
        };
    }

    public static JsonObject CreateErrorResponse(string? id, string code, string message, string? detail = null)
    {
        JsonObject error = new()
        {
            ["code"] = code,
            ["message"] = Truncate(message, ProtocolConstants.MaxErrorMessageLength)
        };

        if (!string.IsNullOrEmpty(detail))
        {
            error["detail"] = detail;
        }

        return new JsonObject
        {
            ["version"] = ProtocolConstants.ProtocolVersion,
            ["id"] = id,
            ["type"] = "error",
            ["ok"] = false,
            ["error"] = error
        };
    }

    public static JsonObject CreatePairingCompleteResponse(string id, string desktopId, string deviceId, string token)
    {
        return new JsonObject
        {
            ["version"] = ProtocolConstants.ProtocolVersion,
            ["id"] = id,
            ["type"] = "pairing.complete",
            ["ok"] = true,
            ["payload"] = new JsonObject
            {
                ["desktopId"] = desktopId,
                ["deviceId"] = deviceId,
                ["token"] = token
            },
            ["error"] = null
        };
    }

    private static ProtocolValidationResult ValidateCommandRequest(JsonElement envelope, string type, JsonElement payload)
    {
        if (!TryGetNonEmptyString(envelope, "deviceId", out _)) return Invalid("invalid_message", "Device id is required.");
        if (!TryGetFiniteNumber(envelope, "timestamp", out _)) return Invalid("invalid_message", "Timestamp is required.");
        if (!TryGetNonEmptyString(envelope, "auth", out _)) return Invalid("invalid_auth", "Auth proof is required.");
        if (!IsValidResponseMode(envelope, type)) return Invalid("invalid_payload", "Response mode is invalid.");

        ProtocolValidationResult payloadResult = ValidateCommandPayload(type, payload);
        return payloadResult.Ok ? ProtocolValidationResult.Valid(envelope) : payloadResult;
    }

    private static ProtocolValidationResult ValidatePairingRequest(JsonElement envelope, JsonElement payload)
    {
        if (!TryGetNonEmptyString(payload, "deviceId", out _)) return Invalid("invalid_payload", "Pairing device id is required.");
        if (!TryGetNonEmptyString(payload, "deviceName", out _)) return Invalid("invalid_payload", "Pairing device name is required.");
        if (!TryGetNonEmptyString(payload, "desktopId", out _)) return Invalid("invalid_payload", "Desktop id is required.");
        if (!TryGetNonEmptyString(payload, "requestNonce", out _)) return Invalid("invalid_payload", "Pairing request nonce is required.");
        return ProtocolValidationResult.Valid(envelope);
    }

    private static ProtocolValidationResult ValidateCommandPayload(string type, JsonElement payload)
    {
        return type switch
        {
            "mouse.move" => ValidateBoundedNumbers(payload, ["dx", "dy"], ProtocolConstants.MaxPointerDelta),
            "mouse.scroll" => ValidateBoundedNumbers(payload, ["dx", "dy"], ProtocolConstants.MaxScrollDelta),
            "mouse.click" or "mouse.doubleClick" or "mouse.dragStart" or "mouse.dragEnd" =>
                TryGetString(payload, "button", out string? button) && ProtocolConstants.MouseButtons.Contains(button)
                    ? Valid(payload)
                    : Invalid("invalid_payload", "Mouse button is invalid."),
            "mouse.rightClick" or "connection.ping" or "connection.disconnecting" or "pointer.profile" =>
                ObjectPropertyCount(payload) == 0 ? Valid(payload) : Invalid("invalid_payload", "Payload must be empty."),
            "keyboard.key" =>
                TryGetString(payload, "key", out string? key) && ProtocolConstants.KeyboardKeys.Contains(key)
                    ? Valid(payload)
                    : Invalid("invalid_payload", "Keyboard key is invalid."),
            "keyboard.shortcut" => ValidateShortcutPayload(payload),
            "keyboard.typeText" =>
                TryGetString(payload, "text", out string? text) && IsSafeTextPayload(text)
                    ? Valid(payload)
                    : Invalid("invalid_payload", "Text payload is invalid."),
            "keyboard.textStream.open" =>
                TryGetString(payload, "streamId", out string? streamId) && IsSafeTextStreamId(streamId)
                    ? Valid(payload)
                    : Invalid("invalid_payload", "Text stream id is invalid."),
            "keyboard.textStream.char" => ValidateTextStreamChar(payload),
            "keyboard.textStream.chunk" => ValidateTextStreamChunk(payload),
            "keyboard.textStream.key" => ValidateTextStreamKey(payload),
            "keyboard.textStream.close" => ValidateTextStreamClose(payload),
            "media.control" =>
                TryGetString(payload, "action", out string? mediaAction) && ProtocolConstants.MediaActions.Contains(mediaAction)
                    ? Valid(payload)
                    : Invalid("invalid_payload", "Media action is invalid."),
            "window.control" =>
                TryGetString(payload, "action", out string? windowAction) && ProtocolConstants.WindowControlActions.Contains(windowAction)
                    ? Valid(payload)
                    : Invalid("invalid_payload", "Window control action is invalid."),
            _ => Invalid("invalid_type", "Unsupported message type.")
        };
    }

    private static ProtocolValidationResult ValidateShortcutPayload(JsonElement payload)
    {
        if (!payload.TryGetProperty("keys", out JsonElement keys) || keys.ValueKind != JsonValueKind.Array)
        {
            return Invalid("invalid_payload", "Shortcut keys must be an array.");
        }

        int count = keys.GetArrayLength();
        if (count == 0 || count > ProtocolConstants.MaxShortcutKeys)
        {
            return Invalid("invalid_payload", "Shortcut key count is invalid.");
        }

        foreach (JsonElement key in keys.EnumerateArray())
        {
            if (key.ValueKind != JsonValueKind.String || !ProtocolConstants.ShortcutKeys.Contains(key.GetString() ?? ""))
            {
                return Invalid("invalid_payload", "Shortcut contains an invalid key.");
            }
        }

        return Valid(payload);
    }

    private static ProtocolValidationResult ValidateTextStreamChar(JsonElement payload)
    {
        return TryGetString(payload, "streamId", out string? streamId) &&
            IsSafeTextStreamId(streamId) &&
            TryGetInteger(payload, "seq", out int seq) &&
            IsValidTextStreamSequence(seq) &&
            TryGetString(payload, "text", out string? text) &&
            IsSafeTextStreamCharacter(text)
                ? Valid(payload)
                : Invalid("invalid_payload", "Text stream character payload is invalid.");
    }

    private static ProtocolValidationResult ValidateTextStreamChunk(JsonElement payload)
    {
        return TryGetString(payload, "streamId", out string? streamId) &&
            IsSafeTextStreamId(streamId) &&
            TryGetInteger(payload, "seq", out int seq) &&
            IsValidTextStreamSequence(seq) &&
            TryGetString(payload, "text", out string? text) &&
            IsSafeTextStreamChunk(text)
                ? Valid(payload)
                : Invalid("invalid_payload", "Text stream chunk payload is invalid.");
    }

    private static ProtocolValidationResult ValidateTextStreamKey(JsonElement payload)
    {
        return TryGetString(payload, "streamId", out string? streamId) &&
            IsSafeTextStreamId(streamId) &&
            TryGetInteger(payload, "seq", out int seq) &&
            IsValidTextStreamSequence(seq) &&
            TryGetString(payload, "key", out string? key) &&
            ProtocolConstants.KeyboardKeys.Contains(key)
                ? Valid(payload)
                : Invalid("invalid_payload", "Text stream key payload is invalid.");
    }

    private static ProtocolValidationResult ValidateTextStreamClose(JsonElement payload)
    {
        return TryGetString(payload, "streamId", out string? streamId) &&
            IsSafeTextStreamId(streamId) &&
            TryGetInteger(payload, "expectedCount", out int expectedCount) &&
            expectedCount >= 0 &&
            expectedCount <= ProtocolConstants.MaxTextStreamItems
                ? Valid(payload)
                : Invalid("invalid_payload", "Text stream close payload is invalid.");
    }

    private static ProtocolValidationResult ValidateAckResponse(JsonElement value)
    {
        return TryGetNonEmptyString(value, "id", out _) &&
            TryGetBoolean(value, "ok", out bool ok) &&
            ok &&
            HasNull(value, "error")
                ? ProtocolValidationResult.Valid(value)
                : Invalid("invalid_message", "Ack response is malformed.");
    }

    private static ProtocolValidationResult ValidateErrorResponse(JsonElement value)
    {
        if (!HasNull(value, "id") && !TryGetNonEmptyString(value, "id", out _))
        {
            return Invalid("invalid_message", "Error response id must be a string or null.");
        }

        if (!TryGetBoolean(value, "ok", out bool ok) || ok || !TryGetObject(value, "error", out JsonElement error))
        {
            return Invalid("invalid_message", "Error response is malformed.");
        }

        if (!TryGetNonEmptyString(error, "code", out _) || !TryGetNonEmptyString(error, "message", out string? message))
        {
            return Invalid("invalid_message", "Error code and message are required.");
        }

        return message.Length > ProtocolConstants.MaxErrorMessageLength
            ? Invalid("invalid_message", "Error message is too long.")
            : ProtocolValidationResult.Valid(value);
    }

    private static ProtocolValidationResult ValidatePairingCompleteResponse(JsonElement value)
    {
        if (!TryGetNonEmptyString(value, "id", out _) ||
            !TryGetBoolean(value, "ok", out bool ok) ||
            !ok ||
            !HasNull(value, "error") ||
            !TryGetObject(value, "payload", out JsonElement payload))
        {
            return Invalid("invalid_message", "Pairing response is malformed.");
        }

        return TryGetNonEmptyString(payload, "desktopId", out _) &&
            TryGetNonEmptyString(payload, "deviceId", out _) &&
            TryGetNonEmptyString(payload, "token", out _)
                ? ProtocolValidationResult.Valid(value)
                : Invalid("invalid_payload", "Pairing response payload is invalid.");
    }

    private static ProtocolValidationResult ValidatePointerProfileResponse(JsonElement value)
    {
        if (!TryGetNonEmptyString(value, "id", out _) ||
            !TryGetBoolean(value, "ok", out bool ok) ||
            !ok ||
            !HasNull(value, "error") ||
            !TryGetObject(value, "payload", out JsonElement payload))
        {
            return Invalid("invalid_message", "Pointer profile response is malformed.");
        }

        return ValidatePointerProfilePayload(payload).Ok
            ? ProtocolValidationResult.Valid(value)
            : Invalid("invalid_payload", "Pointer profile payload is invalid.");
    }

    private static ProtocolValidationResult ValidatePointerProfilePayload(JsonElement payload)
    {
        if (!TryGetNonEmptyString(payload, "displayId", out _)) return Invalid("invalid_payload", "Display id is required.");
        if (!TryGetPositiveFiniteNumber(payload, "scaleFactor", out _)) return Invalid("invalid_payload", "Scale factor is invalid.");
        if (!TryGetObject(payload, "bounds", out JsonElement bounds) || !IsFiniteBounds(bounds)) return Invalid("invalid_payload", "Bounds are invalid.");
        if (!TryGetPositiveFiniteNumber(payload, "maxDelta", out double maxDelta) || maxDelta > ProtocolConstants.MaxPointerDelta) return Invalid("invalid_payload", "Max delta is invalid.");
        if (!TryGetObject(payload, "recommendedDeltas", out JsonElement recommendedDeltas)) return Invalid("invalid_payload", "Recommended deltas are required.");

        foreach (string key in new[] { "small", "medium", "large" })
        {
            if (!TryGetPositiveFiniteNumber(recommendedDeltas, key, out double delta) || delta > ProtocolConstants.MaxPointerDelta)
            {
                return Invalid("invalid_payload", "Recommended delta is invalid.");
            }
        }

        if (payload.TryGetProperty("capabilities", out JsonElement capabilities))
        {
            if (!IsObject(capabilities)) return Invalid("invalid_payload", "Pointer capabilities are invalid.");
            if (capabilities.TryGetProperty("noAckMouseMove", out JsonElement noAckMouseMove) && noAckMouseMove.ValueKind is not JsonValueKind.True and not JsonValueKind.False)
            {
                return Invalid("invalid_payload", "No-ack mouse move capability is invalid.");
            }

            if (!ValidateCommandArrayCapability(capabilities, "noAckCommands", ProtocolConstants.NoAckControlCommandTypes))
            {
                return Invalid("invalid_payload", "No-ack commands capability is invalid.");
            }

            if (!ValidateCommandArrayCapability(capabilities, "supportedCommands", ProtocolConstants.CommandTypes))
            {
                return Invalid("invalid_payload", "Supported commands capability is invalid.");
            }
        }

        return Valid(payload);
    }

    private static bool ValidateCommandArrayCapability(JsonElement capabilities, string propertyName, IReadOnlySet<string> allowed)
    {
        if (!capabilities.TryGetProperty(propertyName, out JsonElement commands)) return true;
        if (commands.ValueKind != JsonValueKind.Array) return false;
        return commands.EnumerateArray().All(command => command.ValueKind == JsonValueKind.String && allowed.Contains(command.GetString() ?? ""));
    }

    private static bool IsValidResponseMode(JsonElement envelope, string type)
    {
        if (!envelope.TryGetProperty("responseMode", out JsonElement responseMode)) return true;
        if (responseMode.ValueKind != JsonValueKind.String) return false;

        string? mode = responseMode.GetString();
        if (mode is null || !ProtocolConstants.CommandResponseModes.Contains(mode)) return false;
        return mode != "none" || ProtocolConstants.NoAckControlCommandTypes.Contains(type);
    }

    private static ProtocolValidationResult ValidateBoundedNumbers(JsonElement payload, string[] keys, double maxAbsValue)
    {
        foreach (string key in keys)
        {
            if (!TryGetFiniteNumber(payload, key, out double value) || Math.Abs(value) > maxAbsValue)
            {
                return Invalid("invalid_payload", $"{key} is invalid.");
            }
        }

        return Valid(payload);
    }

    private static bool IsFiniteBounds(JsonElement value)
    {
        return TryGetFiniteNumber(value, "x", out _) &&
            TryGetFiniteNumber(value, "y", out _) &&
            TryGetPositiveFiniteNumber(value, "width", out _) &&
            TryGetPositiveFiniteNumber(value, "height", out _);
    }

    private static bool IsSafeTextPayload(string value)
    {
        return value.Length <= ProtocolConstants.MaxTextLength && !ContainsDisallowedControlCharacter(value);
    }

    private static bool IsSafeTextStreamId(string value)
    {
        return value.Length > 0 &&
            value.Length <= ProtocolConstants.MaxTextStreamIdLength &&
            TextStreamIdRegex().IsMatch(value);
    }

    private static bool IsValidTextStreamSequence(int value)
    {
        return value >= 0 && value < ProtocolConstants.MaxTextStreamItems;
    }

    private static bool IsSafeTextStreamCharacter(string value)
    {
        System.Text.Rune[] runes = value.EnumerateRunes().ToArray();
        if (runes.Length != 1) return false;
        int code = runes[0].Value;
        return !(code <= 0x1f || code is >= 0x7f and <= 0x9f);
    }

    private static bool IsSafeTextStreamChunk(string value)
    {
        return value.Length > 0 &&
            value.Length <= ProtocolConstants.MaxTextStreamChunkLength &&
            !ContainsDisallowedControlCharacter(value);
    }

    private static bool ContainsDisallowedControlCharacter(string value)
    {
        foreach (char character in value)
        {
            int code = character;
            if (code >= 0x00 && code <= 0x1f && character is not ('\t' or '\n' or '\r')) return true;
            if (code is >= 0x7f and <= 0x9f) return true;
        }

        return false;
    }

    private static bool HasProtocolVersion(JsonElement value)
    {
        return value.TryGetProperty("version", out JsonElement version) &&
            version.ValueKind == JsonValueKind.Number &&
            version.TryGetInt32(out int protocolVersion) &&
            protocolVersion == ProtocolConstants.ProtocolVersion;
    }

    private static bool IsObject(JsonElement value)
    {
        return value.ValueKind == JsonValueKind.Object;
    }

    private static bool TryGetObject(JsonElement value, string propertyName, out JsonElement property)
    {
        return value.TryGetProperty(propertyName, out property) && property.ValueKind == JsonValueKind.Object;
    }

    private static bool TryGetString(JsonElement value, string propertyName, out string result)
    {
        result = "";
        if (!value.TryGetProperty(propertyName, out JsonElement property) || property.ValueKind != JsonValueKind.String)
        {
            return false;
        }

        result = property.GetString() ?? "";
        return true;
    }

    private static bool TryGetNonEmptyString(JsonElement value, string propertyName, out string result)
    {
        return TryGetString(value, propertyName, out result) && result.Length > 0;
    }

    private static bool TryGetBoolean(JsonElement value, string propertyName, out bool result)
    {
        result = false;
        if (!value.TryGetProperty(propertyName, out JsonElement property))
        {
            return false;
        }

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

    private static bool TryGetFiniteNumber(JsonElement value, string propertyName, out double result)
    {
        result = 0;
        if (!value.TryGetProperty(propertyName, out JsonElement property) || property.ValueKind != JsonValueKind.Number)
        {
            return false;
        }

        return property.TryGetDouble(out result) && double.IsFinite(result);
    }

    private static bool TryGetPositiveFiniteNumber(JsonElement value, string propertyName, out double result)
    {
        return TryGetFiniteNumber(value, propertyName, out result) && result > 0;
    }

    private static bool TryGetInteger(JsonElement value, string propertyName, out int result)
    {
        result = 0;
        return value.TryGetProperty(propertyName, out JsonElement property) &&
            property.ValueKind == JsonValueKind.Number &&
            property.TryGetInt32(out result);
    }

    private static bool HasNull(JsonElement value, string propertyName)
    {
        return value.TryGetProperty(propertyName, out JsonElement property) && property.ValueKind == JsonValueKind.Null;
    }

    private static int ObjectPropertyCount(JsonElement value)
    {
        return value.EnumerateObject().Count();
    }

    private static string Truncate(string value, int maxLength)
    {
        return value.Length <= maxLength ? value : value[..maxLength];
    }

    private static ProtocolValidationResult Valid(JsonElement value)
    {
        return ProtocolValidationResult.Valid(value);
    }

    private static ProtocolValidationResult Invalid(string error, string message)
    {
        return ProtocolValidationResult.Invalid(error, message);
    }

    [GeneratedRegex("^[A-Za-z0-9._:-]+$")]
    private static partial Regex TextStreamIdRegex();
}
