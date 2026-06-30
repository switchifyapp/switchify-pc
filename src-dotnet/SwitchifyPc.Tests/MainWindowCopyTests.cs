using SwitchifyPc.Core.Bluetooth;
using SwitchifyPc.Core.Ui;
using SwitchifyPc.Core.Updates;

namespace SwitchifyPc.Tests;

public sealed class MainWindowCopyTests
{
    [Fact]
    public void ShowsConnectedBluetoothCopy()
    {
        BluetoothPrimaryCopy copy = MainWindowCopy.BluetoothPrimary(DesktopUiState.Connected, BluetoothStatusModel.DefaultStatus);

        Assert.Equal("Your device is connected", copy.Title);
        Assert.Equal("You can control this PC from Switchify over Bluetooth.", copy.Body);
        Assert.Equal("ready", copy.Tone);
        Assert.Equal("Connected", MainWindowCopy.StatusBadgeLabel(DesktopUiState.Connected));
        Assert.Equal("connected", MainWindowCopy.StatusBadgeTone(DesktopUiState.Connected));
        Assert.Equal("Starting...", MainWindowCopy.StatusBadgeLabel(DesktopUiState.Starting));
        Assert.Equal("waiting", MainWindowCopy.StatusBadgeTone(DesktopUiState.Starting));
        Assert.Equal("Needs attention", MainWindowCopy.StatusBadgeLabel(DesktopUiState.ServerError));
        Assert.Equal("error", MainWindowCopy.StatusBadgeTone(DesktopUiState.ServerError));
    }

    [Fact]
    public void ShowsAutoReconnectCopyWhenBluetoothRadioIsOff()
    {
        BluetoothPrimaryCopy copy = MainWindowCopy.BluetoothPrimary(
            DesktopUiState.WaitingForDevice,
            Status(system: BluetoothStatusModel.DefaultSystemStatus with { AdapterPresent = true, RadioState = "off" }));

        Assert.Equal("Waiting for your device", copy.Title);

        copy = MainWindowCopy.BluetoothPrimary(
            DesktopUiState.Starting,
            Status(system: BluetoothStatusModel.DefaultSystemStatus with { AdapterPresent = true, RadioState = "off" }));

        Assert.Equal("Bluetooth is off", copy.Title);
        Assert.Equal("Turn on Bluetooth in Windows. Switchify PC will reconnect automatically.", copy.Body);
        Assert.Equal("error", copy.Tone);
    }

    [Fact]
    public void ShowsRestartingCopyWhenStartingWithRadioOn()
    {
        BluetoothPrimaryCopy copy = MainWindowCopy.BluetoothPrimary(
            DesktopUiState.Starting,
            Status("starting", system: BluetoothStatusModel.DefaultSystemStatus with { AdapterPresent = true, RadioState = "on" }));

        Assert.Equal("Getting Bluetooth ready...", copy.Title);
        Assert.Equal("Switchify PC is restarting nearby device connection.", copy.Body);
    }

    [Fact]
    public void FormatsBluetoothStatusLabels()
    {
        Assert.Equal("Bluetooth status unknown.", MainWindowCopy.BluetoothStatusLabel(null));
        Assert.Equal("Bluetooth adapter not found.", MainWindowCopy.BluetoothStatusLabel(Status()));
        Assert.Equal("Bluetooth radio off.", MainWindowCopy.BluetoothStatusLabel(Status(system: BluetoothStatusModel.DefaultSystemStatus with { AdapterPresent = true, RadioState = "off" })));
        Assert.Equal("Bluetooth ready.", MainWindowCopy.BluetoothStatusLabel(Status("ready", system: BluetoothSystemOn())));
        Assert.Equal("Bluetooth device connected.", MainWindowCopy.BluetoothStatusLabel(Status("connected", connectedClientCount: 1, system: BluetoothSystemOn())));
        Assert.Equal("Bluetooth devices connected.", MainWindowCopy.BluetoothStatusLabel(Status("connected", connectedClientCount: 2, system: BluetoothSystemOn())));
        Assert.Equal("Bluetooth is off.", MainWindowCopy.BluetoothStatusLabel(Status("unavailable", "adapter_off", system: BluetoothSystemOn())));
        Assert.Equal("Bluetooth permission denied.", MainWindowCopy.BluetoothStatusLabel(Status("unavailable", "permission_denied", system: BluetoothSystemOn())));
    }

