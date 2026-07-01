using SwitchifyPc.Core.Ui;

namespace SwitchifyPc.Tests;

public sealed class MainWindowAutoHidePolicyTests
{
    [Fact]
    public void HidesVisibleMainWindowWhenDeviceConnects()
    {
        Assert.True(MainWindowAutoHidePolicy.ShouldHideAfterDeviceConnected(
            isMainWindowVisible: true,
            hasPairingApprovals: false));
    }

    [Fact]
    public void DoesNotHideWhenMainWindowIsAlreadyHidden()
    {
        Assert.False(MainWindowAutoHidePolicy.ShouldHideAfterDeviceConnected(
            isMainWindowVisible: false,
            hasPairingApprovals: false));
    }

    [Fact]
    public void DoesNotHideWhenPairingApprovalIsVisible()
    {
        Assert.False(MainWindowAutoHidePolicy.ShouldHideAfterDeviceConnected(
            isMainWindowVisible: true,
            hasPairingApprovals: true));
    }
}
