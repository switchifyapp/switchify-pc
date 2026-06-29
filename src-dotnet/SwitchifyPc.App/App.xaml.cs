using System.Windows;
using System.IO;
using System.Net.Http;
using SwitchifyPc.Core.Settings;
using SwitchifyPc.Core.AppLifecycle;
using SwitchifyPc.Core.Startup;
using SwitchifyPc.Core.Ui;
using SwitchifyPc.Core.Updates;
using SwitchifyPc.Windows.AppLifecycle;
using SwitchifyPc.Windows.Startup;
using SwitchifyPc.Windows.Updates;

namespace SwitchifyPc.App;

public partial class App : System.Windows.Application
{
    private const string CurrentVersion = "0.2.0";

    private SingleInstanceService? singleInstance;
    private NativeTrayIcon? trayIcon;
    private SettingsWindow? settingsWindow;
    private UpdateService? updateService;
    private bool isQuitting;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        ShutdownMode = ShutdownMode.OnExplicitShutdown;
        singleInstance = new SingleInstanceService(new WindowsSingleInstanceLockFactory());
        SingleInstanceDecision decision = singleInstance.Start(AppLaunchOptions.Parse(e.Args));

        if (!decision.IsPrimaryInstance)
        {
            // TODO: signal the primary instance to show its main window for normal second launches.
            Shutdown();
            return;
        }

        updateService = CreateUpdateService();
        updateService.StartAutomaticUpdateChecks();
        trayIcon = new NativeTrayIcon(ShowMainWindow, ShowSettingsWindow, QuitApplication);

        if (decision.ShowMainWindow)
        {
            ShowMainWindow();
        }
    }

    protected override void OnExit(ExitEventArgs e)
    {
        singleInstance?.Stop();
        singleInstance = null;
        updateService?.StopAutomaticUpdateChecks();
        updateService = null;
        trayIcon?.Dispose();
        trayIcon = null;
        settingsWindow = null;
        base.OnExit(e);
    }

    private void ShowMainWindow()
    {
        Window window = MainWindow ?? CreateMainWindow();
        MainWindow = window;

        window.Show();
        window.WindowState = WindowState.Normal;
        window.Activate();
    }

    private Window CreateMainWindow()
    {
        SwitchifyPc.App.MainWindow window = new();
        window.Closing += (_, eventArgs) =>
        {
            if (isQuitting) return;
            eventArgs.Cancel = true;
            window.Hide();
        };

        return window;
    }

    private void QuitApplication()
    {
        isQuitting = true;
        Shutdown();
    }

    private void ShowSettingsWindow()
    {
        settingsWindow ??= CreateSettingsWindow();
        settingsWindow.Show();
        settingsWindow.WindowState = WindowState.Normal;
        settingsWindow.Activate();
    }

    private SettingsWindow CreateSettingsWindow()
    {
        SettingsWindow window = new(CreateSettingsController());
        window.Closing += (_, eventArgs) =>
        {
            if (isQuitting) return;
            eventArgs.Cancel = true;
            window.Hide();
        };

        return window;
    }

    private SettingsController CreateSettingsController()
    {
        SettingsViewModel viewModel = new();
        string userDataDirectory = UserDataDirectory();
        return new SettingsController(
            viewModel,
            new SystemStartupService(
                platform: "win32",
                isPackaged: IsInstalledApp(),
                executablePath: Environment.ProcessPath ?? string.Empty,
                startupRegistry: new WindowsStartupRegistry(),
                startupCommandFor: WindowsStartupRegistry.StartupCommandFor),
            new JsonPointerMovementSettingsStore(Path.Combine(userDataDirectory, "pointer-movement-settings.json")),
            new JsonCursorOverlaySettingsStore(Path.Combine(userDataDirectory, "cursor-overlay-settings.json")),
            updateService ?? CreateUpdateService());
    }

    private UpdateService CreateUpdateService()
    {
        string localAppData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        string cacheDirectory = Path.Combine(localAppData, "switchify-pc-updater");
        return new UpdateService(new UpdateServiceOptions(
            CurrentVersion: CurrentVersion,
            IsPackaged: IsInstalledApp(),
            Platform: "win32",
            Backend: new GitHubReleaseUpdateBackend(new HttpClient(), "switchifyapp", "switchify-pc", cacheDirectory),
            InstallerLauncher: new WindowsUpdateInstallerLauncher()));
    }

    private static string UserDataDirectory()
    {
        string appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
        return Path.Combine(appData, "switchify-pc");
    }

    private static bool IsInstalledApp()
    {
        string? executablePath = Environment.ProcessPath;
        if (string.IsNullOrWhiteSpace(executablePath)) return false;
        string programFiles = Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles);
        return executablePath.StartsWith(programFiles, StringComparison.OrdinalIgnoreCase);
    }
}
