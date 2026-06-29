namespace SwitchifyPc.Protocol;

public static class ProtocolConstants
{
    public const int ProtocolVersion = 1;
    public const int MaxTextLength = 2000;
    public const int MaxTextStreamIdLength = 80;
    public const int MaxTextStreamItems = 2000;
    public const int MaxTextStreamChunkLength = 120;
    public const int MaxPointerDelta = 500;
    public const int MaxScrollDelta = 50;
    public const int MaxShortcutKeys = 6;
    public const int MaxErrorMessageLength = 300;

    public static readonly IReadOnlySet<string> CommandTypes = new HashSet<string>(StringComparer.Ordinal)
    {
        "mouse.move",
        "mouse.click",
        "mouse.doubleClick",
        "mouse.rightClick",
        "mouse.scroll",
        "mouse.dragStart",
        "mouse.dragEnd",
        "keyboard.key",
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
        "connection.ping",
        "connection.disconnecting"
    };

    public static readonly IReadOnlySet<string> PairingRequestTypes = new HashSet<string>(StringComparer.Ordinal)
    {
        "pairing.request"
    };
}
