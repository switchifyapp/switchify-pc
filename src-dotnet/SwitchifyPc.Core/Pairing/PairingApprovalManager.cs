namespace SwitchifyPc.Core.Pairing;

public sealed record PendingPairingApproval(
    string RequestId,
    string DeviceId,
    string DeviceName,
    string DesktopId,
    string RequestNonce,
    double RequestedAt,
    double ExpiresAt,
    string? RemoteAddress);

public sealed record PendingPairingApprovalView(
    string RequestId,
    string DeviceName,
    string VerificationCode,
    double RequestedAt,
    double ExpiresAt,
    string? RemoteAddress);

public sealed record CreatePairingApprovalRequestInput(
    string RequestId,
    string DeviceId,
    string DeviceName,
    string DesktopId,
    string RequestNonce,
    string? RemoteAddress);

public sealed record CreatePairingApprovalRequestResult(
    PendingPairingApproval Request,
    string? ReplacedRequestId);

public sealed record PairingApprovalResponseResult(
    bool Ok,
    string? Reason = null,
    string? DesktopId = null,
    string? DeviceId = null,
    string? Token = null)
{
    public static PairingApprovalResponseResult Accepted(string desktopId, string deviceId, string token) =>
        new(true, DesktopId: desktopId, DeviceId: deviceId, Token: token);

    public static PairingApprovalResponseResult Rejected() => new(true);

    public static PairingApprovalResponseResult Failure(string reason) => new(false, Reason: reason);
}

public sealed class PairingApprovalManager
{
    public const int PairingApprovalRequestTtlMs = 2 * 60 * 1000;

    private readonly IPairingStore pairingStore;
    private readonly Func<double> now;
    private readonly Func<string> createToken;
    private readonly Dictionary<string, PendingPairingApproval> pendingRequests = new(StringComparer.Ordinal);

    public PairingApprovalManager(
        IPairingStore pairingStore,
        Func<double>? now = null,
        Func<string>? createToken = null)
    {
        this.pairingStore = pairingStore;
        this.now = now ?? (() => DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());
        this.createToken = createToken ?? (() => PairingToken.CreateToken());
    }

    public CreatePairingApprovalRequestResult CreateRequest(CreatePairingApprovalRequestInput input)
    {
        ExpirePendingRequests();

        string? replacedRequestId = FindRequestIdByDeviceId(input.DeviceId);
        if (replacedRequestId is not null)
        {
            pendingRequests.Remove(replacedRequestId);
        }

        double requestedAt = now();
        PendingPairingApproval request = new(
            input.RequestId,
            input.DeviceId,
            input.DeviceName,
            input.DesktopId,
            input.RequestNonce,
            requestedAt,
            requestedAt + PairingApprovalRequestTtlMs,
            input.RemoteAddress);
        pendingRequests[request.RequestId] = request;
        return new CreatePairingApprovalRequestResult(request, replacedRequestId);
    }

    public IReadOnlyList<PendingPairingApproval> ListPendingRequests()
    {
        ExpirePendingRequests();
        return pendingRequests.Values
            .OrderByDescending(request => request.RequestedAt)
            .Select(request => request with { })
            .ToArray();
    }

    public IReadOnlyList<PendingPairingApprovalView> ListPendingRequestViews()
    {
        return ListPendingRequests()
            .Select(request => new PendingPairingApprovalView(
                request.RequestId,
                request.DeviceName,
                PairingVerificationCode.Create(request.DesktopId, request.DeviceId, request.RequestNonce),
                request.RequestedAt,
                request.ExpiresAt,
                request.RemoteAddress))
            .ToArray();
    }

    public PendingPairingApproval? GetRequest(string requestId)
    {
        ExpirePendingRequests();
        return pendingRequests.TryGetValue(requestId, out PendingPairingApproval? request)
            ? request with { }
            : null;
    }

    public async Task<PairingApprovalResponseResult> AcceptAsync(string requestId, CancellationToken cancellationToken = default)
    {
        PendingPairingApproval? request = GetRequest(requestId);
        if (request is null)
        {
            return PairingApprovalResponseResult.Failure("pairing_request_not_found");
        }

        PairingState state = await pairingStore.LoadAsync(cancellationToken).ConfigureAwait(false);
        string token = createToken();
        await pairingStore.SaveAsync(
            PairingStateHelpers.UpsertPairedDevice(
                state,
                new PairedDevice(request.DeviceId, request.DeviceName, token, now(), null)),
            cancellationToken).ConfigureAwait(false);
        pendingRequests.Remove(requestId);
        return PairingApprovalResponseResult.Accepted(state.DesktopId, request.DeviceId, token);
    }

    public PairingApprovalResponseResult Reject(string requestId)
    {
        ExpirePendingRequests();
        return pendingRequests.Remove(requestId)
            ? PairingApprovalResponseResult.Rejected()
            : PairingApprovalResponseResult.Failure("pairing_request_not_found");
    }

    public IReadOnlyList<PendingPairingApproval> ExpirePendingRequests()
    {
        double currentTime = now();
        List<PendingPairingApproval> expired = [];
        foreach ((string requestId, PendingPairingApproval request) in pendingRequests.ToArray())
        {
            if (request.ExpiresAt <= currentTime)
            {
                pendingRequests.Remove(requestId);
                expired.Add(request with { });
            }
        }

        return expired;
    }

    private string? FindRequestIdByDeviceId(string deviceId)
    {
        foreach ((string requestId, PendingPairingApproval request) in pendingRequests)
        {
            if (request.DeviceId == deviceId)
            {
                return requestId;
            }
        }

        return null;
    }
}
