using System.Text.Json;
using System.Text.Json.Nodes;
using SwitchifyPc.Core.Storage;

namespace SwitchifyPc.Core.Settings;

public sealed record MainWindowPromptSettings(
    bool AndroidDownloadDismissed,
    bool SetupGuideShown = false,
    bool SetupGuideCompleted = false);

public static class MainWindowPromptSettingsModel
{
    public static readonly MainWindowPromptSettings Default = new(
        AndroidDownloadDismissed: false,
        SetupGuideShown: false,
        SetupGuideCompleted: false);

    public static MainWindowPromptSettings Normalize(JsonNode? value)
    {
        if (value is not JsonObject candidate)
        {
            return Default;
        }

        bool completed = BooleanValue(candidate, "setupGuideCompleted");
        return new MainWindowPromptSettings(
            AndroidDownloadDismissed: BooleanValue(candidate, "androidDownloadDismissed"),
            SetupGuideShown: completed || BooleanValue(candidate, "setupGuideShown"),
            SetupGuideCompleted: completed);
    }

    public static JsonObject ToJsonObject(MainWindowPromptSettings settings)
    {
        MainWindowPromptSettings normalized = Normalize(settings);
        return new JsonObject
        {
            ["androidDownloadDismissed"] = normalized.AndroidDownloadDismissed,
            ["setupGuideShown"] = normalized.SetupGuideShown,
            ["setupGuideCompleted"] = normalized.SetupGuideCompleted
        };
    }

    public static MainWindowPromptSettings Normalize(MainWindowPromptSettings settings)
    {
        return new MainWindowPromptSettings(
            settings.AndroidDownloadDismissed,
            settings.SetupGuideShown || settings.SetupGuideCompleted,
            settings.SetupGuideCompleted);
    }

    private static bool BooleanValue(JsonObject candidate, string propertyName)
    {
        return candidate.TryGetPropertyValue(propertyName, out JsonNode? value) &&
            value is not null &&
            value.GetValueKind() is JsonValueKind.True or JsonValueKind.False &&
            value.GetValue<bool>();
    }
}

public interface IMainWindowPromptSettingsStore
{
    MainWindowPromptSettings Load();

    MainWindowPromptSettings Save(MainWindowPromptSettings settings);
}

public sealed class JsonMainWindowPromptSettingsStore : IMainWindowPromptSettingsStore
{
    private readonly string filePath;
    private readonly Action<string> warn;

    public JsonMainWindowPromptSettingsStore(string filePath, Action<string>? warn = null)
    {
        this.filePath = filePath;
        this.warn = warn ?? Console.WriteLine;
    }

    public MainWindowPromptSettings Load()
    {
        try
        {
            return MainWindowPromptSettingsModel.Normalize(JsonNode.Parse(File.ReadAllText(filePath)));
        }
        catch (Exception error) when (error is FileNotFoundException or DirectoryNotFoundException)
        {
            return MainWindowPromptSettingsModel.Default;
        }
        catch
        {
            warn("Switchify main window prompt settings could not be loaded. Defaults will be used.");
            return MainWindowPromptSettingsModel.Default;
        }
    }

    public MainWindowPromptSettings Save(MainWindowPromptSettings settings)
    {
        MainWindowPromptSettings normalized = MainWindowPromptSettingsModel.Normalize(settings);
        string content = JsonSerializer.Serialize(MainWindowPromptSettingsModel.ToJsonObject(normalized), JsonOptions) + "\n";
        JsonFileStore.WriteJsonFileAtomicSync(filePath, content);
        return normalized;
    }

    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true
    };
}
