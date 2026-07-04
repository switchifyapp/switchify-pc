using System.Text.Json;
using SwitchifyPc.Core.Pairing;
using SwitchifyPc.Protocol;

namespace SwitchifyPc.Core.Control;

public sealed record RemoteSessionOutgoingMessage(string ConnectionId, string ResponseJson, bool CloseConnection = false);

public sealed record RemoteSessionResult(
    IReadOnlyList<RemoteSessionOutgoingMessage> OutgoingMessages,
    string? AuthenticatedConnectionId = null,
    string? AuthenticatedDeviceId = null,
    string? AuthFailureReason = null)
{
    public static RemoteSessionResult None { get; } = new([]);

    public static RemoteSessionResult One(RemoteSessionOutgoingMessage message) => new([message]);
}

public sealed class RemoteControlSession
{
    private readonly PairingManager pairingManager;
    private readonly PairingApprovalManager pairingApprovalManager;
    private readonly ControlSession commandSession;
    private readonly Action? onPendingPairingRequestsChanged;
    private readonly Dictionary<string, PendingConnection> pendingConnectionsByRequestId = new(StringComparer.Ordinal);

    public RemoteControlSession(
        PairingManager pairingManager,
        PairingApprovalManager pairingApprovalManager,
        ControlSession commandSession,
        Action? onPendingPairingRequestsChanged = null)
    {
        this.pairingManager = pairingManager;
        this.pairingApprovalManager = pairingApprovalManager;
        this.commandSession = commandSession;
        this.onPendingPairingRequestsChanged = onPendingPairingRequestsChanged;
    }

    public async Task<RemoteSessionResult> ProcessMessageAsync(
        string connectionId,
        string rawMessage,
        string? remoteAddress = null,
        CancellationToken cancellationToken = default)
    {
        using JsonDocument? document = TryParse(rawMessage, out string? invalidJsonResponse);
        if (document is null)
        {
            return RemoteSessionResult.One(new RemoteSessionOutgoingMessage(connectionId, invalidJsonResponse!));
        }

        JsonElement request = document.RootElement;
        ProtocolValidationResult validation = ProtocolValidator.ValidateProtocolRequest(request);
        if (!validation.Ok)
        {
            return RemoteSessionResult.One(new RemoteSessionOutgoingMessage(
                connectionId,
                ErrorResponse(RequestIdOrNull(request), validation.Error ?? "invalid_message", validation.Message ?? "Message is invalid.")));
        }

        string type = request.GetProperty("type").GetString() ?? "";
        if (type == "pairing.request")
        {
            return await HandlePairingRequestAsync(connectionId, request, remoteAddress, cancellationToken).ConfigureAwait(false);
        }

        ControlSessionResult commandResult = await commandSession.ProcessMessageAsync(rawMessage, cancellationToken).ConfigureAwait(false);
        return new RemoteSessionResult(
            commandResult.HasResponse
                ? [new RemoteSessionOutgoingMessage(connectionId, commandResult.ResponseJson!)]
                : [],
            commandResult.HasAuthenticatedDevice ? connectionId : null,
            commandResult.AuthenticatedDeviceId,
            commandResult.AuthFailureReason);
    }

    public async Task<RemoteSessionResult> AcceptPairingRequestAsync(
        string requestId,
        CancellationToken cancellationToken = default)
    {
        PairingApprovalResponseResult result = await pairingApprovalManager.AcceptAsync(requestId, cancellationToken).ConfigureAwait(false);
        PendingConnection? pendingConnection = ClearPendingConnection(requestId);
        onPendingPairingRequestsChanged?.Invoke();

        if (!result.Ok || pendingConnection is null)
        {
            return RemoteSessionResult.None;
        }

        return RemoteSessionResult.One(new RemoteSessionOutgoingMessage(
            pendingConnection.ConnectionId,
            ProtocolValidator.CreatePairingCompleteResponse(
                requestId,
                result.DesktopId ?? "",
                result.DeviceId ?? "",
                result.Token ?? "").ToJsonString()));
    }

    public RemoteSessionResult RejectPairingRequest(string requestId)
    {
        PairingApprovalResponseResult result = pairingApprovalManager.Reject(requestId);
        PendingConnection? pendingConnection = ClearPendingConnection(requestId);
        onPendingPairingRequestsChanged?.Invoke();

        return result.Ok && pendingConnection is not null
            ? RemoteSessionResult.One(new RemoteSessionOutgoingMessage(
                pendingConnection.ConnectionId,
                ErrorResponse(requestId, "invalid_auth", "pairing_rejected"),
                CloseConnection: true))
            : RemoteSessionResult.None;
    }

