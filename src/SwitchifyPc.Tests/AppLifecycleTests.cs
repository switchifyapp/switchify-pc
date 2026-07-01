using SwitchifyPc.Core.AppLifecycle;
using SwitchifyPc.Windows.AppLifecycle;

namespace SwitchifyPc.Tests;

public sealed class AppLifecycleTests
{
    [Fact]
    public void ParsesStartHiddenArgument()
    {
        Assert.False(AppLaunchOptions.Parse([]).StartHidden);
        Assert.True(AppLaunchOptions.Parse(["--start-hidden"]).StartHidden);
        Assert.True(AppLaunchOptions.Parse(["--START-HIDDEN"]).StartHidden);
    }

    [Fact]
    public void ParsesQuitForInstallArgument()
    {
        Assert.False(AppLaunchOptions.Parse([]).QuitForInstall);
        Assert.True(AppLaunchOptions.Parse(["--quit-for-install"]).QuitForInstall);
        Assert.True(AppLaunchOptions.Parse(["--QUIT-FOR-INSTALL"]).QuitForInstall);
    }

    [Fact]
    public void PrimaryNormalLaunchShowsMainWindow()
    {
        SingleInstanceService service = new(new FakeLockFactory(acquired: true));

        SingleInstanceDecision decision = service.Start(new AppLaunchOptions(StartHidden: false));

        Assert.True(decision.IsPrimaryInstance);
        Assert.True(decision.ShowMainWindow);
        Assert.Equal(ExistingInstanceAction.None, decision.ExistingInstanceAction);
    }

    [Fact]
    public void PrimaryStartHiddenLaunchDoesNotShowMainWindow()
    {
        SingleInstanceService service = new(new FakeLockFactory(acquired: true));

        SingleInstanceDecision decision = service.Start(new AppLaunchOptions(StartHidden: true));

        Assert.True(decision.IsPrimaryInstance);
        Assert.False(decision.ShowMainWindow);
        Assert.Equal(ExistingInstanceAction.None, decision.ExistingInstanceAction);
    }

    [Fact]
    public void PrimaryQuitForInstallLaunchExitsQuietly()
    {
        FakeLock appLock = new(acquired: true);
        SingleInstanceService service = new(new FakeLockFactory(appLock));

        SingleInstanceDecision decision = service.Start(new AppLaunchOptions(StartHidden: false, QuitForInstall: true));

        Assert.False(decision.IsPrimaryInstance);
        Assert.False(decision.ShowMainWindow);
        Assert.Equal(ExistingInstanceAction.ExitQuietly, decision.ExistingInstanceAction);
        Assert.True(appLock.Disposed);
    }

    [Fact]
    public void SecondNormalLaunchRequestsExistingWindowShow()
    {
        SingleInstanceService service = new(new FakeLockFactory(acquired: false));

        SingleInstanceDecision decision = service.Start(new AppLaunchOptions(StartHidden: false));

        Assert.False(decision.IsPrimaryInstance);
        Assert.False(decision.ShowMainWindow);
        Assert.Equal(ExistingInstanceAction.ShowMainWindow, decision.ExistingInstanceAction);
    }

    [Fact]
    public void SecondStartHiddenLaunchExitsQuietly()
    {
        SingleInstanceService service = new(new FakeLockFactory(acquired: false));

        SingleInstanceDecision decision = service.Start(new AppLaunchOptions(StartHidden: true));

        Assert.False(decision.IsPrimaryInstance);
        Assert.False(decision.ShowMainWindow);
        Assert.Equal(ExistingInstanceAction.ExitQuietly, decision.ExistingInstanceAction);
    }

    [Fact]
    public void SecondQuitForInstallLaunchRequestsExistingInstanceQuit()
    {
        SingleInstanceService service = new(new FakeLockFactory(acquired: false));

        SingleInstanceDecision decision = service.Start(new AppLaunchOptions(StartHidden: false, QuitForInstall: true));

        Assert.False(decision.IsPrimaryInstance);
        Assert.False(decision.ShowMainWindow);
        Assert.Equal(ExistingInstanceAction.QuitForInstall, decision.ExistingInstanceAction);
    }

    [Fact]
    public void StopDisposesAcquiredLock()
    {
        FakeLock appLock = new(acquired: true);
        SingleInstanceService service = new(new FakeLockFactory(appLock));

        service.Start(new AppLaunchOptions(StartHidden: false));
        service.Stop();

        Assert.True(appLock.Disposed);
    }

    [Fact]
    public async Task ExistingInstanceSignalNotifiesListener()
    {
        string showSignalName = $@"Local\SwitchifyPc.Tests.Show.{Guid.NewGuid()}";
        string quitSignalName = $@"Local\SwitchifyPc.Tests.Quit.{Guid.NewGuid()}";
        using WindowsExistingInstanceSignal listener = new(showSignalName, quitSignalName);
        using WindowsExistingInstanceSignal signaler = new(showSignalName, quitSignalName);
        TaskCompletionSource notified = new(TaskCreationOptions.RunContinuationsAsynchronously);
        listener.Start(() => notified.TrySetResult(), () => { });

        Assert.True(signaler.SignalShowMainWindow());

        Task completed = await Task.WhenAny(notified.Task, Task.Delay(TimeSpan.FromSeconds(2)));
        Assert.Same(notified.Task, completed);
    }

    [Fact]
    public async Task ExistingInstanceSignalNotifiesQuitListener()
    {
        string showSignalName = $@"Local\SwitchifyPc.Tests.Show.{Guid.NewGuid()}";
        string quitSignalName = $@"Local\SwitchifyPc.Tests.Quit.{Guid.NewGuid()}";
        using WindowsExistingInstanceSignal listener = new(showSignalName, quitSignalName);
        using WindowsExistingInstanceSignal signaler = new(showSignalName, quitSignalName);
        TaskCompletionSource notified = new(TaskCreationOptions.RunContinuationsAsynchronously);
        listener.Start(() => { }, () => notified.TrySetResult());

        Assert.True(signaler.SignalQuitForInstall());

        Task completed = await Task.WhenAny(notified.Task, Task.Delay(TimeSpan.FromSeconds(2)));
        Assert.Same(notified.Task, completed);
    }

    [Fact]
    public void ExistingInstanceSignalReturnsFalseWhenNoListenerExists()
    {
        using WindowsExistingInstanceSignal signaler = new(
            $@"Local\SwitchifyPc.Tests.Show.{Guid.NewGuid()}",
            $@"Local\SwitchifyPc.Tests.Quit.{Guid.NewGuid()}");

        Assert.False(signaler.SignalShowMainWindow());
        Assert.False(signaler.SignalQuitForInstall());
    }

    private sealed class FakeLockFactory : ISingleInstanceLockFactory
    {
        private readonly FakeLock appLock;

        public FakeLockFactory(bool acquired)
            : this(new FakeLock(acquired))
        {
        }

        public FakeLockFactory(FakeLock appLock)
        {
            this.appLock = appLock;
        }

        public ISingleInstanceLock TryAcquire(string name)
        {
            Assert.Equal(SingleInstanceService.LockName, name);
            return appLock;
        }
    }

    private sealed class FakeLock(bool acquired) : ISingleInstanceLock
    {
        public bool IsAcquired { get; } = acquired;
        public bool Disposed { get; private set; }

        public void Dispose()
        {
            Disposed = true;
        }
    }
}
