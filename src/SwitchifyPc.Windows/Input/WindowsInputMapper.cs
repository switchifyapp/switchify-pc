namespace SwitchifyPc.Windows.Input;

public static class WindowsInputMapper
{
    public const ushort VkLeftButton = 0x01;
    public const ushort VkRightButton = 0x02;
    public const ushort VkMiddleButton = 0x04;
    public const ushort VkBackspace = 0x08;
    public const ushort VkTab = 0x09;
    public const ushort VkEnter = 0x0D;
    public const ushort VkShift = 0x10;
    public const ushort VkControl = 0x11;
    public const ushort VkAlt = 0x12;
    public const ushort VkEscape = 0x1B;
    public const ushort VkSpace = 0x20;
    public const ushort VkPageUp = 0x21;
    public const ushort VkPageDown = 0x22;
    public const ushort VkEnd = 0x23;
    public const ushort VkHome = 0x24;
    public const ushort VkLeft = 0x25;
    public const ushort VkUp = 0x26;
    public const ushort VkRight = 0x27;
    public const ushort VkDown = 0x28;
    public const ushort VkDelete = 0x2E;
    public const ushort VkLeftWindows = 0x5B;
    public const ushort VkF1 = 0x70;
    public const ushort VkMediaNextTrack = 0xB0;
    public const ushort VkMediaPreviousTrack = 0xB1;
    public const ushort VkMediaPlayPause = 0xB3;
    public const ushort VkVolumeMute = 0xAD;
    public const ushort VkVolumeDown = 0xAE;
    public const ushort VkVolumeUp = 0xAF;
    public const ushort VkF4 = 0x73;

    public static ushort MouseButtonVirtualKey(string button)
    {
        return button switch
        {
            "left" => VkLeftButton,
            "right" => VkRightButton,
            "middle" => VkMiddleButton,
            _ => throw new ArgumentOutOfRangeException(nameof(button), button, null)
        };
    }

    public static ushort KeyboardVirtualKey(string key)
    {
        return key switch
        {
            "Backspace" => VkBackspace,
            "Delete" => VkDelete,
            "Enter" => VkEnter,
            "Escape" => VkEscape,
            "Space" => VkSpace,
            "Tab" => VkTab,
            "ArrowUp" => VkUp,
            "ArrowDown" => VkDown,
            "ArrowLeft" => VkLeft,
            "ArrowRight" => VkRight,
            "Home" => VkHome,
            "End" => VkEnd,
            "PageUp" => VkPageUp,
            "PageDown" => VkPageDown,
            "Ctrl" => VkControl,
            "Alt" => VkAlt,
            "Shift" => VkShift,
            "Meta" => VkLeftWindows,
            _ when IsUppercaseLetterKey(key) => key[0],
            _ when IsFunctionKey(key, out ushort virtualKey) => virtualKey,
            _ => throw new ArgumentOutOfRangeException(nameof(key), key, null)
        };
    }

    public static ushort MediaVirtualKey(string action)
    {
        return action switch
        {
            "playPause" => VkMediaPlayPause,
            "nextTrack" => VkMediaNextTrack,
            "previousTrack" => VkMediaPreviousTrack,
            "volumeUp" => VkVolumeUp,
            "volumeDown" => VkVolumeDown,
            "mute" => VkVolumeMute,
            _ => throw new ArgumentOutOfRangeException(nameof(action), action, null)
        };
    }

    public static IReadOnlyList<ushort> WindowControlShortcut(string action)
    {
        return action switch
        {
            "switchNext" => [VkAlt, VkTab],
            "switchPrevious" => [VkAlt, VkShift, VkTab],
            _ => []
        };
    }

    private static bool IsFunctionKey(string key, out ushort virtualKey)
    {
        virtualKey = 0;
        if (key.Length < 2 || key[0] != 'F' || !int.TryParse(key[1..], out int functionIndex) || functionIndex is < 1 or > 12)
        {
            return false;
        }

        virtualKey = (ushort)(VkF1 + functionIndex - 1);
        return true;
    }

    private static bool IsUppercaseLetterKey(string key)
    {
        return key.Length == 1 && key[0] is >= 'A' and <= 'Z';
    }
}
