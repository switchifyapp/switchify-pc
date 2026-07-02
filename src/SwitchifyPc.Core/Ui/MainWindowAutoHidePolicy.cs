namespace SwitchifyPc.Core.Ui;

public static class MainWindowAutoHidePolicy
{
    public static bool ShouldHideAfterPreviouslyUsedDeviceControl(
        bool isMainWindowVisible,
        bool hasPairingApprovals,
        bool isPreviouslyUsedDeviceSession)
    {
        return isMainWindowVisible &&
            isPreviouslyUsedDeviceSession &&
            !hasPairingApprovals;
    }
}
