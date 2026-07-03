using System.Windows;
using System.Windows.Media;
using SwitchifyPc.App;
using WpfColor = System.Windows.Media.Color;
using WpfColorConverter = System.Windows.Media.ColorConverter;

namespace SwitchifyPc.Tests;

[Collection(WpfTestCollection.Name)]
public sealed class ThemeResourceTests
{
    private static readonly string[] RequiredKeys =
    [
        "BrandPrimary",
        "BrandPrimaryHover",
        "ChromeBackground",
        "ChromeBackgroundHover",
        "ChromeBackgroundPressed",
        "ChromeCloseHover",
        "ChromeClosePressed",
        "ChromeForeground",
        "ChromeSubtleForeground",
        "AppBackground",
        "Surface",
        "SurfaceMuted",
        "Text",
        "TextMuted",
        "BorderColor",
        "StatusOk",
        "StatusWarn",
        "StatusError",
        "AccentContainer",
        "OnAccentContainer",
        "SegmentSelectedBackground",
        "ShadowColor"
    ];

    [Fact]
    public void LightAndDarkThemesDefineSameResourceKeys()
    {
        RunOnSta(() =>
        {
            ResourceDictionary light = LoadTheme("Theme.Light.xaml");
            ResourceDictionary dark = LoadTheme("Theme.Dark.xaml");

            Assert.Equal(light.Keys.Cast<object>().Select(key => key.ToString()).Order().ToArray(),
                dark.Keys.Cast<object>().Select(key => key.ToString()).Order().ToArray());
            foreach (string key in RequiredKeys)
            {
                Assert.True(light.Contains(key), key);
                Assert.True(dark.Contains(key), key);
            }
        });
    }

    [Fact]
    public void ThemeDictionariesLoadBrushResources()
    {
        RunOnSta(() =>
        {
            foreach (ResourceDictionary theme in new[] { LoadTheme("Theme.Light.xaml"), LoadTheme("Theme.Dark.xaml") })
            {
                foreach (string key in RequiredKeys)
                {
                    Assert.IsType<SolidColorBrush>(theme[key]);
                }
            }
        });
    }

    [Fact]
    public void DarkThemeUsesQuietCharcoalPalette()
    {
        RunOnSta(() =>
        {
            ResourceDictionary dark = LoadTheme("Theme.Dark.xaml");

            Assert.Equal(ColorFrom("#A8231C"), BrushColor(dark, "ChromeBackground"));
            Assert.Equal(ColorFrom("#141318"), BrushColor(dark, "AppBackground"));
            Assert.Equal(ColorFrom("#1F1F23"), BrushColor(dark, "Surface"));
            Assert.Equal(ColorFrom("#F4EFF4"), BrushColor(dark, "Text"));
        });
    }

    private static ResourceDictionary LoadTheme(string fileName)
    {
        string assemblyName = Uri.EscapeDataString(typeof(SwitchifyPc.App.App).Assembly.GetName().Name ?? "Switchify PC");
        return new ResourceDictionary
        {
            Source = new Uri($"pack://application:,,,/{assemblyName};component/Themes/{fileName}", UriKind.Absolute)
        };
    }

    private static WpfColor BrushColor(ResourceDictionary dictionary, string key)
    {
        return Assert.IsType<SolidColorBrush>(dictionary[key]).Color;
    }

    private static WpfColor ColorFrom(string value)
    {
        return (WpfColor)WpfColorConverter.ConvertFromString(value);
    }

    private static void RunOnSta(Action action)
    {
        Exception? exception = null;
        Thread thread = new(() =>
        {
            try
            {
                action();
            }
            catch (Exception error)
            {
                exception = error;
            }
        });

        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        thread.Join();

        if (exception is not null) throw exception;
    }
}
