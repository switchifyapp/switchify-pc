using System.Windows;
using SwitchifyPc.Core.AppLifecycle;
using SwitchifyPc.Windows.AppLifecycle;

namespace SwitchifyPc.App;

public partial class App : System.Windows.Application
{
    private SingleInstanceService? singleInstance;

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

        if (decision.ShowMainWindow)
        {
            ShowMainWindow();
        }
    }

    protected override void OnExit(ExitEventArgs e)
    {
        singleInstance?.Stop();
        singleInstance = null;
        base.OnExit(e);
    }

    private void ShowMainWindow()
    {
        SwitchifyPc.App.MainWindow window = new();
        window.Closed += (_, _) => Shutdown();
        MainWindow = window;
        window.Show();
    }
}
