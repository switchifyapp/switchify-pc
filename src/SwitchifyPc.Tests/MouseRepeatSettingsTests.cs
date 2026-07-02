using SwitchifyPc.Core.Settings;

namespace SwitchifyPc.Tests;

public sealed class MouseRepeatSettingsTests
{
    [Fact]
    public void NormalizesIntervalToBoundsAndStep()
    {
        Assert.Equal(new MouseRepeatSettings(true, 100), MouseRepeatSettingsModel.Normalize(new MouseRepeatSettings(true, 47)));
        Assert.Equal(new MouseRepeatSettings(false, 2000), MouseRepeatSettingsModel.Normalize(new MouseRepeatSettings(false, 2501)));
        Assert.Equal(new MouseRepeatSettings(true, 300), MouseRepeatSettingsModel.Normalize(new MouseRepeatSettings(true, 276)));
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

            MouseRepeatSettings saved = store.Save(new MouseRepeatSettings(false, 47));

            Assert.Equal(new MouseRepeatSettings(false, 100), saved);
            Assert.Equal(saved, store.Load());
        }
        finally
        {
            File.Delete(path);
        }
    }
}
