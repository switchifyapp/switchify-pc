using System.ComponentModel;
using System.Runtime.CompilerServices;
using SwitchifyPc.Core.Bluetooth;
using SwitchifyPc.Core.Pairing;
using SwitchifyPc.Core.Updates;

namespace SwitchifyPc.Core.Ui;

public sealed class MainWindowViewModel : INotifyPropertyChanged
{
    private DesktopUiState desktopState = DesktopUiState.Starting;
    private BluetoothStatus bluetooth = BluetoothStatusModel.DefaultStatus with
    {
        Status = "starting",
        System = BluetoothStatusModel.DefaultSystemStatus with { AdapterPresent = true, RadioState = "unknown" }
    };
    private UpdateState updateState = UpdateState.CreateInitial("0.2.0");
    private IReadOnlyList<PendingPairingApprovalView> pairingApprovals = [];

    public event PropertyChangedEventHandler? PropertyChanged;

    public string AppTitle => "Switchify PC";

    public string StatusTitle => MainWindowCopy.BluetoothPrimary(desktopState, bluetooth).Title;

    public string StatusBody => MainWindowCopy.BluetoothPrimary(desktopState, bluetooth).Body;

    public string StatusTone => MainWindowCopy.BluetoothPrimary(desktopState, bluetooth).Tone;

    public string BluetoothStatus => MainWindowCopy.BluetoothStatusLabel(bluetooth);

    public string SystemBluetooth => MainWindowCopy.BluetoothSystemRadioState(bluetooth);

    public string BluetoothCapabilities => MainWindowCopy.BluetoothSystemCapabilities(bluetooth);

    public string BluetoothLastChecked => MainWindowCopy.Timestamp(bluetooth.System.LastCheckedAt);

    public string BluetoothLastChanged => MainWindowCopy.Timestamp(bluetooth.System.LastChangedAt);

    public string LastBluetoothEvent => MainWindowCopy.BluetoothEventSummary(bluetooth);

    public string RecentBluetoothEvents => MainWindowCopy.BluetoothRecentEvents(bluetooth);

    public string LastDisconnectReason => MainWindowCopy.BluetoothDisconnectSummary(bluetooth);

    public string RecentBluetoothError => MainWindowCopy.BluetoothRecentError(bluetooth);

    public bool HasUpdateBanner => MainWindowCopy.UpdateBanner(updateState) is not null;

    public string UpdateBannerTitle => MainWindowCopy.UpdateBanner(updateState)?.Title ?? "";

    public string UpdateBannerBody => MainWindowCopy.UpdateBanner(updateState)?.Body ?? "";

    public string UpdateBannerButtonText => MainWindowCopy.UpdateBanner(updateState)?.ButtonText ?? "Open updates";

    public string UpdateBannerTone => MainWindowCopy.UpdateBanner(updateState)?.Tone ?? "";

    public bool HasPairingApprovals => pairingApprovals.Count > 0;

    public IReadOnlyList<PendingPairingApprovalView> PairingApprovals => pairingApprovals;

    public void SetBluetoothState(DesktopUiState state, BluetoothStatus status)
    {
        desktopState = state;
        bluetooth = status;
        NotifyStatusChanged();
    }

    public void SetUpdateState(UpdateState state)
    {
        updateState = state;
        OnPropertyChanged(nameof(HasUpdateBanner));
        OnPropertyChanged(nameof(UpdateBannerTitle));
        OnPropertyChanged(nameof(UpdateBannerBody));
        OnPropertyChanged(nameof(UpdateBannerButtonText));
        OnPropertyChanged(nameof(UpdateBannerTone));
    }

    public void SetPairingApprovals(IReadOnlyList<PendingPairingApprovalView> approvals)
    {
        pairingApprovals = approvals
            .Select(approval => approval with { })
            .ToArray();
        OnPropertyChanged(nameof(HasPairingApprovals));
        OnPropertyChanged(nameof(PairingApprovals));
    }

    private void NotifyStatusChanged()
    {
        OnPropertyChanged(nameof(StatusTitle));
        OnPropertyChanged(nameof(StatusBody));
        OnPropertyChanged(nameof(StatusTone));
        OnPropertyChanged(nameof(BluetoothStatus));
        OnPropertyChanged(nameof(SystemBluetooth));
        OnPropertyChanged(nameof(BluetoothCapabilities));
        OnPropertyChanged(nameof(BluetoothLastChecked));
        OnPropertyChanged(nameof(BluetoothLastChanged));
        OnPropertyChanged(nameof(LastBluetoothEvent));
        OnPropertyChanged(nameof(RecentBluetoothEvents));
        OnPropertyChanged(nameof(LastDisconnectReason));
        OnPropertyChanged(nameof(RecentBluetoothError));
    }

    private void OnPropertyChanged([CallerMemberName] string? propertyName = null)
    {
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
    }
}
