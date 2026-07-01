using SwitchifyPc.Core.AppLifecycle;

namespace SwitchifyPc.Windows.AppLifecycle;

public sealed class WindowsExistingInstanceSignal : IDisposable
{
    public const string ShowMainWindowSignalName = $@"Local\{SingleInstanceService.LockName}.ShowMainWindow";
    public const string QuitForInstallSignalName = $@"Local\{SingleInstanceService.LockName}.QuitForInstall";

    private readonly string showMainWindowSignalName;
    private readonly string quitForInstallSignalName;
    private EventWaitHandle? showMainWindowListener;
    private EventWaitHandle? quitForInstallListener;
    private CancellationTokenSource? cancellation;
    private Task? listenerTask;
    private bool disposed;

    public WindowsExistingInstanceSignal(string? showMainWindowSignalName = null, string? quitForInstallSignalName = null)
    {
        this.showMainWindowSignalName = showMainWindowSignalName ?? ShowMainWindowSignalName;
        this.quitForInstallSignalName = quitForInstallSignalName ?? QuitForInstallSignalName;
    }

    public void Start(Action onShowMainWindow, Action onQuitForInstall)
    {
        ObjectDisposedException.ThrowIf(disposed, this);
        if (showMainWindowListener is not null || quitForInstallListener is not null) return;

        showMainWindowListener = new EventWaitHandle(false, EventResetMode.AutoReset, showMainWindowSignalName);
        quitForInstallListener = new EventWaitHandle(false, EventResetMode.AutoReset, quitForInstallSignalName);
        cancellation = new CancellationTokenSource();
        listenerTask = Task.Run(() => Listen(onShowMainWindow, onQuitForInstall, cancellation.Token));
    }

    public bool SignalShowMainWindow()
    {
        ObjectDisposedException.ThrowIf(disposed, this);
        return Signal(showMainWindowSignalName);
    }

    public bool SignalQuitForInstall()
    {
        ObjectDisposedException.ThrowIf(disposed, this);
        return Signal(quitForInstallSignalName);
    }

    public void Dispose()
    {
        if (disposed) return;
        disposed = true;

        cancellation?.Cancel();
        try
        {
            listenerTask?.Wait(TimeSpan.FromSeconds(2));
        }
        catch (AggregateException)
        {
            // Shutdown should not fail if the listener exits while being cancelled.
        }

        cancellation?.Dispose();
        showMainWindowListener?.Dispose();
        quitForInstallListener?.Dispose();
    }

    private static bool Signal(string signalName)
    {
        try
        {
            using EventWaitHandle signal = EventWaitHandle.OpenExisting(signalName);
            return signal.Set();
        }
        catch (WaitHandleCannotBeOpenedException)
        {
            return false;
        }
    }

    private void Listen(Action onShowMainWindow, Action onQuitForInstall, CancellationToken cancellationToken)
    {
        WaitHandle[] handles = [showMainWindowListener!, quitForInstallListener!, cancellationToken.WaitHandle];
        while (!cancellationToken.IsCancellationRequested)
        {
            int index = WaitHandle.WaitAny(handles);
            if (index == 0)
            {
                onShowMainWindow();
            }
            else if (index == 1)
            {
                onQuitForInstall();
            }
            else
            {
                return;
            }
        }
    }
}
