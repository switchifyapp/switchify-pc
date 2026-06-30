using SwitchifyPc.Core.Bluetooth;
using SwitchifyPc.Protocol;

namespace SwitchifyPc.Tests;

public sealed class BluetoothHelperProtocolTests
{
    [Fact]
    public void ExposesBluetoothUuidConstants()
    {
        Assert.Equal(Guid.Parse("7a78f7e8-1d6d-4d92-9ef0-1f89d3db21f4"), BluetoothHelperProtocol.ServiceUuid);
        Assert.Equal(Guid.Parse("7a78f7e9-1d6d-4d92-9ef0-1f89d3db21f4"), BluetoothHelperProtocol.RxCharacteristicUuid);
        Assert.Equal(Guid.Parse("7a78f7ea-1d6d-4d92-9ef0-1f89d3db21f4"), BluetoothHelperProtocol.TxCharacteristicUuid);
        Assert.Equal(Guid.Parse("7a78f7eb-1d6d-4d92-9ef0-1f89d3db21f4"), BluetoothHelperProtocol.StatusCharacteristicUuid);
    }

    [Fact]
    public void ParsesDiagnosticAndDisconnectedEvents()
    {
        AssertParsed("""{"type":"diagnostic","event":"subscribed"}""", new BluetoothDiagnosticEvent("subscribed"));
        AssertParsed("""{"type":"diagnostic","event":"system_radio_on"}""", new BluetoothDiagnosticEvent("system_radio_on"));
        AssertParsed("""{"type":"diagnostic","event":"system_radio_off"}""", new BluetoothDiagnosticEvent("system_radio_off"));
        AssertParsed("""{"type":"diagnostic","event":"advertising_restarted"}""", new BluetoothDiagnosticEvent("advertising_restarted"));
        AssertParsed("""{"type":"disconnected","connectionId":"ble","reason":"adapter_off"}""", new BluetoothDisconnectedEvent("ble", "adapter_off"));
    }

    [Fact]
    public void ParsesLiveSystemBluetoothStatusWithoutRawIdentifiers()
    {
        Assert.True(BluetoothHelperProtocol.TryParseEvent(
            """{"type":"systemStatus","adapterPresent":true,"radioState":"on","isLowEnergySupported":true,"isPeripheralRoleSupported":true,"deviceId":"not-forwarded"}""",
            out BluetoothHelperEvent? helperEvent));

        BluetoothSystemStatusEvent status = Assert.IsType<BluetoothSystemStatusEvent>(helperEvent);
        Assert.True(status.AdapterPresent);
        Assert.Equal("on", status.RadioState);
        Assert.True(status.IsLowEnergySupported);
        Assert.True(status.IsPeripheralRoleSupported);
    }

    [Fact]
    public void ParsesSystemBluetoothStatusWithNullCapabilities()
    {
        AssertParsed(
            """{"type":"systemStatus","adapterPresent":false,"radioState":"unknown","isLowEnergySupported":null,"isPeripheralRoleSupported":null}""",
            new BluetoothSystemStatusEvent(false, "unknown", null, null));
    }

    [Fact]
    public void RejectsMalformedSystemStatusAndDiagnostics()
    {
        Assert.False(BluetoothHelperProtocol.TryParseEvent(
            """{"type":"systemStatus","adapterPresent":true,"radioState":"pairing-token","isLowEnergySupported":true,"isPeripheralRoleSupported":true}""",
            out _));
        Assert.False(BluetoothHelperProtocol.TryParseEvent("""{"type":"diagnostic","event":"payload:secret"}""", out _));
    }

    [Fact]
    public void ParsesMessageEventsWithValidFrames()
    {
        string frame = $$"""
        {
          "type": "message",
          "connectionId": "ble",
          "frame": {
            "version": {{BluetoothFrameCodec.BluetoothFrameVersion}},
            "messageId": "message-1",
            "sequence": 0,
            "isFinal": true,
            "totalBytes": 2,
            "payloadBase64": "e30="
          }
        }
        """;

        Assert.True(BluetoothHelperProtocol.TryParseEvent(frame, out BluetoothHelperEvent? helperEvent));
        BluetoothMessageEvent message = Assert.IsType<BluetoothMessageEvent>(helperEvent);
        Assert.Equal("ble", message.ConnectionId);
        Assert.Equal("message-1", message.Frame.MessageId);
    }

    private static void AssertParsed(string json, BluetoothHelperEvent expected)
    {
        Assert.True(BluetoothHelperProtocol.TryParseEvent(json, out BluetoothHelperEvent? actual));
        Assert.Equal(expected, actual);
    }
}
