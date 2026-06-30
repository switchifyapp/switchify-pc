namespace SwitchifyPc.Core.Pairing;

public static class PairingVerificationCode
{
    public const int CodeLength = 6;

    public static string Create(string desktopId, string deviceId, string requestNonce)
    {
        string canonical = $"{desktopId}\n{deviceId}\n{requestNonce}";
        int hash = unchecked((int)2166136261);
        foreach (char character in canonical)
        {
            hash ^= character;
            hash = unchecked(hash * 16777619);
        }

        int value = Math.Abs(hash) % 1_000_000;
        return value.ToString().PadLeft(CodeLength, '0');
    }
}
