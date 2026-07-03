using System.Windows;
using System.Windows.Media;
using SwitchifyPc.App.Themes;

namespace SwitchifyPc.Tests;

[Collection(WpfTestCollection.Name)]
public sealed class AppThemeManagerTests
{
    [Fact]
    public void AppliesLightThemeDictionary()
    {
        RunOnSta(() =>
        {
            ResourceDictionary resources = new();
            AppThemeManager manager = Manager(resources, AppTheme.Light);

            manager.ApplyCurrentTheme();

            ResourceDictionary dictionary = Assert.Single(resources.MergedDictionaries);
            Assert.Equal(AppTheme.Light.ToString(), dictionary[AppThemeManager.SwitchifyThemeResourceKey]);
        });
    }

    [Fact]
    public void AppliesDarkThemeDictionary()
    {
        RunOnSta(() =>
        {
            ResourceDictionary resources = new();
            AppThemeManager manager = Manager(resources, AppTheme.Dark);

            manager.ApplyCurrentTheme();

            ResourceDictionary dictionary = Assert.Single(resources.MergedDictionaries);
            Assert.Equal(AppTheme.Dark.ToString(), dictionary[AppThemeManager.SwitchifyThemeResourceKey]);
        });
    }

    [Fact]
    public void SwitchingThemeReplacesPreviousThemeDictionary()
    {
        RunOnSta(() =>
        {
            ResourceDictionary resources = new();
            MutableThemeProvider provider = new(AppTheme.Light);
            AppThemeManager manager = new(resources, System.Windows.Threading.Dispatcher.CurrentDispatcher, provider, FakeDictionary);

            manager.ApplyCurrentTheme();
            provider.Theme = AppTheme.Dark;
            manager.ApplyCurrentTheme();

            ResourceDictionary dictionary = Assert.Single(resources.MergedDictionaries);
            Assert.Equal(AppTheme.Dark.ToString(), dictionary[AppThemeManager.SwitchifyThemeResourceKey]);
        });
    }

    [Fact]
    public void DoesNotRemoveNonThemeMergedDictionaries()
    {
        RunOnSta(() =>
        {
            ResourceDictionary resources = new();
            ResourceDictionary unrelated = new()
            {
                ["OtherBrush"] = new SolidColorBrush(Colors.Magenta)
            };
            resources.MergedDictionaries.Add(unrelated);
            AppThemeManager manager = Manager(resources, AppTheme.Dark);

            manager.ApplyCurrentTheme();

            Assert.Contains(unrelated, resources.MergedDictionaries);
            Assert.Contains(resources.MergedDictionaries, dictionary =>
                dictionary.Contains(AppThemeManager.SwitchifyThemeResourceKey) &&
                string.Equals(dictionary[AppThemeManager.SwitchifyThemeResourceKey]?.ToString(), AppTheme.Dark.ToString(), StringComparison.Ordinal));
        });
    }

    private static AppThemeManager Manager(ResourceDictionary resources, AppTheme theme)
    {
        return new AppThemeManager(
            resources,
            System.Windows.Threading.Dispatcher.CurrentDispatcher,
            new MutableThemeProvider(theme),
            FakeDictionary);
    }

    private static ResourceDictionary FakeDictionary(AppTheme theme)
    {
        return new ResourceDictionary();
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

    private sealed class MutableThemeProvider(AppTheme theme) : IAppThemeProvider
    {
        public AppTheme Theme { get; set; } = theme;

        public AppTheme GetCurrentTheme() => Theme;
    }
}
