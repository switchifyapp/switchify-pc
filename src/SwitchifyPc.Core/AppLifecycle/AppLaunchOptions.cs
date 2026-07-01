namespace SwitchifyPc.Core.AppLifecycle;

public sealed record AppLaunchOptions(bool StartHidden, bool QuitForInstall = false)
{
    public const string StartHiddenArg = "--start-hidden";
    public const string QuitForInstallArg = "--quit-for-install";

    public static AppLaunchOptions Parse(IEnumerable<string> args)
    {
        return new AppLaunchOptions(
            StartHidden: args.Any(arg => string.Equals(arg, StartHiddenArg, StringComparison.OrdinalIgnoreCase)),
            QuitForInstall: args.Any(arg => string.Equals(arg, QuitForInstallArg, StringComparison.OrdinalIgnoreCase)));
    }
}

public enum ExistingInstanceAction
{
    None,
    ShowMainWindow,
    ExitQuietly,
    QuitForInstall
}

public sealed record SingleInstanceDecision(bool IsPrimaryInstance, bool ShowMainWindow, ExistingInstanceAction ExistingInstanceAction);

public interface ISingleInstanceLock : IDisposable
{
    bool IsAcquired { get; }
}

public interface ISingleInstanceLockFactory
{
    ISingleInstanceLock TryAcquire(string name);
}

public sealed class SingleInstanceService
{
    public const string LockName = "SwitchifyPc.SingleInstance";

    private readonly ISingleInstanceLockFactory lockFactory;
    private ISingleInstanceLock? activeLock;

    public SingleInstanceService(ISingleInstanceLockFactory lockFactory)
    {
        this.lockFactory = lockFactory;
    }

    public SingleInstanceDecision Start(AppLaunchOptions options)
    {
        activeLock = lockFactory.TryAcquire(LockName);
        if (activeLock.IsAcquired)
        {
            if (options.QuitForInstall)
            {
                activeLock.Dispose();
                activeLock = null;
                return new SingleInstanceDecision(
                    IsPrimaryInstance: false,
                    ShowMainWindow: false,
                    ExistingInstanceAction: ExistingInstanceAction.ExitQuietly);
            }

            return new SingleInstanceDecision(
                IsPrimaryInstance: true,
                ShowMainWindow: !options.StartHidden,
                ExistingInstanceAction: ExistingInstanceAction.None);
        }

        activeLock.Dispose();
        activeLock = null;
        return new SingleInstanceDecision(
            IsPrimaryInstance: false,
            ShowMainWindow: false,
            ExistingInstanceAction: options.QuitForInstall
                ? ExistingInstanceAction.QuitForInstall
                : options.StartHidden
                    ? ExistingInstanceAction.ExitQuietly
                    : ExistingInstanceAction.ShowMainWindow);
    }

    public void Stop()
    {
        activeLock?.Dispose();
        activeLock = null;
    }
}
