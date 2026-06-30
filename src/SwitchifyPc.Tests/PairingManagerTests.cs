using SwitchifyPc.Core.Pairing;

namespace SwitchifyPc.Tests;

public sealed class PairingManagerTests
{
    [Fact]
    public async Task CreatesPersistentDesktopIdOnFirstLoad()
    {
        MemoryPairingStore store = new();
        PairingManager manager = new(store);

        string first = await manager.GetDesktopIdAsync();
        string second = await manager.GetDesktopIdAsync();

        Assert.True(Guid.TryParse(first, out _));
        Assert.Equal(first, second);
    }

    [Fact]
    public void CreatesSharedTokens()
    {
        string first = PairingToken.CreateToken();
        string second = PairingToken.CreateToken();

        Assert.True(first.Length > 20);
        Assert.NotEqual(first, second);
    }
}
