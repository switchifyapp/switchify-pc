using System.Text.Json;
using SwitchifyPc.Core.Pairing;
using SwitchifyPc.Protocol;

namespace SwitchifyPc.Tests;

public sealed class CommandAuthTests
{
    private const string Token = "shared-token";
    private const double Now = 1_724_000_000_000;

    [Fact]
    public async Task AcceptsCommandsFromPairedDevicesWithValidAuthProof()
    {
        MemoryPairingStore store = CreateStore();
        CommandAuthValidator validator = new(store, () => Now);
        JsonElement command = CreateCommand();

        AuthValidationResult result = await validator.ValidateAsync(command);

        Assert.True(result.Ok, result.Reason);
        Assert.Equal("android-1", result.DeviceId);
        Assert.False(result.DeviceWasPreviouslyUsed);
        Assert.Equal(Now, (await store.LoadAsync()).PairedDevices[0].LastSeenAt);
    }

    [Fact]
    public async Task ReportsPreviouslyUsedDeviceWhenLastSeenAlreadyExists()
    {
        MemoryPairingStore store = CreateStore(lastSeenAt: Now - 500);
        CommandAuthValidator validator = new(store, () => Now);

        AuthValidationResult result = await validator.ValidateAsync(CreateCommand());

        Assert.True(result.Ok, result.Reason);
        Assert.Equal("android-1", result.DeviceId);
        Assert.True(result.DeviceWasPreviouslyUsed);
        Assert.Equal(Now, (await store.LoadAsync()).PairedDevices[0].LastSeenAt);
    }

    [Fact]
    public async Task RejectsUnknownDevicesBeforeAuthSucceeds()
    {
        CommandAuthValidator validator = new(CreateStore(), () => Now);

        AuthValidationResult result = await validator.ValidateAsync(CreateCommand(new Dictionary<string, object?> { ["deviceId"] = "android-unknown" }));

        Assert.False(result.Ok);
        Assert.Equal("unknown_device", result.Reason);
        Assert.Null(result.DeviceId);
        Assert.False(result.DeviceWasPreviouslyUsed);
    }

    [Fact]
    public async Task RejectsInvalidAuthProofs()
    {
        MemoryPairingStore store = CreateStore();
        CommandAuthValidator validator = new(store, () => Now);

        AuthValidationResult result = await validator.ValidateAsync(CreateCommand(new Dictionary<string, object?> { ["auth"] = "bad-proof" }));

        Assert.False(result.Ok);
        Assert.Equal("invalid_auth", result.Reason);
        Assert.Null(result.DeviceId);
        Assert.False(result.DeviceWasPreviouslyUsed);
        Assert.Null((await store.LoadAsync()).PairedDevices[0].LastSeenAt);
    }

    [Fact]
    public void CanonicalizesMissingResponseModeAsAck()
    {
        JsonElement implicitAck = CreateCommand();
        JsonElement explicitAck = CreateCommand(new Dictionary<string, object?> { ["responseMode"] = "ack" });

        Assert.Equal(CommandAuth.CreateCommandAuthProof(implicitAck, Token), CommandAuth.CreateCommandAuthProof(explicitAck, Token));
    }

    [Fact]
    public void IncludesNoResponseModeInAuthProofs()
    {
        JsonElement ackMove = CreateMouseMoveCommand();
        JsonElement noAckMove = CreateMouseMoveCommand(new Dictionary<string, object?> { ["responseMode"] = "none" });

        Assert.NotEqual(CommandAuth.CreateCommandAuthProof(ackMove, Token), CommandAuth.CreateCommandAuthProof(noAckMove, Token));
    }

    [Fact]
    public async Task RejectsResponseModeTamperingAfterSigning()
    {
        CommandAuthValidator validator = new(CreateStore(), () => Now);
        JsonElement signedAck = CreateMouseMoveCommand();
        Dictionary<string, object?> tampered = CommandToDictionary(signedAck);
        tampered["responseMode"] = "none";

        AuthValidationResult result = await validator.ValidateAsync(Json(tampered));

        Assert.False(result.Ok);
        Assert.Equal("invalid_auth", result.Reason);
    }

    [Fact]
    public async Task RejectsExpiredTimestamps()
    {
        MemoryPairingStore store = CreateStore();
        CommandAuthValidator validator = new(store, () => Now);

        AuthValidationResult result = await validator.ValidateAsync(CreateCommand(new Dictionary<string, object?>
        {
            ["timestamp"] = Now - CommandAuth.CommandTimestampToleranceMs - 1
        }));

        Assert.False(result.Ok);
        Assert.Equal("expired_timestamp", result.Reason);
        Assert.Null((await store.LoadAsync()).PairedDevices[0].LastSeenAt);
    }

    [Fact]
    public async Task RejectsDuplicateRequestIdsInsideReplayCacheWindow()
    {
        CommandAuthValidator validator = new(CreateStore(), () => Now);
        JsonElement command = CreateCommand();

        Assert.True((await validator.ValidateAsync(command)).Ok);
        AuthValidationResult duplicate = await validator.ValidateAsync(command);

        Assert.False(duplicate.Ok);
        Assert.Equal("duplicate_request", duplicate.Reason);
    }

