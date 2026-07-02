using SwitchifyPc.Core.Ui;

namespace SwitchifyPc.Tests;

public sealed class MainWindowAutoHidePolicyTests
{
    [Fact]
    public void HidesVisibleMainWindowForPreviouslyUsedDeviceControl()
    {
        Assert.True(MainWindowAutoHidePolicy.ShouldHideAfterPreviouslyUsedDeviceControl(
            isMainWindowVisible: true,
            hasPairingApprovals: false,
            isPreviouslyUsedDeviceSession: true));
    }

    [Fact]
    public void DoesNotHideWhenMainWindowIsAlreadyHidden()
    {
        Assert.False(MainWindowAutoHidePolicy.ShouldHideAfterPreviouslyUsedDeviceControl(
            isMainWindowVisible: false,
            hasPairingApprovals: false,
            isPreviouslyUsedDeviceSession: true));
    }

    [Fact]
    public void DoesNotHideWhenPairingApprovalIsVisible()
    {
        Assert.False(MainWindowAutoHidePolicy.ShouldHideAfterPreviouslyUsedDeviceControl(
            isMainWindowVisible: true,
            hasPairingApprovals: true,
            isPreviouslyUsedDeviceSession: true));
    }

    [Fact]
    public void DoesNotHideForNewDeviceSession()
    {
        Assert.False(MainWindowAutoHidePolicy.ShouldHideAfterPreviouslyUsedDeviceControl(
            isMainWindowVisible: true,
            hasPairingApprovals: false,
            isPreviouslyUsedDeviceSession: false));
    }
}
