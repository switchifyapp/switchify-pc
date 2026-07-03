using System.Text.Json.Nodes;
using SwitchifyPc.Core.Settings;

namespace SwitchifyPc.Tests;

public sealed class MouseRepeatSettingsTests
{
    [Fact]
    public void NormalizesIntervalToBoundsAndStep()
    {
        Assert.Equal(new MouseRepeatSettings(true, 100, 100), MouseRepeatSettingsModel.Normalize(new MouseRepeatSettings(true, 47, 52)));
        Assert.Equal(new MouseRepeatSettings(false, 2000, 2000), MouseRepeatSettingsModel.Normalize(new MouseRepeatSettings(false, 2501, 2501)));
        Assert.Equal(new MouseRepeatSettings(true, 300, 350), MouseRepeatSettingsModel.Normalize(new MouseRepeatSettings(true, 276, 326)));
    }

    [Fact]
    public void LegacyJsonIntervalAppliesToMoveAndScrollIntervals()
    {
        MouseRepeatSettings settings = MouseRepeatSettingsModel.Normalize(JsonNode.Parse("""
            {
              "enabled": false,
              "intervalMs": 500
            }
            """));

        Assert.Equal(new MouseRepeatSettings(false, 500, 500), settings);
    }

    [Fact]
    public void NewJsonIntervalsLoadIndependently()
    {
        MouseRepeatSettings settings = MouseRepeatSettingsModel.Normalize(JsonNode.Parse("""
            {
              "enabled": true,
              "moveIntervalMs": 100,
              "scrollIntervalMs": 1000
            }
            """));

        Assert.Equal(new MouseRepeatSettings(true, 100, 1000), settings);
    }

    [Fact]
    public void JsonStoreFallsBackToDefaultsForMissingFile()
    {
        string path = Path.Combine(Path.GetTempPath(), $"switchify-mouse-repeat-{Guid.NewGuid()}.json");

        JsonMouseRepeatSettingsStore store = new(path, _ => { });

        Assert.Equal(MouseRepeatSettingsModel.Default, store.Load());
    }

    [Fact]
    public void JsonStoreSavesNormalizedSettings()
    {
        string path = Path.Combine(Path.GetTempPath(), $"switchify-mouse-repeat-{Guid.NewGuid()}.json");
        try
        {
            JsonMouseRepeatSettingsStore store = new(path, _ => { });

            MouseRepeatSettings saved = store.Save(new MouseRepeatSettings(false, 47, 1001));

            Assert.Equal(new MouseRepeatSettings(false, 100, 1000), saved);
            Assert.Equal(saved, store.Load());
            string json = File.ReadAllText(path);
            Assert.Contains("\"moveIntervalMs\": 100", json);
            Assert.Contains("\"scrollIntervalMs\": 1000", json);
            Assert.DoesNotContain("\"intervalMs\"", json);
        }
        finally
        {
            File.Delete(path);
        }
    }
}
