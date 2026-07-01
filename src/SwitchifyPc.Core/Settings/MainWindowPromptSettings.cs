using System.Text.Json;
using System.Text.Json.Nodes;
using SwitchifyPc.Core.Storage;

namespace SwitchifyPc.Core.Settings;

public sealed record MainWindowPromptSettings(bool AndroidDownloadDismissed);

public static class MainWindowPromptSettingsModel
{
    public static readonly MainWindowPromptSettings Default = new(AndroidDownloadDismissed: false);

    public static MainWindowPromptSettings Normalize(JsonNode? value)
    {
        if (value is not JsonObject candidate)
        {
            return Default;
        }

        return new MainWindowPromptSettings(
            AndroidDownloadDismissed: candidate.TryGetPropertyValue("androidDownloadDismissed", out JsonNode? dismissed) &&
                dismissed is not null &&
                dismissed.GetValueKind() is JsonValueKind.True or JsonValueKind.False &&
                dismissed.GetValue<bool>());
    }

    public static JsonObject ToJsonObject(MainWindowPromptSettings settings)
    {
        MainWindowPromptSettings normalized = Normalize(settings);
        return new JsonObject
        {
            ["androidDownloadDismissed"] = normalized.AndroidDownloadDismissed
        };
    }

    public static MainWindowPromptSettings Normalize(MainWindowPromptSettings settings)
    {
        return new MainWindowPromptSettings(settings.AndroidDownloadDismissed);
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
