using System.Text.Json;
using System.Text.Json.Nodes;
using SwitchifyPc.Protocol;

namespace SwitchifyPc.Tests;

public sealed class ProtocolValidatorTests
{
    [Fact]
    public void AcceptsCurrentCommandPayloads()
    {
        object[] commands =
        [
            new { type = "mouse.move", payload = new { dx = 12, dy = -6 } },
            new { type = "mouse.click", payload = new { button = "left" } },
            new { type = "mouse.doubleClick", payload = new { button = "middle" } },
            new { type = "mouse.rightClick", payload = new { } },
            new { type = "mouse.scroll", payload = new { dx = 0, dy = -3 } },
            new { type = "mouse.dragStart", payload = new { button = "left" } },
            new { type = "mouse.dragEnd", payload = new { button = "left" } },
            new { type = "keyboard.key", payload = new { key = "Enter" } },
            new { type = "keyboard.key", payload = new { key = "Meta" } },
            new { type = "keyboard.modifierDown", payload = new { key = "Ctrl" } },
            new { type = "keyboard.modifierDown", payload = new { key = "Alt" } },
            new { type = "keyboard.modifierDown", payload = new { key = "Shift" } },
            new { type = "keyboard.modifierDown", payload = new { key = "Meta" } },
            new { type = "keyboard.modifierUp", payload = new { key = "Ctrl" } },
            new { type = "keyboard.modifierUp", payload = new { key = "Alt" } },
            new { type = "keyboard.modifierUp", payload = new { key = "Shift" } },
            new { type = "keyboard.modifierUp", payload = new { key = "Meta" } },
            new { type = "keyboard.shortcut", payload = new { keys = new[] { "Ctrl", "C" } } },
            new { type = "keyboard.shortcut", payload = new { keys = new[] { "Ctrl", "A" } } },
            new { type = "keyboard.shortcut", payload = new { keys = new[] { "Ctrl", "Shift", "Z" } } },
            new { type = "keyboard.shortcut", payload = new { keys = new[] { "Alt", "F" } } },
            new { type = "keyboard.shortcut", payload = new { keys = new[] { "Meta" } } },
            new { type = "keyboard.typeText", payload = new { text = "Hello" } },
            new { type = "keyboard.textStream.open", payload = new { streamId = "android-stream-1" } },
            new { type = "keyboard.textStream.char", payload = new { streamId = "android-stream-1", seq = 0, text = "H" } },
            new { type = "keyboard.textStream.chunk", payload = new { streamId = "android-stream-1", seq = 0, text = "Hello" } },
            new { type = "keyboard.textStream.key", payload = new { streamId = "android-stream-1", seq = 1, key = "Meta" } },
            new { type = "keyboard.textStream.close", payload = new { streamId = "android-stream-1", expectedCount = 2 } },
            new { type = "media.control", payload = new { action = "playPause" } },
            new { type = "window.control", payload = new { action = "switchNext" } },
            new { type = "pointer.profile", payload = new { } },
            new { type = "pointer.speed.set", payload = new { scalePercent = 125 } },
            new { type = "connection.ping", payload = new { } },
            new { type = "connection.disconnecting", payload = new { } }
        ];

        foreach (object command in commands)
        {
            ProtocolValidationResult result = ProtocolValidator.ValidateProtocolRequest(Json(MergeBaseCommand(command)));

            Assert.True(result.Ok, result.Message);
        }
    }

