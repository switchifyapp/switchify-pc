using System.Text.Json.Serialization;

namespace Switchify.CursorOverlay;

internal sealed class OverlayCommand
{
    [JsonPropertyName("type")]
    public string? Type { get; init; }

    [JsonPropertyName("event")]
    public string? Event { get; init; }

    [JsonPropertyName("x")]
    public double X { get; init; }

    [JsonPropertyName("y")]
    public double Y { get; init; }

    [JsonPropertyName("size")]
    public int Size { get; init; }

    [JsonPropertyName("durationMs")]
    public int DurationMs { get; init; }

    [JsonPropertyName("crosshairs")]
    public bool Crosshairs { get; init; }

    [JsonPropertyName("persistent")]
    public bool Persistent { get; init; }

    [JsonPropertyName("color")]
    public OverlayColor? Color { get; init; }
}

internal sealed class OverlayColor
{
    [JsonPropertyName("red")]
    public int Red { get; init; }

    [JsonPropertyName("green")]
    public int Green { get; init; }

    [JsonPropertyName("blue")]
    public int Blue { get; init; }
}
