using System.Net.Http.Headers;
using System.Net.Http.Json;
using System.Text.Json;

namespace SwitchifyPc.Core.Diagnostics;

public sealed record TelemetryBreadcrumb(string Name, string Dataset, string Level, string Timestamp);

public sealed record TelemetryLog(
    string Level,
    string Message,
    string Source,
    string Environment,
    string Dataset,
    string Version,
    IReadOnlyDictionary<string, string>? Data,
    string? ErrorName,
    string? ErrorStack,
    IReadOnlyList<string> Tags,
    string UserId,
    string Timestamp,
    IReadOnlyList<TelemetryBreadcrumb>? Breadcrumbs = null);

public interface ITelemetryReporter : IDisposable
{
    bool Enabled { get; }
    void RecordBreadcrumb(string name, string dataset = "health", string level = "info");
    Task ReportExceptionAsync(Exception error, string dataset, IReadOnlyDictionary<string, string>? data = null, CancellationToken cancellationToken = default);
    Task ReportHealthAsync(string name, IReadOnlyDictionary<string, string>? data = null, CancellationToken cancellationToken = default);
    Task FlushAsync(CancellationToken cancellationToken = default);
    void SetEnabled(bool enabled);
}

public sealed class TelemetryReporter : ITelemetryReporter
{
    public static readonly Uri Endpoint = new("https://timberlogs-ingest.enaboapps.workers.dev/v1/logs");
    private const int MaximumQueuedLogs = 20;
    private const int MaximumBreadcrumbs = 30;
    private readonly ITelemetrySettingsStore settingsStore;
    private readonly HttpClient httpClient;
    private readonly string apiKey;
    private readonly string version;
    private readonly string queuePath;
    private readonly object sync = new();
    private readonly SemaphoreSlim sendLock = new(1, 1);
    private readonly Queue<TelemetryBreadcrumb> breadcrumbs = new();
    private readonly JsonSerializerOptions jsonOptions = new(JsonSerializerDefaults.Web) { WriteIndented = true };
    private TelemetrySettings settings;

    public TelemetryReporter(
        ITelemetrySettingsStore settingsStore,
        HttpClient httpClient,
        string apiKey,
        string version,
        string queuePath)
    {
        this.settingsStore = settingsStore;
        this.httpClient = httpClient;
        this.apiKey = apiKey.Trim();
        this.version = version;
        this.queuePath = queuePath;
        settings = settingsStore.Load();
        httpClient.Timeout = TimeSpan.FromSeconds(5);
    }

    public bool Enabled => settings.Enabled && !string.IsNullOrWhiteSpace(apiKey);

    public void SetEnabled(bool enabled)
    {
        settings = settingsStore.Save(enabled);
        if (enabled && !string.IsNullOrWhiteSpace(apiKey)) return;
        lock (sync)
        {
            breadcrumbs.Clear();
            TryDeleteQueue();
        }
    }

    public void RecordBreadcrumb(string name, string dataset = "health", string level = "info")
    {
        if (!Enabled) return;
        string? safeName = TelemetrySanitizer.Message(name);
        if (safeName is null) return;
        lock (sync)
        {
            breadcrumbs.Enqueue(new(safeName, SafeLabel(dataset), SafeLabel(level), DateTimeOffset.UtcNow.ToString("O")));
            while (breadcrumbs.Count > MaximumBreadcrumbs) breadcrumbs.Dequeue();
        }
    }

    public async Task ReportExceptionAsync(Exception error, string dataset, IReadOnlyDictionary<string, string>? data = null, CancellationToken cancellationToken = default)
    {
        if (!Enabled) return;
        try
        {
            TelemetryLog log = CreateLog(
                "error",
                TelemetrySanitizer.Message(error.Message) ?? error.GetType().Name,
                SafeLabel(dataset),
                SafeData(data),
                error.GetType().FullName,
                TelemetrySanitizer.Stack(error.ToString()),
                ["exception"]);
            Enqueue(log);
            await FlushAsync(cancellationToken).ConfigureAwait(false);
        }
        catch
        {
            // Reporting must never affect the app's error path.
        }
    }

