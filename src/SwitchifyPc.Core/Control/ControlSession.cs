using System.Text.Json;
using System.Text.Json.Nodes;
using SwitchifyPc.Core.Input;
using SwitchifyPc.Core.Pairing;
using SwitchifyPc.Protocol;

namespace SwitchifyPc.Core.Control;

public sealed record ControlSessionResult(
    string? ResponseJson,
    string? AuthenticatedDeviceId = null,
    bool AuthenticatedDeviceWasPreviouslyUsed = false,
    string? AuthFailureReason = null)
{
    public bool HasResponse => ResponseJson is not null;
    public bool HasAuthenticatedDevice => AuthenticatedDeviceId is not null;

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
    private readonly MouseRepeatController? mouseRepeatController;
    private readonly PointerSpeedController? pointerSpeedController;

    public ControlSession(
        CommandAuthValidator authValidator,
        DesktopCommandExecutor commandExecutor,
        IPointerProfileProvider pointerProfileProvider,
        MouseRepeatController? mouseRepeatController = null,
        PointerSpeedController? pointerSpeedController = null)
    {
        this.authValidator = authValidator;
        this.commandExecutor = commandExecutor;
        this.pointerProfileProvider = pointerProfileProvider;
        this.mouseRepeatController = mouseRepeatController;
        this.pointerSpeedController = pointerSpeedController;
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
            if (TryGetRequestDeviceId(request, out string? failedDeviceId))
            {
                await StopRepeatAsync(failedDeviceId!).ConfigureAwait(false);
            }

            await commandExecutor.ReleaseHeldInputsAsync(cancellationToken).ConfigureAwait(false);
            commandExecutor.EndControlSession();
            return ControlSessionResult.Response(ErrorResponse(RequestIdOrNull(request), auth.Reason ?? "invalid_auth", "Command authentication failed."))
                with { AuthFailureReason = auth.Reason ?? "invalid_auth" };
        }

        if (type == "connection.disconnecting")
        {
            await StopRepeatAsync(auth.DeviceId ?? "").ConfigureAwait(false);
            await commandExecutor.ReleaseHeldInputsAsync(cancellationToken).ConfigureAwait(false);
            commandExecutor.EndControlSession();
            return WithAuth(AckOrNoResponse(request), auth);
        }

        if (type == "pointer.profile")
        {
            return WithAuth(
                ResponseOrNoResponse(request, PointerProfileResponse(request.GetProperty("id").GetString() ?? "", pointerProfileProvider.GetPointerProfile())),
                auth);
        }

        CommandExecutionResult result;
        if (type == "mouse.repeat.start")
        {
            if (mouseRepeatController is null)
            {
                result = CommandExecutionResult.Failure("unsupported_command", "Mouse repeat is not available.");
            }
            else
            {
                result = await mouseRepeatController.StartAsync(auth.DeviceId ?? "", request.GetProperty("payload").GetProperty("command"), cancellationToken).ConfigureAwait(false);
            }
        }
        else if (type == "mouse.repeat.stop")
        {
            await StopRepeatAsync(auth.DeviceId ?? "").ConfigureAwait(false);
            result = CommandExecutionResult.Success;
        }
        else if (type == "pointer.speed.set")
        {
            if (pointerSpeedController is null)
            {
                result = CommandExecutionResult.Failure("unsupported_command", "Pointer speed settings are not available.");
            }
            else
            {
                pointerSpeedController.SetScalePercent(request.GetProperty("payload").GetProperty("scalePercent").GetDouble());
                result = CommandExecutionResult.Success;
            }
        }
        else
        {
            if (type is not ("connection.ping" or "pointer.profile" or "pointer.speed.set"))
            {
                await StopRepeatAsync(auth.DeviceId ?? "").ConfigureAwait(false);
            }

            result = await commandExecutor.ExecuteAsync(auth.Command.Value, cancellationToken).ConfigureAwait(false);
        }

        if (!result.Ok)
        {
            return WithAuth(
                ControlSessionResult.Response(ErrorResponse(
                    RequestIdOrNull(request),
                    result.Code ?? "command_failed",
                    result.Message ?? "Command failed.")),
                auth);
        }

        return WithAuth(AckOrNoResponse(request), auth);
    }

    public Task StopAllRepeatsAsync() => mouseRepeatController?.StopAllAsync() ?? Task.CompletedTask;

    public async Task EndControlSessionAsync(CancellationToken cancellationToken = default)
    {
        await StopAllRepeatsAsync().ConfigureAwait(false);
        await commandExecutor.ReleaseHeldInputsAsync(cancellationToken).ConfigureAwait(false);
        commandExecutor.EndControlSession();
    }

    private static ControlSessionResult WithAuth(ControlSessionResult result, AuthValidationResult auth)
    {
        return result with
        {
            AuthenticatedDeviceId = auth.DeviceId,
            AuthenticatedDeviceWasPreviouslyUsed = auth.DeviceWasPreviouslyUsed
        };
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

    private Task StopRepeatAsync(string deviceId)
    {
        return string.IsNullOrWhiteSpace(deviceId) || mouseRepeatController is null
            ? Task.CompletedTask
            : mouseRepeatController.StopAsync(deviceId);
    }

    private static bool TryGetRequestDeviceId(JsonElement request, out string? deviceId)
    {
        deviceId = null;
        if (!request.TryGetProperty("deviceId", out JsonElement deviceIdElement) || deviceIdElement.ValueKind != JsonValueKind.String)
        {
            return false;
        }

        deviceId = deviceIdElement.GetString();
        return !string.IsNullOrWhiteSpace(deviceId);
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
                    ["supportedCommands"] = new JsonArray(profile.Capabilities.SupportedCommands.Select(command => JsonValue.Create(command)).ToArray<JsonNode?>()),
                    ["mouseRepeat"] = new JsonObject
                    {
                        ["supported"] = profile.Capabilities.MouseRepeat.Supported,
                        ["enabled"] = profile.Capabilities.MouseRepeat.Enabled,
                        ["intervalMs"] = profile.Capabilities.MouseRepeat.IntervalMs,
                        ["moveIntervalMs"] = profile.Capabilities.MouseRepeat.MoveIntervalMs,
                        ["scrollIntervalMs"] = profile.Capabilities.MouseRepeat.ScrollIntervalMs,
                        ["minIntervalMs"] = profile.Capabilities.MouseRepeat.MinIntervalMs,
                        ["maxIntervalMs"] = profile.Capabilities.MouseRepeat.MaxIntervalMs
                    },
                    ["pointerSpeed"] = new JsonObject
                    {
                        ["supported"] = profile.Capabilities.PointerSpeed.Supported,
                        ["setSupported"] = profile.Capabilities.PointerSpeed.SetSupported,
                        ["scalePercent"] = profile.Capabilities.PointerSpeed.ScalePercent,
                        ["minScalePercent"] = profile.Capabilities.PointerSpeed.MinScalePercent,
                        ["maxScalePercent"] = profile.Capabilities.PointerSpeed.MaxScalePercent,
                        ["stepPercent"] = profile.Capabilities.PointerSpeed.StepPercent,
                        ["baseMoveDelta"] = profile.Capabilities.PointerSpeed.BaseMoveDelta,
                        ["effectiveMoveDelta"] = profile.Capabilities.PointerSpeed.EffectiveMoveDelta
                    }
                }
            },
            ["error"] = null
        };
    }
}
