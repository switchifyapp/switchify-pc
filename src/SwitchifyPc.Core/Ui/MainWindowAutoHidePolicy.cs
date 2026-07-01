namespace SwitchifyPc.Core.Ui;

public static class MainWindowAutoHidePolicy
{
    public static bool ShouldHideAfterDeviceConnected(
        bool isMainWindowVisible,
        bool hasPairingApprovals)
    {
        return isMainWindowVisible && !hasPairingApprovals;
    }
}