    [Fact]
    public void AcceptsNoResponseModeOnlyForUserControlCommands()
    {
        var noAckPayloads = new Dictionary<string, object>
        {
            ["mouse.move"] = new { dx = 12, dy = -6 },
            ["mouse.click"] = new { button = "left" },
            ["mouse.doubleClick"] = new { button = "middle" },
            ["mouse.rightClick"] = new { },
            ["mouse.scroll"] = new { dx = 0, dy = -3 },
            ["mouse.dragStart"] = new { button = "left" },
            ["mouse.dragEnd"] = new { button = "left" },
            ["keyboard.key"] = new { key = "Meta" },
            ["keyboard.modifierDown"] = new { key = "Ctrl" },
            ["keyboard.modifierUp"] = new { key = "Ctrl" },
            ["keyboard.shortcut"] = new { keys = new[] { "Ctrl", "C" } },
            ["keyboard.typeText"] = new { text = "Hello" },
            ["keyboard.textStream.char"] = new { streamId = "stream-1", seq = 0, text = "H" },
            ["keyboard.textStream.chunk"] = new { streamId = "stream-1", seq = 0, text = "Hello" },
            ["keyboard.textStream.key"] = new { streamId = "stream-1", seq = 1, key = "Meta" },
            ["media.control"] = new { action = "playPause" },
            ["window.control"] = new { action = "switchNext" }
        };

        foreach (string type in ProtocolConstants.NoAckControlCommandTypes)
        {
            ProtocolValidationResult result = ProtocolValidator.ValidateProtocolRequest(Json(BaseCommand(type, noAckPayloads[type], "none")));

            Assert.True(result.Ok, result.Message);
        }

        foreach ((string type, object payload) in new (string, object)[]
        {
            ("connection.ping", new { }),
            ("connection.disconnecting", new { }),
            ("pointer.profile", new { }),
            ("pointer.speed.set", new { scalePercent = 125 }),
            ("keyboard.textStream.open", new { streamId = "stream-1" }),
            ("keyboard.textStream.close", new { streamId = "stream-1", expectedCount = 0 })
        })
        {
            ProtocolValidationResult result = ProtocolValidator.ValidateProtocolRequest(Json(BaseCommand(type, payload, "none")));

            Assert.False(result.Ok);
            Assert.Equal("invalid_payload", result.Error);
        }
    }

    [Fact]
    public void AcceptsPairingRequestsWithoutCommandAuthFields()
    {
        ProtocolValidationResult result = ProtocolValidator.ValidateProtocolRequest(Json(new
        {
            version = ProtocolConstants.ProtocolVersion,
            id = "pairing-1",
            type = "pairing.request",
            payload = new
            {
                deviceId = "android-device-1",
                deviceName = "Android device",
                desktopId = "desktop-1",
                requestNonce = "nonce"
            }
        }));

        Assert.True(result.Ok, result.Message);
    }

    [Fact]
    public void RejectsInvalidJson()
    {
        ProtocolValidationResult result = ProtocolValidator.ParseProtocolRequest("{");

        Assert.False(result.Ok);
        Assert.Equal("invalid_json", result.Error);
    }

    [Fact]
    public void RejectsUnsafePayloads()
    {
        object[] invalidRequests =
        [
            BaseCommand("mouse.move", new { dx = 501, dy = 0 }),
            BaseCommand("keyboard.shortcut", new { keys = Array.Empty<string>() }),
            BaseCommand("keyboard.shortcut", new { keys = new[] { "Ctrl", "a" } }),
            BaseCommand("keyboard.shortcut", new { keys = new[] { "Ctrl", "1" } }),
            BaseCommand("keyboard.shortcut", new { keys = new[] { "Ctrl", "." } }),
            BaseCommand("keyboard.key", new { key = "F13" }),
            BaseCommand("keyboard.key", new { key = "Win" }),
            BaseCommand("keyboard.key", new { key = "Windows" }),
            BaseCommand("keyboard.key", new { key = "Super" }),
            BaseCommand("keyboard.modifierDown", new { key = "Win" }),
            BaseCommand("keyboard.modifierDown", new { key = "A" }),
            BaseCommand("keyboard.modifierDown", new { key = "Enter" }),
            BaseCommand("keyboard.modifierDown", new { }),
            BaseCommand("keyboard.modifierUp", new { key = 1 }),
            BaseCommand("keyboard.typeText", new { text = new string('x', 2001) }),
            BaseCommand("keyboard.typeText", new { text = "hello\0world" }),
            BaseCommand("keyboard.textStream.open", new { streamId = "bad stream" }),
            BaseCommand("keyboard.textStream.char", new { streamId = "stream-1", seq = 0, text = "Hi" }),
            BaseCommand("keyboard.textStream.chunk", new { streamId = "stream-1", seq = 0, text = "" }),
            BaseCommand("keyboard.textStream.key", new { streamId = "stream-1", seq = 0, key = "F13" }),
            BaseCommand("keyboard.textStream.close", new { streamId = "stream-1", expectedCount = -1 }),
            BaseCommand("pointer.profile", new { includeDisplays = true }),
            BaseCommand("pointer.speed.set", new { }),
            BaseCommand("pointer.speed.set", new { scalePercent = "125" }),
            BaseCommand("pointer.speed.set", new { scalePercent = -1 }),
            BaseCommand("connection.disconnecting", new { reason = "leaving" })
        ];

        foreach (object request in invalidRequests)
        {
            ProtocolValidationResult result = ProtocolValidator.ValidateProtocolRequest(Json(request));

            Assert.False(result.Ok);
            Assert.Equal("invalid_payload", result.Error);
        }
    }

