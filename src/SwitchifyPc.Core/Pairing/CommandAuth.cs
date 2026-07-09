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
        return CreateCommandAuthProof(command, token, JsonSlashEscapingMode.AndroidHtmlSafe);
    }

    internal static IReadOnlyList<string> CreateAcceptedCommandAuthProofs(JsonElement command, string token)
    {
        string htmlSafeProof = CreateCommandAuthProof(command, token, JsonSlashEscapingMode.AndroidHtmlSafe);
        string allSlashesProof = CreateCommandAuthProof(command, token, JsonSlashEscapingMode.AllSlashes);
        return htmlSafeProof == allSlashesProof
            ? [htmlSafeProof]
            : [htmlSafeProof, allSlashesProof];
    }

    private static string CreateCommandAuthProof(JsonElement command, string token, JsonSlashEscapingMode slashEscapingMode)
    {
        using HMACSHA256 hmac = new(Encoding.UTF8.GetBytes(token));
        byte[] hash = hmac.ComputeHash(Encoding.UTF8.GetBytes(CanonicalCommandString(command, slashEscapingMode)));
        return Base64UrlEncode(hash);
    }

    internal static string CanonicalCommandString(JsonElement command)
    {
        return CanonicalCommandString(command, JsonSlashEscapingMode.AndroidHtmlSafe);
    }

    private static string CanonicalCommandString(JsonElement command, JsonSlashEscapingMode slashEscapingMode)
    {
        return string.Join(
            "\n",
            RequiredProperty(command, "version").GetInt32().ToString(CultureInfo.InvariantCulture),
            RequiredProperty(command, "id").GetString(),
            RequiredProperty(command, "deviceId").GetString(),
            TimestampString(RequiredProperty(command, "timestamp")),
            RequiredProperty(command, "type").GetString(),
            StableStringify(RequiredProperty(command, "payload"), slashEscapingMode),
            CommandResponseMode(command));
    }

    internal static string StableStringify(JsonElement value)
    {
        return StableStringify(value, JsonSlashEscapingMode.AndroidHtmlSafe);
    }

    private static string StableStringify(JsonElement value, JsonSlashEscapingMode slashEscapingMode)
    {
        return value.ValueKind switch
        {
            JsonValueKind.Object => StableObjectStringify(value, slashEscapingMode),
            JsonValueKind.Array => $"[{string.Join(",", value.EnumerateArray().Select(item => StableStringify(item, slashEscapingMode)))}]",
            JsonValueKind.String => AndroidCompatibleJsonQuote(value.GetString() ?? "", slashEscapingMode),
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

    private static string StableObjectStringify(JsonElement value, JsonSlashEscapingMode slashEscapingMode)
    {
        return "{" + string.Join(
            ",",
            value.EnumerateObject()
                .OrderBy(property => property.Name, StringComparer.Ordinal)
                .Select(property => $"{AndroidCompatibleJsonQuote(property.Name, slashEscapingMode)}:{StableStringify(property.Value, slashEscapingMode)}")) + "}";
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

    private static string AndroidCompatibleJsonQuote(string value, JsonSlashEscapingMode slashEscapingMode)
    {
        string quoted = JsonSerializer.Serialize(value, CanonicalJsonOptions);
        return slashEscapingMode switch
        {
            JsonSlashEscapingMode.AndroidHtmlSafe => quoted.Replace("</", "<\\/", StringComparison.Ordinal),
            JsonSlashEscapingMode.AllSlashes => quoted.Replace("/", "\\/", StringComparison.Ordinal),
            _ => quoted
        };
    }

    private static readonly JsonSerializerOptions CanonicalJsonOptions = new()
    {
        Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping
    };

    private enum JsonSlashEscapingMode
    {
        AndroidHtmlSafe,
        AllSlashes
    }
}

public sealed record AuthValidationResult
{
    private AuthValidationResult(bool ok, JsonElement? command, string? reason, string? deviceId, bool deviceWasPreviouslyUsed)
    {
        Ok = ok;
        Command = command;
        Reason = reason;
        DeviceId = deviceId;
        DeviceWasPreviouslyUsed = deviceWasPreviouslyUsed;
    }

    public bool Ok { get; }
    public JsonElement? Command { get; }
    public string? Reason { get; }
    public string? DeviceId { get; }
    public bool DeviceWasPreviouslyUsed { get; }

    public static AuthValidationResult Valid(JsonElement command, string deviceId, bool deviceWasPreviouslyUsed)
    {
        return new AuthValidationResult(true, command.Clone(), null, deviceId, deviceWasPreviouslyUsed);
    }

    public static AuthValidationResult Invalid(string reason)
    {
        return new AuthValidationResult(false, null, reason, null, false);
    }
}

public sealed class CommandAuthValidator
{
    private const double LastSeenPersistIntervalMs = 60_000;

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

        double currentTime = now();
        double timestamp = value.GetProperty("timestamp").GetDouble();
        if (Math.Abs(currentTime - timestamp) > CommandAuth.CommandTimestampToleranceMs)
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
        if (!CommandAuth.CreateAcceptedCommandAuthProofs(value, pairedDevice.Token).Any(expectedAuth => CommandAuth.SafeEquals(auth, expectedAuth)))
        {
            return AuthValidationResult.Invalid("invalid_auth");
        }

        bool deviceWasPreviouslyUsed = pairedDevice.LastSeenAt is not null;
        seenRequestIds[replayKey] = currentTime + CommandAuth.ReplayCacheTtlMs;
        if (ShouldPersistLastSeen(pairedDevice, currentTime))
        {
            await store.SaveAsync(UpdateLastSeen(state, deviceId, currentTime), cancellationToken);
        }

        return AuthValidationResult.Valid(value, deviceId, deviceWasPreviouslyUsed);
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

    private static bool ShouldPersistLastSeen(PairedDevice pairedDevice, double currentTime)
    {
        return pairedDevice.LastSeenAt is null ||
            currentTime - pairedDevice.LastSeenAt.Value >= LastSeenPersistIntervalMs;
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
