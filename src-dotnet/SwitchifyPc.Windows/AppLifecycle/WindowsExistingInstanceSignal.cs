using SwitchifyPc.Core.AppLifecycle;

namespace SwitchifyPc.Windows.AppLifecycle;

public sealed class WindowsExistingInstanceSignal : IDisposable
{
    public const string SignalName = $@"Local\{SingleInstanceService.LockName}.ShowMainWindow";

    private readonly string signalName;
    private EventWaitHandle? listener;
    private CancellationTokenSource? cancellation;
    private Task? listenerTask;
    private bool disposed;

    public WindowsExistingInstanceSignal(string? signalName = null)
    {
        this.signalName = signalName ?? SignalName;
    }

    public void Start(Action onShowMainWindow)
    {
        ObjectDisposedException.ThrowIf(disposed, this);
        if (listener is not null) return;

        listener = new EventWaitHandle(false, EventResetMode.AutoReset, signalName);
        cancellation = new CancellationTokenSource();
        listenerTask = Task.Run(() => Listen(onShowMainWindow, cancellation.Token));
    }

    public bool SignalShowMainWindow()
    {
        ObjectDisposedException.ThrowIf(disposed, this);
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
        listener?.Dispose();
    }

    private void Listen(Action onShowMainWindow, CancellationToken cancellationToken)
    {
        WaitHandle[] handles = [listener!, cancellationToken.WaitHandle];
        while (!cancellationToken.IsCancellationRequested)
        {
            int index = WaitHandle.WaitAny(handles);
            if (index == 0)
            {
                onShowMainWindow();
            }
            else
            {
                return;
            }
        }
    }
}
