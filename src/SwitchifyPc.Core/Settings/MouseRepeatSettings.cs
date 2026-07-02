using System.Text.Json;
using System.Text.Json.Nodes;
using SwitchifyPc.Core.Storage;

namespace SwitchifyPc.Core.Settings;

public sealed record MouseRepeatSettings(bool Enabled, int IntervalMs);

public static class MouseRepeatSettingsModel
{
    public const bool DefaultEnabled = true;
    public const int DefaultIntervalMs = 250;
    public const int MinIntervalMs = 100;
    public const int MaxIntervalMs = 2000;
    public const int IntervalStepMs = 50;

    public static readonly MouseRepeatSettings Default = new(DefaultEnabled, DefaultIntervalMs);

    public static MouseRepeatSettings Normalize(MouseRepeatSettings settings)
    {
        return settings with { IntervalMs = NormalizeInterval(settings.IntervalMs) };
    }

    public static MouseRepeatSettings Normalize(JsonNode? value)
    {
        if (value is not JsonObject candidate)
        {
            return Default;
        }

        bool enabled = DefaultEnabled;
        if (candidate.TryGetPropertyValue("enabled", out JsonNode? enabledNode) &&
            TryGetBoolean(enabledNode, out bool enabledValue))
        {
            enabled = enabledValue;
        }

        int intervalMs = DefaultIntervalMs;
        if (candidate.TryGetPropertyValue("intervalMs", out JsonNode? intervalNode) &&
            TryGetInteger(intervalNode, out int intervalValue))
        {
            intervalMs = intervalValue;
        }

        return Normalize(new MouseRepeatSettings(enabled, intervalMs));
    }

    public static JsonObject ToJsonObject(MouseRepeatSettings settings)
    {
        MouseRepeatSettings normalized = Normalize(settings);
        return new JsonObject
        {
            ["enabled"] = normalized.Enabled,
            ["intervalMs"] = normalized.IntervalMs
        };
    }

    private static int NormalizeInterval(int value)
    {
        int rounded = (int)Math.Round(value / (double)IntervalStepMs, MidpointRounding.AwayFromZero) * IntervalStepMs;
        return Math.Clamp(rounded, MinIntervalMs, MaxIntervalMs);
    }

    private static bool TryGetBoolean(JsonNode? value, out bool result)
    {
        result = false;
        if (value is null) return false;

        try
        {
            result = value.GetValue<bool>();
            return true;
        }
        catch (InvalidOperationException)
        {
            return false;
        }
        catch (FormatException)
        {
            return false;
        }
    }

    private static bool TryGetInteger(JsonNode? value, out int result)
    {
        result = 0;
        if (value is null) return false;

        try
        {
            result = value.GetValue<int>();
            return true;
        }
        catch (InvalidOperationException)
        {
            return false;
        }
        catch (FormatException)
        {
            return false;
        }
    }
}

public interface IMouseRepeatSettingsStore
{
    MouseRepeatSettings Load();

    MouseRepeatSettings Save(MouseRepeatSettings settings);
}

public sealed class JsonMouseRepeatSettingsStore : IMouseRepeatSettingsStore
{
    private readonly string filePath;
    private readonly Action<string> warn;

    public JsonMouseRepeatSettingsStore(string filePath, Action<string>? warn = null)
    {
        this.filePath = filePath;
        this.warn = warn ?? Console.WriteLine;
    }

    public MouseRepeatSettings Load()
    {
        try
        {
            return MouseRepeatSettingsModel.Normalize(JsonNode.Parse(File.ReadAllText(filePath)));
        }
        catch (Exception error) when (error is FileNotFoundException or DirectoryNotFoundException)
        {
            return MouseRepeatSettingsModel.Default;
        }
        catch
        {
            warn("Switchify mouse repeat settings could not be loaded. Defaults will be used.");
            return MouseRepeatSettingsModel.Default;
        }
    }

    public MouseRepeatSettings Save(MouseRepeatSettings settings)
    {
        MouseRepeatSettings normalized = MouseRepeatSettingsModel.Normalize(settings);
        string content = JsonSerializer.Serialize(MouseRepeatSettingsModel.ToJsonObject(normalized), JsonOptions) + "\n";
        JsonFileStore.WriteJsonFileAtomicSync(filePath, content);
        return normalized;
    }

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true
    };
}
