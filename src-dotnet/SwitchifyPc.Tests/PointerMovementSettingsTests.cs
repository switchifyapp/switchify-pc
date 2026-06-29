using System.Text.Json.Nodes;
using SwitchifyPc.Core.Settings;

namespace SwitchifyPc.Tests;

public sealed class PointerMovementSettingsTests : IDisposable
{
    private readonly string tempDir = Path.Combine(Path.GetTempPath(), $"switchify-pointer-settings-{Guid.NewGuid():N}");

    public void Dispose()
    {
        if (Directory.Exists(tempDir))
        {
            Directory.Delete(tempDir, recursive: true);
        }
    }

    [Fact]
    public void NormalizesScalePercent()
    {
        Assert.Equal(PointerMovementSettingsModel.Default, PointerMovementSettingsModel.Normalize((JsonNode?)null));
        Assert.Equal(new PointerMovementSettings(125), PointerMovementSettingsModel.Normalize(JsonNode.Parse("""{"scalePercent":125}""")));
        Assert.Equal(new PointerMovementSettings(25), PointerMovementSettingsModel.Normalize(JsonNode.Parse("""{"scalePercent":10}""")));
        Assert.Equal(new PointerMovementSettings(200), PointerMovementSettingsModel.Normalize(JsonNode.Parse("""{"scalePercent":1000}""")));
        Assert.Equal(new PointerMovementSettings(125), PointerMovementSettingsModel.Normalize(JsonNode.Parse("""{"scalePercent":123}""")));
        Assert.Equal(PointerMovementSettingsModel.Default, PointerMovementSettingsModel.Normalize(JsonNode.Parse("""{"scalePercent":"125"}""")));
    }

    [Fact]
    public void MigratesOldPercentageAndMultiplierShapes()
    {
        Assert.Equal(
            new PointerMovementSettings(195),
            PointerMovementSettingsModel.Normalize(JsonNode.Parse("""{"percentages":{"small":9,"medium":24,"large":50}}""")));
        Assert.Equal(
            new PointerMovementSettings(115),
            PointerMovementSettingsModel.Normalize(JsonNode.Parse("""{"multipliers":{"small":200,"medium":50,"large":100}}""")));
    }

    [Fact]
    public void DerivesMovementPercentagesAndFractions()
    {
        PointerMovementSettings settings = new(150);

        Assert.Equal(7, PointerMovementSettingsModel.PercentageFor(settings, PointerMovementSizeKey.Small));
        Assert.Equal(18, PointerMovementSettingsModel.PercentageFor(settings, PointerMovementSizeKey.Medium));
        Assert.Equal(39, PointerMovementSettingsModel.PercentageFor(settings, PointerMovementSizeKey.Large));
        Assert.Equal(0.045, PointerMovementSettingsModel.FractionFor(PointerMovementSettingsModel.Default, PointerMovementSizeKey.Small));
        Assert.Equal(0.12, PointerMovementSettingsModel.FractionFor(PointerMovementSettingsModel.Default, PointerMovementSizeKey.Medium));
        Assert.Equal(0.26, PointerMovementSettingsModel.FractionFor(PointerMovementSettingsModel.Default, PointerMovementSizeKey.Large));
    }

    [Fact]
    public void StoreLoadsDefaultsForMissingOrInvalidJson()
    {
        List<string> warnings = [];
        string filePath = Path.Combine(tempDir, "pointer-movement-settings.json");

        Assert.Equal(PointerMovementSettingsModel.Default, new JsonPointerMovementSettingsStore(filePath, warnings.Add).Load());

        Directory.CreateDirectory(tempDir);
        File.WriteAllText(filePath, "{");

        Assert.Equal(PointerMovementSettingsModel.Default, new JsonPointerMovementSettingsStore(filePath, warnings.Add).Load());
        Assert.Single(warnings);
    }

    [Fact]
    public void StoreSavesNormalizedJsonAndLeavesNoTempFiles()
    {
        string filePath = Path.Combine(tempDir, "nested", "pointer-movement-settings.json");

        PointerMovementSettings saved = new JsonPointerMovementSettingsStore(filePath).Save(new PointerMovementSettings(123));

        Assert.Equal(new PointerMovementSettings(125), saved);
        Assert.Contains("\"scalePercent\": 125", File.ReadAllText(filePath), StringComparison.Ordinal);
        Assert.Empty(Directory.EnumerateFiles(Path.GetDirectoryName(filePath)!, "*.tmp"));
    }
}