    public RemoteSessionResult ExpirePendingPairingRequests()
    {
        List<RemoteSessionOutgoingMessage> outgoingMessages = [];
        foreach (PendingPairingApproval request in pairingApprovalManager.ExpirePendingRequests())
        {
            PendingConnection? pendingConnection = ClearPendingConnection(request.RequestId);
            if (pendingConnection is null) continue;
            outgoingMessages.Add(new RemoteSessionOutgoingMessage(
                pendingConnection.ConnectionId,
                ErrorResponse(request.RequestId, "invalid_auth", "pairing_request_expired"),
                CloseConnection: true));
        }

        if (outgoingMessages.Count > 0)
        {
            onPendingPairingRequestsChanged?.Invoke();
        }

        return new RemoteSessionResult(outgoingMessages);
    }

    public void RemoveConnection(string connectionId)
    {
        _ = commandSession.EndControlSessionAsync();
        string[] requestIds = pendingConnectionsByRequestId
            .Where(entry => entry.Value.ConnectionId == connectionId)
            .Select(entry => entry.Key)
            .ToArray();

        foreach (string requestId in requestIds)
        {
            pendingConnectionsByRequestId.Remove(requestId);
            pairingApprovalManager.Reject(requestId);
        }

        if (requestIds.Length > 0)
        {
            onPendingPairingRequestsChanged?.Invoke();
        }
    }

    private async Task<RemoteSessionResult> HandlePairingRequestAsync(
        string connectionId,
        JsonElement request,
        string? remoteAddress,
        CancellationToken cancellationToken)
    {
        JsonElement payload = request.GetProperty("payload");
        string desktopId = await pairingManager.GetDesktopIdAsync(cancellationToken).ConfigureAwait(false);
        if (payload.GetProperty("desktopId").GetString() != desktopId)
        {
            return RemoteSessionResult.One(new RemoteSessionOutgoingMessage(
                connectionId,
                ErrorResponse(RequestIdOrNull(request), "invalid_auth", "pairing_mismatch")));
        }

        CreatePairingApprovalRequestResult result = pairingApprovalManager.CreateRequest(new CreatePairingApprovalRequestInput(
            RequestId: request.GetProperty("id").GetString() ?? "",
            DeviceId: payload.GetProperty("deviceId").GetString() ?? "",
            DeviceName: payload.GetProperty("deviceName").GetString() ?? "",
            DesktopId: desktopId,
            RequestNonce: payload.GetProperty("requestNonce").GetString() ?? "",
            RemoteAddress: remoteAddress));

        List<RemoteSessionOutgoingMessage> outgoingMessages = [];
        if (result.ReplacedRequestId is not null)
        {
            PendingConnection? replacedConnection = ClearPendingConnection(result.ReplacedRequestId);
            if (replacedConnection is not null)
            {
                outgoingMessages.Add(new RemoteSessionOutgoingMessage(
                    replacedConnection.ConnectionId,
                    ErrorResponse(result.ReplacedRequestId, "invalid_auth", "pairing_request_expired"),
                    CloseConnection: true));
            }
        }

        pendingConnectionsByRequestId[result.Request.RequestId] = new PendingConnection(connectionId);
        onPendingPairingRequestsChanged?.Invoke();
        return new RemoteSessionResult(outgoingMessages);
    }

    private PendingConnection? ClearPendingConnection(string requestId)
    {
        if (!pendingConnectionsByRequestId.Remove(requestId, out PendingConnection? pendingConnection))
        {
            return null;
        }

        return pendingConnection;
    }

    private static JsonDocument? TryParse(string rawMessage, out string? errorResponse)
    {
        try
        {
            errorResponse = null;
            return JsonDocument.Parse(rawMessage);
        }
        catch (JsonException)
        {
            errorResponse = ErrorResponse(null, "invalid_json", "Message must be valid JSON.");
            return null;
        }
    }

    private static string ErrorResponse(string? id, string code, string message)
    {
        return ProtocolValidator.CreateErrorResponse(id, code, message).ToJsonString();
    }

    private static string? RequestIdOrNull(JsonElement request)
    {
        return request.ValueKind == JsonValueKind.Object &&
            request.TryGetProperty("id", out JsonElement id) &&
            id.ValueKind == JsonValueKind.String
                ? id.GetString()
                : null;
    }

    private sealed record PendingConnection(string ConnectionId);
}
