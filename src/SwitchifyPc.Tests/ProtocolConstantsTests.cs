using SwitchifyPc.Protocol;

namespace SwitchifyPc.Tests;

public sealed class ProtocolConstantsTests
{
    [Fact]
    public void UsesCurrentAndroidProtocolVersion()
    {
        Assert.Equal(1, ProtocolConstants.ProtocolVersion);
    }

    [Fact]
    public void PreservesCurrentProtocolLimits()
    {
        Assert.Equal(2000, ProtocolConstants.MaxTextLength);
        Assert.Equal(80, ProtocolConstants.MaxTextStreamIdLength);
        Assert.Equal(2000, ProtocolConstants.MaxTextStreamItems);
        Assert.Equal(120, ProtocolConstants.MaxTextStreamChunkLength);
        Assert.Equal(500, ProtocolConstants.MaxPointerDelta);
        Assert.Equal(50, ProtocolConstants.MaxScrollDelta);
        Assert.Equal(6, ProtocolConstants.MaxShortcutKeys);
        Assert.Equal(300, ProtocolConstants.MaxErrorMessageLength);
    }

    [Fact]
    public void ShortcutKeysIncludeUppercaseAlphabetOnly()
    {
        foreach (string key in Enumerable.Range('A', 26).Select(code => ((char)code).ToString()))
        {
            Assert.Contains(key, ProtocolConstants.ShortcutKeys);
        }

        Assert.DoesNotContain("a", ProtocolConstants.ShortcutKeys);
        Assert.Equal(6, ProtocolConstants.MaxShortcutKeys);
    }

    [Fact]
    public void IncludesCurrentCommandTypes()
    {
        string[] expected =
        [
            "mouse.move",
            "mouse.click",
            "mouse.doubleClick",
            "mouse.rightClick",
            "mouse.scroll",
            "mouse.repeat.start",
            "mouse.repeat.stop",
            "mouse.dragStart",
            "mouse.dragEnd",
            "keyboard.key",
            "keyboard.modifierDown",
            "keyboard.modifierUp",
            "keyboard.shortcut",
            "keyboard.typeText",
            "keyboard.textStream.open",
            "keyboard.textStream.char",
            "keyboard.textStream.chunk",
            "keyboard.textStream.key",
            "keyboard.textStream.close",
            "media.control",
            "window.control",
            "pointer.profile",
            "pointer.speed.set",
            "connection.ping",
            "connection.disconnecting"
        ];

        Assert.Equal(expected.Order(StringComparer.Ordinal), ProtocolConstants.CommandTypes.Order(StringComparer.Ordinal));
    }

    [Fact]
    public void IncludesPairingRequestType()
    {
        Assert.Contains("pairing.request", ProtocolConstants.PairingRequestTypes);
    }

    [Fact]
    public void RepeatCommandsRequireAckResponses()
    {
        Assert.DoesNotContain("mouse.repeat.start", ProtocolConstants.NoAckControlCommandTypes);
        Assert.DoesNotContain("mouse.repeat.stop", ProtocolConstants.NoAckControlCommandTypes);
        Assert.DoesNotContain("pointer.speed.set", ProtocolConstants.NoAckControlCommandTypes);
    }

    [Fact]
    public void ModifierCommandsCanUseNoAckResponses()
    {
        Assert.Contains("keyboard.modifierDown", ProtocolConstants.NoAckControlCommandTypes);
        Assert.Contains("keyboard.modifierUp", ProtocolConstants.NoAckControlCommandTypes);
        Assert.Equal(
            ["Alt", "Ctrl", "Meta", "Shift"],
            ProtocolConstants.ModifierKeys.Order(StringComparer.Ordinal));
    }
}
