using SwitchifyPc.Core.Bluetooth;

namespace SwitchifyPc.Tests;

public sealed class BluetoothStatusTrackerTests
{
    [Fact]
    public void SystemStatusSetsCheckedAndChangedTimestamps()
    {
        double now = 1000;
        BluetoothStatusTracker tracker = new(() => now);

        BluetoothStatus status = tracker.SetSystemStatus(new BluetoothSystemStatusEvent(true, "on", true, true));

        Assert.Equal(1000, status.System.LastCheckedAt);
        Assert.Equal(1000, status.System.LastChangedAt);
    }

    [Fact]
    public void IdenticalSystemStatusUpdatesCheckedButPreservesChangedTimestamp()
    {
        double now = 1000;
        BluetoothStatusTracker tracker = new(() => now);
        tracker.SetSystemStatus(new BluetoothSystemStatusEvent(true, "on", true, true));
        now = 2000;

        BluetoothStatus status = tracker.SetSystemStatus(new BluetoothSystemStatusEvent(true, "on", true, true));

        Assert.Equal(2000, status.System.LastCheckedAt);
        Assert.Equal(1000, status.System.LastChangedAt);
    }

    [Fact]
    public void RadioOffRemovesConnectionsAndMarksAdapterOff()
    {
        double now = 1000;
        BluetoothStatusTracker tracker = new(() => now);
        tracker.SetReady();
        tracker.AddConnection("ble");

        BluetoothStatus status = tracker.SetSystemStatus(new BluetoothSystemStatusEvent(true, "off", true, true));

        Assert.Equal("unavailable", status.Status);
        Assert.Equal("adapter_off", status.Reason);
        Assert.Equal(0, status.ConnectedClientCount);
        Assert.Equal("adapter_off", status.LastDisconnectReason);
    }

    [Fact]
    public void RadioOnAfterAdapterOffMovesToStarting()
    {
        BluetoothStatusTracker tracker = new(() => 1000);
        tracker.SetSystemStatus(new BluetoothSystemStatusEvent(true, "off", true, true));

        BluetoothStatus status = tracker.SetSystemStatus(new BluetoothSystemStatusEvent(true, "on", true, true));

        Assert.Equal("starting", status.Status);
        Assert.Null(status.Reason);
    }

    [Fact]
    public void AdapterMissingReportsUnsupported()
    {
        BluetoothStatusTracker tracker = new(() => 1000);

        BluetoothStatus status = tracker.SetSystemStatus(new BluetoothSystemStatusEvent(false, "unknown", null, null));

        Assert.Equal("unavailable", status.Status);
        Assert.Equal("unsupported", status.Reason);
    }

    [Fact]
    public void DiagnosticsKeepNewestFiveEvents()
    {
        double now = 1000;
        BluetoothStatusTracker tracker = new(() => now++);

        foreach (string diagnosticEvent in new[] { "one", "two", "three", "four", "five", "six" })
        {
            tracker.RecordDiagnostic(diagnosticEvent);
        }

        Assert.Equal("six", tracker.Status.LastEvent);
        Assert.Equal(["two", "three", "four", "five", "six"], tracker.Status.RecentEvents.Select(record => record.Event).ToArray());
    }

    [Fact]
    public void NotifiesOnStatusChange()
    {
        List<BluetoothStatus> changes = [];
        BluetoothStatusTracker tracker = new(() => 1000, changes.Add);

        tracker.SetReady();

        Assert.Single(changes);
        Assert.Equal("ready", changes[0].Status);
    }
}
