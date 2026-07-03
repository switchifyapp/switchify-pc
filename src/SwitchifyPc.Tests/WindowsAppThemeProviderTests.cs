using SwitchifyPc.App.Themes;

namespace SwitchifyPc.Tests;

public sealed class WindowsAppThemeProviderTests
{
    [Fact]
    public void ReadsDarkWhenAppsUseLightThemeIsZero()
    {
        WindowsAppThemeProvider provider = new(() => 0);

        Assert.Equal(AppTheme.Dark, provider.GetCurrentTheme());
    }

    [Fact]
    public void ReadsLightWhenAppsUseLightThemeIsOne()
    {
        WindowsAppThemeProvider provider = new(() => 1);

        Assert.Equal(AppTheme.Light, provider.GetCurrentTheme());
    }

    [Fact]
    public void FallsBackToLightWhenValueMissing()
    {
        WindowsAppThemeProvider provider = new(() => null);

        Assert.Equal(AppTheme.Light, provider.GetCurrentTheme());
    }

    [Fact]
    public void FallsBackToLightWhenRegistryReadFails()
    {
        WindowsAppThemeProvider provider = new(() => throw new InvalidOperationException("registry unavailable"));

        Assert.Equal(AppTheme.Light, provider.GetCurrentTheme());
    }
}
