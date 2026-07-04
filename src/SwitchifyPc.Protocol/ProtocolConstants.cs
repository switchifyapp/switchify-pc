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
        "connection.ping",
        "connection.disconnecting"
    };

    public static readonly IReadOnlySet<string> PairingRequestTypes = new HashSet<string>(StringComparer.Ordinal)
    {
        "pairing.request"
    };

    public static readonly IReadOnlySet<string> NoAckControlCommandTypes = new HashSet<string>(StringComparer.Ordinal)
    {
        "mouse.move",
        "mouse.click",
        "mouse.doubleClick",
        "mouse.rightClick",
        "mouse.scroll",
        "mouse.dragStart",
        "mouse.dragEnd",
        "keyboard.key",
        "keyboard.modifierDown",
        "keyboard.modifierUp",
        "keyboard.shortcut",
        "keyboard.typeText",
        "keyboard.textStream.char",
        "keyboard.textStream.key",
        "media.control",
        "window.control"
    };

    public static readonly IReadOnlySet<string> MouseButtons = new HashSet<string>(StringComparer.Ordinal)
    {
        "left",
        "right",
        "middle"
    };

    public static readonly IReadOnlySet<string> KeyboardKeys = new HashSet<string>(StringComparer.Ordinal)
    {
        "Backspace",
        "Delete",
        "Enter",
        "Escape",
        "Space",
        "Tab",
        "Meta",
        "ArrowUp",
        "ArrowDown",
        "ArrowLeft",
        "ArrowRight",
        "Home",
        "End",
        "PageUp",
        "PageDown",
        "F1",
        "F2",
        "F3",
        "F4",
        "F5",
        "F6",
        "F7",
        "F8",
        "F9",
        "F10",
        "F11",
        "F12"
    };

    public static readonly IReadOnlySet<string> ShortcutKeys = new HashSet<string>(
        KeyboardKeys.Concat(["Ctrl", "Alt", "Shift", "Meta"]).Concat(Enumerable.Range('A', 26).Select(code => ((char)code).ToString())),
        StringComparer.Ordinal);

    public static readonly IReadOnlySet<string> ModifierKeys = new HashSet<string>(StringComparer.Ordinal)
    {
        "Ctrl",
        "Alt",
        "Shift",
        "Meta"
    };

    public static readonly IReadOnlySet<string> MediaActions = new HashSet<string>(StringComparer.Ordinal)
    {
        "playPause",
        "nextTrack",
        "previousTrack",
        "volumeUp",
        "volumeDown",
        "mute"
    };

    public static readonly IReadOnlySet<string> WindowControlActions = new HashSet<string>(StringComparer.Ordinal)
    {
        "switchNext",
        "switchPrevious",
        "taskView",
        "showDesktop",
        "closeFocused",
        "minimizeFocused",
        "maximizeFocused"
    };

    public static readonly IReadOnlySet<string> CommandResponseModes = new HashSet<string>(StringComparer.Ordinal)
    {
        "ack",
        "none"
    };
}
