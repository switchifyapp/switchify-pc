using System.ComponentModel;
using System.Runtime.CompilerServices;
using SwitchifyPc.Core.Bluetooth;
using SwitchifyPc.Core.Pairing;
using SwitchifyPc.Core.Settings;
using SwitchifyPc.Core.Startup;

namespace SwitchifyPc.Core.Ui;

public enum SetupGuideStep
{
    Bluetooth,
    AndroidApp,
    PairDevice,
    Startup,
    Diagnostics
}

public static class SetupGuidePrompt
{
    public static bool ShouldAutoOpen(MainWindowPromptSettings settings, bool hasPairedDevices)
    {
        return !settings.SetupGuideShown &&
            !settings.SetupGuideCompleted &&
            !settings.AndroidDownloadDismissed &&
            !hasPairedDevices;
    }

    public static MainWindowPromptSettings MarkShown(MainWindowPromptSettings settings)
    {
        return settings with { SetupGuideShown = true };
    }

    public static MainWindowPromptSettings MarkCompleted(MainWindowPromptSettings settings)
    {
        return settings with { SetupGuideShown = true, SetupGuideCompleted = true };
    }
}

public sealed class SetupGuideViewModel : INotifyPropertyChanged
{
    private SetupGuideStep currentStep;
    private BluetoothStatus bluetoothStatus = BluetoothStatusModel.DefaultStatus;
    private IReadOnlyList<PendingPairingApprovalView> pairingApprovals = [];
    private bool hasPairedDevices;
    private SystemStartupSettings startupSettings = new(
        Supported: false,
        StartWithSystem: false,
        StartsHidden: true,
        Reason: "unpackaged");
    private bool? shareDiagnosticData;

    public event PropertyChangedEventHandler? PropertyChanged;

    public SetupGuideStep CurrentStep => currentStep;

    public int StepNumber => (int)currentStep + 1;

    public string StepProgress => $"Step {StepNumber} of 5";

    public bool IsBluetoothStep => currentStep == SetupGuideStep.Bluetooth;

    public bool IsAndroidAppStep => currentStep == SetupGuideStep.AndroidApp;

    public bool IsPairDeviceStep => currentStep == SetupGuideStep.PairDevice;

    public bool IsStartupStep => currentStep == SetupGuideStep.Startup;

    public bool IsDiagnosticsStep => currentStep == SetupGuideStep.Diagnostics;

    public bool? ShareDiagnosticData => shareDiagnosticData;

    public bool IsDiagnosticChoiceRequired => IsDiagnosticsStep && shareDiagnosticData is null;

    public bool CanGoBack => currentStep != SetupGuideStep.Bluetooth;

    public bool CanGoNext => (currentStep != SetupGuideStep.PairDevice || hasPairedDevices) && !IsDiagnosticChoiceRequired;

    public bool IsFinalStep => currentStep == SetupGuideStep.Diagnostics;

    public string NextButtonText => IsFinalStep ? "Finish" : "Next";

    public string AndroidDownloadUrl => AndroidDownloadPrompt.GooglePlayUrl;

    public bool BluetoothReady => bluetoothStatus.System.AdapterPresent &&
        bluetoothStatus.System.RadioState == "on" &&
        bluetoothStatus.System.IsLowEnergySupported == true &&
        bluetoothStatus.System.IsPeripheralRoleSupported == true;

    public string BluetoothSetupTitle
    {
        get
        {
            if (!bluetoothStatus.System.AdapterPresent) return "No Bluetooth adapter found";
            if (bluetoothStatus.System.RadioState == "off") return "Bluetooth is off";
            if (BluetoothReady) return "Bluetooth is ready";
            if (bluetoothStatus.System.RadioState == "on") return "Bluetooth support is limited";
            return "Checking Bluetooth...";
        }
    }

    public string BluetoothSetupBody
    {
        get
        {
            if (!bluetoothStatus.System.AdapterPresent) return "Connect a Bluetooth adapter before pairing an Android device.";
            if (bluetoothStatus.System.RadioState == "off") return "Turn on Bluetooth in Windows, then return here.";
            if (BluetoothReady) return "This PC can advertise the Switchify pairing service.";
            if (bluetoothStatus.System.RadioState == "on") return "This adapter may not support the Bluetooth Low Energy peripheral role required by Switchify.";
            return "Switchify PC is reading the Windows Bluetooth state.";
        }
    }

