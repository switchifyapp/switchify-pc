using System.Text.Json;
using SwitchifyPc.Core.Pairing;

namespace SwitchifyPc.Tests;

public sealed class PairingApprovalManagerTests
{
    private const double Now = 1_724_000_000_000;

    [Fact]
    public void CreatesPendingRequestsWithExpiry()
    {
        PairingApprovalManager manager = CreateManager();

        CreatePairingApprovalRequestResult result = manager.CreateRequest(RequestInput());

        Assert.Null(result.ReplacedRequestId);
        Assert.Equal("approval-1", result.Request.RequestId);
        Assert.Equal("android-1", result.Request.DeviceId);
        Assert.Equal("Android smoke", result.Request.DeviceName);
        Assert.Equal(Now, result.Request.RequestedAt);
        Assert.Equal(Now + PairingApprovalManager.PairingApprovalRequestTtlMs, result.Request.ExpiresAt);
    }

    [Fact]
    public void ListsRendererSafePendingRequestViewsWithoutSecrets()
    {
        PairingApprovalManager manager = CreateManager();
        manager.CreateRequest(RequestInput(requestNonce: "secret-nonce"));

        PendingPairingApprovalView view = Assert.Single(manager.ListPendingRequestViews());

        Assert.Equal("approval-1", view.RequestId);
        Assert.Equal("Android smoke", view.DeviceName);
        Assert.Equal(PairingVerificationCode.Create("desktop-1", "android-1", "secret-nonce"), view.VerificationCode);
        Assert.Equal(Now, view.RequestedAt);
        Assert.Equal(Now + PairingApprovalManager.PairingApprovalRequestTtlMs, view.ExpiresAt);
        Assert.Equal("192.168.1.50", view.RemoteAddress);
        string serialized = JsonSerializer.Serialize(view);
        Assert.DoesNotContain("secret-nonce", serialized, StringComparison.Ordinal);
        Assert.DoesNotContain("token", serialized, StringComparison.OrdinalIgnoreCase);
    }

    [Fact]
    public void CreatesDeterministicVerificationCodes()
    {
        Assert.Equal("610717", PairingVerificationCode.Create("desktop-1", "android-1", "nonce-1"));
        Assert.NotEqual(
            PairingVerificationCode.Create("desktop-1", "android-1", "nonce-1"),
            PairingVerificationCode.Create("desktop-1", "android-1", "nonce-2"));
    }

    [Fact]
    public async Task AcceptsRequestStoresPairedDeviceAndReturnsToken()
    {
        MemoryPairingStore store = CreateStore();
        PairingApprovalManager manager = CreateManager(store, createToken: () => "paired-token");
        manager.CreateRequest(RequestInput());

        PairingApprovalResponseResult result = await manager.AcceptAsync("approval-1");

        Assert.True(result.Ok);
        Assert.Equal("desktop-1", result.DesktopId);
        Assert.Equal("android-1", result.DeviceId);
        Assert.Equal("paired-token", result.Token);

        PairedDevice device = Assert.Single((await store.LoadAsync()).PairedDevices);
        Assert.Equal("android-1", device.DeviceId);
        Assert.Equal("Android smoke", device.DeviceName);
        Assert.Equal("paired-token", device.Token);
        Assert.Equal(Now, device.PairedAt);
        Assert.Null(device.LastSeenAt);
        Assert.Empty(manager.ListPendingRequests());
    }

    [Fact]
    public void RejectsRequestAndRemovesIt()
    {
        PairingApprovalManager manager = CreateManager();
        manager.CreateRequest(RequestInput());

        PairingApprovalResponseResult result = manager.Reject("approval-1");

        Assert.True(result.Ok);
        Assert.Empty(manager.ListPendingRequests());
    }

    [Fact]
    public void ExpiresPendingRequests()
    {
        double currentTime = Now;
        PairingApprovalManager manager = CreateManager(now: () => currentTime);
        manager.CreateRequest(RequestInput());
        currentTime += PairingApprovalManager.PairingApprovalRequestTtlMs + 1;

        IReadOnlyList<PendingPairingApproval> expired = manager.ExpirePendingRequests();

        PendingPairingApproval request = Assert.Single(expired);
        Assert.Equal("approval-1", request.RequestId);
        Assert.Empty(manager.ListPendingRequests());
    }

    [Fact]
    public void ReplacesOlderPendingRequestsForSameDevice()
    {
        PairingApprovalManager manager = CreateManager();
        manager.CreateRequest(RequestInput(requestId: "approval-1"));

        CreatePairingApprovalRequestResult result = manager.CreateRequest(RequestInput(requestId: "approval-2"));

        Assert.Equal("approval-1", result.ReplacedRequestId);
        PendingPairingApproval request = Assert.Single(manager.ListPendingRequests());
        Assert.Equal("approval-2", request.RequestId);
    }

    [Fact]
    public async Task MissingRequestReturnsFailure()
    {
        PairingApprovalManager manager = CreateManager();

        Assert.Equal("pairing_request_not_found", (await manager.AcceptAsync("missing")).Reason);
        Assert.Equal("pairing_request_not_found", manager.Reject("missing").Reason);
    }

    private static PairingApprovalManager CreateManager(
        MemoryPairingStore? store = null,
        Func<double>? now = null,
        Func<string>? createToken = null)
    {
        return new PairingApprovalManager(store ?? CreateStore(), now ?? (() => Now), createToken);
    }

    private static MemoryPairingStore CreateStore()
    {
        return new MemoryPairingStore(new PairingState("desktop-1", []));
    }

    private static CreatePairingApprovalRequestInput RequestInput(
        string requestId = "approval-1",
        string deviceId = "android-1",
        string deviceName = "Android smoke",
        string desktopId = "desktop-1",
        string requestNonce = "nonce",
        string? remoteAddress = "192.168.1.50")
    {
        return new CreatePairingApprovalRequestInput(
            requestId,
            deviceId,
            deviceName,
            desktopId,
            requestNonce,
            remoteAddress);
    }
}
