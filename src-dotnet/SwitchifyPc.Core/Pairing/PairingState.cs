namespace SwitchifyPc.Core.Pairing;

public sealed record PairedDevice(
    string DeviceId,
    string DeviceName,
    string Token,
    double PairedAt,
    double? LastSeenAt);

public sealed record PairingState(string DesktopId, IReadOnlyList<PairedDevice> PairedDevices);

public sealed record PairedDeviceView(string DeviceId, string DeviceName, double PairedAt, double? LastSeenAt);
