using System.Text.Json;
using System.Text.Json.Nodes;
using SwitchifyPc.Core.Storage;

namespace SwitchifyPc.Core.Settings;

public enum PointerMovementSizeKey
{
    Small,
    Medium,
    Large
}

public sealed record PointerMovementSettings(double ScalePercent);

public static class PointerMovementSettingsModel
{
    public const double PointerMovementScaleMin = 5;
    public const double PointerMovementScaleMax = 225;
    public const double PointerMovementScaleStep = 5;
    public const int BaseMoveDelta = 128;

    private const double DisplayPercentageMin = 1;
    private const double DisplayPercentageMax = 50;
    private const double DisplayPercentageStep = 0.5;

    public static readonly PointerMovementSettings Default = new(100);

    public static readonly IReadOnlyDictionary<PointerMovementSizeKey, double> BasePercentages =
        new Dictionary<PointerMovementSizeKey, double>
        {
            [PointerMovementSizeKey.Small] = 4.5,
            [PointerMovementSizeKey.Medium] = 12,
            [PointerMovementSizeKey.Large] = 26
        };

    public static PointerMovementSettings Normalize(JsonNode? value)
    {
        if (value is not JsonObject candidate)
        {
            return Default;
        }

        if (candidate.TryGetPropertyValue("scalePercent", out JsonNode? scalePercent))
        {
            return new PointerMovementSettings(NormalizeScale(scalePercent));
        }

        if (candidate.TryGetPropertyValue("percentages", out JsonNode? percentages) && percentages is JsonObject percentageObject)
        {
            return new PointerMovementSettings(NormalizeScale(ScaleFromPercentages(percentageObject)));
        }

        if (candidate.TryGetPropertyValue("multipliers", out JsonNode? multipliers) && multipliers is JsonObject multiplierObject)
        {
            return new PointerMovementSettings(NormalizeScale(ScaleFromLegacyMultipliers(multiplierObject)));
        }

        return Default;
    }

    public static PointerMovementSettings Normalize(object? value)
    {
        return Normalize(JsonSerializer.SerializeToNode(value));
    }

    public static double ScalePercentFor(PointerMovementSettings settings)
    {
        return Normalize(settings).ScalePercent;
    }

    public static double PercentageFor(PointerMovementSettings settings, PointerMovementSizeKey size)
    {
        double scale = ScalePercentFor(settings) / 100;
        return NormalizeDisplayPercentage(BasePercentages[size] * scale);
    }

    public static double FractionFor(PointerMovementSettings settings, PointerMovementSizeKey size)
    {
        return PercentageFor(settings, size) / 100;
    }

    public static JsonObject ToJsonObject(PointerMovementSettings settings)
    {
        PointerMovementSettings normalized = Normalize(settings);
        return new JsonObject
        {
            ["scalePercent"] = normalized.ScalePercent
        };
    }

    private static double NormalizeScale(JsonNode? value)
    {
        return TryGetFiniteNumber(value, out double number)
            ? Clamp(RoundToStep(number, PointerMovementScaleStep), PointerMovementScaleMin, PointerMovementScaleMax)
            : Default.ScalePercent;
    }

    public static PointerMovementSettings Normalize(PointerMovementSettings settings)
    {
        return new PointerMovementSettings(Clamp(RoundToStep(settings.ScalePercent, PointerMovementScaleStep), PointerMovementScaleMin, PointerMovementScaleMax));
    }

    private static double ScaleFromPercentages(JsonObject value)
    {
        List<double> scales = [];
        foreach (PointerMovementSizeKey size in Enum.GetValues<PointerMovementSizeKey>())
        {
            if (value.TryGetPropertyValue(JsonKey(size), out JsonNode? percentage) && TryGetFiniteNumber(percentage, out double number))
            {
                scales.Add((NormalizeDisplayPercentage(number) / BasePercentages[size]) * 100);
            }
        }

        return scales.Count == 0 ? Default.ScalePercent : scales.Average();
    }

    private static double ScaleFromLegacyMultipliers(JsonObject value)
    {
        List<double> scales = [];
        foreach (PointerMovementSizeKey size in Enum.GetValues<PointerMovementSizeKey>())
        {
            if (value.TryGetPropertyValue(JsonKey(size), out JsonNode? multiplier) && TryGetFiniteNumber(multiplier, out double number))
            {
                scales.Add(number);
            }
        }

        return scales.Count == 0 ? Default.ScalePercent : scales.Average();
    }

    private static double NormalizeDisplayPercentage(double value)
    {
        return Clamp(RoundToStep(value, DisplayPercentageStep), DisplayPercentageMin, DisplayPercentageMax);
    }

    private static bool TryGetFiniteNumber(JsonNode? value, out double number)
    {
        number = 0;
        if (value is null)
        {
            return false;
        }

        try
        {
            number = value.GetValue<double>();
            return double.IsFinite(number);
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

    private static string JsonKey(PointerMovementSizeKey size)
    {
        return size switch
        {
            PointerMovementSizeKey.Small => "small",
            PointerMovementSizeKey.Medium => "medium",
            PointerMovementSizeKey.Large => "large",
            _ => throw new ArgumentOutOfRangeException(nameof(size), size, null)
        };
    }

    private static double RoundToStep(double value, double step)
    {
        return Math.Round(value / step, MidpointRounding.AwayFromZero) * step;
    }

    private static double Clamp(double value, double min, double max)
    {
        return Math.Min(max, Math.Max(min, value));
    }
}

public interface IPointerMovementSettingsStore
{
    PointerMovementSettings Load();

    PointerMovementSettings Save(PointerMovementSettings settings);
}

public sealed class JsonPointerMovementSettingsStore : IPointerMovementSettingsStore
{
    private readonly string filePath;
    private readonly Action<string> warn;

    public JsonPointerMovementSettingsStore(string filePath, Action<string>? warn = null)
    {
        this.filePath = filePath;
        this.warn = warn ?? Console.WriteLine;
    }

    public PointerMovementSettings Load()
    {
        try
        {
            return PointerMovementSettingsModel.Normalize(JsonNode.Parse(File.ReadAllText(filePath)));
        }
        catch (Exception error) when (error is FileNotFoundException or DirectoryNotFoundException)
        {
            return PointerMovementSettingsModel.Default;
        }
        catch
        {
            warn("Switchify pointer movement settings could not be loaded. Defaults will be used.");
            return PointerMovementSettingsModel.Default;
        }
    }

    public PointerMovementSettings Save(PointerMovementSettings settings)
    {
        PointerMovementSettings normalized = PointerMovementSettingsModel.Normalize(settings);
        string content = JsonSerializer.Serialize(PointerMovementSettingsModel.ToJsonObject(normalized), JsonOptions) + "\n";
        JsonFileStore.WriteJsonFileAtomicSync(filePath, content);
        return normalized;
    }

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true
    };
}
