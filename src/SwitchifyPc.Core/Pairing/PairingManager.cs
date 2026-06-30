using System.Security.Cryptography;

namespace SwitchifyPc.Core.Pairing;

public sealed class PairingManager
{
    private readonly IPairingStore store;

    public PairingManager(IPairingStore store)
    {
        this.store = store;
    }

    public async Task<string> GetDesktopIdAsync(CancellationToken cancellationToken = default)
    {
        return (await store.LoadAsync(cancellationToken)).DesktopId;
    }
}

public static class PairingToken
{
    public const int TokenByteLength = 32;

    public static string CreateToken(int byteLength = TokenByteLength)
    {
        return Base64UrlEncode(RandomNumberGenerator.GetBytes(byteLength));
    }

    private static string Base64UrlEncode(byte[] value)
    {
        return Convert.ToBase64String(value)
            .TrimEnd('=')
            .Replace("+", "-", StringComparison.Ordinal)
            .Replace("/", "_", StringComparison.Ordinal);
    }
}
