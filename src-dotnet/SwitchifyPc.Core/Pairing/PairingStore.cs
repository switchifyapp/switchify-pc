using System.Text.Json;
using System.Text.Json.Nodes;
using SwitchifyPc.Core.Storage;

namespace SwitchifyPc.Core.Pairing;

public interface IPairingStore
{
    Task<PairingState> LoadAsync(CancellationToken cancellationToken = default);
    Task SaveAsync(PairingState state, CancellationToken cancellationToken = default);
}

public sealed class JsonPairingStore : IPairingStore
{
    private readonly string filePath;
    private readonly Action<string> warn;

    public JsonPairingStore(string filePath, Action<string>? warn = null)
    {
        this.filePath = filePath;
        this.warn = warn ?? Console.WriteLine;
    }

    public async Task<PairingState> LoadAsync(CancellationToken cancellationToken = default)
    {
        try
        {
            string raw = await File.ReadAllTextAsync(filePath, cancellationToken);
            return ParsePairingState(JsonNode.Parse(raw));
        }
        catch (Exception error) when (IsMissingFileError(error))
        {
            PairingState state = PairingStateHelpers.CreateEmptyPairingState();
            await SaveAsync(state, cancellationToken);
            return state;
        }
        catch (Exception error) when (IsCorruptPairingStateError(error))
        {
            CorruptJsonBackupResult backup = await JsonFileStore.BackupCorruptJsonFileAsync(filePath);
            warn(
                backup.BackupPath is not null
                    ? "Switchify pairing state could not be loaded. The corrupt file was backed up and a fresh pairing state will be used."
                    : "Switchify pairing state could not be loaded. A fresh pairing state will be used.");
            PairingState state = PairingStateHelpers.CreateEmptyPairingState();
            await SaveAsync(state, cancellationToken);
            return state;
        }
    }

    public Task SaveAsync(PairingState state, CancellationToken cancellationToken = default)
    {
        string content = JsonSerializer.Serialize(ToJsonObject(state), JsonOptions) + "\n";
        return JsonFileStore.WriteJsonFileAtomicAsync(filePath, content, cancellationToken);
    }

    private static JsonObject ToJsonObject(PairingState state)
    {
        JsonArray devices = new(state.PairedDevices.Select(device => new JsonObject
        {
            ["deviceId"] = device.DeviceId,
            ["deviceName"] = device.DeviceName,
            ["token"] = device.Token,
            ["pairedAt"] = device.PairedAt,
            ["lastSeenAt"] = device.LastSeenAt
        }).ToArray<JsonNode?>());

        return new JsonObject
        {
            ["desktopId"] = state.DesktopId,
            ["pairedDevices"] = devices
        };
    }

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true
    };

    private static PairingState ParsePairingState(JsonNode? value)
    {
        if (value is not JsonObject obj ||
            obj["desktopId"]?.GetValueKind() != JsonValueKind.String ||
            obj["pairedDevices"] is not JsonArray devices)
        {
            throw new InvalidDataException("Invalid pairing state.");
        }

        return new PairingState(
            obj["desktopId"]!.GetValue<string>(),
            devices.Select(ParsePairedDevice).ToArray());
    }

    private static PairedDevice ParsePairedDevice(JsonNode? value)
    {
        if (value is not JsonObject obj ||
            obj["deviceId"]?.GetValueKind() != JsonValueKind.String ||
            obj["deviceName"]?.GetValueKind() != JsonValueKind.String ||
            obj["token"]?.GetValueKind() != JsonValueKind.String ||
            obj["pairedAt"]?.GetValueKind() != JsonValueKind.Number ||
            !IsNumberOrNull(obj["lastSeenAt"]))
        {
            throw new InvalidDataException("Invalid paired device.");
        }

        return new PairedDevice(
            obj["deviceId"]!.GetValue<string>(),
            obj["deviceName"]!.GetValue<string>(),
            obj["token"]!.GetValue<string>(),
            obj["pairedAt"]!.GetValue<double>(),
            obj["lastSeenAt"] is null ? null : obj["lastSeenAt"]!.GetValue<double>());
    }

    private static bool IsNumberOrNull(JsonNode? value)
    {
        return value is null || value.GetValueKind() == JsonValueKind.Number;
    }

    private static bool IsMissingFileError(Exception error)
    {
        return error is FileNotFoundException or DirectoryNotFoundException;
    }

    private static bool IsCorruptPairingStateError(Exception error)
    {
        return error is JsonException ||
            error is InvalidDataException { Message: "Invalid pairing state." or "Invalid paired device." };
    }
}

public sealed class MemoryPairingStore : IPairingStore
{
    private PairingState state;

    public MemoryPairingStore(PairingState? initialState = null)
    {
        state = PairingStateHelpers.CloneState(initialState ?? PairingStateHelpers.CreateEmptyPairingState());
    }

    public Task<PairingState> LoadAsync(CancellationToken cancellationToken = default)
    {
        return Task.FromResult(PairingStateHelpers.CloneState(state));
    }

    public Task SaveAsync(PairingState state, CancellationToken cancellationToken = default)
    {
        this.state = PairingStateHelpers.CloneState(state);
        return Task.CompletedTask;
    }
}

public static class PairingStateHelpers
{
    public static PairingState CreateEmptyPairingState()
    {
        return new PairingState(Guid.NewGuid().ToString(), []);
    }

    public static PairedDevice? FindPairedDevice(PairingState state, string deviceId)
    {
        return state.PairedDevices.FirstOrDefault(device => device.DeviceId == deviceId);
    }

    public static IReadOnlyList<PairedDeviceView> ToPairedDeviceViews(PairingState state)
    {
        return state.PairedDevices
            .Select(device => new PairedDeviceView(device.DeviceId, device.DeviceName, device.PairedAt, device.LastSeenAt))
            .ToArray();
    }

    public static PairingState UpsertPairedDevice(PairingState state, PairedDevice device)
    {
        List<PairedDevice> pairedDevices = state.PairedDevices
            .Where(existing => existing.DeviceId != device.DeviceId)
            .ToList();
        pairedDevices.Add(device);
        return state with { PairedDevices = pairedDevices };
    }

    public static PairingState RemovePairedDevice(PairingState state, string deviceId)
    {
        return state with
        {
            PairedDevices = state.PairedDevices
                .Where(device => device.DeviceId != deviceId)
                .ToArray()
        };
    }

    public static PairingState CloneState(PairingState state)
    {
        return new PairingState(
            state.DesktopId,
            state.PairedDevices
                .Select(device => device with { })
                .ToArray());
    }
}
