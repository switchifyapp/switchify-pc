using System.Globalization;
using System.Security.Cryptography;
using System.Text;
using System.Text.Encodings.Web;
using System.Text.Json;
using SwitchifyPc.Protocol;

namespace SwitchifyPc.Core.Pairing;

public static class CommandAuth
{
    public const int CommandTimestampToleranceMs = 2 * 60 * 1000;
    public const int ReplayCacheTtlMs = CommandTimestampToleranceMs;

    public static string CreateCommandAuthProof(JsonElement command, string token)
    {
        using HMACSHA256 hmac = new(Encoding.UTF8.GetBytes(token));
        byte[] hash = hmac.ComputeHash(Encoding.UTF8.GetBytes(CanonicalCommandString(command)));
        return Base64UrlEncode(hash);
    }

    internal static string CanonicalCommandString(JsonElement command)
    {
        return string.Join(
            "\n",
            RequiredProperty(command, "version").GetInt32().ToString(CultureInfo.InvariantCulture),
            RequiredProperty(command, "id").GetString(),
            RequiredProperty(command, "deviceId").GetString(),
            TimestampString(RequiredProperty(command, "timestamp")),
            RequiredProperty(command, "type").GetString(),
            StableStringify(RequiredProperty(command, "payload")),
            CommandResponseMode(command));
    }

    internal static string StableStringify(JsonElement value)
    {
        return value.ValueKind switch
        {
            JsonValueKind.Object => StableObjectStringify(value),
            JsonValueKind.Array => $"[{string.Join(",", value.EnumerateArray().Select(StableStringify))}]",
            JsonValueKind.String => JsonSerializer.Serialize(value.GetString(), CanonicalJsonOptions),
            JsonValueKind.Number => NumberString(value),
            JsonValueKind.True => "true",
            JsonValueKind.False => "false",
            JsonValueKind.Null => "null",
            _ => throw new InvalidDataException("Unsupported JSON value.")
        };
    }

    internal static bool SafeEquals(string actual, string expected)
    {
        byte[] actualBytes = Encoding.UTF8.GetBytes(actual);
        byte[] expectedBytes = Encoding.UTF8.GetBytes(expected);
        return actualBytes.Length == expectedBytes.Length && CryptographicOperations.FixedTimeEquals(actualBytes, expectedBytes);
    }

    private static string StableObjectStringify(JsonElement value)
    {
        return "{" + string.Join(
            ",",
            value.EnumerateObject()
                .OrderBy(property => property.Name, StringComparer.Ordinal)
                .Select(property => $"{JsonSerializer.Serialize(property.Name, CanonicalJsonOptions)}:{StableStringify(property.Value)}")) + "}";
    }

    private static string CommandResponseMode(JsonElement command)
    {
        return command.TryGetProperty("responseMode", out JsonElement responseMode) && responseMode.ValueKind == JsonValueKind.String
            ? responseMode.GetString() ?? "ack"
            : "ack";
    }

    private static string TimestampString(JsonElement value)
    {
        if (value.TryGetInt64(out long integer))
        {
            return integer.ToString(CultureInfo.InvariantCulture);
        }

        return NumberString(value);
    }

    private static string NumberString(JsonElement value)
    {
        if (value.TryGetInt64(out long integer))
        {
            return integer.ToString(CultureInfo.InvariantCulture);
        }

        return value.GetDouble().ToString("G17", CultureInfo.InvariantCulture);
    }

    private static JsonElement RequiredProperty(JsonElement value, string propertyName)
    {
        return value.GetProperty(propertyName);
    }

    private static string Base64UrlEncode(byte[] value)
    {
        return Convert.ToBase64String(value)
            .TrimEnd('=')
            .Replace("+", "-", StringComparison.Ordinal)
            .Replace("/", "_", StringComparison.Ordinal);
    }

    private static readonly JsonSerializerOptions CanonicalJsonOptions = new()
    {
        Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping
    };
}

public sealed record AuthValidationResult
{
    private AuthValidationResult(bool ok, JsonElement? command, string? reason)
    {
        Ok = ok;
        Command = command;
        Reason = reason;
    }

    public bool Ok { get; }
    public JsonElement? Command { get; }
    public string? Reason { get; }

    public static AuthValidationResult Valid(JsonElement command)
    {
        return new AuthValidationResult(true, command.Clone(), null);
    }

    public static AuthValidationResult Invalid(string reason)
    {
        return new AuthValidationResult(false, null, reason);
    }
}

public sealed class CommandAuthValidator
{
    private readonly IPairingStore store;
    private readonly Func<double> now;
    private readonly Dictionary<string, double> seenRequestIds = new(StringComparer.Ordinal);

    public CommandAuthValidator(IPairingStore store, Func<double>? now = null)
    {
        this.store = store;
        this.now = now ?? (() => DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());
    }

    public async Task<AuthValidationResult> ValidateAsync(JsonElement value, CancellationToken cancellationToken = default)
    {
        ProtocolValidationResult parsed = ProtocolValidator.ValidateProtocolRequest(value);
        if (!parsed.Ok || !IsCommandRequest(value))
        {
            return AuthValidationResult.Invalid("invalid_payload");
        }

        PairingState state = await store.LoadAsync(cancellationToken);
        string deviceId = value.GetProperty("deviceId").GetString() ?? "";
        PairedDevice? pairedDevice = PairingStateHelpers.FindPairedDevice(state, deviceId);
        if (pairedDevice is null)
        {
            return AuthValidationResult.Invalid("unknown_device");
        }

        double timestamp = value.GetProperty("timestamp").GetDouble();
        if (Math.Abs(now() - timestamp) > CommandAuth.CommandTimestampToleranceMs)
        {
            return AuthValidationResult.Invalid("expired_timestamp");
        }

        PruneReplayCache();
        string replayKey = ReplayKey(value);
        if (seenRequestIds.ContainsKey(replayKey))
        {
            return AuthValidationResult.Invalid("duplicate_request");
        }

        string auth = value.GetProperty("auth").GetString() ?? "";
        string expectedAuth = CommandAuth.CreateCommandAuthProof(value, pairedDevice.Token);
        if (!CommandAuth.SafeEquals(auth, expectedAuth))
        {
            return AuthValidationResult.Invalid("invalid_auth");
        }

        seenRequestIds[replayKey] = now() + CommandAuth.ReplayCacheTtlMs;
        await store.SaveAsync(UpdateLastSeen(state, deviceId, now()), cancellationToken);
        return AuthValidationResult.Valid(value);
    }

    private void PruneReplayCache()
    {
        double currentTime = now();
        foreach (string key in seenRequestIds.Where(entry => entry.Value <= currentTime).Select(entry => entry.Key).ToArray())
        {
            seenRequestIds.Remove(key);
        }
    }

    private static bool IsCommandRequest(JsonElement value)
    {
        return value.ValueKind == JsonValueKind.Object &&
            value.TryGetProperty("auth", out _) &&
            value.TryGetProperty("deviceId", out _) &&
            value.TryGetProperty("timestamp", out _);
    }

    private static string ReplayKey(JsonElement command)
    {
        return $"{command.GetProperty("deviceId").GetString()}:{command.GetProperty("id").GetString()}";
    }

    private static PairingState UpdateLastSeen(PairingState state, string deviceId, double lastSeenAt)
    {
        return state with
        {
            PairedDevices = state.PairedDevices
                .Select(device => device.DeviceId == deviceId ? device with { LastSeenAt = lastSeenAt } : device)
                .ToArray()
        };
    }
}
