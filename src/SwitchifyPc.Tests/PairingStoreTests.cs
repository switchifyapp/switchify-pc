using System.Text.Json;
using SwitchifyPc.Core.Pairing;

namespace SwitchifyPc.Tests;

public sealed class PairingStoreTests : IDisposable
{
    private readonly string tempDir = Path.Combine(Path.GetTempPath(), $"switchify-pairing-store-{Guid.NewGuid():N}");

    public void Dispose()
    {
        if (Directory.Exists(tempDir))
        {
            Directory.Delete(tempDir, recursive: true);
        }
    }

    [Fact]
    public void ToPairedDeviceViewsRemovesSharedTokens()
    {
        PairingState state = CreateState();

        IReadOnlyList<PairedDeviceView> views = PairingStateHelpers.ToPairedDeviceViews(state);

        Assert.Equal([new PairedDeviceView("android-1", "Phone", 1000, 2000), new PairedDeviceView("android-2", "Tablet", 3000, null)], views);
        Assert.DoesNotContain("secret-token", JsonSerializer.Serialize(views), StringComparison.Ordinal);
    }

    [Fact]
    public void RemovePairedDeviceRemovesOnlyMatchingDevice()
    {
        PairingState state = CreateState();

        PairingState nextState = PairingStateHelpers.RemovePairedDevice(state, "android-1");

        Assert.Equal("desktop-1", nextState.DesktopId);
        Assert.Single(nextState.PairedDevices);
        Assert.Equal("android-2", nextState.PairedDevices[0].DeviceId);
        Assert.Equal(2, state.PairedDevices.Count);
    }

    [Fact]
    public async Task MissingFileCreatesAndSavesFreshState()
    {
        string filePath = PairingPath();

        PairingState state = await new JsonPairingStore(filePath).LoadAsync();

        Assert.True(Guid.TryParse(state.DesktopId, out _));
        Assert.Empty(state.PairedDevices);
        Assert.Equal(state.DesktopId, JsonDocument.Parse(await File.ReadAllTextAsync(filePath)).RootElement.GetProperty("desktopId").GetString());
    }

    [Fact]
    public async Task ValidFileLoadsExistingState()
    {
        string filePath = PairingPath();
        Directory.CreateDirectory(tempDir);
        await File.WriteAllTextAsync(filePath, JsonSerializer.Serialize(new
        {
            desktopId = "desktop-1",
            pairedDevices = new[]
            {
                new { deviceId = "android-1", deviceName = "Phone", token = "secret-token-1", pairedAt = 1000, lastSeenAt = (int?)2000 }
            }
        }));

        PairingState state = await new JsonPairingStore(filePath).LoadAsync();

        Assert.Equal("desktop-1", state.DesktopId);
        Assert.Single(state.PairedDevices);
        Assert.Equal("secret-token-1", state.PairedDevices[0].Token);
    }

    [Fact]
    public async Task SaveWritesFormattedJsonAndCreatesParentDirectory()
    {
        string filePath = Path.Combine(tempDir, "nested", "pairing-state.json");

        await new JsonPairingStore(filePath).SaveAsync(CreateState());

        string raw = await File.ReadAllTextAsync(filePath);
        Assert.Contains("\"desktopId\": \"desktop-1\"", raw, StringComparison.Ordinal);
        Assert.EndsWith("\n", raw, StringComparison.Ordinal);
    }

    [Fact]
    public async Task SaveLeavesNoTempFilesAfterSuccess()
    {
        string filePath = PairingPath();

        await new JsonPairingStore(filePath).SaveAsync(CreateState());

        Assert.Empty(Directory.EnumerateFiles(tempDir, "*.tmp"));
    }

    [Fact]
    public async Task InvalidJsonIsBackedUpAndReplacedWithFreshState()
    {
        List<string> warnings = [];
        string filePath = PairingPath();
        Directory.CreateDirectory(tempDir);
        await File.WriteAllTextAsync(filePath, "{");

        PairingState state = await new JsonPairingStore(filePath, warnings.Add).LoadAsync();

        Assert.True(Guid.TryParse(state.DesktopId, out _));
        Assert.Empty(state.PairedDevices);
        Assert.Single(CorruptBackups());
        Assert.DoesNotContain("{", string.Join("\n", warnings), StringComparison.Ordinal);
    }

    [Fact]
    public async Task NulByteFileIsBackedUpAndReplacedWithFreshState()
    {
        string filePath = PairingPath();
        Directory.CreateDirectory(tempDir);
        await File.WriteAllTextAsync(filePath, new string('\0', 562));

        PairingState state = await new JsonPairingStore(filePath).LoadAsync();

        Assert.True(Guid.TryParse(state.DesktopId, out _));
        Assert.Empty(state.PairedDevices);
        Assert.Single(CorruptBackups());
        Assert.Contains("\"pairedDevices\": []", await File.ReadAllTextAsync(filePath), StringComparison.Ordinal);
    }

    [Fact]
    public async Task InvalidSchemaIsBackedUpAndReplacedWithFreshState()
    {
        string filePath = PairingPath();
        Directory.CreateDirectory(tempDir);
        await File.WriteAllTextAsync(filePath, "{\"desktopId\":1,\"pairedDevices\":[]}");

        PairingState state = await new JsonPairingStore(filePath).LoadAsync();

        Assert.Empty(state.PairedDevices);
        Assert.Single(CorruptBackups());
    }

    [Fact]
    public async Task InvalidPairedDeviceEntryDoesNotLogToken()
    {
        List<string> warnings = [];
        string filePath = PairingPath();
        Directory.CreateDirectory(tempDir);
        await File.WriteAllTextAsync(filePath, "{\"desktopId\":\"desktop-1\",\"pairedDevices\":[{\"deviceId\":\"android-1\",\"token\":\"secret-token\"}]}");

        await new JsonPairingStore(filePath, warnings.Add).LoadAsync();

        string warningText = string.Join("\n", warnings);
        Assert.DoesNotContain("secret-token", warningText, StringComparison.Ordinal);
        Assert.DoesNotContain("android-1", warningText, StringComparison.Ordinal);
    }

    private string PairingPath()
    {
        return Path.Combine(tempDir, "pairing-state.json");
    }

    private string[] CorruptBackups()
    {
        return Directory.EnumerateFiles(tempDir, "pairing-state.corrupt-*.json")
            .Select(Path.GetFileName)
            .Where(name => name is not null)
            .Cast<string>()
            .ToArray();
    }

    private static PairingState CreateState()
    {
        return new PairingState(
            "desktop-1",
            [
                new PairedDevice("android-1", "Phone", "secret-token-1", 1000, 2000),
                new PairedDevice("android-2", "Tablet", "secret-token-2", 3000, null)
            ]);
    }
}