    [Fact]
    public void FormatsBluetoothSystemDetailsAndEvents()
    {
        Assert.Equal("Bluetooth radio on.", MainWindowCopy.BluetoothSystemRadioState(Status(system: BluetoothSystemOn())));
        Assert.Equal("Bluetooth LE peripheral supported.", MainWindowCopy.BluetoothSystemCapabilities(Status(system: BluetoothSystemOn())));
        Assert.Equal("Bluetooth LE peripheral not supported.", MainWindowCopy.BluetoothSystemCapabilities(Status(system: BluetoothSystemOn() with { IsPeripheralRoleSupported = false })));
        Assert.Equal("Not yet.", MainWindowCopy.Timestamp(null));
        Assert.Equal("Not yet.", MainWindowCopy.Timestamp(0));
        Assert.Equal("01:02:03", MainWindowCopy.Timestamp(3_723_000));
        Assert.Equal("Bluetooth turned on.", MainWindowCopy.BluetoothDiagnosticEvent("system_radio_on"));
        Assert.Equal("Advertising restarted.", MainWindowCopy.BluetoothDiagnosticEvent("advertising_restarted"));
        Assert.Equal("Bluetooth was turned off.", MainWindowCopy.BluetoothDisconnectReason("adapter_off"));
        Assert.Equal(
            "Bluetooth turned on. 01:02:03",
            MainWindowCopy.BluetoothEventSummary(Status(lastEvent: "system_radio_on", lastEventAt: 3_723_000)));
        Assert.Equal(
            "Bluetooth turned on. 01:02:03 Advertising restarted. 01:02:04",
            MainWindowCopy.BluetoothRecentEvents(Status(recentEvents:
            [
                new BluetoothDiagnosticRecord("system_radio_on", 3_723_000),
                new BluetoothDiagnosticRecord("advertising_restarted", 3_724_000)
            ])));
        Assert.Equal(
            "Bluetooth was turned off. 01:02:03",
            MainWindowCopy.BluetoothDisconnectSummary(Status(lastDisconnectReason: "adapter_off", lastDisconnectAt: 3_723_000)));
        Assert.Equal("No recent errors.", MainWindowCopy.BluetoothRecentError(Status()));
        Assert.Equal("startup_failed", MainWindowCopy.BluetoothRecentError(Status(lastError: "startup_failed")));
    }

    [Fact]
    public void ShowsUpdateBannerForAvailableOrDownloadedUpdates()
    {
        UpdateState available = UpdateState.CreateInitial("0.1.20") with
        {
            Info = UpdateState.CreateInitial("0.1.20").Info with
            {
                Status = UpdateCheckStatus.UpdateAvailable,
                LatestVersion = "0.2.0"
            }
        };
        UpdateState downloaded = available with
        {
            Download = available.Download with { Status = UpdateDownloadStatus.Downloaded }
        };

        UpdateBannerCopy? availableCopy = MainWindowCopy.UpdateBanner(available);
        UpdateBannerCopy? downloadedCopy = MainWindowCopy.UpdateBanner(downloaded);

        Assert.NotNull(availableCopy);
        Assert.Equal("Update available", availableCopy.Title);
        Assert.Equal("A new Switchify PC update is ready to download.", availableCopy.Body);
        Assert.Equal("Open updates", availableCopy.ButtonText);
        Assert.NotNull(downloadedCopy);
        Assert.Equal("Update ready to install", downloadedCopy.Title);
        Assert.Equal("The update has been downloaded and is ready to install.", downloadedCopy.Body);
        Assert.Null(MainWindowCopy.UpdateBanner(UpdateState.CreateInitial("0.1.20")));
    }

    private static BluetoothSystemStatus BluetoothSystemOn() =>
        BluetoothStatusModel.DefaultSystemStatus with
        {
            AdapterPresent = true,
            RadioState = "on",
            IsLowEnergySupported = true,
            IsPeripheralRoleSupported = true
        };

    private static BluetoothStatus Status(
        string status = "disabled",
        string? reason = null,
        int connectedClientCount = 0,
        BluetoothSystemStatus? system = null,
        string? lastEvent = null,
        double? lastEventAt = null,
        IReadOnlyList<BluetoothDiagnosticRecord>? recentEvents = null,
        string? lastDisconnectReason = null,
        double? lastDisconnectAt = null,
        string? lastError = null) =>
        BluetoothStatusModel.DefaultStatus with
        {
            Status = status,
            Reason = reason,
            ConnectedClientCount = connectedClientCount,
            System = system ?? BluetoothStatusModel.DefaultSystemStatus,
            LastEvent = lastEvent,
            LastEventAt = lastEventAt,
            RecentEvents = recentEvents ?? [],
            LastDisconnectReason = lastDisconnectReason,
            LastDisconnectAt = lastDisconnectAt,
            LastError = lastError
        };
}