    [Fact]
    public async Task RejectsOversizedPayloadsThroughProtocolValidation()
    {
        CommandAuthValidator validator = new(CreateStore(), () => Now);

        AuthValidationResult result = await validator.ValidateAsync(CreateCommand(new Dictionary<string, object?>
        {
            ["type"] = "keyboard.typeText",
            ["payload"] = new Dictionary<string, object?> { ["text"] = new string('x', 2001) }
        }));

        Assert.False(result.Ok);
        Assert.Equal("invalid_payload", result.Reason);
    }

    [Fact]
    public async Task AcceptsApostropheTextPayloadWithAndroidCompatibleAuthProof()
    {
        CommandAuthValidator validator = new(CreateStore(), () => Now);

        AuthValidationResult result = await validator.ValidateAsync(CreateTextCommand("apostrophe-1", "'", "W0OLnbhllDOCd0Gf_00WLpHRvfidYjHeY69nbcmTFYA"));

        Assert.True(result.Ok, result.Reason);
    }

    [Fact]
    public void EscapesTextPayloadsLikeAndroidProtocolAuth()
    {
        Assert.Equal("W0OLnbhllDOCd0Gf_00WLpHRvfidYjHeY69nbcmTFYA", CommandAuth.CreateCommandAuthProof(CreateTextCommand("apostrophe-1", "'"), Token));
        Assert.Equal("UfVuw9DJjDofw7UeXBZcIFAu5V2YSkEfJ5p0h8_z2UY", CommandAuth.CreateCommandAuthProof(CreateTextCommand("backslash-1", "\\"), Token));
        Assert.Equal("syal1vfAvjac7RNhm8S_2fj-Edk5k1KkGeIMc7eBelw", CommandAuth.CreateCommandAuthProof(CreateTextCommand("html-1", "<>&'"), Token));
    }

    [Fact]
    public void MatchesKnownTypeScriptAuthFixtures()
    {
        Assert.Equal("g2ZSWFSZkawXgBIpFkAsXgdZ2a4np-QT7t8Y5yj7Wf0", CommandAuth.CreateCommandAuthProof(CreateCommand(), Token));
        Assert.Equal("mEa9-5UXE45exXk85Sr887-bs1VLSaf9LVNWTGrFCSU", CommandAuth.CreateCommandAuthProof(CreateMouseMoveCommand(), Token));
        Assert.Equal("pt4LxoCG9dZGwACgwRdlnbo5hG3VIg4w1JHbfPaln8E", CommandAuth.CreateCommandAuthProof(CreateMouseMoveCommand(new Dictionary<string, object?> { ["responseMode"] = "none" }), Token));
    }

    private static MemoryPairingStore CreateStore(double? lastSeenAt = null)
    {
        return new MemoryPairingStore(new PairingState(
            "desktop-1",
            [new PairedDevice("android-1", "Android device", Token, Now - 1000, lastSeenAt)]));
    }

    private static JsonElement CreateCommand(Dictionary<string, object?>? overrides = null)
    {
        return CreateSignedCommand(new Dictionary<string, object?>
        {
            ["version"] = ProtocolConstants.ProtocolVersion,
            ["id"] = "request-1",
            ["deviceId"] = "android-1",
            ["timestamp"] = Now,
            ["type"] = "connection.ping",
            ["payload"] = new Dictionary<string, object?>(),
            ["auth"] = ""
        }, overrides);
    }

    private static JsonElement CreateMouseMoveCommand(Dictionary<string, object?>? overrides = null)
    {
        return CreateSignedCommand(new Dictionary<string, object?>
        {
            ["version"] = ProtocolConstants.ProtocolVersion,
            ["id"] = "move-1",
            ["deviceId"] = "android-1",
            ["timestamp"] = Now,
            ["type"] = "mouse.move",
            ["payload"] = new Dictionary<string, object?> { ["dx"] = 12, ["dy"] = -6 },
            ["auth"] = ""
        }, overrides);
    }

    private static JsonElement CreateTextCommand(string id, string text, string? authOverride = null)
    {
        return CreateSignedCommand(new Dictionary<string, object?>
        {
            ["version"] = ProtocolConstants.ProtocolVersion,
            ["id"] = id,
            ["deviceId"] = "android-1",
            ["timestamp"] = Now,
            ["type"] = "keyboard.typeText",
            ["payload"] = new Dictionary<string, object?> { ["text"] = text },
            ["auth"] = authOverride ?? ""
        }, authOverride is null ? null : new Dictionary<string, object?> { ["auth"] = authOverride });
    }

    private static JsonElement CreateSignedCommand(Dictionary<string, object?> command, Dictionary<string, object?>? overrides)
    {
        if (overrides is not null)
        {
            foreach ((string key, object? value) in overrides)
            {
                command[key] = value;
            }
        }

        if (overrides is null || !overrides.ContainsKey("auth"))
        {
            command["auth"] = CommandAuth.CreateCommandAuthProof(Json(command), Token);
        }

        return Json(command);
    }

    private static Dictionary<string, object?> CommandToDictionary(JsonElement command)
    {
        return JsonSerializer.Deserialize<Dictionary<string, object?>>(command.GetRawText())!;
    }

    private static JsonElement Json(object value)
    {
        return JsonSerializer.SerializeToElement(value);
    }
}
