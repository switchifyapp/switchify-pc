using System.Text.RegularExpressions;

namespace SwitchifyPc.Core.Diagnostics;

public static partial class TelemetrySanitizer
{
    public const int MaximumMessageLength = 300;
    public const int MaximumStackLength = 128 * 1024;

    public static string? Message(string? value) => Sanitize(value, MaximumMessageLength);

    public static string? Stack(string? value) => Sanitize(value, MaximumStackLength);

    private static string? Sanitize(string? value, int maximumLength)
    {
        if (string.IsNullOrWhiteSpace(value)) return null;
        string text = value.Replace('\0', ' ').Trim();
        text = BearerPattern().Replace(text, "$1[redacted]");
        text = SecretPattern().Replace(text, "$1[redacted]");
        text = TimberlogsKeyPattern().Replace(text, "[redacted]");
        text = UserPathPattern().Replace(text, "$1\\Users\\[redacted]");
        return text.Length <= maximumLength ? text : text[..maximumLength];
    }

    [GeneratedRegex("(?i)(authorization\\s*[:=]\\s*bearer\\s+|bearer\\s+)[^\\s,;]+")]
    private static partial Regex BearerPattern();

    [GeneratedRegex("(?i)(api[_ -]?key|token|password|secret|pairing[_ -]?code)(\\s*[:=]\\s*)[^\\s,;]+")]
    private static partial Regex SecretPattern();

    [GeneratedRegex("(?i)\\btb_[a-z0-9_-]+")]
    private static partial Regex TimberlogsKeyPattern();

    [GeneratedRegex("(?i)([a-z]:)\\\\Users\\\\[^\\\\\r\n]+")]
    private static partial Regex UserPathPattern();
}