    public IReadOnlyList<PendingPairingApprovalView> PairingApprovals => pairingApprovals;

    public bool HasPairingApprovals => pairingApprovals.Count > 0;

    public bool HasPairedDevices => hasPairedDevices;

    public string PairingStatusTitle => hasPairedDevices ? "Android device paired" : "Waiting for an Android device";

    public string PairingStatusBody => hasPairedDevices
        ? "Secure pairing is complete. You can continue setup."
        : "Open Switchify on Android near this PC and choose this computer.";

    public bool StartWithSystemSupported => startupSettings.Supported;

    public bool StartWithSystem => startupSettings.StartWithSystem;

    public string StartWithSystemMessage
    {
        get
        {
            if (!startupSettings.Supported)
            {
                return startupSettings.Reason == "unpackaged"
                    ? "Start with system is available in the installed app."
                    : "Start with system is not available on this platform.";
            }

            return startupSettings.StartWithSystem
                ? "Switchify PC will start hidden when you sign in."
                : "Switchify PC will not start when you sign in.";
        }
    }

    public bool MoveNext()
    {
        if (!CanGoNext || IsFinalStep) return false;
        currentStep++;
        NotifyStepChanged();
        return true;
    }

    public bool MoveBack()
    {
        if (!CanGoBack) return false;
        currentStep--;
        NotifyStepChanged();
        return true;
    }

    public void SetBluetoothStatus(BluetoothStatus status)
    {
        bluetoothStatus = status;
        OnPropertyChanged(nameof(BluetoothReady));
        OnPropertyChanged(nameof(BluetoothSetupTitle));
        OnPropertyChanged(nameof(BluetoothSetupBody));
    }

    public void SetPairingApprovals(IReadOnlyList<PendingPairingApprovalView> approvals)
    {
        pairingApprovals = approvals.Select(approval => approval with { }).ToArray();
        OnPropertyChanged(nameof(PairingApprovals));
        OnPropertyChanged(nameof(HasPairingApprovals));
    }

    public void SetHasPairedDevices(bool value)
    {
        hasPairedDevices = value;
        OnPropertyChanged(nameof(HasPairedDevices));
        OnPropertyChanged(nameof(PairingStatusTitle));
        OnPropertyChanged(nameof(PairingStatusBody));
        OnPropertyChanged(nameof(CanGoNext));
    }

    public void SetStartupSettings(SystemStartupSettings settings)
    {
        startupSettings = settings;
        OnPropertyChanged(nameof(StartWithSystemSupported));
        OnPropertyChanged(nameof(StartWithSystem));
        OnPropertyChanged(nameof(StartWithSystemMessage));
    }

    public void SetShareDiagnosticData(bool enabled)
    {
        shareDiagnosticData = enabled;
        OnPropertyChanged(nameof(ShareDiagnosticData));
        OnPropertyChanged(nameof(IsDiagnosticChoiceRequired));
        OnPropertyChanged(nameof(CanGoNext));
    }

    private void NotifyStepChanged()
    {
        OnPropertyChanged(nameof(CurrentStep));
        OnPropertyChanged(nameof(StepNumber));
        OnPropertyChanged(nameof(StepProgress));
        OnPropertyChanged(nameof(IsBluetoothStep));
        OnPropertyChanged(nameof(IsAndroidAppStep));
        OnPropertyChanged(nameof(IsPairDeviceStep));
        OnPropertyChanged(nameof(IsStartupStep));
        OnPropertyChanged(nameof(IsDiagnosticsStep));
        OnPropertyChanged(nameof(IsDiagnosticChoiceRequired));
        OnPropertyChanged(nameof(CanGoBack));
        OnPropertyChanged(nameof(CanGoNext));
        OnPropertyChanged(nameof(IsFinalStep));
        OnPropertyChanged(nameof(NextButtonText));
    }

    private void OnPropertyChanged([CallerMemberName] string? propertyName = null)
    {
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
    }
}
