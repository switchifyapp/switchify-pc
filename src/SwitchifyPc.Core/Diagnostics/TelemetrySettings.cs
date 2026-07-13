using System.Text.Json;

namespace SwitchifyPc.Core.Diagnostics;

public sealed record TelemetrySettings(bool Enabled, bool ConsentRecorded, string InstallId);

public interface ITelemetrySettingsStore
{
    TelemetrySettings Load();
    TelemetrySettings Save(bool enabled, bool consentRecorded = true);
}

public sealed class JsonTelemetrySettingsStore(string path) : ITelemetrySettingsStore
{
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web) { WriteIndented = true };

    public TelemetrySettings Load()
    {
        try
        {
            if (File.Exists(path))
            {
                TelemetrySettings? settings = JsonSerializer.Deserialize<TelemetrySettings>(File.ReadAllText(path), JsonOptions);
                if (settings is not null && Guid.TryParse(settings.InstallId, out _)) return settings;
            }
        }
        catch
        {
            // Invalid settings fail closed.
        }

        return SaveNewInstall();
    }

    public TelemetrySettings Save(bool enabled, bool consentRecorded = true)
    {
        TelemetrySettings current = LoadWithoutCreating() ?? new(false, false, Guid.NewGuid().ToString("D"));
        TelemetrySettings next = current with { Enabled = enabled, ConsentRecorded = consentRecorded };
        Write(next);
        return next;
    }

    private TelemetrySettings SaveNewInstall()
    {
        TelemetrySettings settings = new(false, false, Guid.NewGuid().ToString("D"));
        Write(settings);
        return settings;
    }

    private TelemetrySettings? LoadWithoutCreating()
    {
        try
        {
            if (!File.Exists(path)) return null;
            TelemetrySettings? settings = JsonSerializer.Deserialize<TelemetrySettings>(File.ReadAllText(path), JsonOptions);
            return settings is not null && Guid.TryParse(settings.InstallId, out _) ? settings : null;
        }
        catch
        {
            return null;
        }
    }

    private void Write(TelemetrySettings settings)
    {
        string? directory = Path.GetDirectoryName(path);
        if (!string.IsNullOrEmpty(directory)) Directory.CreateDirectory(directory);
        string temporaryPath = path + ".tmp";
        File.WriteAllText(temporaryPath, JsonSerializer.Serialize(settings, JsonOptions));
        File.Move(temporaryPath, path, true);
    }
}
