using Xunit;
using SwitchifyPc.App.Themes;
using WpfResourceDictionary = System.Windows.ResourceDictionary;
using WpfApplication = System.Windows.Application;

namespace SwitchifyPc.Tests;

[CollectionDefinition(Name, DisableParallelization = true)]
public sealed class WpfTestCollection
{
    public const string Name = "WPF tests";
}

public static class WpfTestApplication
{
    public static void ApplyTheme(AppTheme theme)
    {
        WpfApplication application = WpfApplication.Current ?? new WpfApplication
        {
            ShutdownMode = System.Windows.ShutdownMode.OnExplicitShutdown
        };

        new AppThemeManager(
            application.Resources,
            application.Dispatcher,
            new FixedThemeProvider(theme),
            ThemeDictionary).ApplyCurrentTheme();
    }

    private static WpfResourceDictionary ThemeDictionary(AppTheme theme)
    {
        string assemblyName = Uri.EscapeDataString(typeof(SwitchifyPc.App.App).Assembly.GetName().Name ?? "Switchify PC");
        string fileName = theme == AppTheme.Dark ? "Theme.Dark.xaml" : "Theme.Light.xaml";
        return new WpfResourceDictionary
        {
            Source = new Uri($"pack://application:,,,/{assemblyName};component/Themes/{fileName}", UriKind.Absolute)
        };
    }

    private sealed class FixedThemeProvider(AppTheme theme) : IAppThemeProvider
    {
        public AppTheme GetCurrentTheme() => theme;
    }
}
