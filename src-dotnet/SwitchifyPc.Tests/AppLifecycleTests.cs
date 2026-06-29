using SwitchifyPc.Core.AppLifecycle;

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
    public void StopDisposesAcquiredLock()
    {
        FakeLock appLock = new(acquired: true);
        SingleInstanceService service = new(new FakeLockFactory(appLock));

        service.Start(new AppLaunchOptions(StartHidden: false));
        service.Stop();

        Assert.True(appLock.Disposed);
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
