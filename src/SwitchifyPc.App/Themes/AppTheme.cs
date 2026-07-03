using System.Windows;
using System.Windows.Threading;
using System.IO.Packaging;
using Microsoft.Win32;
using WpfApplication = System.Windows.Application;

namespace SwitchifyPc.App.Themes;

public enum AppTheme
{
    Light,
    Dark
}

public interface IAppThemeProvider
{
    AppTheme GetCurrentTheme();
}

public sealed class WindowsAppThemeProvider : IAppThemeProvider
{
    private const string PersonalizeRegistryPath = @"HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
    private const string AppsUseLightThemeValueName = "AppsUseLightTheme";
    private readonly Func<object?> readAppsUseLightTheme;

    public WindowsAppThemeProvider(Func<object?>? readAppsUseLightTheme = null)
    {
        this.readAppsUseLightTheme = readAppsUseLightTheme ?? (() =>
            Registry.GetValue(PersonalizeRegistryPath, AppsUseLightThemeValueName, 1));
    }

    public AppTheme GetCurrentTheme()
    {
        try
        {
            return readAppsUseLightTheme() switch
            {
                int value when value == 0 => AppTheme.Dark,
                long value when value == 0 => AppTheme.Dark,
                string value when string.Equals(value, "0", StringComparison.Ordinal) => AppTheme.Dark,
                _ => AppTheme.Light
            };
        }
        catch
        {
            return AppTheme.Light;
        }
    }
}

public sealed class AppThemeManager : IDisposable
{
    public const string SwitchifyThemeResourceKey = "__SwitchifyTheme";
    public static readonly Uri LightThemeUri = PackUriHelper.Create(
        new Uri("application:///", UriKind.Absolute),
        new Uri("/Themes/Theme.Light.xaml", UriKind.Relative));
    public static readonly Uri DarkThemeUri = PackUriHelper.Create(
        new Uri("application:///", UriKind.Absolute),
        new Uri("/Themes/Theme.Dark.xaml", UriKind.Relative));

    private readonly ResourceDictionary resources;
    private readonly Dispatcher dispatcher;
    private readonly IAppThemeProvider themeProvider;
    private readonly Func<AppTheme, ResourceDictionary> createDictionary;
    private bool isListening;

    public AppThemeManager(
        ResourceDictionary resources,
        Dispatcher dispatcher,
        IAppThemeProvider themeProvider,
        Func<AppTheme, ResourceDictionary>? createDictionary = null)
    {
        this.resources = resources;
        this.dispatcher = dispatcher;
        this.themeProvider = themeProvider;
        this.createDictionary = createDictionary ?? CreateThemeDictionary;
    }

    public static AppThemeManager ForCurrentApplication(IAppThemeProvider themeProvider)
    {
        WpfApplication application = WpfApplication.Current ?? throw new InvalidOperationException("No WPF application is available.");
        return new AppThemeManager(application.Resources, application.Dispatcher, themeProvider);
    }

    public void Start()
    {
        ApplyCurrentTheme();
        if (isListening) return;

        SystemEvents.UserPreferenceChanged += SystemEvents_UserPreferenceChanged;
        isListening = true;
    }

    public void ApplyCurrentTheme()
    {
        ApplyTheme(themeProvider.GetCurrentTheme());
    }

    public void ApplyTheme(AppTheme theme)
    {
        ResourceDictionary next;
        try
        {
            next = createDictionary(theme);
        }
        catch
        {
            return;
        }

        next[SwitchifyThemeResourceKey] = theme.ToString();

        for (int index = resources.MergedDictionaries.Count - 1; index >= 0; index--)
        {
            ResourceDictionary dictionary = resources.MergedDictionaries[index];
            if (IsSwitchifyThemeDictionary(dictionary))
            {
                resources.MergedDictionaries.RemoveAt(index);
            }
        }

        resources.MergedDictionaries.Add(next);
    }

    public void Dispose()
    {
        if (!isListening) return;

        SystemEvents.UserPreferenceChanged -= SystemEvents_UserPreferenceChanged;
        isListening = false;
    }

    private void SystemEvents_UserPreferenceChanged(object sender, UserPreferenceChangedEventArgs e)
    {
        if (dispatcher.CheckAccess())
        {
            ApplyCurrentTheme();
            return;
        }

        dispatcher.BeginInvoke(ApplyCurrentTheme);
    }

    private static ResourceDictionary CreateThemeDictionary(AppTheme theme)
    {
        return new ResourceDictionary
        {
            Source = theme == AppTheme.Dark ? DarkThemeUri : LightThemeUri
        };
    }

    private static bool IsSwitchifyThemeDictionary(ResourceDictionary dictionary)
    {
        return dictionary.Source is not null &&
            (dictionary.Source == LightThemeUri || dictionary.Source == DarkThemeUri) ||
            dictionary.Contains(SwitchifyThemeResourceKey);
    }
}
