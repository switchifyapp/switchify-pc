using System.Windows;
using SwitchifyPc.Core.AppLifecycle;
using SwitchifyPc.Windows.AppLifecycle;

namespace SwitchifyPc.App;

public partial class App : System.Windows.Application
{
    private SingleInstanceService? singleInstance;
    private NativeTrayIcon? trayIcon;
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

        trayIcon = new NativeTrayIcon(ShowMainWindow, QuitApplication);

        if (decision.ShowMainWindow)
        {
            ShowMainWindow();
        }
    }

    protected override void OnExit(ExitEventArgs e)
    {
        singleInstance?.Stop();
        singleInstance = null;
        trayIcon?.Dispose();
        trayIcon = null;
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
}
