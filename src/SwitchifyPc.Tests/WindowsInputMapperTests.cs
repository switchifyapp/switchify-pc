using SwitchifyPc.Windows.Input;

namespace SwitchifyPc.Tests;

public sealed class WindowsInputMapperTests
{
    [Fact]
    public void MapsProtocolKeysToWindowsVirtualKeys()
    {
        Assert.Equal(0x08, WindowsInputMapper.KeyboardVirtualKey("Backspace"));
        Assert.Equal(0x2E, WindowsInputMapper.KeyboardVirtualKey("Delete"));
        Assert.Equal(0x0D, WindowsInputMapper.KeyboardVirtualKey("Enter"));
        Assert.Equal(0x1B, WindowsInputMapper.KeyboardVirtualKey("Escape"));
        Assert.Equal(0x20, WindowsInputMapper.KeyboardVirtualKey("Space"));
        Assert.Equal(0x09, WindowsInputMapper.KeyboardVirtualKey("Tab"));
        Assert.Equal(0x21, WindowsInputMapper.KeyboardVirtualKey("PageUp"));
        Assert.Equal(0x22, WindowsInputMapper.KeyboardVirtualKey("PageDown"));
        Assert.Equal(0x70, WindowsInputMapper.KeyboardVirtualKey("F1"));
        Assert.Equal(0x7B, WindowsInputMapper.KeyboardVirtualKey("F12"));
        Assert.Equal(0x11, WindowsInputMapper.KeyboardVirtualKey("Ctrl"));
        Assert.Equal(0x12, WindowsInputMapper.KeyboardVirtualKey("Alt"));
        Assert.Equal(0x10, WindowsInputMapper.KeyboardVirtualKey("Shift"));
        Assert.Equal(0x41, WindowsInputMapper.KeyboardVirtualKey("A"));
    }

    [Fact]
    public void MapsMetaToWindowsKey()
    {
        Assert.Equal(0x5B, WindowsInputMapper.KeyboardVirtualKey("Meta"));
    }

    [Fact]
    public void MapsMouseAndMediaValues()
    {
        Assert.Equal(0x01, WindowsInputMapper.MouseButtonVirtualKey("left"));
        Assert.Equal(0x02, WindowsInputMapper.MouseButtonVirtualKey("right"));
        Assert.Equal(0x04, WindowsInputMapper.MouseButtonVirtualKey("middle"));
        Assert.Equal(0xB3, WindowsInputMapper.MediaVirtualKey("playPause"));
        Assert.Equal(0xB0, WindowsInputMapper.MediaVirtualKey("nextTrack"));
        Assert.Equal(0xB1, WindowsInputMapper.MediaVirtualKey("previousTrack"));
        Assert.Equal(0xAF, WindowsInputMapper.MediaVirtualKey("volumeUp"));
        Assert.Equal(0xAE, WindowsInputMapper.MediaVirtualKey("volumeDown"));
        Assert.Equal(0xAD, WindowsInputMapper.MediaVirtualKey("mute"));
    }

    [Fact]
    public void MapsWindowSwitchShortcuts()
    {
        Assert.Equal([0x12, 0x09], WindowsInputMapper.WindowControlShortcut("switchNext"));
        Assert.Equal([0x12, 0x10, 0x09], WindowsInputMapper.WindowControlShortcut("switchPrevious"));
        Assert.Empty(WindowsInputMapper.WindowControlShortcut("taskView"));
    }

    [Fact]
    public void RejectsUnknownAliases()
    {
        Assert.Throws<ArgumentOutOfRangeException>(() => WindowsInputMapper.KeyboardVirtualKey("Win"));
        Assert.Throws<ArgumentOutOfRangeException>(() => WindowsInputMapper.KeyboardVirtualKey("Windows"));
        Assert.Throws<ArgumentOutOfRangeException>(() => WindowsInputMapper.KeyboardVirtualKey("Super"));
    }
}
