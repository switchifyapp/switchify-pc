using System.Text.Json.Nodes;
using SwitchifyPc.Core.Settings;

namespace SwitchifyPc.Tests;

public sealed class MainWindowPromptSettingsTests
{
    [Fact]
    public void MissingSettingsFileReturnsDefault()
    {
        string filePath = Path.Combine(CreateTempDirectory(), "main-window-prompt-settings.json");
        JsonMainWindowPromptSettingsStore store = new(filePath);

        MainWindowPromptSettings settings = store.Load();

        Assert.False(settings.AndroidDownloadDismissed);
        Assert.False(settings.SetupGuideShown);
        Assert.False(settings.SetupGuideCompleted);
    }

    [Fact]
    public void InvalidJsonReturnsDefault()
    {
        string directory = CreateTempDirectory();
        string filePath = Path.Combine(directory, "main-window-prompt-settings.json");
        File.WriteAllText(filePath, "{not json");
        List<string> warnings = [];
        JsonMainWindowPromptSettingsStore store = new(filePath, warnings.Add);

        MainWindowPromptSettings settings = store.Load();

        Assert.False(settings.AndroidDownloadDismissed);
        Assert.False(settings.SetupGuideShown);
        Assert.False(settings.SetupGuideCompleted);
        Assert.Single(warnings);
    }

    [Fact]
    public void SavesDismissedState()
    {
        string filePath = Path.Combine(CreateTempDirectory(), "main-window-prompt-settings.json");
        JsonMainWindowPromptSettingsStore store = new(filePath);

        MainWindowPromptSettings saved = store.Save(new MainWindowPromptSettings(
            AndroidDownloadDismissed: true,
            SetupGuideShown: true,
            SetupGuideCompleted: true));
        MainWindowPromptSettings loaded = store.Load();

        Assert.True(saved.AndroidDownloadDismissed);
        Assert.True(loaded.AndroidDownloadDismissed);
        Assert.True(loaded.SetupGuideShown);
        Assert.True(loaded.SetupGuideCompleted);
        Assert.Contains("\"androidDownloadDismissed\": true", File.ReadAllText(filePath));
        Assert.Contains("\"setupGuideShown\": true", File.ReadAllText(filePath));
        Assert.Contains("\"setupGuideCompleted\": true", File.ReadAllText(filePath));
    }

    [Fact]
    public void NormalizesNonBooleanDismissedValueToDefault()
    {
        MainWindowPromptSettings settings = MainWindowPromptSettingsModel.Normalize(new JsonObject
        {
            ["androidDownloadDismissed"] = "true"
        });

        Assert.False(settings.AndroidDownloadDismissed);
        Assert.False(settings.SetupGuideShown);
        Assert.False(settings.SetupGuideCompleted);
    }

    [Fact]
    public void ExistingAndroidPromptSettingsGainGuideDefaults()
    {
        MainWindowPromptSettings settings = MainWindowPromptSettingsModel.Normalize(new JsonObject
        {
            ["androidDownloadDismissed"] = true
        });

        Assert.True(settings.AndroidDownloadDismissed);
        Assert.False(settings.SetupGuideShown);
        Assert.False(settings.SetupGuideCompleted);
    }

    [Fact]
    public void CompletedGuideIsAlwaysNormalizedAsShown()
    {
        MainWindowPromptSettings settings = MainWindowPromptSettingsModel.Normalize(new JsonObject
        {
            ["setupGuideShown"] = false,
            ["setupGuideCompleted"] = true
        });

        Assert.True(settings.SetupGuideShown);
        Assert.True(settings.SetupGuideCompleted);
    }

    [Fact]
    public void SaveCreatesSettingsDirectory()
    {
        string filePath = Path.Combine(CreateTempDirectory(), "nested", "main-window-prompt-settings.json");
        JsonMainWindowPromptSettingsStore store = new(filePath);

        store.Save(new MainWindowPromptSettings(AndroidDownloadDismissed: true));

        Assert.True(File.Exists(filePath));
    }

    private static string CreateTempDirectory()
    {
        string directory = Path.Combine(Path.GetTempPath(), "switchify-tests", Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(directory);
        return directory;
    }
}
