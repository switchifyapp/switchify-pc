using System.Text.Json;
using System.Text.Json.Nodes;
using SwitchifyPc.Core.Storage;

namespace SwitchifyPc.Core.Settings;

public sealed record MouseRepeatSettings(
    bool Enabled,
    int MoveIntervalMs,
    int ScrollIntervalMs,
    int AccelerationDurationMs = MouseRepeatSettingsModel.DefaultAccelerationDurationMs);

public static class MouseRepeatSettingsModel
{
    public const bool DefaultEnabled = true;
    public const int DefaultIntervalMs = 250;
    public const int MinIntervalMs = 100;
    public const int MaxIntervalMs = 2000;
    public const int IntervalStepMs = 50;
    public const int AccelerationInitialScalePercent = 25;
    public const int DefaultAccelerationDurationMs = 1000;

    public static readonly IReadOnlyList<int> AccelerationDurationOptionsMs = [0, 500, 1000, 2000];

    public static readonly MouseRepeatSettings Default = new(
        DefaultEnabled,
        DefaultIntervalMs,
        DefaultIntervalMs,
        DefaultAccelerationDurationMs);

    public static MouseRepeatSettings Normalize(MouseRepeatSettings settings)
    {
        return settings with
        {
            MoveIntervalMs = NormalizeInterval(settings.MoveIntervalMs),
            ScrollIntervalMs = NormalizeInterval(settings.ScrollIntervalMs),
            AccelerationDurationMs = NormalizeAccelerationDuration(settings.AccelerationDurationMs)
        };
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

        int legacyIntervalMs = DefaultIntervalMs;
        if (candidate.TryGetPropertyValue("intervalMs", out JsonNode? intervalNode) &&
            TryGetInteger(intervalNode, out int intervalValue))
        {
            legacyIntervalMs = intervalValue;
        }

        int moveIntervalMs = legacyIntervalMs;
        if (candidate.TryGetPropertyValue("moveIntervalMs", out JsonNode? moveIntervalNode) &&
            TryGetInteger(moveIntervalNode, out int moveIntervalValue))
        {
            moveIntervalMs = moveIntervalValue;
        }

        int scrollIntervalMs = legacyIntervalMs;
        if (candidate.TryGetPropertyValue("scrollIntervalMs", out JsonNode? scrollIntervalNode) &&
            TryGetInteger(scrollIntervalNode, out int scrollIntervalValue))
        {
            scrollIntervalMs = scrollIntervalValue;
        }

        int accelerationDurationMs = DefaultAccelerationDurationMs;
        if (candidate.TryGetPropertyValue("accelerationDurationMs", out JsonNode? accelerationNode) &&
            TryGetInteger(accelerationNode, out int accelerationValue))
        {
            accelerationDurationMs = accelerationValue;
        }

        return Normalize(new MouseRepeatSettings(enabled, moveIntervalMs, scrollIntervalMs, accelerationDurationMs));
    }

    public static JsonObject ToJsonObject(MouseRepeatSettings settings)
    {
        MouseRepeatSettings normalized = Normalize(settings);
        return new JsonObject
        {
            ["enabled"] = normalized.Enabled,
            ["moveIntervalMs"] = normalized.MoveIntervalMs,
            ["scrollIntervalMs"] = normalized.ScrollIntervalMs,
            ["accelerationDurationMs"] = normalized.AccelerationDurationMs
        };
    }

    public static double AccelerationScale(int durationMs, TimeSpan elapsed)
    {
        int normalizedDuration = NormalizeAccelerationDuration(durationMs);
        if (normalizedDuration == 0)
        {
            return 1;
        }

        double progress = Math.Clamp(elapsed.TotalMilliseconds / normalizedDuration, 0, 1);
        double easedProgress = progress * progress * (3 - (2 * progress));
        double initialScale = AccelerationInitialScalePercent / 100d;
        return initialScale + ((1 - initialScale) * easedProgress);
    }

    private static int NormalizeInterval(int value)
    {
        int rounded = (int)Math.Round(value / (double)IntervalStepMs, MidpointRounding.AwayFromZero) * IntervalStepMs;
        return Math.Clamp(rounded, MinIntervalMs, MaxIntervalMs);
    }

    private static int NormalizeAccelerationDuration(int value)
    {
        return AccelerationDurationOptionsMs
            .OrderBy(option => Math.Abs((long)option - value))
            .ThenBy(option => option)
            .First();
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