    [Fact]
    public void CreatesAndValidatesResponses()
    {
        JsonObject ack = ProtocolValidator.CreateAckResponse("request-1");
        JsonObject error = ProtocolValidator.CreateErrorResponse("request-1", "invalid_payload", "Payload rejected.");
        JsonObject pairing = ProtocolValidator.CreatePairingCompleteResponse("pairing-1", "desktop-1", "android-1", "paired-token");

        Assert.True(ProtocolValidator.ValidateProtocolResponse(Json(ack)).Ok);
        Assert.True(ProtocolValidator.ValidateProtocolResponse(Json(error)).Ok);
        Assert.True(ProtocolValidator.ValidateProtocolResponse(Json(pairing)).Ok);
    }

    [Fact]
    public void ValidatesPointerProfileResponses()
    {
        ProtocolValidationResult result = ProtocolValidator.ValidateProtocolResponse(Json(new
        {
            version = ProtocolConstants.ProtocolVersion,
            id = "profile-1",
            type = "pointer.profile",
            ok = true,
            payload = new
            {
                displayId = "0:0:1280:720:1.5",
                scaleFactor = 1.5,
                bounds = new { x = 0, y = 0, width = 1280, height = 720 },
                maxDelta = ProtocolConstants.MaxPointerDelta,
                recommendedDeltas = new { small = 50, medium = 130, large = 252 },
                capabilities = new
                {
                    noAckMouseMove = true,
                    noAckCommands = ProtocolConstants.NoAckControlCommandTypes.ToArray(),
                    supportedCommands = ProtocolConstants.CommandTypes.ToArray(),
                    mouseRepeat = new
                    {
                        supported = true,
                        enabled = true,
                        intervalMs = 250,
                        moveIntervalMs = 250,
                        scrollIntervalMs = 500,
                        minIntervalMs = 100,
                        maxIntervalMs = 2000
                    },
                    pointerSpeed = new
                    {
                        supported = true,
                        setSupported = true,
                        scalePercent = 100,
                        minScalePercent = 5,
                        maxScalePercent = 225,
                        stepPercent = 5,
                        baseMoveDelta = 128,
                        effectiveMoveDelta = 128
                    }
                }
            },
            error = (object?)null
        }));

        Assert.True(result.Ok, result.Message);
    }

