using System.Text.Json;
using System.Text.Json.Nodes;
using SwitchifyPc.Core.Input;
using SwitchifyPc.Core.Pairing;
using SwitchifyPc.Protocol;

namespace SwitchifyPc.Core.Control;

public sealed record ControlSessionResult(string? ResponseJson)
{
    public bool HasResponse => ResponseJson is not null;

    public static ControlSessionResult NoResponse { get; } = new((string?)null);

    public static ControlSessionResult Response(JsonObject response)
    {
        return new ControlSessionResult(response.ToJsonString(JsonOptions));
    }

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase
    };
}

public interface IPointerProfileProvider
{
    PointerMovementProfile GetPointerProfile();
}

public sealed class FixedPointerProfileProvider(PointerMovementProfile profile) : IPointerProfileProvider
{
    public PointerMovementProfile GetPointerProfile() => profile;
}

public sealed class ControlSession
{
    private readonly CommandAuthValidator authValidator;
    private readonly DesktopCommandExecutor commandExecutor;
    private readonly IPointerProfileProvider pointerProfileProvider;

    public ControlSession(
        CommandAuthValidator authValidator,
        DesktopCommandExecutor commandExecutor,
        IPointerProfileProvider pointerProfileProvider)
    {
        this.authValidator = authValidator;
        this.commandExecutor = commandExecutor;
        this.pointerProfileProvider = pointerProfileProvider;
    }

    public async Task<ControlSessionResult> ProcessMessageAsync(string rawMessage, CancellationToken cancellationToken = default)
    {
        using JsonDocument? document = TryParse(rawMessage, out JsonObject? parseError);
        if (document is null)
        {
            return ControlSessionResult.Response(parseError!);
        }

        JsonElement request = document.RootElement;
        ProtocolValidationResult validation = ProtocolValidator.ValidateProtocolRequest(request);
        if (!validation.Ok)
        {
            return ControlSessionResult.Response(ErrorResponse(RequestIdOrNull(request), validation.Error ?? "invalid_message", validation.Message ?? "Message is invalid."));
        }

        string type = request.GetProperty("type").GetString() ?? "";
        if (!ProtocolConstants.CommandTypes.Contains(type))
        {
            return ControlSessionResult.Response(ErrorResponse(RequestIdOrNull(request), "unsupported_command", "Only authenticated control commands are supported by this session."));
        }

        AuthValidationResult auth = await authValidator.ValidateAsync(request, cancellationToken).ConfigureAwait(false);
        if (!auth.Ok || auth.Command is null)
        {
            return ControlSessionResult.Response(ErrorResponse(RequestIdOrNull(request), auth.Reason ?? "invalid_auth", "Command authentication failed."));
        }

        if (type == "connection.disconnecting")
        {
            await commandExecutor.ReleaseHeldMouseButtonsAsync(cancellationToken).ConfigureAwait(false);
            commandExecutor.EndControlSession();
            return AckOrNoResponse(request);
        }

        if (type == "pointer.profile")
        {
            return ResponseOrNoResponse(request, PointerProfileResponse(request.GetProperty("id").GetString() ?? "", pointerProfileProvider.GetPointerProfile()));
        }

        CommandExecutionResult result = await commandExecutor.ExecuteAsync(auth.Command.Value, cancellationToken).ConfigureAwait(false);
        if (!result.Ok)
        {
            return ControlSessionResult.Response(ErrorResponse(
                RequestIdOrNull(request),
                result.Code ?? "command_failed",
                result.Message ?? "Command failed."));
        }

        return AckOrNoResponse(request);
    }

    private static JsonDocument? TryParse(string rawMessage, out JsonObject? error)
    {
        try
        {
            error = null;
            return JsonDocument.Parse(rawMessage);
        }
        catch (JsonException)
        {
            error = ErrorResponse(null, "invalid_json", "Message must be valid JSON.");
            return null;
        }
    }

    private static ControlSessionResult AckOrNoResponse(JsonElement request)
    {
        return ShouldSuppressResponse(request)
            ? ControlSessionResult.NoResponse
            : ControlSessionResult.Response(ProtocolValidator.CreateAckResponse(request.GetProperty("id").GetString() ?? ""));
    }

    private static ControlSessionResult ResponseOrNoResponse(JsonElement request, JsonObject response)
    {
        return ShouldSuppressResponse(request)
            ? ControlSessionResult.NoResponse
            : ControlSessionResult.Response(response);
    }

    private static bool ShouldSuppressResponse(JsonElement request)
    {
        return request.TryGetProperty("responseMode", out JsonElement responseMode) &&
            responseMode.ValueKind == JsonValueKind.String &&
            responseMode.GetString() == "none";
    }

    private static JsonObject ErrorResponse(string? id, string code, string message)
    {
        return ProtocolValidator.CreateErrorResponse(id, code, message);
    }

    private static string? RequestIdOrNull(JsonElement request)
    {
        return request.ValueKind == JsonValueKind.Object &&
            request.TryGetProperty("id", out JsonElement id) &&
            id.ValueKind == JsonValueKind.String
                ? id.GetString()
                : null;
    }

    private static JsonObject PointerProfileResponse(string id, PointerMovementProfile profile)
    {
        return new JsonObject
        {
            ["version"] = ProtocolConstants.ProtocolVersion,
            ["id"] = id,
            ["type"] = "pointer.profile",
            ["ok"] = true,
            ["payload"] = new JsonObject
            {
                ["displayId"] = profile.DisplayId,
                ["scaleFactor"] = profile.ScaleFactor,
                ["bounds"] = new JsonObject
                {
                    ["x"] = profile.Bounds.X,
                    ["y"] = profile.Bounds.Y,
                    ["width"] = profile.Bounds.Width,
                    ["height"] = profile.Bounds.Height
                },
                ["maxDelta"] = profile.MaxDelta,
                ["recommendedDeltas"] = new JsonObject
                {
                    ["small"] = profile.RecommendedDeltas.Small,
                    ["medium"] = profile.RecommendedDeltas.Medium,
                    ["large"] = profile.RecommendedDeltas.Large
                },
                ["capabilities"] = new JsonObject
                {
                    ["noAckMouseMove"] = profile.Capabilities.NoAckMouseMove,
                    ["noAckCommands"] = new JsonArray(profile.Capabilities.NoAckCommands.Select(command => JsonValue.Create(command)).ToArray<JsonNode?>()),
                    ["supportedCommands"] = new JsonArray(profile.Capabilities.SupportedCommands.Select(command => JsonValue.Create(command)).ToArray<JsonNode?>())
                }
            },
            ["error"] = null
        };
    }
}
