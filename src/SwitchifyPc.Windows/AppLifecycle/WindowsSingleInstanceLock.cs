using System.Threading;
using SwitchifyPc.Core.AppLifecycle;

namespace SwitchifyPc.Windows.AppLifecycle;

public sealed class WindowsSingleInstanceLockFactory : ISingleInstanceLockFactory
{
    public ISingleInstanceLock TryAcquire(string name)
    {
        Mutex mutex = new(initiallyOwned: true, name: $@"Local\{name}", createdNew: out bool createdNew);
        return new WindowsSingleInstanceLock(mutex, createdNew);
    }
}

internal sealed class WindowsSingleInstanceLock : ISingleInstanceLock
{
    private readonly Mutex mutex;
    private bool disposed;

    public WindowsSingleInstanceLock(Mutex mutex, bool isAcquired)
    {
        this.mutex = mutex;
        IsAcquired = isAcquired;
    }

    public bool IsAcquired { get; }

    public void Dispose()
    {
        if (disposed) return;
        disposed = true;

        try
        {
            if (IsAcquired) mutex.ReleaseMutex();
        }
        catch (ApplicationException)
        {
            // The mutex may already have been abandoned or released during shutdown.
        }
        finally
        {
            mutex.Dispose();
        }
    }
}