    [Fact]
    public void RejectsInvalidPointerSpeedCapabilities()
    {
        ProtocolValidationResult result = ProtocolValidator.ValidateProtocolResponse(Json(new
        {
            version = ProtocolConstants.ProtocolVersion,
            id = "profile-1",
            type = "pointer.profile",
            ok = true,
            payload = new
            {
                displayId = "0:0:1280:720:1.5",
                scaleFactor = 1.5,
                bounds = new { x = 0, y = 0, width = 1280, height = 720 },
                maxDelta = ProtocolConstants.MaxPointerDelta,
                recommendedDeltas = new { small = 50, medium = 130, large = 252 },
                capabilities = new
                {
                    pointerSpeed = new
                    {
                        supported = true,
                        setSupported = true,
                        scalePercent = 300,
                        minScalePercent = 5,
                        maxScalePercent = 225,
                        stepPercent = 5,
                        baseMoveDelta = 128,
                        effectiveMoveDelta = 128
                    }
                }
            },
            error = (object?)null
        }));

        Assert.False(result.Ok);
        Assert.Equal("invalid_payload", result.Error);

        ProtocolValidationResult malformedSetSupportedResult = ProtocolValidator.ValidateProtocolResponse(Json(new
        {
            version = ProtocolConstants.ProtocolVersion,
            id = "profile-1",
            type = "pointer.profile",
            ok = true,
            payload = new
            {
                displayId = "0:0:1280:720:1.5",
                scaleFactor = 1.5,
                bounds = new { x = 0, y = 0, width = 1280, height = 720 },
                maxDelta = ProtocolConstants.MaxPointerDelta,
                recommendedDeltas = new { small = 50, medium = 130, large = 252 },
                capabilities = new
                {
                    pointerSpeed = new
                    {
                        supported = true,
                        setSupported = "yes",
                        scalePercent = 100,
                        minScalePercent = 5,
                        maxScalePercent = 225,
                        stepPercent = 5,
                        baseMoveDelta = 128,
                        effectiveMoveDelta = 128
                    }
                }
            },
            error = (object?)null
        }));

        Assert.False(malformedSetSupportedResult.Ok);
        Assert.Equal("invalid_payload", malformedSetSupportedResult.Error);
    }

    [Fact]
    public void RejectsInvalidMouseRepeatIntervalCapabilities()
    {
        ProtocolValidationResult result = ProtocolValidator.ValidateProtocolResponse(Json(new
        {
            version = ProtocolConstants.ProtocolVersion,
            id = "profile-1",
            type = "pointer.profile",
            ok = true,
            payload = new
            {
                displayId = "0:0:1280:720:1.5",
                scaleFactor = 1.5,
                bounds = new { x = 0, y = 0, width = 1280, height = 720 },
                maxDelta = ProtocolConstants.MaxPointerDelta,
                recommendedDeltas = new { small = 50, medium = 130, large = 252 },
                capabilities = new
                {
                    mouseRepeat = new
                    {
                        supported = true,
                        enabled = true,
                        intervalMs = 250,
                        moveIntervalMs = 250,
                        scrollIntervalMs = 2501,
                        minIntervalMs = 100,
                        maxIntervalMs = 2000
                    }
                }
            },
            error = (object?)null
        }));

        Assert.False(result.Ok);
        Assert.Equal("invalid_payload", result.Error);
    }

    [Fact]
    public void RejectsMalformedPointerProfileResponses()
    {
        ProtocolValidationResult result = ProtocolValidator.ValidateProtocolResponse(Json(new
        {
            version = ProtocolConstants.ProtocolVersion,
            id = "profile-1",
            type = "pointer.profile",
            ok = true,
            payload = new
            {
                displayId = "0:0:1280:720:1.5",
                scaleFactor = 1.5,
                bounds = new { x = 0, y = 0, width = 1280, height = 720 },
                maxDelta = ProtocolConstants.MaxPointerDelta,
                recommendedDeltas = new { small = 50, medium = 130, large = 252 },
                capabilities = new { supportedCommands = new[] { "keyboard.textStream.open", "unknown.command" } }
            },
            error = (object?)null
        }));

        Assert.False(result.Ok);
        Assert.Equal("invalid_payload", result.Error);
    }

    private static object BaseCommand(string type, object payload, string? responseMode = null)
    {
        Dictionary<string, object?> command = new(StringComparer.Ordinal)
        {
            ["version"] = ProtocolConstants.ProtocolVersion,
            ["id"] = "request-1",
            ["deviceId"] = "android-device-1",
            ["timestamp"] = 1_724_000_000_000,
            ["auth"] = "proof",
            ["type"] = type,
            ["payload"] = payload
        };

        if (responseMode is not null)
        {
            command["responseMode"] = responseMode;
        }

        return command;
    }

    private static object MergeBaseCommand(object command)
    {
        JsonObject commandNode = JsonSerializer.SerializeToNode(command)!.AsObject();

        return BaseCommand(
            commandNode["type"]!.GetValue<string>(),
            commandNode["payload"]!.Deserialize<object>()!);
    }

    private static JsonElement Json(object value)
    {
        return JsonSerializer.SerializeToElement(value);
    }
}
