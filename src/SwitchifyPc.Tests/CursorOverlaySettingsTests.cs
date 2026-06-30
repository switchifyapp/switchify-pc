using System.Text.Json.Nodes;
using SwitchifyPc.Core.Settings;

namespace SwitchifyPc.Tests;

public sealed class CursorOverlaySettingsTests : IDisposable
{
    private readonly string tempDir = Path.Combine(Path.GetTempPath(), $"switchify-cursor-settings-{Guid.NewGuid():N}");

    public void Dispose()
    {
        if (Directory.Exists(tempDir))
        {
            Directory.Delete(tempDir, recursive: true);
        }
    }

    [Fact]
    public void NormalizesCursorOverlaySettings()
    {
        Assert.Equal(CursorOverlaySettingsModel.Default, CursorOverlaySettingsModel.Normalize((JsonNode?)null));
        Assert.Equal(
            CursorOverlaySettingsModel.Default with { Size = "large", Crosshairs = true, Color = "green" },
            CursorOverlaySettingsModel.Normalize(JsonNode.Parse("""{"size":"large","crosshairs":true,"color":"green"}""")));
        Assert.Equal(
            CursorOverlaySettingsModel.Default,
            CursorOverlaySettingsModel.Normalize(JsonNode.Parse("""{"enabled":"yes","size":"huge","visibility":"forever","crosshairs":"true","color":"purple"}""")));
    }

    [Fact]
    public void ResolvesSizeAndColorPresets()
    {
        Assert.Equal(96, CursorOverlaySettingsModel.ResolveSizePixels("small"));
        Assert.Equal(128, CursorOverlaySettingsModel.ResolveSizePixels("medium"));
        Assert.Equal(176, CursorOverlaySettingsModel.ResolveSizePixels("large"));
        Assert.Equal([211, 47, 47], CursorOverlaySettingsModel.ResolveColorRgb("red"));
        Assert.Equal([132, 255, 145], CursorOverlaySettingsModel.ResolveColorRgb("green"));
        Assert.Equal([100, 166, 255], CursorOverlaySettingsModel.ResolveColorRgb("blue"));
        Assert.Equal([255, 209, 102], CursorOverlaySettingsModel.ResolveColorRgb("yellow"));
        Assert.Equal([255, 255, 255], CursorOverlaySettingsModel.ResolveColorRgb("white"));
    }

    [Fact]
    public void StoreLoadsDefaultsForMissingOrInvalidJson()
    {
        List<string> warnings = [];
        string filePath = Path.Combine(tempDir, "cursor-overlay-settings.json");

        Assert.Equal(CursorOverlaySettingsModel.Default, new JsonCursorOverlaySettingsStore(filePath, warnings.Add).Load());

        Directory.CreateDirectory(tempDir);
        File.WriteAllText(filePath, "{");

        Assert.Equal(CursorOverlaySettingsModel.Default, new JsonCursorOverlaySettingsStore(filePath, warnings.Add).Load());
        Assert.Single(warnings);
    }

    [Fact]
    public void StoreSavesNormalizedJsonAndLeavesNoTempFiles()
    {
        string filePath = Path.Combine(tempDir, "nested", "cursor-overlay-settings.json");

        CursorOverlaySettings saved = new JsonCursorOverlaySettingsStore(filePath).Save(CursorOverlaySettingsModel.Default with { Size = "large" });

        Assert.Equal("large", saved.Size);
        Assert.Contains("\"size\": \"large\"", File.ReadAllText(filePath), StringComparison.Ordinal);
        Assert.Empty(Directory.EnumerateFiles(Path.GetDirectoryName(filePath)!, "*.tmp"));
    }
}
