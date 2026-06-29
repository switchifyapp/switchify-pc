using System.Text.Json;
using System.Text.Json.Nodes;
using SwitchifyPc.Core.Storage;

namespace SwitchifyPc.Core.Settings;

public sealed record CursorOverlayColorInfo(string Label, int[] Rgb, string Hex);

public sealed record CursorOverlaySettings(
    bool Enabled,
    string Size,
    string Visibility,
    bool Crosshairs,
    string Color);

public static class CursorOverlaySettingsModel
{
    public static readonly CursorOverlaySettings Default = new(
        Enabled: true,
        Size: "medium",
        Visibility: "onInput",
        Crosshairs: false,
        Color: "red");

    public static readonly IReadOnlyDictionary<string, int> SizePixels = new Dictionary<string, int>(StringComparer.Ordinal)
    {
        ["small"] = 96,
        ["medium"] = 128,
        ["large"] = 176
    };

    public static readonly IReadOnlyDictionary<string, CursorOverlayColorInfo> Colors =
        new Dictionary<string, CursorOverlayColorInfo>(StringComparer.Ordinal)
        {
            ["red"] = new("Red", [211, 47, 47], "#d32f2f"),
            ["green"] = new("Green", [132, 255, 145], "#84ff91"),
            ["blue"] = new("Blue", [100, 166, 255], "#64a6ff"),
            ["yellow"] = new("Yellow", [255, 209, 102], "#ffd166"),
            ["white"] = new("White", [255, 255, 255], "#ffffff")
        };

    private static readonly HashSet<string> VisibilityValues = new(StringComparer.Ordinal)
    {
        "onInput",
        "whileControlling"
    };

    public static CursorOverlaySettings Normalize(JsonNode? value)
    {
        if (value is not JsonObject candidate)
        {
            return Default;
        }

        return new CursorOverlaySettings(
            Enabled: BooleanOrDefault(candidate, "enabled", Default.Enabled),
            Size: StringSetOrDefault(candidate, "size", SizePixels.Keys, Default.Size),
            Visibility: StringSetOrDefault(candidate, "visibility", VisibilityValues, Default.Visibility),
            Crosshairs: BooleanOrDefault(candidate, "crosshairs", Default.Crosshairs),
            Color: StringSetOrDefault(candidate, "color", Colors.Keys, Default.Color));
    }

    public static CursorOverlaySettings Normalize(object? value)
    {
        return Normalize(JsonSerializer.SerializeToNode(value));
    }

    public static CursorOverlaySettings Normalize(CursorOverlaySettings settings)
    {
        return new CursorOverlaySettings(
            Enabled: settings.Enabled,
            Size: SizePixels.ContainsKey(settings.Size) ? settings.Size : Default.Size,
            Visibility: VisibilityValues.Contains(settings.Visibility) ? settings.Visibility : Default.Visibility,
            Crosshairs: settings.Crosshairs,
            Color: Colors.ContainsKey(settings.Color) ? settings.Color : Default.Color);
    }

    public static int ResolveSizePixels(string size)
    {
        return SizePixels.TryGetValue(size, out int pixels) ? pixels : SizePixels[Default.Size];
    }

    public static int[] ResolveColorRgb(string color)
    {
        return Colors.TryGetValue(color, out CursorOverlayColorInfo? value)
            ? value.Rgb.ToArray()
            : Colors[Default.Color].Rgb.ToArray();
    }

    public static JsonObject ToJsonObject(CursorOverlaySettings settings)
    {
        CursorOverlaySettings normalized = Normalize(settings);
        return new JsonObject
        {
            ["enabled"] = normalized.Enabled,
            ["size"] = normalized.Size,
            ["visibility"] = normalized.Visibility,
            ["crosshairs"] = normalized.Crosshairs,
            ["color"] = normalized.Color
        };
    }

    private static bool BooleanOrDefault(JsonObject value, string propertyName, bool fallback)
    {
        return value.TryGetPropertyValue(propertyName, out JsonNode? property) &&
            property is not null &&
            property.GetValueKind() is JsonValueKind.True or JsonValueKind.False
                ? property.GetValue<bool>()
                : fallback;
    }

    private static string StringSetOrDefault(JsonObject value, string propertyName, IEnumerable<string> allowed, string fallback)
    {
        return value.TryGetPropertyValue(propertyName, out JsonNode? property) &&
            property?.GetValueKind() == JsonValueKind.String &&
            allowed.Contains(property.GetValue<string>(), StringComparer.Ordinal)
                ? property.GetValue<string>()
                : fallback;
    }
}

public sealed class JsonCursorOverlaySettingsStore
{
    private readonly string filePath;
    private readonly Action<string> warn;

    public JsonCursorOverlaySettingsStore(string filePath, Action<string>? warn = null)
    {
        this.filePath = filePath;
        this.warn = warn ?? Console.WriteLine;
    }

    public CursorOverlaySettings Load()
    {
        try
        {
            return CursorOverlaySettingsModel.Normalize(JsonNode.Parse(File.ReadAllText(filePath)));
        }
        catch (Exception error) when (error is FileNotFoundException or DirectoryNotFoundException)
        {
            return CursorOverlaySettingsModel.Default;
        }
        catch
        {
            warn("Switchify cursor overlay settings could not be loaded. Defaults will be used.");
            return CursorOverlaySettingsModel.Default;
        }
    }

    public CursorOverlaySettings Save(CursorOverlaySettings settings)
    {
        CursorOverlaySettings normalized = CursorOverlaySettingsModel.Normalize(settings);
        string content = JsonSerializer.Serialize(CursorOverlaySettingsModel.ToJsonObject(normalized), JsonOptions) + "\n";
        JsonFileStore.WriteJsonFileAtomicSync(filePath, content);
        return normalized;
    }

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true
    };
}
