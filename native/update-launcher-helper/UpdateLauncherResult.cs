using System.Text.Json.Serialization;

namespace Switchify.UpdateLauncher;

internal sealed class UpdateLauncherResult
{
    [JsonPropertyName("ok")]
    public required bool Ok { get; init; }

    [JsonPropertyName("status")]
    public required string Status { get; init; }

    [JsonPropertyName("pid")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public int? Pid { get; init; }

    [JsonPropertyName("win32Error")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public int? Win32Error { get; init; }
}