    public async Task ReportHealthAsync(string name, IReadOnlyDictionary<string, string>? data = null, CancellationToken cancellationToken = default)
    {
        if (!Enabled) return;
        try
        {
            TelemetryLog log = CreateLog("info", TelemetrySanitizer.Message(name) ?? "health", "health", SafeData(data), null, null, ["health"]);
            await SendAsync([log], cancellationToken).ConfigureAwait(false);
        }
        catch
        {
            // Health reporting is best effort and is never queued.
        }
    }

    public async Task FlushAsync(CancellationToken cancellationToken = default)
    {
        if (!Enabled) return;
        await sendLock.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            List<TelemetryLog> queued = ReadQueue();
            if (queued.Count == 0) return;
            if (await SendAsync(queued, cancellationToken).ConfigureAwait(false)) TryDeleteQueue();
        }
        finally
        {
            sendLock.Release();
        }
    }

    private TelemetryLog CreateLog(string level, string message, string dataset, IReadOnlyDictionary<string, string>? data, string? errorName, string? errorStack, IReadOnlyList<string> tags)
    {
        IReadOnlyList<TelemetryBreadcrumb>? snapshot;
        lock (sync) snapshot = breadcrumbs.Count == 0 ? null : breadcrumbs.ToArray();
        return new(level, message, "switchify-pc", "production", dataset, version, data,
            TelemetrySanitizer.Message(errorName), errorStack, tags, settings.InstallId, DateTimeOffset.UtcNow.ToString("O"), snapshot);
    }

    private async Task<bool> SendAsync(IReadOnlyList<TelemetryLog> logs, CancellationToken cancellationToken)
    {
        try
        {
            using HttpRequestMessage request = new(HttpMethod.Post, Endpoint)
            {
                Content = JsonContent.Create(new { logs })
            };
            request.Headers.Authorization = new AuthenticationHeaderValue("Bearer", apiKey);
            using HttpResponseMessage response = await httpClient.SendAsync(request, cancellationToken).ConfigureAwait(false);
            return response.IsSuccessStatusCode;
        }
        catch
        {
            return false;
        }
    }

    private void Enqueue(TelemetryLog log)
    {
        lock (sync)
        {
            List<TelemetryLog> logs = ReadQueueUnsafe();
            logs.Add(log);
            if (logs.Count > MaximumQueuedLogs) logs.RemoveRange(0, logs.Count - MaximumQueuedLogs);
            string? directory = Path.GetDirectoryName(queuePath);
            if (!string.IsNullOrEmpty(directory)) Directory.CreateDirectory(directory);
            string temporaryPath = queuePath + ".tmp";
            File.WriteAllText(temporaryPath, JsonSerializer.Serialize(logs, jsonOptions));
            File.Move(temporaryPath, queuePath, true);
        }
    }

    private List<TelemetryLog> ReadQueue()
    {
        lock (sync) return ReadQueueUnsafe();
    }

    private List<TelemetryLog> ReadQueueUnsafe()
    {
        try
        {
            if (!File.Exists(queuePath)) return [];
            return JsonSerializer.Deserialize<List<TelemetryLog>>(File.ReadAllText(queuePath), jsonOptions) ?? [];
        }
        catch
        {
            TryDeleteQueue();
            return [];
        }
    }

    private void TryDeleteQueue()
    {
        try { File.Delete(queuePath); } catch { }
    }

    private static IReadOnlyDictionary<string, string>? SafeData(IReadOnlyDictionary<string, string>? data)
    {
        if (data is null || data.Count == 0) return null;
        Dictionary<string, string> safe = new(StringComparer.Ordinal);
        foreach (KeyValuePair<string, string> pair in data.Take(12))
        {
            safe[SafeLabel(pair.Key)] = TelemetrySanitizer.Message(pair.Value) ?? string.Empty;
        }
        return safe;
    }

    private static string SafeLabel(string value)
    {
        string safe = new(value.Where(character => char.IsLetterOrDigit(character) || character is '.' or '_' or '-').Take(64).ToArray());
        return string.IsNullOrEmpty(safe) ? "unknown" : safe;
    }

    public void Dispose()
    {
        sendLock.Dispose();
        httpClient.Dispose();
    }
}
